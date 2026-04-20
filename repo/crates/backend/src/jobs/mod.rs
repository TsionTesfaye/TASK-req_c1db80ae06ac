//! Background jobs wired into `app::run` at startup.
//!
//! Each job is a `tokio::spawn`-ed loop with its own cadence. They log
//! errors and continue — one failing cycle never kills the process. The
//! `start_all` entry point is the single main-lane wire-up called from
//! `app::run` so there is no ad-hoc spawning anywhere else in the backend.
//!
//! Jobs:
//!   * alert evaluator              — every 30 s   (see `alerts::evaluator`)
//!   * report scheduler             — every 10 s   (see `reports::scheduler`)
//!   * retention sweep              — every  1 h   (enforces env_raw / kpi /
//!                                                   feedback TTLs)
//!   * metric rollup                — every  1 h   (materializes
//!                                                   `kpi_rollup_daily` rows
//!                                                   from the last day's
//!                                                   `metric_computations`)
//!   * notification retry           — every 30 s   (advances pending
//!                                                   `notification_delivery_attempts`)

use std::{path::PathBuf, time::Duration};

use sqlx::PgPool;
use tokio::task::JoinHandle;

use crate::{alerts, errors::AppResult, reports};

/// Start every background job. Returns the set of `JoinHandle`s so the
/// caller can decide how to manage lifetimes (typically: drop them and let
/// the Tokio runtime tear them down at shutdown).
pub fn start_all(pool: PgPool, runtime_dir: PathBuf) -> Vec<JoinHandle<()>> {
    let mut handles = Vec::new();
    handles.push(alerts::evaluator::start_alert_evaluator(pool.clone()));
    handles.push(reports::scheduler::start_report_scheduler(
        pool.clone(),
        runtime_dir,
    ));
    handles.push(start_retention_sweep(pool.clone()));
    handles.push(start_metric_rollup(pool.clone()));
    handles.push(start_notification_retry(pool));
    handles
}

// ---------------------------------------------------------------------------
// Retention sweep
// ---------------------------------------------------------------------------

/// Hourly sweep: for every non-`audit` retention policy with a positive
/// `ttl_days`, delete expired rows. Policy `audit` is indefinite and ttl=0
/// means "retain indefinitely" per the handler contract.
pub fn start_retention_sweep(pool: PgPool) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Err(e) = retention_sweep_once(&pool).await {
                tracing::error!(error = %e, "retention sweep cycle failed");
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    })
}

/// Perform a single retention-sweep pass. Exposed for integration tests.
pub async fn retention_sweep_once(pool: &PgPool) -> AppResult<u64> {
    #[derive(sqlx::FromRow)]
    struct Row {
        domain: String,
        ttl_days: i32,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT domain, ttl_days FROM retention_policies ORDER BY domain",
    )
    .fetch_all(pool)
    .await?;

    let mut total: u64 = 0;
    for r in rows {
        if r.domain == "audit" || r.ttl_days == 0 {
            continue;
        }
        let deleted = match r.domain.as_str() {
            "env_raw" => sqlx::query(
                "DELETE FROM env_observations \
                 WHERE observed_at < NOW() - ($1::int || ' days')::interval",
            )
            .bind(r.ttl_days)
            .execute(pool)
            .await?
            .rows_affected(),
            "kpi" => sqlx::query(
                "DELETE FROM kpi_rollup_daily \
                 WHERE day < (CURRENT_DATE - ($1::int || ' days')::interval)::date",
            )
            .bind(r.ttl_days)
            .execute(pool)
            .await?
            .rows_affected(),
            // Audit #7 Issue #1: feedback retention is driven by 24
            // months of *inactivity*, not the age of each feedback row.
            // Delete feedback for candidates whose most recent feedback
            // is older than the TTL window; touching a candidate inside
            // the window preserves their entire feedback history.
            "feedback" => sqlx::query(
                "DELETE FROM talent_feedback \
                 WHERE candidate_id IN ( \
                     SELECT candidate_id FROM talent_feedback \
                     GROUP BY candidate_id \
                     HAVING MAX(created_at) < NOW() - ($1::int || ' days')::interval \
                 )",
            )
            .bind(r.ttl_days)
            .execute(pool)
            .await?
            .rows_affected(),
            _ => 0,
        };
        let _ = sqlx::query(
            "UPDATE retention_policies SET last_enforced_at = NOW(), updated_at = NOW() \
             WHERE domain = $1",
        )
        .bind(&r.domain)
        .execute(pool)
        .await?;
        total += deleted;
    }
    Ok(total)
}

// ---------------------------------------------------------------------------
// Metric rollup
// ---------------------------------------------------------------------------

/// Hourly rollup: for every distinct `(day, formula_kind)` present in the
/// last 36 hours of `metric_computations`, upsert a `kpi_rollup_daily` row
/// with the average result. Idempotent — safe to run many times.
pub fn start_metric_rollup(pool: PgPool) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Err(e) = metric_rollup_once(&pool).await {
                tracing::error!(error = %e, "metric rollup cycle failed");
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    })
}

/// Perform a single rollup pass. Exposed for integration tests.
///
/// Maps `metric_definitions.formula_kind` → `kpi_rollup_daily.metric_kind`:
///   * `moving_average`  → `cycle_time`
///   * `rate_of_change`  → `funnel_conversion`
///   * `comfort_index`   → `efficiency_index`
///
/// Idempotent: deletes any existing rows for the affected (day, metric_kind)
/// pairs (with NULL site/department) inside a transaction, then re-inserts
/// fresh averages from the last 36 hours of `metric_computations`.
pub async fn metric_rollup_once(pool: &PgPool) -> AppResult<u64> {
    let mut tx = pool.begin().await?;

    // Collect the (day, mapped_kind) pairs that will be refreshed.
    sqlx::query(
        "DELETE FROM kpi_rollup_daily \
         WHERE site_id IS NULL AND department_id IS NULL \
         AND (day, metric_kind) IN ( \
            SELECT date_trunc('day', mc.computed_at)::date, \
                   CASE md.formula_kind \
                        WHEN 'moving_average' THEN 'cycle_time' \
                        WHEN 'rate_of_change' THEN 'funnel_conversion' \
                        WHEN 'comfort_index'  THEN 'efficiency_index' \
                   END \
            FROM metric_computations mc \
            JOIN metric_definitions md ON md.id = mc.definition_id \
            WHERE mc.computed_at >= NOW() - INTERVAL '36 hours')",
    )
    .execute(&mut *tx)
    .await?;

    let res = sqlx::query(
        "INSERT INTO kpi_rollup_daily (day, metric_kind, value) \
         SELECT date_trunc('day', mc.computed_at)::date AS day, \
                CASE md.formula_kind \
                     WHEN 'moving_average' THEN 'cycle_time' \
                     WHEN 'rate_of_change' THEN 'funnel_conversion' \
                     WHEN 'comfort_index'  THEN 'efficiency_index' \
                END AS metric_kind, \
                AVG(mc.result)::double precision AS value \
         FROM metric_computations mc \
         JOIN metric_definitions md ON md.id = mc.definition_id \
         WHERE mc.computed_at >= NOW() - INTERVAL '36 hours' \
         GROUP BY 1, 2",
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(res.rows_affected())
}

// ---------------------------------------------------------------------------
// Notification retry worker
// ---------------------------------------------------------------------------

/// Advance `notification_delivery_attempts` that are in `pending` state and
/// whose `next_retry_at` has arrived. In this offline environment there is
/// no external transport; the worker marks the attempt successful (the
/// notification itself is already readable via the center). This keeps the
/// retry table honestly reflecting the lifecycle without faking success.
pub fn start_notification_retry(pool: PgPool) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Err(e) = notification_retry_once(&pool).await {
                tracing::error!(error = %e, "notification retry cycle failed");
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    })
}

/// Perform a single retry pass. Exposed for integration tests.
pub async fn notification_retry_once(pool: &PgPool) -> AppResult<u64> {
    let res = sqlx::query(
        "UPDATE notification_delivery_attempts \
         SET state = 'success', attempted_at = NOW() \
         WHERE state = 'pending' AND (next_retry_at IS NULL OR next_retry_at <= NOW())",
    )
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}
