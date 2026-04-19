//! Pure scoring functions for talent recommendations.
//!
//! No database access here — all inputs are plain values so the functions
//! are fully unit-testable without a live Postgres.
//!
//! ## Scoring rules
//!
//! ### Cold-start (total_feedback < 10)
//!   score = 0.5 * recency_score + 0.5 * completeness_score
//!
//! ### Blended (total_feedback >= 10)
//!   score = (W.skills/100) * skill_match
//!         + (W.experience/100) * experience_match
//!         + (W.recency/100) * recency_score
//!         + (W.completeness/100) * completeness_score
//!
//! All component scores are in [0, 1].

use std::collections::HashSet;

/// Threshold at which the system switches from cold-start to blended scoring.
pub const COLD_START_THRESHOLD: i64 = 10;

/// Default weights when the user has no stored weights.
pub const DEFAULT_SKILLS_W: i32 = 40;
pub const DEFAULT_EXPERIENCE_W: i32 = 30;
pub const DEFAULT_RECENCY_W: i32 = 15;
pub const DEFAULT_COMPLETENESS_W: i32 = 15;

// ── Component score functions ────────────────────────────────────────────────

/// |C.skills ∩ R.required_skills| / max(1, |R.required_skills|), in [0, 1].
pub fn skill_match(candidate_skills: &[String], required_skills: &[String]) -> f64 {
    if required_skills.is_empty() {
        return 1.0;
    }
    let req: HashSet<&str> = required_skills.iter().map(|s| s.as_str()).collect();
    let matched = candidate_skills
        .iter()
        .filter(|s| req.contains(s.as_str()))
        .count();
    matched as f64 / req.len() as f64
}

/// min(1.0, C.years_experience / max(1, R.min_years)), clamped [0, 1].
pub fn experience_match(candidate_years: i32, min_years: i32) -> f64 {
    let denom = min_years.max(1) as f64;
    (candidate_years as f64 / denom).min(1.0).max(0.0)
}

/// max(0, 1 - days_since_last_active / 180), in [0, 1].
pub fn recency_score(days_since_last_active: f64) -> f64 {
    (1.0 - days_since_last_active / 180.0).max(0.0)
}

/// C.completeness_score / 100, in [0, 1].
pub fn completeness_score(raw: i32) -> f64 {
    (raw as f64 / 100.0).clamp(0.0, 1.0)
}

// ── Aggregate scorers ────────────────────────────────────────────────────────

/// Inputs passed to the scorer for a single candidate.
pub struct CandidateInputs<'a> {
    pub skills: &'a [String],
    pub years_experience: i32,
    pub days_since_last_active: f64,
    pub completeness_raw: i32,
}

/// Output of scoring a single candidate against a role.
pub struct ScoredCandidate {
    pub score: f64,
    pub reasons: Vec<String>,
}

/// Cold-start scorer: rank by recency + completeness (0.5 / 0.5).
pub fn score_cold_start(inputs: &CandidateInputs<'_>, total_feedback: i64) -> ScoredCandidate {
    let r = recency_score(inputs.days_since_last_active);
    let c = completeness_score(inputs.completeness_raw);
    let score = 0.5 * r + 0.5 * c;
    ScoredCandidate {
        score,
        reasons: vec![format!(
            "Cold-start: ranked by recency and profile completeness until >=10 feedback \
             datapoints collected (current: {total_feedback})"
        )],
    }
}

/// Blended scorer: weighted combination of the four components.
pub struct BlendWeights {
    pub skills: i32,
    pub experience: i32,
    pub recency: i32,
    pub completeness: i32,
}

impl Default for BlendWeights {
    fn default() -> Self {
        Self {
            skills: DEFAULT_SKILLS_W,
            experience: DEFAULT_EXPERIENCE_W,
            recency: DEFAULT_RECENCY_W,
            completeness: DEFAULT_COMPLETENESS_W,
        }
    }
}

pub struct RoleInputs<'a> {
    pub required_skills: &'a [String],
    pub min_years: i32,
}

pub fn score_blended(
    candidate: &CandidateInputs<'_>,
    role: &RoleInputs<'_>,
    weights: &BlendWeights,
) -> ScoredCandidate {
    let sm = skill_match(candidate.skills, role.required_skills);
    let em = experience_match(candidate.years_experience, role.min_years);
    let rs = recency_score(candidate.days_since_last_active);
    let cs = completeness_score(candidate.completeness_raw);

    let score = (weights.skills as f64 / 100.0) * sm
        + (weights.experience as f64 / 100.0) * em
        + (weights.recency as f64 / 100.0) * rs
        + (weights.completeness as f64 / 100.0) * cs;

    let matched_count = {
        let req: HashSet<&str> = role.required_skills.iter().map(|s| s.as_str()).collect();
        candidate
            .skills
            .iter()
            .filter(|s| req.contains(s.as_str()))
            .count()
    };

    let reasons = vec![
        format!(
            "Skill match: {}/{} required skills",
            matched_count,
            role.required_skills.len()
        ),
        format!(
            "Experience: {} vs {} min years",
            candidate.years_experience, role.min_years
        ),
        format!(
            "Recency score: {:.2} (last active {:.0} days ago)",
            rs, candidate.days_since_last_active
        ),
        format!(
            "Profile completeness: {}%",
            candidate.completeness_raw
        ),
    ];

    ScoredCandidate { score, reasons }
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn skills(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // ---- skill_match --------------------------------------------------------

    #[test]
    fn skill_match_full_overlap() {
        let c = skills(&["rust", "postgres"]);
        let r = skills(&["rust", "postgres"]);
        assert!((skill_match(&c, &r) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn skill_match_no_overlap() {
        let c = skills(&["python"]);
        let r = skills(&["rust", "postgres"]);
        assert!((skill_match(&c, &r) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn skill_match_partial() {
        let c = skills(&["rust", "python"]);
        let r = skills(&["rust", "postgres", "redis"]);
        // 1/3
        let got = skill_match(&c, &r);
        assert!((got - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn skill_match_empty_required_returns_one() {
        let c = skills(&["rust"]);
        let r = skills(&[]);
        assert!((skill_match(&c, &r) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn skill_match_empty_candidate_zero() {
        let c = skills(&[]);
        let r = skills(&["rust"]);
        assert!((skill_match(&c, &r) - 0.0).abs() < 1e-9);
    }

    // ---- experience_match ---------------------------------------------------

    #[test]
    fn experience_match_exceeds_clamps_to_one() {
        assert!((experience_match(10, 3) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn experience_match_zero_min_years_one() {
        // min_years=0 → denom=1, so 5/1 = 5.0 → clamp 1.0
        assert!((experience_match(5, 0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn experience_match_partial() {
        // 3 / 6 = 0.5
        assert!((experience_match(3, 6) - 0.5).abs() < 1e-9);
    }

    // ---- recency_score ------------------------------------------------------

    #[test]
    fn recency_score_zero_days() {
        assert!((recency_score(0.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn recency_score_180_days() {
        assert!((recency_score(180.0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn recency_score_over_180_clamps_zero() {
        assert!((recency_score(365.0) - 0.0).abs() < 1e-9);
    }

    // ---- cold_start scoring -------------------------------------------------

    #[test]
    fn cold_start_score_uses_recency_and_completeness() {
        let inp = CandidateInputs {
            skills: &skills(&[]),
            years_experience: 0,
            days_since_last_active: 0.0,   // recency = 1.0
            completeness_raw: 80,          // completeness = 0.8
        };
        let out = score_cold_start(&inp, 5);
        // 0.5 * 1.0 + 0.5 * 0.8 = 0.9
        assert!((out.score - 0.9).abs() < 1e-9);
        assert!(out.reasons[0].contains("Cold-start"));
        assert!(out.reasons[0].contains("current: 5"));
    }

    #[test]
    fn cold_start_below_threshold_9() {
        let inp = CandidateInputs {
            skills: &skills(&["rust"]),
            years_experience: 5,
            days_since_last_active: 90.0,  // recency = 0.5
            completeness_raw: 100,         // completeness = 1.0
        };
        let out = score_cold_start(&inp, 9);
        // 0.5 * 0.5 + 0.5 * 1.0 = 0.75
        assert!((out.score - 0.75).abs() < 1e-9);
    }

    // ---- blended scoring ----------------------------------------------------

    #[test]
    fn blended_score_perfect_candidate() {
        let c = CandidateInputs {
            skills: &skills(&["rust", "postgres"]),
            years_experience: 5,
            days_since_last_active: 0.0,
            completeness_raw: 100,
        };
        let r = RoleInputs {
            required_skills: &skills(&["rust", "postgres"]),
            min_years: 3,
        };
        let w = BlendWeights::default(); // 40/30/15/15
        let out = score_blended(&c, &r, &w);
        // skill=1.0, exp=1.0, rec=1.0, comp=1.0
        // score = (40+30+15+15)/100 = 1.0
        assert!((out.score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn blended_score_zero_candidate() {
        let c = CandidateInputs {
            skills: &[],
            years_experience: 0,
            days_since_last_active: 360.0, // > 180 → recency=0
            completeness_raw: 0,
        };
        let r = RoleInputs {
            required_skills: &skills(&["rust"]),
            min_years: 5,
        };
        let w = BlendWeights::default();
        let out = score_blended(&c, &r, &w);
        assert!((out.score - 0.0).abs() < 1e-9);
    }

    #[test]
    fn blended_reasons_include_all_factors() {
        let c = CandidateInputs {
            skills: &skills(&["rust"]),
            years_experience: 4,
            days_since_last_active: 30.0,
            completeness_raw: 75,
        };
        let r = RoleInputs {
            required_skills: &skills(&["rust", "postgres"]),
            min_years: 4,
        };
        let w = BlendWeights::default();
        let out = score_blended(&c, &r, &w);
        assert!(out.reasons.iter().any(|r| r.contains("Skill match")));
        assert!(out.reasons.iter().any(|r| r.contains("Experience")));
        assert!(out.reasons.iter().any(|r| r.contains("Recency")));
        assert!(out.reasons.iter().any(|r| r.contains("completeness")));
    }

    // ---- threshold crossing at exactly 10 -----------------------------------

    #[test]
    fn threshold_10_is_blended() {
        // At total_feedback == 10, the caller should use blended not cold-start.
        // This tests the COLD_START_THRESHOLD constant is correct.
        assert_eq!(COLD_START_THRESHOLD, 10);
        // The scoring functions themselves don't have access to total_feedback;
        // the caller decides. We verify the constant.
    }

    // ---- weight normalization -----------------------------------------------

    #[test]
    fn weights_default_sum_to_100() {
        let w = BlendWeights::default();
        assert_eq!(w.skills + w.experience + w.recency + w.completeness, 100);
    }
}
