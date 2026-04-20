//! Database operations for metric_definitions and metric_computations.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use terraops_shared::dto::metric::{MetricDefinitionDto, SeriesPoint};

#[derive(sqlx::FromRow)]
struct DefRow {
    id: Uuid,
    name: String,
    formula_kind: String,
    params: Value,
    source_ids: Vec<Uuid>,
    window_seconds: i32,
    enabled: bool,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<DefRow> for MetricDefinitionDto {
    fn from(r: DefRow) -> Self {
        MetricDefinitionDto {
            id: r.id,
            name: r.name,
            formula_kind: r.formula_kind,
            params: r.params,
            source_ids: r.source_ids,
            window_seconds: r.window_seconds,
            enabled: r.enabled,
            created_by: r.created_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub async fn list(pool: &PgPool, limit: i64, offset: i64) -> AppResult<(Vec<MetricDefinitionDto>, i64)> {
    let rows: Vec<DefRow> = sqlx::query_as(
        "SELECT id, name, formula_kind, params, source_ids, window_seconds, \
                enabled, created_by, created_at, updated_at \
         FROM metric_definitions WHERE deleted_at IS NULL \
         ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM metric_definitions WHERE deleted_at IS NULL",
    )
    .fetch_one(pool)
    .await?;

    Ok((rows.into_iter().map(Into::into).collect(), total.0))
}

pub async fn create(
    pool: &PgPool,
    name: &str,
    formula_kind: &str,
    params: Value,
    source_ids: &[Uuid],
    window_seconds: i32,
    created_by: Uuid,
) -> AppResult<MetricDefinitionDto> {
    let row: DefRow = sqlx::query_as(
        "INSERT INTO metric_definitions (name, formula_kind, params, source_ids, window_seconds, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING id, name, formula_kind, params, source_ids, window_seconds, \
                   enabled, created_by, created_at, updated_at",
    )
    .bind(name)
    .bind(formula_kind)
    .bind(params)
    .bind(source_ids)
    .bind(window_seconds)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(row.into())
}

pub async fn get(pool: &PgPool, id: Uuid) -> AppResult<MetricDefinitionDto> {
    let row: DefRow = sqlx::query_as(
        "SELECT id, name, formula_kind, params, source_ids, window_seconds, \
                enabled, created_by, created_at, updated_at \
         FROM metric_definitions WHERE id = $1 AND deleted_at IS NULL",
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
    formula_kind: Option<&str>,
    params: Option<Value>,
    source_ids: Option<&[Uuid]>,
    window_seconds: Option<i32>,
    enabled: Option<bool>,
) -> AppResult<MetricDefinitionDto> {
    let existing = get(pool, id).await?;

    let new_name = name.unwrap_or(&existing.name);
    let new_formula_kind = formula_kind.unwrap_or(&existing.formula_kind);
    let new_params = params.unwrap_or(existing.params);
    let new_source_ids: Vec<Uuid> = source_ids
        .map(|s| s.to_vec())
        .unwrap_or(existing.source_ids);
    let new_window = window_seconds.unwrap_or(existing.window_seconds);
    let new_enabled = enabled.unwrap_or(existing.enabled);

    let row: DefRow = sqlx::query_as(
        "UPDATE metric_definitions \
         SET name=$1, formula_kind=$2, params=$3, source_ids=$4, window_seconds=$5, enabled=$6 \
         WHERE id=$7 AND deleted_at IS NULL \
         RETURNING id, name, formula_kind, params, source_ids, window_seconds, \
                   enabled, created_by, created_at, updated_at",
    )
    .bind(new_name)
    .bind(new_formula_kind)
    .bind(new_params)
    .bind(&new_source_ids)
    .bind(new_window)
    .bind(new_enabled)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(row.into())
}

pub async fn soft_delete(pool: &PgPool, id: Uuid) -> AppResult<()> {
    let affected = sqlx::query(
        "UPDATE metric_definitions SET deleted_at=NOW(), enabled=FALSE \
         WHERE id=$1 AND deleted_at IS NULL",
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
// Computations
// ---------------------------------------------------------------------------

/// Store a completed computation in `metric_computations`, including the
/// optional `alignment` + `confidence` quality dimensions (migration 0023).
///
/// `id` is supplied by the caller so the same value can be stamped on the
/// live `SeriesPoint.computation_id` **and** used to locate the row later
/// via the `/metrics/computations/{id}/lineage` endpoint. Collisions on the
/// primary key would be an application-level bug (every caller must mint a
/// fresh `Uuid::new_v4()`).
#[allow(clippy::too_many_arguments)]
pub async fn save_computation(
    pool: &PgPool,
    id: Uuid,
    definition_id: Uuid,
    result: f64,
    inputs: Value,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    alignment: Option<f64>,
    confidence: Option<f64>,
) -> AppResult<Uuid> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO metric_computations \
            (id, definition_id, result, inputs, window_start, window_end, alignment, confidence) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
    )
    .bind(id)
    .bind(definition_id)
    .bind(result)
    .bind(inputs)
    .bind(window_start)
    .bind(window_end)
    .bind(alignment)
    .bind(confidence)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Return the latest N computed series points for the given definition.
pub async fn latest_series(
    pool: &PgPool,
    definition_id: Uuid,
    limit: i64,
) -> AppResult<Vec<SeriesPoint>> {
    #[derive(sqlx::FromRow)]
    struct Pt {
        id: Uuid,
        computed_at: DateTime<Utc>,
        result: f64,
    }
    let pts: Vec<Pt> = sqlx::query_as(
        "SELECT id, computed_at, result FROM metric_computations \
         WHERE definition_id = $1 ORDER BY computed_at DESC LIMIT $2",
    )
    .bind(definition_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(pts
        .into_iter()
        .map(|p| SeriesPoint {
            at: p.computed_at,
            value: p.result,
            computation_id: Some(p.id),
        })
        .collect())
}
