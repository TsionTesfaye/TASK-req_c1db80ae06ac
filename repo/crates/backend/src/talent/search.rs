//! TSV full-text search + filter helpers for candidate queries.
//!
//! Used by `candidates::list_candidates` to build the WHERE clause
//! dynamically based on query parameters.

use serde::Deserialize;

/// Query parameters accepted by GET /api/v1/talent/candidates.
#[derive(Debug, Default, Deserialize)]
pub struct CandidateQuery {
    /// Free-text search (matched against the `search_tsv` column).
    pub q: Option<String>,
    /// Comma-separated list of required skills (intersection filter).
    pub skills: Option<String>,
    /// Minimum years of experience.
    pub min_years: Option<i32>,
    /// Location substring match (case-insensitive).
    pub location: Option<String>,
    /// Field of study substring match (migration 0031).
    pub major: Option<String>,
    /// Minimum education level: highschool|associate|bachelor|master|phd
    /// (migration 0031). Compared with an inline ordinal ranking.
    pub min_education: Option<String>,
    /// Availability substring match (migration 0031), e.g. "immediate".
    pub availability: Option<String>,
    /// Page number (1-based, defaults to 1).
    pub page: Option<u32>,
    /// Page size (defaults to 50, max 200).
    pub page_size: Option<u32>,
}

impl CandidateQuery {
    /// Resolve pagination defaults.
    pub fn resolved_page(&self) -> (u32, u32) {
        let page = self.page.unwrap_or(1).max(1);
        let page_size = self
            .page_size
            .unwrap_or(50)
            .clamp(1, 200);
        (page, page_size)
    }

    /// Parse `skills` CSV into a Vec<String>.
    pub fn parsed_skills(&self) -> Vec<String> {
        self.skills
            .as_deref()
            .map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_lowercase())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
}
