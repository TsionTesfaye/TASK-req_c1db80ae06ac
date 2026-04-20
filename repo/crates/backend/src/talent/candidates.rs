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

/// List candidates with optional TSV search + filters.
/// Returns (rows, total_count).
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
    limit: i64,
    offset: i64,
) -> Result<(Vec<CandidateRow>, i64), AppError> {
    let rows = build_list_query(
        pool, q, skills_filter, min_years, location, major, min_education, availability, limit,
        offset,
    )
    .await?;
    let total = build_count_query(
        pool, q, skills_filter, min_years, location, major, min_education, availability,
    )
    .await?;
    Ok((rows, total))
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

    qb.push(" ORDER BY last_active_at DESC LIMIT ");
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
