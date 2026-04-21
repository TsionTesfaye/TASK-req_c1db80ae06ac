//! Candidate repository functions used by the talent handlers.
//!
//! Handles T1 (list + search), T2 (create/upsert), T3 (get by id).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use terraops_shared::dto::talent::{CandidateDetail, CandidateListItem, UpsertCandidateRequest};
use uuid::Uuid;

use crate::errors::AppError;

#[derive(FromRow)]
pub struct CandidateRow {
    pub id: Uuid,
    pub full_name: String,
    pub email_mask: String,
    pub location: Option<String>,
    pub years_experience: i32,
    pub skills: Vec<String>,
    pub bio: Option<String>,
    // Migration 0031 — extended candidate profile dimensions.
    pub major: Option<String>,
    pub education: Option<String>,
    pub availability: Option<String>,
    pub completeness_score: i32,
    pub last_active_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<CandidateRow> for CandidateListItem {
    fn from(r: CandidateRow) -> Self {
        CandidateListItem {
            id: r.id,
            full_name: r.full_name,
            email_mask: r.email_mask,
            location: r.location,
            years_experience: r.years_experience,
            skills: r.skills,
            major: r.major,
            education: r.education,
            availability: r.availability,
            completeness_score: r.completeness_score,
            last_active_at: r.last_active_at,
        }
    }
}

impl From<CandidateRow> for CandidateDetail {
    fn from(r: CandidateRow) -> Self {
        CandidateDetail {
            id: r.id,
            full_name: r.full_name,
            email_mask: r.email_mask,
            location: r.location,
            years_experience: r.years_experience,
            skills: r.skills,
            bio: r.bio,
            major: r.major,
            education: r.education,
            availability: r.availability,
            completeness_score: r.completeness_score,
            last_active_at: r.last_active_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

/// List candidates with optional TSV search + filters + sort.
/// Returns (rows, total_count).
///
/// Audit #10 issue #3: `sort_by` and `sort_dir` are user-selectable;
/// both must already have been validated against the whitelists in
/// `crate::talent::search` (the handler rejects unknown tokens with a
/// 400 before calling this function). Unknown values fall back to the
/// safe default `last_active_at DESC`.
#[allow(clippy::too_many_arguments)]
pub async fn list(
    pool: &PgPool,
    q: Option<&str>,
    skills_filter: &[String],
    min_years: Option<i32>,
    location: Option<&str>,
    major: Option<&str>,
    min_education: Option<&str>,
    availability: Option<&str>,
    sort_by: Option<&str>,
    sort_dir: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<CandidateRow>, i64), AppError> {
    let rows = build_list_query(
        pool, q, skills_filter, min_years, location, major, min_education, availability,
        sort_by, sort_dir, limit, offset,
    )
    .await?;
    let total = build_count_query(
        pool, q, skills_filter, min_years, location, major, min_education, availability,
    )
    .await?;
    Ok((rows, total))
}

/// Emit a safe ORDER BY clause for a user-selected (column, direction)
/// pair. The column is re-matched against a hard-coded whitelist here
/// so no untrusted identifier reaches SQL even if a caller forgets the
/// handler-level validation. Falls back to `last_active_at DESC` on
/// unknown values.
fn order_by_clause(sort_by: Option<&str>, sort_dir: Option<&str>) -> &'static str {
    let dir_desc = matches!(sort_dir.map(|s| s.to_ascii_lowercase()).as_deref(), Some("desc") | None);
    match sort_by.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("created_at") => if dir_desc { " ORDER BY created_at DESC" } else { " ORDER BY created_at ASC" },
        Some("updated_at") => if dir_desc { " ORDER BY updated_at DESC" } else { " ORDER BY updated_at ASC" },
        Some("full_name") => if dir_desc { " ORDER BY full_name DESC" } else { " ORDER BY full_name ASC" },
        Some("years_experience") => if dir_desc { " ORDER BY years_experience DESC" } else { " ORDER BY years_experience ASC" },
        Some("completeness_score") => if dir_desc { " ORDER BY completeness_score DESC" } else { " ORDER BY completeness_score ASC" },
        // last_active_at or anything else → safe default.
        _ => if dir_desc { " ORDER BY last_active_at DESC" } else { " ORDER BY last_active_at ASC" },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_by_defaults_to_last_active_desc() {
        assert_eq!(order_by_clause(None, None), " ORDER BY last_active_at DESC");
    }

    #[test]
    fn order_by_unknown_column_falls_back_to_last_active() {
        assert_eq!(order_by_clause(Some("bogus"), None), " ORDER BY last_active_at DESC");
        assert_eq!(order_by_clause(Some("bogus"), Some("asc")), " ORDER BY last_active_at ASC");
    }

    #[test]
    fn order_by_created_at() {
        assert_eq!(order_by_clause(Some("created_at"), None), " ORDER BY created_at DESC");
        assert_eq!(order_by_clause(Some("created_at"), Some("asc")), " ORDER BY created_at ASC");
    }

    #[test]
    fn order_by_updated_at() {
        assert_eq!(order_by_clause(Some("updated_at"), None), " ORDER BY updated_at DESC");
        assert_eq!(order_by_clause(Some("updated_at"), Some("asc")), " ORDER BY updated_at ASC");
    }

    #[test]
    fn order_by_full_name() {
        assert_eq!(order_by_clause(Some("full_name"), None), " ORDER BY full_name DESC");
        assert_eq!(order_by_clause(Some("full_name"), Some("asc")), " ORDER BY full_name ASC");
    }

    #[test]
    fn order_by_years_experience() {
        assert_eq!(order_by_clause(Some("years_experience"), None), " ORDER BY years_experience DESC");
        assert_eq!(order_by_clause(Some("years_experience"), Some("asc")), " ORDER BY years_experience ASC");
    }

    #[test]
    fn order_by_completeness_score() {
        assert_eq!(order_by_clause(Some("completeness_score"), None), " ORDER BY completeness_score DESC");
        assert_eq!(order_by_clause(Some("completeness_score"), Some("asc")), " ORDER BY completeness_score ASC");
    }

    #[test]
    fn order_by_case_insensitive() {
        assert_eq!(order_by_clause(Some("CREATED_AT"), Some("ASC")), " ORDER BY created_at ASC");
        assert_eq!(order_by_clause(Some("Full_Name"), Some("DESC")), " ORDER BY full_name DESC");
    }
}

#[allow(clippy::too_many_arguments)]
async fn build_list_query(
    pool: &PgPool,
    q: Option<&str>,
    skills_filter: &[String],
    min_years: Option<i32>,
    location: Option<&str>,
    major: Option<&str>,
    min_education: Option<&str>,
    availability: Option<&str>,
    sort_by: Option<&str>,
    sort_dir: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<CandidateRow>, AppError> {
    // Build as a parameterized query using sqlx's QueryBuilder.
    // Since sqlx 0.7 has QueryBuilder, we use it.
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id, full_name, email_mask, location, years_experience, skills, \
         bio, major, education, availability, \
         completeness_score, last_active_at, created_at, updated_at \
         FROM candidates WHERE deleted_at IS NULL",
    );

    if let Some(q_str) = q {
        if !q_str.trim().is_empty() {
            qb.push(" AND search_tsv @@ plainto_tsquery('english', ");
            qb.push_bind(q_str.to_string());
            qb.push(")");
        }
    }
    if let Some(my) = min_years {
        qb.push(" AND years_experience >= ");
        qb.push_bind(my);
    }
    if let Some(loc) = location {
        if !loc.trim().is_empty() {
            qb.push(" AND location ILIKE ");
            qb.push_bind(format!("%{}%", loc));
        }
    }
    if !skills_filter.is_empty() {
        qb.push(" AND skills @> ");
        qb.push_bind(skills_filter.to_vec());
    }
    if let Some(m) = major {
        if !m.trim().is_empty() {
            qb.push(" AND major ILIKE ");
            qb.push_bind(format!("%{}%", m));
        }
    }
    if let Some(me) = min_education {
        if !me.trim().is_empty() {
            // Rank education levels inline: higher = more advanced.
            qb.push(
                " AND COALESCE(CASE lower(education) \
                    WHEN 'highschool' THEN 1 WHEN 'high_school' THEN 1 \
                    WHEN 'associate' THEN 2 \
                    WHEN 'bachelor' THEN 3 \
                    WHEN 'master' THEN 4 \
                    WHEN 'phd' THEN 5 WHEN 'doctorate' THEN 5 \
                    ELSE 0 END, 0) >= COALESCE(CASE lower(",
            );
            qb.push_bind(me.to_string());
            qb.push(
                ") \
                    WHEN 'highschool' THEN 1 WHEN 'high_school' THEN 1 \
                    WHEN 'associate' THEN 2 \
                    WHEN 'bachelor' THEN 3 \
                    WHEN 'master' THEN 4 \
                    WHEN 'phd' THEN 5 WHEN 'doctorate' THEN 5 \
                    ELSE 0 END, 0)",
            );
        }
    }
    if let Some(av) = availability {
        if !av.trim().is_empty() {
            qb.push(" AND availability ILIKE ");
            qb.push_bind(format!("%{}%", av));
        }
    }

    qb.push(order_by_clause(sort_by, sort_dir));
    qb.push(" LIMIT ");
    qb.push_bind(limit);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    let rows = qb
        .build_query_as::<CandidateRow>()
        .fetch_all(pool)
        .await?;
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
async fn build_count_query(
    pool: &PgPool,
    q: Option<&str>,
    skills_filter: &[String],
    min_years: Option<i32>,
    location: Option<&str>,
    major: Option<&str>,
    min_education: Option<&str>,
    availability: Option<&str>,
) -> Result<i64, AppError> {
    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT COUNT(*)::BIGINT FROM candidates WHERE deleted_at IS NULL",
    );

    if let Some(q_str) = q {
        if !q_str.trim().is_empty() {
            qb.push(" AND search_tsv @@ plainto_tsquery('english', ");
            qb.push_bind(q_str.to_string());
            qb.push(")");
        }
    }
    if let Some(my) = min_years {
        qb.push(" AND years_experience >= ");
        qb.push_bind(my);
    }
    if let Some(loc) = location {
        if !loc.trim().is_empty() {
            qb.push(" AND location ILIKE ");
            qb.push_bind(format!("%{}%", loc));
        }
    }
    if !skills_filter.is_empty() {
        qb.push(" AND skills @> ");
        qb.push_bind(skills_filter.to_vec());
    }
    if let Some(m) = major {
        if !m.trim().is_empty() {
            qb.push(" AND major ILIKE ");
            qb.push_bind(format!("%{}%", m));
        }
    }
    if let Some(me) = min_education {
        if !me.trim().is_empty() {
            qb.push(
                " AND COALESCE(CASE lower(education) \
                    WHEN 'highschool' THEN 1 WHEN 'high_school' THEN 1 \
                    WHEN 'associate' THEN 2 \
                    WHEN 'bachelor' THEN 3 \
                    WHEN 'master' THEN 4 \
                    WHEN 'phd' THEN 5 WHEN 'doctorate' THEN 5 \
                    ELSE 0 END, 0) >= COALESCE(CASE lower(",
            );
            qb.push_bind(me.to_string());
            qb.push(
                ") \
                    WHEN 'highschool' THEN 1 WHEN 'high_school' THEN 1 \
                    WHEN 'associate' THEN 2 \
                    WHEN 'bachelor' THEN 3 \
                    WHEN 'master' THEN 4 \
                    WHEN 'phd' THEN 5 WHEN 'doctorate' THEN 5 \
                    ELSE 0 END, 0)",
            );
        }
    }
    if let Some(av) = availability {
        if !av.trim().is_empty() {
            qb.push(" AND availability ILIKE ");
            qb.push_bind(format!("%{}%", av));
        }
    }

    let (count,): (i64,) = qb
        .build_query_as::<(i64,)>()
        .fetch_one(pool)
        .await?;
    Ok(count)
}

/// Get a single candidate by id (excluding soft-deleted).
pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<CandidateRow, AppError> {
    let row = sqlx::query_as::<_, CandidateRow>(
        "SELECT id, full_name, email_mask, location, years_experience, skills, \
         bio, major, education, availability, \
         completeness_score, last_active_at, created_at, updated_at \
         FROM candidates WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(row)
}

/// Create a new candidate.
pub async fn create(
    pool: &PgPool,
    req: &UpsertCandidateRequest,
) -> Result<CandidateRow, AppError> {
    if req.completeness_score < 0 || req.completeness_score > 100 {
        return Err(AppError::Validation(
            "completeness_score must be 0–100".into(),
        ));
    }
    if req.years_experience < 0 {
        return Err(AppError::Validation(
            "years_experience must be >= 0".into(),
        ));
    }

    let last_active = req.last_active_at.unwrap_or_else(Utc::now);

    let row = sqlx::query_as::<_, CandidateRow>(
        "INSERT INTO candidates \
         (full_name, email_mask, location, years_experience, skills, bio, \
          major, education, availability, \
          completeness_score, last_active_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
         RETURNING id, full_name, email_mask, location, years_experience, skills, \
         bio, major, education, availability, \
         completeness_score, last_active_at, created_at, updated_at",
    )
    .bind(&req.full_name)
    .bind(&req.email_mask)
    .bind(&req.location)
    .bind(req.years_experience)
    .bind(&req.skills)
    .bind(&req.bio)
    .bind(&req.major)
    .bind(&req.education)
    .bind(&req.availability)
    .bind(req.completeness_score)
    .bind(last_active)
    .fetch_one(pool)
    .await?;
    Ok(row)
}
