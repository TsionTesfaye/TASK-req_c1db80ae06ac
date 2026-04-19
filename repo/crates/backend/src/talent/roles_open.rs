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
            status: r.status,
            opened_at: r.opened_at,
            created_at: r.created_at,
        }
    }
}

const SELECT_COLS: &str =
    "id, title, department_id, required_skills, min_years, site_id, status, opened_at, created_at";

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

    let row = sqlx::query_as::<_, RoleOpenRow>(&format!(
        "INSERT INTO roles_open \
         (title, department_id, required_skills, min_years, site_id, status, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         RETURNING {SELECT_COLS}"
    ))
    .bind(&req.title)
    .bind(req.department_id)
    .bind(&req.required_skills)
    .bind(req.min_years)
    .bind(req.site_id)
    .bind(status)
    .bind(created_by)
    .fetch_one(pool)
    .await?;

    Ok(row)
}
