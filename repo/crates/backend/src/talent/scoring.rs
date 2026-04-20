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
    /// Migration 0031: candidate's field of study. `None` when unknown.
    pub major: Option<&'a str>,
    /// Migration 0031: highest attained education level. `None` when unknown.
    pub education: Option<&'a str>,
    /// Migration 0031: availability note. `None` when unknown.
    pub availability: Option<&'a str>,
}

impl<'a> CandidateInputs<'a> {
    /// Helper for tests and callers that do not care about the extended
    /// profile dimensions. Produces a minimal input set with None for all
    /// 0031 fields.
    pub fn basic(
        skills: &'a [String],
        years_experience: i32,
        days_since_last_active: f64,
        completeness_raw: i32,
    ) -> Self {
        Self {
            skills,
            years_experience,
            days_since_last_active,
            completeness_raw,
            major: None,
            education: None,
            availability: None,
        }
    }
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
    /// Migration 0031: required field of study for the role (optional).
    pub required_major: Option<&'a str>,
    /// Migration 0031: minimum education level for the role (optional).
    pub min_education: Option<&'a str>,
    /// Migration 0031: required availability for the role (optional).
    pub required_availability: Option<&'a str>,
}

impl<'a> RoleInputs<'a> {
    /// Helper for tests that do not exercise the 0031 dimensions.
    pub fn basic(required_skills: &'a [String], min_years: i32) -> Self {
        Self {
            required_skills,
            min_years,
            required_major: None,
            min_education: None,
            required_availability: None,
        }
    }
}

// ── Extended (migration 0031) component matchers ────────────────────────────

/// Ordinal rank for an education level string. Lower-case comparison.
/// Unknown / missing → 0.
pub fn education_rank(level: Option<&str>) -> i32 {
    match level.map(|s| s.trim().to_lowercase()) {
        Some(s) if s == "highschool" || s == "high_school" => 1,
        Some(s) if s == "associate" => 2,
        Some(s) if s == "bachelor" => 3,
        Some(s) if s == "master" => 4,
        Some(s) if s == "phd" || s == "doctorate" => 5,
        _ => 0,
    }
}

/// 1.0 when the role does not require a specific major OR the candidate's
/// major contains the required substring (case-insensitive); 0.0 otherwise.
pub fn major_match(candidate_major: Option<&str>, required_major: Option<&str>) -> f64 {
    match required_major {
        None => 1.0,
        Some(req) if req.trim().is_empty() => 1.0,
        Some(req) => match candidate_major {
            Some(m) if m.to_lowercase().contains(&req.to_lowercase()) => 1.0,
            _ => 0.0,
        },
    }
}

/// 1.0 when the role has no min-education requirement OR the candidate's
/// education rank is ≥ the required rank; scaled partial credit otherwise.
pub fn education_level_match(
    candidate_education: Option<&str>,
    min_education: Option<&str>,
) -> f64 {
    let required = education_rank(min_education);
    if required == 0 {
        return 1.0;
    }
    let actual = education_rank(candidate_education);
    if actual >= required {
        1.0
    } else {
        actual as f64 / required as f64
    }
}

/// 1.0 when the role has no availability requirement OR the candidate's
/// availability contains the required substring (case-insensitive).
pub fn availability_match(
    candidate_availability: Option<&str>,
    required_availability: Option<&str>,
) -> f64 {
    match required_availability {
        None => 1.0,
        Some(req) if req.trim().is_empty() => 1.0,
        Some(req) => match candidate_availability {
            Some(a) if a.to_lowercase().contains(&req.to_lowercase()) => 1.0,
            _ => 0.0,
        },
    }
}

/// Extended-match multiplier applied on top of the blended score.
/// Returns 1.0 when the role imposes no 0031 constraints, preserving
/// backward-compatible test expectations. When constraints are present,
/// returns the mean of the three match scores (each in [0,1]).
fn extended_match_multiplier(candidate: &CandidateInputs<'_>, role: &RoleInputs<'_>) -> f64 {
    let any_constraint = role.required_major.map(|s| !s.trim().is_empty()).unwrap_or(false)
        || role.min_education.map(|s| !s.trim().is_empty()).unwrap_or(false)
        || role
            .required_availability
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
    if !any_constraint {
        return 1.0;
    }
    let mm = major_match(candidate.major, role.required_major);
    let em = education_level_match(candidate.education, role.min_education);
    let am = availability_match(candidate.availability, role.required_availability);
    ((mm + em + am) / 3.0).clamp(0.0, 1.0)
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

    let core_score = (weights.skills as f64 / 100.0) * sm
        + (weights.experience as f64 / 100.0) * em
        + (weights.recency as f64 / 100.0) * rs
        + (weights.completeness as f64 / 100.0) * cs;

    // Apply the extended (migration 0031) match multiplier. When the role
    // imposes no major/education/availability constraint this multiplier
    // is 1.0 and the score is unchanged.
    let ext_mul = extended_match_multiplier(candidate, role);
    let score = core_score * ext_mul;

    let matched_count = {
        let req: HashSet<&str> = role.required_skills.iter().map(|s| s.as_str()).collect();
        candidate
            .skills
            .iter()
            .filter(|s| req.contains(s.as_str()))
            .count()
    };

    let mut reasons = vec![
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

    // Extended (migration 0031) reasons — emitted only when the role
    // actually imposes a constraint, so vanilla roles stay concise.
    if role
        .required_major
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        reasons.push(format!(
            "Major match: required \"{}\" vs candidate \"{}\" → {:.2}",
            role.required_major.unwrap_or(""),
            candidate.major.unwrap_or("(none)"),
            major_match(candidate.major, role.required_major),
        ));
    }
    if role
        .min_education
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        reasons.push(format!(
            "Education: required \"{}\" vs candidate \"{}\" → {:.2}",
            role.min_education.unwrap_or(""),
            candidate.education.unwrap_or("(none)"),
            education_level_match(candidate.education, role.min_education),
        ));
    }
    if role
        .required_availability
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        reasons.push(format!(
            "Availability: required \"{}\" vs candidate \"{}\" → {:.2}",
            role.required_availability.unwrap_or(""),
            candidate.availability.unwrap_or("(none)"),
            availability_match(candidate.availability, role.required_availability),
        ));
    }

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
            major: None,
            education: None,
            availability: None,
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
            major: None,
            education: None,
            availability: None,
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
            major: None,
            education: None,
            availability: None,
        };
        let r = RoleInputs {
            required_skills: &skills(&["rust", "postgres"]),
            min_years: 3,
            required_major: None,
            min_education: None,
            required_availability: None,
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
            major: None,
            education: None,
            availability: None,
        };
        let r = RoleInputs {
            required_skills: &skills(&["rust"]),
            min_years: 5,
            required_major: None,
            min_education: None,
            required_availability: None,
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
            major: None,
            education: None,
            availability: None,
        };
        let r = RoleInputs {
            required_skills: &skills(&["rust", "postgres"]),
            min_years: 4,
            required_major: None,
            min_education: None,
            required_availability: None,
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

    // ---- extended (migration 0031) component matchers ----------------------

    #[test]
    fn education_rank_known_levels() {
        assert_eq!(education_rank(Some("bachelor")), 3);
        assert_eq!(education_rank(Some("PhD")), 5);
        assert_eq!(education_rank(Some("master")), 4);
        assert_eq!(education_rank(Some("associate")), 2);
        assert_eq!(education_rank(Some("highschool")), 1);
        assert_eq!(education_rank(Some("unknown")), 0);
        assert_eq!(education_rank(None), 0);
    }

    #[test]
    fn major_match_role_no_requirement_is_one() {
        assert!((major_match(None, None) - 1.0).abs() < 1e-9);
        assert!((major_match(Some("CS"), None) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn major_match_substring_hit() {
        let m = major_match(Some("Industrial Engineering"), Some("engineering"));
        assert!((m - 1.0).abs() < 1e-9);
    }

    #[test]
    fn major_match_miss_is_zero() {
        let m = major_match(Some("Philosophy"), Some("engineering"));
        assert!((m - 0.0).abs() < 1e-9);
    }

    #[test]
    fn education_level_match_meets_or_exceeds() {
        assert!((education_level_match(Some("master"), Some("bachelor")) - 1.0).abs() < 1e-9);
        assert!((education_level_match(Some("phd"), Some("bachelor")) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn education_level_match_below_gives_partial_credit() {
        // candidate=associate (2), required=master (4) → 2/4 = 0.5
        let m = education_level_match(Some("associate"), Some("master"));
        assert!((m - 0.5).abs() < 1e-9);
    }

    #[test]
    fn availability_match_immediate_required_and_met() {
        let m = availability_match(Some("immediate"), Some("immediate"));
        assert!((m - 1.0).abs() < 1e-9);
    }

    #[test]
    fn availability_match_miss() {
        let m = availability_match(Some("2 weeks notice"), Some("immediate"));
        assert!((m - 0.0).abs() < 1e-9);
    }

    #[test]
    fn blended_with_extended_constraints_penalizes_mismatch() {
        // Fully-matching core (skills/exp/rec/comp all 1.0) but wrong major
        // + wrong education + wrong availability → extended multiplier = 0.0
        // → final score should be 0.0.
        let s = skills(&["rust"]);
        let c = CandidateInputs {
            skills: &s,
            years_experience: 10,
            days_since_last_active: 0.0,
            completeness_raw: 100,
            major: Some("Philosophy"),
            education: Some("highschool"),
            availability: Some("part-time"),
        };
        let r = RoleInputs {
            required_skills: &s,
            min_years: 3,
            required_major: Some("engineering"),
            min_education: Some("master"),
            required_availability: Some("immediate"),
        };
        let w = BlendWeights::default();
        let out = score_blended(&c, &r, &w);
        assert!(out.score < 0.3, "expected low score for mismatches, got {}", out.score);
        assert!(out.reasons.iter().any(|r| r.contains("Major match")));
        assert!(out.reasons.iter().any(|r| r.contains("Education:")));
        assert!(out.reasons.iter().any(|r| r.contains("Availability:")));
    }

    #[test]
    fn blended_with_all_extended_matches_preserves_score() {
        // All constraints satisfied → multiplier 1.0, score unchanged.
        let s = skills(&["rust", "postgres"]);
        let c = CandidateInputs {
            skills: &s,
            years_experience: 5,
            days_since_last_active: 0.0,
            completeness_raw: 100,
            major: Some("Industrial Engineering"),
            education: Some("master"),
            availability: Some("immediate"),
        };
        let r = RoleInputs {
            required_skills: &s,
            min_years: 3,
            required_major: Some("engineering"),
            min_education: Some("bachelor"),
            required_availability: Some("immediate"),
        };
        let w = BlendWeights::default();
        let out = score_blended(&c, &r, &w);
        assert!((out.score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn weights_default_sum_to_100() {
        let w = BlendWeights::default();
        assert_eq!(w.skills + w.experience + w.recency + w.completeness, 100);
    }
}
