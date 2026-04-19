//! Database operations for env_sources.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use terraops_shared::dto::env_source::{EnvSourceDto, ObservationDto, ObservationInput};

#[derive(sqlx::FromRow)]
pub struct EnvSourceRow {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub unit_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<EnvSourceRow> for EnvSourceDto {
    fn from(r: EnvSourceRow) -> Self {
        EnvSourceDto {
            id: r.id,
            name: r.name,
            kind: r.kind,
            site_id: r.site_id,
            department_id: r.department_id,
            unit_id: r.unit_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub async fn list(pool: &PgPool, limit: i64, offset: i64) -> AppResult<(Vec<EnvSourceDto>, i64)> {
    let rows: Vec<EnvSourceRow> = sqlx::query_as(
        "SELECT id, name, kind, site_id, department_id, unit_id, created_at, updated_at \
         FROM env_sources WHERE deleted_at IS NULL \
         ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM env_sources WHERE deleted_at IS NULL",
    )
    .fetch_one(pool)
    .await?;

    Ok((rows.into_iter().map(Into::into).collect(), total.0))
}

pub async fn create(
    pool: &PgPool,
    name: &str,
    kind: &str,
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    unit_id: Option<Uuid>,
) -> AppResult<EnvSourceDto> {
    let row: EnvSourceRow = sqlx::query_as(
        "INSERT INTO env_sources (name, kind, site_id, department_id, unit_id) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, name, kind, site_id, department_id, unit_id, created_at, updated_at",
    )
    .bind(name)
    .bind(kind)
    .bind(site_id)
    .bind(department_id)
    .bind(unit_id)
    .fetch_one(pool)
    .await?;
    Ok(row.into())
}

pub async fn get(pool: &PgPool, id: Uuid) -> AppResult<EnvSourceDto> {
    let row: EnvSourceRow = sqlx::query_as(
        "SELECT id, name, kind, site_id, department_id, unit_id, created_at, updated_at \
         FROM env_sources WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(row.into())
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    name: Option<&str>,
    kind: Option<&str>,
    site_id: Option<Option<Uuid>>,
    department_id: Option<Option<Uuid>>,
    unit_id: Option<Option<Uuid>>,
) -> AppResult<EnvSourceDto> {
    // Verify exists first
    let existing = get(pool, id).await?;

    let new_name = name.unwrap_or(&existing.name);
    let new_kind = kind.unwrap_or(&existing.kind);
    let new_site = site_id.unwrap_or(existing.site_id);
    let new_dept = department_id.unwrap_or(existing.department_id);
    let new_unit = unit_id.unwrap_or(existing.unit_id);

    let row: EnvSourceRow = sqlx::query_as(
        "UPDATE env_sources SET name=$1, kind=$2, site_id=$3, department_id=$4, unit_id=$5 \
         WHERE id=$6 AND deleted_at IS NULL \
         RETURNING id, name, kind, site_id, department_id, unit_id, created_at, updated_at",
    )
    .bind(new_name)
    .bind(new_kind)
    .bind(new_site)
    .bind(new_dept)
    .bind(new_unit)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(row.into())
}

pub async fn soft_delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
    let affected = sqlx::query(
        "UPDATE env_sources SET deleted_at=NOW() WHERE id=$1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?
    .rows_affected();
    if affected == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Observations
// ---------------------------------------------------------------------------

pub async fn bulk_insert_observations(
    pool: &PgPool,
    source_id: Uuid,
    obs: &[ObservationInput],
) -> AppResult<usize> {
    // Verify source exists
    get(pool, source_id).await?;

    let mut inserted = 0usize;
    for o in obs {
        sqlx::query(
            "INSERT INTO env_observations (source_id, observed_at, value, unit) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(source_id)
        .bind(o.observed_at)
        .bind(o.value)
        .bind(&o.unit)
        .execute(pool)
        .await?;
        inserted += 1;
    }
    Ok(inserted)
}

#[derive(sqlx::FromRow)]
struct ObsRow {
    id: Uuid,
    source_id: Uuid,
    observed_at: DateTime<Utc>,
    value: f64,
    unit: String,
}

pub async fn list_observations(
    pool: &PgPool,
    source_id: Option<Uuid>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<ObservationDto>, i64)> {
    let rows: Vec<ObsRow> = sqlx::query_as(
        "SELECT id, source_id, observed_at, value, unit \
         FROM env_observations \
         WHERE ($1::uuid IS NULL OR source_id = $1) \
           AND ($2::timestamptz IS NULL OR observed_at >= $2) \
           AND ($3::timestamptz IS NULL OR observed_at <= $3) \
         ORDER BY observed_at DESC LIMIT $4 OFFSET $5",
    )
    .bind(source_id)
    .bind(from)
    .bind(to)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM env_observations \
         WHERE ($1::uuid IS NULL OR source_id = $1) \
           AND ($2::timestamptz IS NULL OR observed_at >= $2) \
           AND ($3::timestamptz IS NULL OR observed_at <= $3)",
    )
    .bind(source_id)
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;

    let dtos = rows
        .into_iter()
        .map(|r| ObservationDto {
            id: r.id,
            source_id: r.source_id,
            observed_at: r.observed_at,
            value: r.value,
            unit: r.unit,
        })
        .collect();
    Ok((dtos, total.0))
}

/// Fetch raw window points for a given source for formula computation.
pub async fn fetch_window(
    pool: &PgPool,
    source_id: Uuid,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> AppResult<Vec<(DateTime<Utc>, f64)>> {
    #[derive(sqlx::FromRow)]
    struct Pt {
        observed_at: DateTime<Utc>,
        value: f64,
    }
    let pts: Vec<Pt> = sqlx::query_as(
        "SELECT observed_at, value FROM env_observations \
         WHERE source_id = $1 AND observed_at >= $2 AND observed_at <= $3 \
         ORDER BY observed_at ASC",
    )
    .bind(source_id)
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;
    Ok(pts.into_iter().map(|p| (p.observed_at, p.value)).collect())
}
