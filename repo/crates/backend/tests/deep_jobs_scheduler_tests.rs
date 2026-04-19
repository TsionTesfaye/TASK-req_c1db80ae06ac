//! Deep coverage for background-job modules exercised in-process.
//!
//! These tests call the scheduler, retention/rollup/notification jobs, and
//! the alert evaluator directly against a real Postgres via `TestCtx`.
//! They exist to close the `reports/scheduler.rs`, `jobs/mod.rs`, and
//! `alerts/evaluator.rs` gaps that cannot be reached over HTTP — those
//! modules live under `tokio::spawn` loops in production but expose a
//! `*_once` / `evaluate_all` / `run_due_jobs` function for integration
//! tests, which is exactly what we use here.
//!
//! Each test seeds the DB through sqlx, calls the job function, and
//! asserts observable state changes (row status, artifact path,
//! notifications emitted, etc.). No mocks.

#[path = "common/mod.rs"]
mod common;

use std::path::PathBuf;

use chrono::Utc;
use terraops_backend::{
    alerts::{evaluator, rules},
    jobs, reports,
};
use terraops_shared::roles::Role;
use uuid::Uuid;

use common::{authed, TestCtx};

// ---------------------------------------------------------------------------
// Helpers.
// ---------------------------------------------------------------------------

async fn seed_metric_def(pool: &sqlx::PgPool, formula_kind: &str) -> Uuid {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO metric_definitions (name, formula_kind, window_seconds) \
         VALUES ($1, $2, 3600) RETURNING id",
    )
    .bind(format!("def-{formula_kind}-{}", Uuid::new_v4()))
    .bind(formula_kind)
    .fetch_one(pool)
    .await
    .unwrap();
    id
}

async fn insert_computation(pool: &sqlx::PgPool, def_id: Uuid, result: f64, at: chrono::DateTime<Utc>) -> Uuid {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO metric_computations (definition_id, computed_at, result, window_start, window_end) \
         VALUES ($1, $2, $3, $2 - INTERVAL '1 hour', $2) RETURNING id",
    )
    .bind(def_id)
    .bind(at)
    .bind(result)
    .fetch_one(pool)
    .await
    .unwrap();
    id
}

async fn seed_env_source_and_obs(pool: &sqlx::PgPool) -> (Uuid, Uuid) {
    let (sid,): (Uuid,) = sqlx::query_as(
        "INSERT INTO env_sources (name, kind) VALUES ($1, 'temperature') RETURNING id",
    )
    .bind(format!("src-{}", Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .unwrap();
    let (oid,): (Uuid,) = sqlx::query_as(
        "INSERT INTO env_observations (source_id, observed_at, value, unit) \
         VALUES ($1, NOW(), 72.5, 'F') RETURNING id",
    )
    .bind(sid)
    .fetch_one(pool)
    .await
    .unwrap();
    (sid, oid)
}

// ---------------------------------------------------------------------------
// Report scheduler: happy-path for all 3×3 kind×format combos plus retry
// promotion and terminal-failed guard.
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn deep_scheduler_renders_all_three_kinds() {
    let ctx = TestCtx::new().await;
    let (owner, _tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepjobs-owner@example.com",
        &[Role::Administrator, Role::Analyst],
    )
    .await;

    // Seed each kind's upstream data.
    let def_id = seed_metric_def(&ctx.pool, "moving_average").await;
    insert_computation(&ctx.pool, def_id, 42.0, Utc::now()).await;
    let _ = seed_env_source_and_obs(&ctx.pool).await;

    // An alert_rule + alert_event for alert_digest.
    let rule = rules::create_rule(&ctx.pool, def_id, 10.0, ">", 0, "warning", owner)
        .await
        .unwrap();
    let _evt = rules::fire_event(&ctx.pool, rule.id, 99.0).await.unwrap();

    // Three scheduled report_jobs, one per kind (pdf/csv/xlsx rotated).
    for (kind, format) in [
        ("kpi_summary", "pdf"),
        ("env_series", "csv"),
        ("alert_digest", "xlsx"),
    ] {
        sqlx::query(
            "INSERT INTO report_jobs (owner_id, kind, format, params, status, retry_count) \
             VALUES ($1, $2, $3, '{}', 'scheduled', 0)",
        )
        .bind(owner)
        .bind(kind)
        .bind(format)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    let runtime_dir = PathBuf::from(format!("/tmp/terraops-test-scheduler-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&runtime_dir).unwrap();

    reports::scheduler::run_due_jobs(&ctx.pool, &runtime_dir)
        .await
        .unwrap();

    // All three jobs should now be done with an artifact.
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT status, last_artifact_path FROM report_jobs ORDER BY created_at ASC",
    )
    .fetch_all(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3);
    for (status, artifact) in &rows {
        assert_eq!(status, "done", "artifact={artifact:?}");
        assert!(artifact.as_ref().unwrap().starts_with(runtime_dir.to_str().unwrap()));
    }

    // Owner got a notification per completed job.
    let (n_notifs,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM notifications WHERE user_id = $1 AND topic = 'report.done'",
    )
    .bind(owner)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(n_notifs, 3);

    // Clean up.
    let _ = std::fs::remove_dir_all(&runtime_dir);
}

#[actix_web::test]
async fn deep_scheduler_promotes_retryable_failure_and_keeps_terminal() {
    let ctx = TestCtx::new().await;
    let (owner, _tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepjobs-retry@example.com",
        &[Role::Administrator],
    )
    .await;

    // Retryable: status=failed, retry_count=0 → should be promoted to scheduled
    // and then processed successfully this tick.
    sqlx::query(
        "INSERT INTO report_jobs (owner_id, kind, format, params, status, retry_count) \
         VALUES ($1, 'kpi_summary', 'csv', '{}', 'failed', 0)",
    )
    .bind(owner)
    .execute(&ctx.pool)
    .await
    .unwrap();

    // Terminal: status=failed, retry_count=1 → should NOT be promoted.
    let (terminal_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO report_jobs (owner_id, kind, format, params, status, retry_count) \
         VALUES ($1, 'env_series', 'csv', '{}', 'failed', 1) RETURNING id",
    )
    .bind(owner)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let runtime_dir = PathBuf::from(format!("/tmp/terraops-test-retry-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&runtime_dir).unwrap();
    reports::scheduler::run_due_jobs(&ctx.pool, &runtime_dir)
        .await
        .unwrap();

    // Retryable should now be done.
    let rows: Vec<(String, i32)> = sqlx::query_as(
        "SELECT status, retry_count FROM report_jobs WHERE id != $1",
    )
    .bind(terminal_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "done");

    // Terminal stays failed — no promotion because retry_count >= 1.
    let terminal: (String, i32) = sqlx::query_as(
        "SELECT status, retry_count FROM report_jobs WHERE id = $1",
    )
    .bind(terminal_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(terminal.0, "failed");
    assert_eq!(terminal.1, 1);

    let _ = std::fs::remove_dir_all(&runtime_dir);
}

// ---------------------------------------------------------------------------
// jobs/mod.rs: retention_sweep_once + metric_rollup_once +
// notification_retry_once direct-call coverage.
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn deep_jobs_retention_sweep_deletes_expired_env_observations() {
    let ctx = TestCtx::new().await;

    // Seed an env_source and two observations: one well-expired (still
    // inside the partition window 2024-01-01 → 2099-01-01) and one fresh.
    let (sid, _) = seed_env_source_and_obs(&ctx.pool).await;
    sqlx::query(
        "INSERT INTO env_observations (source_id, observed_at, value, unit) \
         VALUES ($1, NOW() - INTERVAL '400 days', 1.0, 'F'), \
                ($1, NOW() - INTERVAL '1 day', 2.0, 'F')",
    )
    .bind(sid)
    .execute(&ctx.pool)
    .await
    .unwrap();

    // Set env_raw retention to 30 days so the 400-day-old row is expired.
    sqlx::query(
        "UPDATE retention_policies SET ttl_days = 30 WHERE domain = 'env_raw'",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();

    let deleted = jobs::retention_sweep_once(&ctx.pool).await.unwrap();
    assert!(deleted >= 1, "retention sweep should delete the 10-year-old row");

    // The fresh row + the NOW() row from seed_env_source_and_obs remain.
    let (remaining,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM env_observations WHERE source_id = $1",
    )
    .bind(sid)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(remaining, 2);

    // last_enforced_at was bumped for env_raw.
    let (last,): (Option<chrono::DateTime<Utc>>,) = sqlx::query_as(
        "SELECT last_enforced_at FROM retention_policies WHERE domain = 'env_raw'",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert!(last.is_some());
}

#[actix_web::test]
async fn deep_jobs_metric_rollup_maps_formula_kinds_to_metric_kinds() {
    let ctx = TestCtx::new().await;

    // Seed one metric_definition per formula_kind + a computation per def.
    let now = Utc::now();
    let ma = seed_metric_def(&ctx.pool, "moving_average").await;
    let roc = seed_metric_def(&ctx.pool, "rate_of_change").await;
    let ci = seed_metric_def(&ctx.pool, "comfort_index").await;
    insert_computation(&ctx.pool, ma, 10.0, now).await;
    insert_computation(&ctx.pool, ma, 20.0, now).await;
    insert_computation(&ctx.pool, roc, 0.5, now).await;
    insert_computation(&ctx.pool, ci, 80.0, now).await;

    let inserted = jobs::metric_rollup_once(&ctx.pool).await.unwrap();
    assert!(inserted >= 3);

    // kpi_rollup_daily rows exist with mapped metric_kinds.
    let kinds: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT metric_kind FROM kpi_rollup_daily ORDER BY metric_kind",
    )
    .fetch_all(&ctx.pool)
    .await
    .unwrap();
    let kinds: Vec<String> = kinds.into_iter().map(|(k,)| k).collect();
    for expected in ["cycle_time", "efficiency_index", "funnel_conversion"] {
        assert!(kinds.contains(&expected.to_string()), "missing {expected} in {kinds:?}");
    }

    // Idempotency: running again should not duplicate rows for same (day, kind).
    let _ = jobs::metric_rollup_once(&ctx.pool).await.unwrap();
    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM kpi_rollup_daily \
         WHERE site_id IS NULL AND department_id IS NULL",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(total, 3, "idempotent: same 3 (day, kind) rows");
}

#[actix_web::test]
async fn deep_jobs_notification_retry_advances_pending_to_success() {
    let ctx = TestCtx::new().await;
    let (owner, _tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepjobs-notif@example.com",
        &[Role::RegularUser],
    )
    .await;

    let (notif_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO notifications (user_id, topic, title, body) \
         VALUES ($1, 't', 'T', 'B') RETURNING id",
    )
    .bind(owner)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Two pending attempts: one due (next_retry_at NULL), one due in the past.
    sqlx::query(
        "INSERT INTO notification_delivery_attempts \
            (notification_id, attempt_no, state, next_retry_at) VALUES \
            ($1, 1, 'pending', NULL), \
            ($1, 2, 'pending', NOW() - INTERVAL '1 minute')",
    )
    .bind(notif_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    // One future pending — must NOT be advanced.
    sqlx::query(
        "INSERT INTO notification_delivery_attempts \
            (notification_id, attempt_no, state, next_retry_at) VALUES \
            ($1, 3, 'pending', NOW() + INTERVAL '10 minutes')",
    )
    .bind(notif_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    let advanced = jobs::notification_retry_once(&ctx.pool).await.unwrap();
    assert_eq!(advanced, 2);

    let (still_pending,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM notification_delivery_attempts \
         WHERE notification_id = $1 AND state = 'pending'",
    )
    .bind(notif_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(still_pending, 1);

    let (success,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM notification_delivery_attempts \
         WHERE notification_id = $1 AND state = 'success'",
    )
    .bind(notif_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(success, 2);
}

// ---------------------------------------------------------------------------
// alerts::evaluator — fire + resolve lifecycle + operator matrix + duration.
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn deep_alerts_evaluator_fires_and_resolves() {
    let ctx = TestCtx::new().await;
    let (creator, _tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepeval-admin@example.com",
        &[Role::Administrator],
    )
    .await;
    // Ensure there's at least one alert.ack user so notify_alert_fired has a
    // recipient (exercises the notify insert branch).
    let _ = authed(
        &ctx.pool,
        &ctx.keys,
        "deepeval-analyst@example.com",
        &[Role::Analyst],
    )
    .await;

    let def_id = seed_metric_def(&ctx.pool, "moving_average").await;
    let rule = rules::create_rule(&ctx.pool, def_id, 100.0, ">", 0, "warning", creator)
        .await
        .unwrap();

    // Violating computation → fires.
    insert_computation(&ctx.pool, def_id, 150.0, Utc::now()).await;
    evaluator::evaluate_all(&ctx.pool).await.unwrap();
    assert!(rules::has_active_event(&ctx.pool, rule.id).await.unwrap());
    let (n_notifs,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM notifications WHERE topic = 'alert.fired'",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert!(n_notifs >= 1);

    // Re-running with the same state does NOT create a duplicate event.
    evaluator::evaluate_all(&ctx.pool).await.unwrap();
    let (active_events,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM alert_events \
         WHERE rule_id = $1 AND resolved_at IS NULL",
    )
    .bind(rule.id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(active_events, 1);

    // A non-violating newer point resolves the open event.
    insert_computation(&ctx.pool, def_id, 5.0, Utc::now()).await;
    evaluator::evaluate_all(&ctx.pool).await.unwrap();
    assert!(!rules::has_active_event(&ctx.pool, rule.id).await.unwrap());
}

#[actix_web::test]
async fn deep_alerts_evaluator_respects_duration_window() {
    let ctx = TestCtx::new().await;
    let (creator, _tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepeval-dur@example.com",
        &[Role::Administrator],
    )
    .await;

    let def_id = seed_metric_def(&ctx.pool, "rate_of_change").await;
    // duration=60s: fires only if window has >= 2 violating points.
    let rule = rules::create_rule(&ctx.pool, def_id, 0.0, "<", 60, "info", creator)
        .await
        .unwrap();

    // First single point: does NOT fire (only one in window).
    insert_computation(&ctx.pool, def_id, -1.0, Utc::now()).await;
    evaluator::evaluate_all(&ctx.pool).await.unwrap();
    assert!(
        !rules::has_active_event(&ctx.pool, rule.id).await.unwrap(),
        "duration window requires >=2 points"
    );

    // Add an earlier violating point within the 60s window → now fires.
    insert_computation(
        &ctx.pool,
        def_id,
        -2.0,
        Utc::now() - chrono::Duration::seconds(30),
    )
    .await;
    // Add a newer violating point so its computed_at defines the window.
    insert_computation(&ctx.pool, def_id, -3.0, Utc::now()).await;
    evaluator::evaluate_all(&ctx.pool).await.unwrap();
    assert!(rules::has_active_event(&ctx.pool, rule.id).await.unwrap());
}

// ---------------------------------------------------------------------------
// jobs/mod.rs — spawn-loop coverage.
//
// The `start_*` wrappers are `tokio::spawn`-ed loops that sleep between
// cycles. We can't let them run to completion, but spawning them and then
// aborting the JoinHandle exercises the task body at least once (including
// the inner *_once call + sleep entry), which is enough to cover the
// spawn + closure lines that pure *_once calls skip.
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn deep_jobs_start_helpers_spawn_and_are_abortable() {
    let ctx = TestCtx::new().await;

    let h1 = jobs::start_retention_sweep(ctx.pool.clone());
    let h2 = jobs::start_metric_rollup(ctx.pool.clone());
    let h3 = jobs::start_notification_retry(ctx.pool.clone());

    // Give them a moment to enter the loop body and complete their first
    // cycle so the closure interior executes under coverage.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    h1.abort();
    h2.abort();
    h3.abort();

    // start_all glues them together with the report scheduler; drive it too.
    let runtime_dir = PathBuf::from(format!("/tmp/terraops-test-startall-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&runtime_dir).unwrap();
    let handles = jobs::start_all(ctx.pool.clone(), runtime_dir.clone());
    assert_eq!(handles.len(), 5);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    for h in handles {
        h.abort();
    }
    let _ = std::fs::remove_dir_all(&runtime_dir);
}

#[actix_web::test]
async fn deep_alerts_evaluator_operator_matrix() {
    let ctx = TestCtx::new().await;
    let (creator, _tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepeval-ops@example.com",
        &[Role::Administrator],
    )
    .await;

    let def_id = seed_metric_def(&ctx.pool, "comfort_index").await;
    // Compute value = 50.
    insert_computation(&ctx.pool, def_id, 50.0, Utc::now()).await;

    // Each operator, threshold chosen so value=50 violates it (fires).
    // Note: one rule per definition is allowed; but our evaluator loops all
    // rules. Different thresholds per operator keep semantics clear.
    let cases = [
        (">", 10.0, true),   // 50 > 10
        ("<", 100.0, true),  // 50 < 100
        (">=", 50.0, true),  // 50 >= 50
        ("<=", 50.0, true),  // 50 <= 50
        ("=", 50.0, true),   // |50 - 50| < eps
        (">", 1000.0, false), // 50 > 1000 → no fire
    ];

    let mut rule_ids = Vec::new();
    for (op, thresh, _) in &cases {
        let r = rules::create_rule(&ctx.pool, def_id, *thresh, op, 0, "info", creator)
            .await
            .unwrap();
        rule_ids.push(r.id);
    }

    evaluator::evaluate_all(&ctx.pool).await.unwrap();

    for ((op, thresh, expect_fire), rule_id) in cases.iter().zip(rule_ids.iter()) {
        let active = rules::has_active_event(&ctx.pool, *rule_id).await.unwrap();
        assert_eq!(
            active, *expect_fire,
            "operator {op} threshold {thresh} expected fire={expect_fire}, got {active}"
        );
    }
}
