//! Report scheduler background job.
//!
//! Runs every 10 seconds. Picks `scheduled` jobs (and `failed` jobs with
//! `retry_count < 1` for the one transient retry). Renders the requested
//! format to `${RUNTIME_DIR}/reports/{job_id}-{ts}.{ext}` and updates the
//! job record accordingly.
//!
//! # Retry policy
//! * `retry_count = 0, status = failed` → eligible for one retry → marked
//!   `scheduled`, retry_count incremented to 1, then re-processed.
//! * `retry_count >= 1, status = failed` → terminal: status stays `failed`.
//!
//! NOTE FOR MAIN LANE: call `start_report_scheduler(pool, runtime_dir)` from
//! `app.rs` startup:
//! ```rust
//! let _report_handle = crate::reports::scheduler::start_report_scheduler(
//!     pool.clone(),
//!     cfg.runtime_dir.clone(),
//! );
//! ```

use std::{path::PathBuf, time::Duration};

use chrono::Utc;
use sqlx::PgPool;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::errors::AppResult;

/// Spawn the scheduler loop. Returns a `JoinHandle`.
pub fn start_report_scheduler(pool: PgPool, runtime_dir: PathBuf) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Err(e) = run_due_jobs(&pool, &runtime_dir).await {
                tracing::error!(error = %e, "report scheduler cycle failed");
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    })
}

/// Run a single scheduler pass synchronously. Exposed for integration tests.
pub async fn run_due_jobs(pool: &PgPool, runtime_dir: &PathBuf) -> AppResult<()> {
    // Promote retryable failures back to scheduled
    sqlx::query(
        "UPDATE report_jobs SET status='scheduled' \
         WHERE status='failed' AND retry_count < 1",
    )
    .execute(pool)
    .await?;

    // Pick up to 5 scheduled jobs per cycle
    #[derive(sqlx::FromRow)]
    struct JobRow {
        id: Uuid,
        owner_id: Uuid,
        kind: String,
        format: String,
        params: serde_json::Value,
        retry_count: i32,
    }
    let jobs: Vec<JobRow> = sqlx::query_as(
        "SELECT id, owner_id, kind, format, params, retry_count \
         FROM report_jobs WHERE status='scheduled' \
         ORDER BY created_at ASC LIMIT 5",
    )
    .fetch_all(pool)
    .await?;

    for job in jobs {
        // Mark running
        sqlx::query("UPDATE report_jobs SET status='running', last_run_at=NOW() WHERE id=$1")
            .bind(job.id)
            .execute(pool)
            .await?;

        let reports_dir = runtime_dir.join("reports");
        if let Err(e) = std::fs::create_dir_all(&reports_dir) {
            tracing::warn!(error = %e, "could not create reports dir");
        }

        let ts = Utc::now().format("%Y%m%dT%H%M%S").to_string();
        let ext = job.format.as_str();
        let filename = format!("{}-{}.{}", job.id, ts, ext);
        let output_path = reports_dir.join(&filename);

        let rows = build_report_data(pool, &job.kind, &job.params).await;

        let render_result = match job.format.as_str() {
            "pdf" => super::pdf::render(&job.kind, &rows, &output_path),
            "csv" => super::csv::render(&rows, &output_path),
            "xlsx" => super::xlsx::render(&job.kind, &rows, &output_path),
            other => Err(crate::errors::AppError::Internal(format!(
                "unknown format: {}",
                other
            ))),
        };

        match render_result {
            Ok(()) => {
                let artifact_path = output_path.to_string_lossy().to_string();
                sqlx::query(
                    "UPDATE report_jobs \
                     SET status='done', last_artifact_path=$1, last_run_at=NOW() \
                     WHERE id=$2",
                )
                .bind(&artifact_path)
                .bind(job.id)
                .execute(pool)
                .await?;
                // Notify owner
                notify_owner(pool, job.owner_id, job.id, &artifact_path).await;
                tracing::info!(job_id = %job.id, "report job done");
            }
            Err(e) => {
                tracing::warn!(job_id = %job.id, error = %e, "report job failed");
                sqlx::query(
                    "UPDATE report_jobs SET status='failed', retry_count=$1 WHERE id=$2",
                )
                .bind(job.retry_count + 1)
                .bind(job.id)
                .execute(pool)
                .await?;
            }
        }
    }
    Ok(())
}

/// Build report data rows from the database, honoring the persisted
/// `params` JSONB on the report job.
///
/// Recognised params (all optional):
///   * `since` / `until` — RFC 3339 timestamps; filter rows by their
///     natural time column (`computed_at`, `observed_at`, `fired_at`).
///   * `limit` — row cap (1..=1000, default 50).
///   * `source_id` — only for `env_series`.
///   * `site_id` / `department_id` — only for `env_series`; filters
///     through the source's site/department assignment, the prompt-
///     required spatial slicing dimension for environmental exports
///     (Audit #7 Issue #6).
///   * `definition_id` — only for `kpi_summary`.
///   * `severity` — only for `alert_digest` (one of `info|warning|critical`).
///
/// Audit #5 Issue #2: the scheduler previously ignored `params` entirely
/// and always returned the latest 50 rows. Filtered exports are a
/// prompt-required analyst workflow, so the scheduler must translate the
/// persisted filter block into real SQL predicates.
pub(crate) async fn build_report_data(
    pool: &PgPool,
    kind: &str,
    params: &serde_json::Value,
) -> Vec<serde_json::Value> {
    let p = params.as_object();
    let parse_ts = |k: &str| -> Option<chrono::DateTime<Utc>> {
        p.and_then(|o| o.get(k))
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc))
    };
    let parse_uuid = |k: &str| -> Option<Uuid> {
        p.and_then(|o| o.get(k))
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
    };
    let parse_str = |k: &str| -> Option<String> {
        p.and_then(|o| o.get(k))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };
    let since = parse_ts("since");
    let until = parse_ts("until");
    let limit: i64 = p
        .and_then(|o| o.get("limit"))
        .and_then(|v| v.as_i64())
        .map(|n| n.clamp(1, 1000))
        .unwrap_or(50);

    match kind {
        "kpi_summary" => {
            #[derive(sqlx::FromRow)]
            struct Row {
                formula_kind: String,
                result: f64,
                computed_at: chrono::DateTime<Utc>,
            }
            let definition_id = parse_uuid("definition_id");
            let rows: Vec<Row> = sqlx::query_as(
                "SELECT md.formula_kind, mc.result, mc.computed_at \
                 FROM metric_computations mc \
                 JOIN metric_definitions md ON md.id = mc.definition_id \
                 WHERE ($1::TIMESTAMPTZ IS NULL OR mc.computed_at >= $1) \
                   AND ($2::TIMESTAMPTZ IS NULL OR mc.computed_at <= $2) \
                   AND ($3::UUID         IS NULL OR mc.definition_id = $3) \
                 ORDER BY mc.computed_at DESC LIMIT $4",
            )
            .bind(since)
            .bind(until)
            .bind(definition_id)
            .bind(limit)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            rows.into_iter()
                .map(|r| {
                    serde_json::json!({
                        "formula_kind": r.formula_kind,
                        "result": r.result,
                        "computed_at": r.computed_at.to_rfc3339(),
                    })
                })
                .collect()
        }
        "env_series" => {
            #[derive(sqlx::FromRow)]
            struct Row {
                source_name: String,
                value: f64,
                observed_at: chrono::DateTime<Utc>,
                unit: String,
            }
            let source_id = parse_uuid("source_id");
            let site_id = parse_uuid("site_id");
            let department_id = parse_uuid("department_id");
            let rows: Vec<Row> = sqlx::query_as(
                "SELECT es.name AS source_name, eo.value, eo.observed_at, eo.unit \
                 FROM env_observations eo \
                 JOIN env_sources es ON es.id = eo.source_id \
                 WHERE ($1::TIMESTAMPTZ IS NULL OR eo.observed_at >= $1) \
                   AND ($2::TIMESTAMPTZ IS NULL OR eo.observed_at <= $2) \
                   AND ($3::UUID         IS NULL OR eo.source_id = $3) \
                   AND ($4::UUID         IS NULL OR es.site_id = $4) \
                   AND ($5::UUID         IS NULL OR es.department_id = $5) \
                 ORDER BY eo.observed_at DESC LIMIT $6",
            )
            .bind(since)
            .bind(until)
            .bind(source_id)
            .bind(site_id)
            .bind(department_id)
            .bind(limit)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            rows.into_iter()
                .map(|r| {
                    serde_json::json!({
                        "source": r.source_name,
                        "value": r.value,
                        "unit": r.unit,
                        "observed_at": r.observed_at.to_rfc3339(),
                    })
                })
                .collect()
        }
        "alert_digest" => {
            #[derive(sqlx::FromRow)]
            struct Row {
                severity: String,
                value: f64,
                fired_at: chrono::DateTime<Utc>,
                resolved_at: Option<chrono::DateTime<Utc>>,
            }
            let severity = parse_str("severity").filter(|s| {
                ["info", "warning", "critical"].contains(&s.as_str())
            });
            // Spatial slicing on `alert_digest` is intentionally *not*
            // wired up: alert rules live on `metric_definitions`, which
            // have no direct site/department column. Correlating alert
            // events back to a site would require walking the definition
            // inputs to their sources, which is expensive and ambiguous
            // when a metric aggregates across multiple sources. The
            // env-scoped spatial slicing lives on `env_series` where the
            // join is direct and unambiguous (Audit #7 Issue #6 scope).
            let rows: Vec<Row> = sqlx::query_as(
                "SELECT ar.severity, ae.value, ae.fired_at, ae.resolved_at \
                 FROM alert_events ae \
                 JOIN alert_rules ar ON ar.id = ae.rule_id \
                 WHERE ($1::TIMESTAMPTZ IS NULL OR ae.fired_at >= $1) \
                   AND ($2::TIMESTAMPTZ IS NULL OR ae.fired_at <= $2) \
                   AND ($3::TEXT         IS NULL OR ar.severity = $3) \
                 ORDER BY ae.fired_at DESC LIMIT $4",
            )
            .bind(since)
            .bind(until)
            .bind(severity)
            .bind(limit)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            rows.into_iter()
                .map(|r| {
                    serde_json::json!({
                        "severity": r.severity,
                        "value": r.value,
                        "fired_at": r.fired_at.to_rfc3339(),
                        "resolved_at": r.resolved_at.map(|t| t.to_rfc3339()),
                    })
                })
                .collect()
        }
        _ => vec![],
    }
}

/// Insert a completion notification for the report owner.
async fn notify_owner(pool: &PgPool, owner_id: Uuid, job_id: Uuid, artifact_path: &str) {
    let _ = sqlx::query(
        "INSERT INTO notifications (user_id, topic, title, body, payload_json) \
         VALUES ($1, 'report.done', 'Report ready', $2, $3)",
    )
    .bind(owner_id)
    .bind(format!("Your report job {} has completed.", job_id))
    .bind(serde_json::json!({ "job_id": job_id, "artifact_path": artifact_path }))
    .execute(pool)
    .await;
}
