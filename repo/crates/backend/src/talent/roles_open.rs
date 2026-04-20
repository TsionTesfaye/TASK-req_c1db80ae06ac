//! Open role repository functions (T4 list, T5 create).

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use terraops_shared::dto::talent::{CreateRoleRequest, RoleOpenItem};
use uuid::Uuid;

use crate::errors::AppError;

#[derive(FromRow)]
pub struct RoleOpenRow {
    pub id: Uuid,
    pub title: String,
    pub department_id: Option<Uuid>,
    pub required_skills: Vec<String>,
    pub min_years: i32,
    pub site_id: Option<Uuid>,
    // Migration 0031 — extended role requirements.
    pub required_major: Option<String>,
    pub min_education: Option<String>,
    pub required_availability: Option<String>,
    pub status: String,
    pub opened_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl From<RoleOpenRow> for RoleOpenItem {
    fn from(r: RoleOpenRow) -> Self {
        RoleOpenItem {
            id: r.id,
            title: r.title,
            department_id: r.department_id,
            required_skills: r.required_skills,
            min_years: r.min_years,
            site_id: r.site_id,
            required_major: r.required_major,
            min_education: r.min_education,
            required_availability: r.required_availability,
            status: r.status,
            opened_at: r.opened_at,
            created_at: r.created_at,
        }
    }
}

const SELECT_COLS: &str =
    "id, title, department_id, required_skills, min_years, site_id, \
     required_major, min_education, required_availability, \
     status, opened_at, created_at";

/// List open roles (all statuses for internal use, or optionally filter by status).
pub async fn list(pool: &PgPool, limit: i64, offset: i64) -> Result<(Vec<RoleOpenRow>, i64), AppError> {
    let rows: Vec<RoleOpenRow> = sqlx::query_as::<_, RoleOpenRow>(
        &format!(
            "SELECT {SELECT_COLS} FROM roles_open \
             ORDER BY created_at DESC LIMIT $1 OFFSET $2"
        ),
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM roles_open")
        .fetch_one(pool)
        .await?;

    Ok((rows, total))
}

/// Get a single open role by id.
pub async fn get_by_id(pool: &PgPool, id: Uuid) -> Result<RoleOpenRow, AppError> {
    sqlx::query_as::<_, RoleOpenRow>(&format!(
        "SELECT {SELECT_COLS} FROM roles_open WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

/// Create a new open role.
pub async fn create(
    pool: &PgPool,
    req: &CreateRoleRequest,
    created_by: Uuid,
) -> Result<RoleOpenRow, AppError> {
    let status = req
        .status
        .as_deref()
        .unwrap_or("open");

    if !matches!(status, "open" | "closed" | "filled") {
        return Err(AppError::Validation(
            "status must be 'open', 'closed', or 'filled'".into(),
        ));
    }

    let trim_opt = |o: &Option<String>| -> Option<String> {
        o.as_ref().and_then(|s| {
            let t = s.trim();
            if t.is_empty() { None } else { Some(t.to_string()) }
        })
    };
    let req_major = trim_opt(&req.required_major);
    let min_edu = trim_opt(&req.min_education);
    let req_avail = trim_opt(&req.required_availability);

    let row = sqlx::query_as::<_, RoleOpenRow>(&format!(
        "INSERT INTO roles_open \
         (title, department_id, required_skills, min_years, site_id, \
          required_major, min_education, required_availability, \
          status, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         RETURNING {SELECT_COLS}"
    ))
    .bind(&req.title)
    .bind(req.department_id)
    .bind(&req.required_skills)
    .bind(req.min_years)
    .bind(req.site_id)
    .bind(req_major.as_deref())
    .bind(min_edu.as_deref())
    .bind(req_avail.as_deref())
    .bind(status)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(row)
}
