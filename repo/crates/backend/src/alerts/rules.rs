//! CRUD for alert_rules and alert_events.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use terraops_shared::dto::alert::{AlertEventDto, AlertRuleDto};

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct RuleRow {
    id: Uuid,
    metric_definition_id: Uuid,
    threshold: f64,
    operator: String,
    duration_seconds: i32,
    severity: String,
    enabled: bool,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<RuleRow> for AlertRuleDto {
    fn from(r: RuleRow) -> Self {
        AlertRuleDto {
            id: r.id,
            metric_definition_id: r.metric_definition_id,
            threshold: r.threshold,
            operator: r.operator,
            duration_seconds: r.duration_seconds,
            severity: r.severity,
            enabled: r.enabled,
            created_by: r.created_by,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub async fn list_rules(pool: &PgPool, limit: i64, offset: i64) -> AppResult<(Vec<AlertRuleDto>, i64)> {
    let rows: Vec<RuleRow> = sqlx::query_as(
        "SELECT id, metric_definition_id, threshold, operator, duration_seconds, \
                severity, enabled, created_by, created_at, updated_at \
         FROM alert_rules ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::BIGINT FROM alert_rules")
            .fetch_one(pool)
            .await?;
    Ok((rows.into_iter().map(Into::into).collect(), total.0))
}

pub async fn create_rule(
    pool: &PgPool,
    metric_definition_id: Uuid,
    threshold: f64,
    operator: &str,
    duration_seconds: i32,
    severity: &str,
    created_by: Uuid,
) -> AppResult<AlertRuleDto> {
    let row: RuleRow = sqlx::query_as(
        "INSERT INTO alert_rules (metric_definition_id, threshold, operator, duration_seconds, severity, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING id, metric_definition_id, threshold, operator, duration_seconds, \
                   severity, enabled, created_by, created_at, updated_at",
    )
    .bind(metric_definition_id)
    .bind(threshold)
    .bind(operator)
    .bind(duration_seconds)
    .bind(severity)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(row.into())
}

pub async fn get_rule(pool: &PgPool, id: Uuid) -> AppResult<AlertRuleDto> {
    let row: RuleRow = sqlx::query_as(
        "SELECT id, metric_definition_id, threshold, operator, duration_seconds, \
                severity, enabled, created_by, created_at, updated_at \
         FROM alert_rules WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(row.into())
}

pub async fn update_rule(
    pool: &PgPool,
    id: Uuid,
    threshold: Option<f64>,
    operator: Option<&str>,
    duration_seconds: Option<i32>,
    severity: Option<&str>,
    enabled: Option<bool>,
) -> AppResult<AlertRuleDto> {
    let existing = get_rule(pool, id).await?;
    let row: RuleRow = sqlx::query_as(
        "UPDATE alert_rules \
         SET threshold=$1, operator=$2, duration_seconds=$3, severity=$4, enabled=$5 \
         WHERE id=$6 \
         RETURNING id, metric_definition_id, threshold, operator, duration_seconds, \
                   severity, enabled, created_by, created_at, updated_at",
    )
    .bind(threshold.unwrap_or(existing.threshold))
    .bind(operator.unwrap_or(&existing.operator))
    .bind(duration_seconds.unwrap_or(existing.duration_seconds))
    .bind(severity.unwrap_or(&existing.severity))
    .bind(enabled.unwrap_or(existing.enabled))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(row.into())
}

pub async fn delete_rule(pool: &PgPool, id: Uuid) -> AppResult<()> {
    let n = sqlx::query("DELETE FROM alert_rules WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct EventRow {
    id: Uuid,
    rule_id: Uuid,
    fired_at: DateTime<Utc>,
    value: f64,
    acked_at: Option<DateTime<Utc>>,
    acked_by: Option<Uuid>,
    resolved_at: Option<DateTime<Utc>>,
    severity: String,
}

impl From<EventRow> for AlertEventDto {
    fn from(r: EventRow) -> Self {
        AlertEventDto {
            id: r.id,
            rule_id: r.rule_id,
            fired_at: r.fired_at,
            value: r.value,
            acked_at: r.acked_at,
            acked_by: r.acked_by,
            resolved_at: r.resolved_at,
            severity: r.severity,
        }
    }
}

pub async fn list_events(
    pool: &PgPool,
    rule_id: Option<Uuid>,
    unacked_only: bool,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<AlertEventDto>, i64)> {
    let rows: Vec<EventRow> = sqlx::query_as(
        "SELECT ae.id, ae.rule_id, ae.fired_at, ae.value, ae.acked_at, ae.acked_by, \
                ae.resolved_at, ar.severity \
         FROM alert_events ae \
         JOIN alert_rules ar ON ar.id = ae.rule_id \
         WHERE ($1::uuid IS NULL OR ae.rule_id = $1) \
           AND (NOT $2 OR ae.acked_at IS NULL) \
         ORDER BY ae.fired_at DESC LIMIT $3 OFFSET $4",
    )
    .bind(rule_id)
    .bind(unacked_only)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM alert_events ae \
         WHERE ($1::uuid IS NULL OR ae.rule_id = $1) \
           AND (NOT $2 OR ae.acked_at IS NULL)",
    )
    .bind(rule_id)
    .bind(unacked_only)
    .fetch_one(pool)
    .await?;

    Ok((rows.into_iter().map(Into::into).collect(), total.0))
}

pub async fn ack_event(pool: &PgPool, event_id: Uuid, user_id: Uuid) -> AppResult<AlertEventDto> {
    let row: EventRow = sqlx::query_as(
        "UPDATE alert_events SET acked_at=NOW(), acked_by=$1 \
         WHERE id=$2 AND acked_at IS NULL \
         RETURNING ae.id, ae.rule_id, ae.fired_at, ae.value, ae.acked_at, ae.acked_by, \
                   ae.resolved_at, \
                   (SELECT severity FROM alert_rules WHERE id = ae.rule_id) AS severity",
    )
    .bind(user_id)
    .bind(event_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(row.into())
}

/// Convenience used by the evaluator: fire a new event row.
pub async fn fire_event(pool: &PgPool, rule_id: Uuid, value: f64) -> AppResult<Uuid> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO alert_events (rule_id, value) VALUES ($1, $2) RETURNING id",
    )
    .bind(rule_id)
    .bind(value)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Resolve the latest unresolved event for the given rule.
pub async fn resolve_event(pool: &PgPool, rule_id: Uuid) -> AppResult<()> {
    sqlx::query(
        "UPDATE alert_events SET resolved_at=NOW() \
         WHERE rule_id=$1 AND resolved_at IS NULL",
    )
    .bind(rule_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Check whether an active (unresolved) event already exists for the rule.
pub async fn has_active_event(pool: &PgPool, rule_id: Uuid) -> AppResult<bool> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM alert_events \
         WHERE rule_id=$1 AND resolved_at IS NULL",
    )
    .bind(rule_id)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}
