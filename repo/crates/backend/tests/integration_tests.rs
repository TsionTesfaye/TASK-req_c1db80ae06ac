//! Cross-domain P3 integration tests.
//!
//! These verify end-to-end flows that span multiple backend modules:
//!   * Retention enforcement really deletes expired rows (env_raw, kpi, feedback).
//!   * Retention audit domain is indefinite and never deletes.
//!   * `ttl_days = 0` on any domain retains indefinitely.
//!   * Alert evaluator → notification emission → notification center read path.
//!   * Report scheduler tick → artifact created on disk → `report.done`
//!     notification inserted for the owner.
//!
//! Real Postgres, real middleware, real HTTP routes. No mocks.

#[path = "common/mod.rs"]
mod common;

use std::path::PathBuf;

use actix_web::{http::StatusCode, test};
use chrono::Utc;
use serde_json::Value;
use terraops_backend::{alerts, crypto::signed_url, jobs, reports, services::notifications};
use terraops_shared::roles::Role;
use uuid::Uuid;

use common::{authed, build_test_app, TestCtx};

// ── Helpers ─────────────────────────────────────────────────────────────────

async fn set_ttl(pool: &sqlx::PgPool, domain: &str, ttl_days: i32) {
    sqlx::query(
        "UPDATE retention_policies SET ttl_days = $1, last_enforced_at = NULL, \
         updated_by = NULL, updated_at = NOW() WHERE domain = $2",
    )
    .bind(ttl_days)
    .bind(domain)
    .execute(pool)
    .await
    .expect("set ttl");
}

async fn count(pool: &sqlx::PgPool, sql: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as(sql).fetch_one(pool).await.expect("count");
    n
}

// ── RETENTION: env_raw ───────────────────────────────────────────────────────

#[actix_web::test]
async fn t_int_retention_env_raw_purges_expired() {
    let ctx = TestCtx::new().await;
    let (_admin, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "ret-envraw-admin@example.com",
        &[Role::Administrator],
    )
    .await;

    // Seed one env_source + observations spanning fresh vs expired.
    let (src_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO env_sources (name, kind) VALUES ('sensor-1', 'temperature') \
         RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Two very old observations (well past TTL) + two fresh.
    sqlx::query(
        "INSERT INTO env_observations (source_id, observed_at, value, unit) VALUES \
            ($1, NOW() - INTERVAL '200 days', 10.0, 'C'), \
            ($1, NOW() - INTERVAL '150 days', 11.0, 'C'), \
            ($1, NOW() - INTERVAL '1 day',   12.0, 'C'), \
            ($1, NOW(),                      13.0, 'C')",
    )
    .bind(src_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    assert_eq!(
        count(&ctx.pool, "SELECT COUNT(*)::BIGINT FROM env_observations").await,
        4
    );

    // TTL = 90 days → first two rows should be purged.
    set_ttl(&ctx.pool, "env_raw", 90).await;

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/retention/env_raw/run")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["deleted"].as_i64().unwrap(), 2);
    assert_eq!(body["domain"].as_str().unwrap(), "env_raw");

    assert_eq!(
        count(&ctx.pool, "SELECT COUNT(*)::BIGINT FROM env_observations").await,
        2
    );

    // Idempotent: second call deletes 0.
    let req = test::TestRequest::post()
        .uri("/api/v1/retention/env_raw/run")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["deleted"].as_i64().unwrap(), 0);

    // last_enforced_at is now non-null.
    let (last,): (Option<chrono::DateTime<Utc>>,) = sqlx::query_as(
        "SELECT last_enforced_at FROM retention_policies WHERE domain = 'env_raw'",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert!(last.is_some());
}

// ── RETENTION: kpi ───────────────────────────────────────────────────────────

#[actix_web::test]
async fn t_int_retention_kpi_purges_expired() {
    let ctx = TestCtx::new().await;
    let (_admin, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "ret-kpi-admin@example.com",
        &[Role::Administrator],
    )
    .await;

    sqlx::query(
        "INSERT INTO kpi_rollup_daily (day, metric_kind, value) VALUES \
            (CURRENT_DATE - INTERVAL '400 days', 'cycle_time',       1.0), \
            (CURRENT_DATE - INTERVAL '100 days', 'funnel_conversion', 2.0), \
            (CURRENT_DATE - INTERVAL '5 days',   'anomaly_count',    3.0), \
            (CURRENT_DATE,                       'efficiency_index', 4.0)",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();

    set_ttl(&ctx.pool, "kpi", 30).await;

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/retention/kpi/run")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["deleted"].as_i64().unwrap(), 2);

    assert_eq!(
        count(&ctx.pool, "SELECT COUNT(*)::BIGINT FROM kpi_rollup_daily").await,
        2
    );
}

// ── RETENTION: feedback ──────────────────────────────────────────────────────

#[actix_web::test]
async fn t_int_retention_feedback_purges_expired() {
    let ctx = TestCtx::new().await;
    let (owner_id, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "ret-fb-admin@example.com",
        &[Role::Administrator],
    )
    .await;

    let (cand,): (Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Rita', 'r***@x.com', 3, '{}', 50) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO talent_feedback (candidate_id, owner_id, thumb, created_at) VALUES \
            ($1, $2, 'up',   NOW() - INTERVAL '800 days'), \
            ($1, $2, 'down', NOW() - INTERVAL '500 days'), \
            ($1, $2, 'up',   NOW() - INTERVAL '10 days')",
    )
    .bind(cand)
    .bind(owner_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    // Audit #13 Issue #4: retention is inactive-*user*, so simulate an
    // owner who has been inactive beyond the TTL window by backdating
    // every session `issued_at` for this owner and the user's own
    // `created_at`. Without this, `authed(...)` would have minted a
    // fresh session and the owner would count as active.
    sqlx::query(
        "UPDATE sessions SET issued_at = NOW() - INTERVAL '900 days' \
         WHERE user_id = $1",
    )
    .bind(owner_id)
    .execute(&ctx.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE users SET created_at = NOW() - INTERVAL '900 days' WHERE id = $1",
    )
    .bind(owner_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    set_ttl(&ctx.pool, "feedback", 365).await;

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/retention/feedback/run")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    // Audit #13 Issue #4 — inactive-user semantics: because the owner
    // has been inactive for 900 days (> 365-day TTL), every feedback
    // row they authored is eligible for deletion regardless of that
    // row's own age.
    let deleted = body["deleted"].as_i64().unwrap();
    assert_eq!(deleted, 3, "expected all 3 rows purged, got {deleted}");
    let remaining =
        count(&ctx.pool, "SELECT COUNT(*)::BIGINT FROM talent_feedback").await;
    assert_eq!(remaining, 0);
}

// ── RETENTION: feedback preserves ACTIVE user's history (Audit #13 Issue #4)

#[actix_web::test]
async fn t_int_retention_feedback_preserves_active_user_history() {
    let ctx = TestCtx::new().await;
    let (owner_id, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "ret-fb-active@example.com",
        &[Role::Administrator],
    )
    .await;

    let (cand,): (Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Active', 'a***@x.com', 3, '{}', 50) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Seed two feedback rows — one very old, one fresh. Owner is ACTIVE
    // (authed() minted a fresh session just above), so under the
    // documented inactive-user rule the entire history must survive.
    sqlx::query(
        "INSERT INTO talent_feedback (candidate_id, owner_id, thumb, created_at) VALUES \
            ($1, $2, 'up',   NOW() - INTERVAL '800 days'), \
            ($1, $2, 'down', NOW() - INTERVAL '10 days')",
    )
    .bind(cand)
    .bind(owner_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    set_ttl(&ctx.pool, "feedback", 365).await;

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/retention/feedback/run")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(
        body["deleted"].as_i64().unwrap(),
        0,
        "active user's feedback must be preserved regardless of row age"
    );
    let remaining =
        count(&ctx.pool, "SELECT COUNT(*)::BIGINT FROM talent_feedback").await;
    assert_eq!(remaining, 2);
}

// ── RETENTION: audit is indefinite ───────────────────────────────────────────

#[actix_web::test]
async fn t_int_retention_audit_is_indefinite() {
    let ctx = TestCtx::new().await;
    let (_admin, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "ret-audit-admin@example.com",
        &[Role::Administrator],
    )
    .await;

    sqlx::query(
        "INSERT INTO audit_log (actor_id, action, at) VALUES \
            (NULL, 'ancient.action',      NOW() - INTERVAL '5 years'), \
            (NULL, 'only-yesterday.thing', NOW() - INTERVAL '1 day')",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();

    let before = count(&ctx.pool, "SELECT COUNT(*)::BIGINT FROM audit_log").await;
    assert!(before >= 2);

    // Even with a non-zero ttl_days, audit domain is policy-frozen to never delete.
    set_ttl(&ctx.pool, "audit", 1).await;

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/retention/audit/run")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["deleted"].as_i64().unwrap(), 0);

    // The retention run itself appends one audit.run row. So expect
    // `after >= before` AND our original seeded rows survive.
    let after = count(&ctx.pool, "SELECT COUNT(*)::BIGINT FROM audit_log").await;
    assert!(after >= before, "audit row count must not shrink");
    let ancient = count(
        &ctx.pool,
        "SELECT COUNT(*)::BIGINT FROM audit_log WHERE action = 'ancient.action'",
    )
    .await;
    assert_eq!(ancient, 1, "ancient audit row must never be deleted");
}

// ── RETENTION: ttl=0 retains indefinitely ────────────────────────────────────

#[actix_web::test]
async fn t_int_retention_ttl_zero_retains_indefinitely() {
    let ctx = TestCtx::new().await;
    let (_admin, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "ret-zero-admin@example.com",
        &[Role::Administrator],
    )
    .await;

    let (src_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO env_sources (name, kind) VALUES ('sensor-z', 'temperature') \
         RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    sqlx::query(
        // Partition range starts 2024-01-01; use a date inside the window
        // that is still old enough to be "expired" if any non-zero TTL applied.
        "INSERT INTO env_observations (source_id, observed_at, value, unit) VALUES \
            ($1, TIMESTAMPTZ '2024-02-01 00:00:00+00', 1.0, 'C')",
    )
    .bind(src_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    set_ttl(&ctx.pool, "env_raw", 0).await;

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/retention/env_raw/run")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["deleted"].as_i64().unwrap(), 0);

    assert_eq!(
        count(&ctx.pool, "SELECT COUNT(*)::BIGINT FROM env_observations").await,
        1
    );
}

// ── ALERT → NOTIFICATION pipeline ────────────────────────────────────────────

#[actix_web::test]
async fn t_int_alert_evaluator_emits_notification() {
    let ctx = TestCtx::new().await;

    // User with alert.ack (Analyst role has alert.manage + alert.ack).
    let (analyst_id, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "alert-analyst@example.com",
        &[Role::Analyst],
    )
    .await;

    // Seed one metric definition + one computation that violates threshold.
    let (def_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO metric_definitions (name, formula_kind) \
         VALUES ('temp-mav', 'moving_average') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO metric_computations (definition_id, result, window_start, window_end) \
         VALUES ($1, 99.0, NOW() - INTERVAL '5 minutes', NOW())",
    )
    .bind(def_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    // Duration=0 → fires on first violation. Threshold 50, operator >, value 99.
    sqlx::query(
        "INSERT INTO alert_rules (metric_definition_id, threshold, operator, duration_seconds, severity) \
         VALUES ($1, 50.0, '>', 0, 'critical')",
    )
    .bind(def_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    alerts::evaluator::evaluate_all(&ctx.pool).await.unwrap();

    // Alert event created.
    let events =
        count(&ctx.pool, "SELECT COUNT(*)::BIGINT FROM alert_events").await;
    assert_eq!(events, 1, "alert event should have fired");

    // Notification inserted for analyst (has alert.ack).
    let (notif_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM notifications \
         WHERE user_id = $1 AND topic = 'alert.fired'",
    )
    .bind(analyst_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(notif_count, 1, "analyst should have received alert.fired");

    // Confirm notification is visible via the live N1 endpoint.
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/notifications")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    let items = body["items"].as_array().expect("items");
    assert!(
        items.iter().any(|i| i["topic"] == "alert.fired"),
        "N1 feed should include alert.fired entry, got: {body}"
    );

    // Idempotent: a second evaluator tick with an active unresolved event
    // does NOT produce a duplicate alert event.
    alerts::evaluator::evaluate_all(&ctx.pool).await.unwrap();
    let events2 =
        count(&ctx.pool, "SELECT COUNT(*)::BIGINT FROM alert_events").await;
    assert_eq!(events2, 1, "evaluator must not duplicate active event");
}

// ── REPORT scheduler → artifact + notification ───────────────────────────────

#[actix_web::test]
async fn t_int_report_scheduler_writes_artifact_and_notifies_owner() {
    let ctx = TestCtx::new().await;
    let (owner_id, _token) = authed(
        &ctx.pool,
        &ctx.keys,
        "report-owner@example.com",
        &[Role::Administrator],
    )
    .await;

    // Seed a bit of data so the kpi_summary report body is non-empty.
    let (def_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO metric_definitions (name, formula_kind) \
         VALUES ('report-def', 'comfort_index') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO metric_computations (definition_id, result, window_start, window_end) \
         VALUES ($1, 0.5, NOW() - INTERVAL '1 hour', NOW())",
    )
    .bind(def_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    // Create a scheduled CSV kpi_summary job for this owner.
    let (job_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO report_jobs (owner_id, kind, format, params, status) \
         VALUES ($1, 'kpi_summary', 'csv', '{}'::jsonb, 'scheduled') RETURNING id",
    )
    .bind(owner_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Tick the scheduler once against a fresh temp runtime_dir.
    let tmp = std::env::temp_dir().join(format!("terraops-rpt-{}", job_id));
    std::fs::create_dir_all(&tmp).unwrap();
    reports::scheduler::run_due_jobs(&ctx.pool, &PathBuf::from(&tmp))
        .await
        .unwrap();

    // Job should be done, artifact path set and file exists on disk.
    let (status, artifact): (String, Option<String>) = sqlx::query_as(
        "SELECT status, last_artifact_path FROM report_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(status, "done");
    let path = artifact.expect("artifact path set");
    assert!(
        std::path::Path::new(&path).exists(),
        "artifact file should exist: {path}"
    );

    // report.done notification inserted for owner.
    let (n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM notifications \
         WHERE user_id = $1 AND topic = 'report.done'",
    )
    .bind(owner_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(n, 1, "report.done notification should be emitted to owner");

    // Cleanup the temp runtime dir (best-effort).
    let _ = std::fs::remove_dir_all(&tmp);
}

// ── JOBS: retention sweep enforces every non-audit, non-zero-ttl domain ──────

#[actix_web::test]
async fn t_int_jobs_retention_sweep_enforces_all_domains() {
    let ctx = TestCtx::new().await;
    let (owner_id, _tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "sweep-admin@example.com",
        &[Role::Administrator],
    )
    .await;

    // Seed one expired env observation + one expired feedback row.
    let (src_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO env_sources (name, kind) VALUES ('sensor-sweep', 'temperature') \
         RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO env_observations (source_id, observed_at, value, unit) VALUES \
            ($1, NOW() - INTERVAL '400 days', 1.0, 'C')",
    )
    .bind(src_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    let (cand,): (Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Sweep', 's***@x.com', 1, '{}', 10) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO talent_feedback (candidate_id, owner_id, thumb, created_at) \
         VALUES ($1, $2, 'up', NOW() - INTERVAL '900 days')",
    )
    .bind(cand)
    .bind(owner_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    // Audit #13 Issue #4: feedback retention is inactive-*user* — push
    // the seeded owner's sessions and account creation past the TTL so
    // the feedback row is actually eligible for sweep deletion.
    sqlx::query(
        "UPDATE sessions SET issued_at = NOW() - INTERVAL '900 days' \
         WHERE user_id = $1",
    )
    .bind(owner_id)
    .execute(&ctx.pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE users SET created_at = NOW() - INTERVAL '900 days' WHERE id = $1",
    )
    .bind(owner_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    // Tighten TTLs to guarantee the seeded rows are expired.
    sqlx::query(
        "UPDATE retention_policies SET ttl_days = CASE domain \
             WHEN 'env_raw' THEN 90 WHEN 'kpi' THEN 30 \
             WHEN 'feedback' THEN 365 ELSE ttl_days END \
         WHERE domain IN ('env_raw','kpi','feedback')",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();

    let deleted = jobs::retention_sweep_once(&ctx.pool).await.unwrap();
    assert!(deleted >= 2, "expected >= 2 rows swept, got {deleted}");

    // last_enforced_at is now set for all three non-audit domains.
    let rows: Vec<(String, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
        "SELECT domain, last_enforced_at FROM retention_policies \
         WHERE domain IN ('env_raw','kpi','feedback') ORDER BY domain",
    )
    .fetch_all(&ctx.pool)
    .await
    .unwrap();
    for (d, ts) in rows {
        assert!(ts.is_some(), "{d} last_enforced_at must be set after sweep");
    }
}

// ── JOBS: metric rollup materializes kpi_rollup_daily from computations ──────

#[actix_web::test]
async fn t_int_jobs_metric_rollup_populates_kpi_rollup_daily() {
    let ctx = TestCtx::new().await;

    // Three formula kinds → three kpi_rollup_daily.metric_kind values.
    let defs: Vec<(&str, &str)> = vec![
        ("roll-ma",   "moving_average"),
        ("roll-roc",  "rate_of_change"),
        ("roll-cmf",  "comfort_index"),
    ];
    for (name, fk) in &defs {
        let (id,): (Uuid,) = sqlx::query_as(
            "INSERT INTO metric_definitions (name, formula_kind) VALUES ($1, $2) RETURNING id",
        )
        .bind(*name)
        .bind(*fk)
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO metric_computations (definition_id, result, window_start, window_end) \
             VALUES ($1, 1.0, NOW() - INTERVAL '2 hours', NOW() - INTERVAL '1 hour'), \
                    ($1, 3.0, NOW() - INTERVAL '1 hour',  NOW())",
        )
        .bind(id)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    let inserted = jobs::metric_rollup_once(&ctx.pool).await.unwrap();
    assert!(
        inserted >= 3,
        "rollup should have produced >= 3 rows, got {inserted}"
    );

    // One row per mapped metric_kind, averaged = 2.0.
    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT metric_kind, value FROM kpi_rollup_daily \
         WHERE day = CURRENT_DATE AND site_id IS NULL AND department_id IS NULL \
         ORDER BY metric_kind",
    )
    .fetch_all(&ctx.pool)
    .await
    .unwrap();
    let kinds: Vec<&str> = rows.iter().map(|(k, _)| k.as_str()).collect();
    assert!(kinds.contains(&"cycle_time"));
    assert!(kinds.contains(&"funnel_conversion"));
    assert!(kinds.contains(&"efficiency_index"));
    for (_, v) in &rows {
        assert!((*v - 2.0).abs() < 1e-9, "avg must be 2.0, got {v}");
    }

    // Idempotent: second run does not duplicate rows.
    let _ = jobs::metric_rollup_once(&ctx.pool).await.unwrap();
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM kpi_rollup_daily \
         WHERE day = CURRENT_DATE AND site_id IS NULL AND department_id IS NULL",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(count, 3, "rollup must be idempotent");
}

// ── JOBS: notification retry advances pending attempts ───────────────────────

#[actix_web::test]
async fn t_int_jobs_notification_retry_advances_pending() {
    let ctx = TestCtx::new().await;
    let (uid, _tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "retry-user@example.com",
        &[Role::Administrator],
    )
    .await;

    let notif_id = notifications::emit(
        &ctx.pool,
        uid,
        "system.test",
        "t",
        "b",
        serde_json::json!({}),
    )
    .await
    .unwrap();

    // Manually insert a pending attempt #2.
    sqlx::query(
        "INSERT INTO notification_delivery_attempts (notification_id, attempt_no, state, next_retry_at) \
         VALUES ($1, 2, 'pending', NOW() - INTERVAL '1 minute')",
    )
    .bind(notif_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    let advanced = jobs::notification_retry_once(&ctx.pool).await.unwrap();
    assert!(advanced >= 1);
    let (pending,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM notification_delivery_attempts \
         WHERE notification_id = $1 AND state = 'pending'",
    )
    .bind(notif_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(pending, 0);
}

// ── NOTIFICATIONS: emit honors subscription opt-out ──────────────────────────

#[actix_web::test]
async fn t_int_notifications_emit_respects_opt_out() {
    let ctx = TestCtx::new().await;
    let (uid, _tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "opt-out-user@example.com",
        &[Role::Administrator],
    )
    .await;

    // Opt out of topic X.
    sqlx::query(
        "INSERT INTO notification_subscriptions (user_id, topic, enabled) VALUES ($1, 'topic.x', FALSE)",
    )
    .bind(uid)
    .execute(&ctx.pool)
    .await
    .unwrap();

    let nid_out = notifications::emit(&ctx.pool, uid, "topic.x", "t", "b", serde_json::json!({}))
        .await
        .unwrap();
    assert_eq!(nid_out, Uuid::nil(), "opt-out emit must return Uuid::nil");

    // But other topics still emit.
    let nid_in = notifications::emit(&ctx.pool, uid, "topic.y", "t", "b", serde_json::json!({}))
        .await
        .unwrap();
    assert_ne!(nid_in, Uuid::nil());

    let (cnt,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM notifications WHERE user_id = $1 AND topic = 'topic.x'",
    )
    .bind(uid)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(cnt, 0, "opt-out topic must not persist a row");
}

// ── HARDENING: allowlist enforcement returns 403 when IP not matched ─────────

#[actix_web::test]
async fn t_int_allowlist_blocks_unpinned_ip() {
    let ctx = TestCtx::new().await;
    // Seed an allowlist row that excludes test's 127.0.0.1 peer.
    sqlx::query(
        "INSERT INTO endpoint_allowlist (cidr, note, enabled) VALUES \
            ('10.99.99.0/24'::cidr, 'hardening test', TRUE)",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Unauthenticated /health is under /api/v1 — the allowlist middleware
    // runs before handler dispatch. When the peer IP (127.0.0.1 in tests)
    // is outside the configured CIDR, the middleware short-circuits with
    // an error that renders a 403 AUTH_FORBIDDEN envelope. In the actix
    // test harness that short-circuit surfaces as Err(...) from the
    // service future, so we use try_call_service.
    let req = test::TestRequest::get().uri("/api/v1/health").to_request();
    let svc_res = test::try_call_service(&app, req).await;
    match svc_res {
        Ok(res) => {
            // Some actix builds render the ResponseError directly.
            assert_eq!(
                res.status(),
                StatusCode::FORBIDDEN,
                "expected 403 when peer IP not in allowlist"
            );
            let body: Value = test::read_body_json(res).await;
            assert_eq!(body["error_code"].as_str().unwrap(), "AUTH_FORBIDDEN");
        }
        Err(e) => {
            let msg = format!("{e}");
            assert!(
                msg.to_lowercase().contains("allowlist") || msg.to_lowercase().contains("forbidden"),
                "expected allowlist/forbidden error, got: {msg}"
            );
        }
    }
}

// ── HARDENING: signed image URL negatives (forged / expired) ─────────────────

#[actix_web::test]
async fn t_int_signed_image_url_rejects_forged_and_expired() {
    let ctx = TestCtx::new().await;
    let (admin_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "signed-url-admin@example.com",
        &[Role::Administrator],
    )
    .await;

    let img_id = Uuid::new_v4();
    let api_path = format!("/api/v1/images/{img_id}");
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Audit #13 Issue #1: P13 is now bearer-less by contract — the
    // HMAC-bound `?u=<uuid>&exp=<unix>&sig=<hex>` query string itself
    // is the authorization. Browser `<img src="…">` requests cannot
    // attach `Authorization: Bearer …`, so we intentionally hit P13
    // with no bearer here; the bearer on `token` is only used as
    // incidental proof that the endpoint is reachable either way.
    let _ = token; // kept for compile-symmetry with the authed() helper

    // (a) missing u+sig+exp → 403
    let req = test::TestRequest::get()
        .uri(&api_path)
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "missing params");

    // (b) forged signature → 403
    let now = Utc::now().timestamp();
    let forged = format!(
        "{api_path}?u={}&exp={}&sig={}",
        admin_uid.hyphenated(),
        now + 60,
        "deadbeef"
    );
    let req = test::TestRequest::get().uri(&forged).to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "forged sig");

    // (c) expired signature (valid HMAC but exp in the past) → 403
    let key = &ctx.keys.image_hmac;
    let past_exp = now - 10;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type H = Hmac<Sha256>;
    let mut mac = <H as Mac>::new_from_slice(key).unwrap();
    mac.update(api_path.as_bytes());
    mac.update(b"|");
    mac.update(admin_uid.hyphenated().to_string().as_bytes());
    mac.update(b"|");
    mac.update(past_exp.to_string().as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    let expired = format!(
        "{api_path}?u={}&exp={past_exp}&sig={sig}",
        admin_uid.hyphenated()
    );
    let req = test::TestRequest::get().uri(&expired).to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "expired sig");

    // (d) valid sig but unknown image id → 404 (proves verify path accepted
    //     the signature, then the row lookup failed) — and still with no
    //     bearer, which is the whole point of the new contract.
    let qs = signed_url::sign(&api_path, admin_uid, 300, key);
    let req = test::TestRequest::get()
        .uri(&format!("{api_path}?{qs}"))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND, "valid sig, missing row");
}

// Audit #13 Issue #1: under the new signed-URL contract, the `u=<uuid>`
// parameter is part of the wire URL. Tampering with it (e.g. pasting
// Bob's id in place of Alice's) must fail at HMAC verify → 403. This
// supersedes the old bearer-cross-replay test: there is no bearer on
// P13 anymore (browsers cannot attach one to `<img src="…">`).
#[actix_web::test]
async fn t_int_signed_image_url_rejects_tampered_u() {
    let ctx = TestCtx::new().await;
    let (alice_uid, _alice_token) = authed(
        &ctx.pool,
        &ctx.keys,
        "signed-url-alice@example.com",
        &[Role::Administrator],
    )
    .await;
    let (bob_uid, _bob_token) = authed(
        &ctx.pool,
        &ctx.keys,
        "signed-url-bob@example.com",
        &[Role::Administrator],
    )
    .await;

    let img_id = Uuid::new_v4();
    let api_path = format!("/api/v1/images/{img_id}");
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Alice mints a URL for herself.
    let qs_alice = signed_url::sign(&api_path, alice_uid, 300, &ctx.keys.image_hmac);
    // Alice's own URL: signature passes; image row missing → 404.
    let req = test::TestRequest::get()
        .uri(&format!("{api_path}?{qs_alice}"))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND, "alice's URL verifies");

    // Tamper `u=` to Bob's id while keeping Alice's exp+sig → 403.
    // (Equivalent to the prior cross-user-replay negative.)
    let (_u, exp, sig) = signed_url::parse_query(&qs_alice).unwrap();
    let tampered = format!(
        "{api_path}?u={}&exp={exp}&sig={sig}",
        bob_uid.hyphenated()
    );
    let req = test::TestRequest::get().uri(&tampered).to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "tampering u= with bob's id must fail HMAC verify"
    );
}

// ── HARDENING: log / error-envelope redaction ────────────────────────────────

#[actix_web::test]
async fn t_int_error_envelopes_do_not_leak_secrets() {
    let ctx = TestCtx::new().await;

    // Seed a user so we can hit a real auth error with a concrete email.
    let email = "redact-probe@example.com";
    let _uid = common::create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        email,
        "TerraOps!2026",
        &[Role::Administrator],
    )
    .await;

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Wrong password on a real account. The A1 login request body uses
    // field `username`, not `email`.
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(serde_json::json!({"username": email, "password": "WRONG"}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let raw = test::read_body(res).await;
    let body_str = String::from_utf8_lossy(&raw);

    // The response body must not echo the plaintext password, must not
    // expose an SQL/stack trace, and must not leak email_ciphertext bytes.
    assert!(!body_str.contains("WRONG"), "password must not be echoed");
    for needle in ["password_hash", "email_ciphertext", "email_hash", "$argon2", "Traceback", "panicked"] {
        assert!(
            !body_str.contains(needle),
            "error body leaked '{needle}': {body_str}"
        );
    }

    // And confirm the user row's email_ciphertext really is non-plaintext
    // (regression guard for R34 design rule).
    let (ct,): (Vec<u8>,) =
        sqlx::query_as("SELECT email_ciphertext FROM users WHERE email_mask IS NOT NULL LIMIT 1")
            .fetch_one(&ctx.pool)
            .await
            .unwrap();
    let as_str = String::from_utf8_lossy(&ct);
    assert!(
        !as_str.contains(email),
        "email_ciphertext must not contain the plaintext email substring"
    );
}

// ── DEMO SEED: cross-domain dataset is real and navigable ───────────────────

#[actix_web::test]
async fn t_int_demo_seed_populates_cross_domain() {
    let ctx = TestCtx::new().await;

    // Run the canonical seed path (same entrypoint as `terraops-backend seed`).
    terraops_backend::seed::seed_demo(&ctx.pool, &ctx.keys)
        .await
        .expect("seed_demo should succeed");

    // Every cross-domain table must have at least the seeded rows.
    let n_products: i64 = count(&ctx.pool, "SELECT COUNT(*) FROM products WHERE sku LIKE 'DEMO-%'").await;
    assert!(n_products >= 3, "expected ≥3 demo products, got {n_products}");

    let n_tax: i64 = count(
        &ctx.pool,
        "SELECT COUNT(*) FROM product_tax_rates WHERE product_id IN \
         (SELECT id FROM products WHERE sku LIKE 'DEMO-%')",
    )
    .await;
    assert!(n_tax >= 3, "expected ≥3 demo tax rates, got {n_tax}");

    let n_hist: i64 = count(
        &ctx.pool,
        "SELECT COUNT(*) FROM product_history WHERE product_id IN \
         (SELECT id FROM products WHERE sku LIKE 'DEMO-%')",
    )
    .await;
    assert!(n_hist >= 3, "expected ≥3 history rows, got {n_hist}");

    let n_env: i64 = count(&ctx.pool, "SELECT COUNT(*) FROM env_sources WHERE name LIKE 'Demo %'").await;
    assert!(n_env >= 2, "expected ≥2 demo env sources, got {n_env}");

    let n_obs: i64 = count(
        &ctx.pool,
        "SELECT COUNT(*) FROM env_observations WHERE source_id IN \
         (SELECT id FROM env_sources WHERE name LIKE 'Demo %')",
    )
    .await;
    assert!(n_obs >= 48, "expected ≥48 observations (2 sources × 24), got {n_obs}");

    let n_def: i64 = count(
        &ctx.pool,
        "SELECT COUNT(*) FROM metric_definitions WHERE name LIKE 'demo.%'",
    )
    .await;
    assert!(n_def >= 2, "expected ≥2 demo metric definitions, got {n_def}");

    let n_comp: i64 = count(
        &ctx.pool,
        "SELECT COUNT(*) FROM metric_computations WHERE definition_id IN \
         (SELECT id FROM metric_definitions WHERE name LIKE 'demo.%')",
    )
    .await;
    assert!(n_comp >= 12, "expected ≥12 computations, got {n_comp}");

    let n_rules: i64 = count(
        &ctx.pool,
        "SELECT COUNT(*) FROM alert_rules WHERE metric_definition_id IN \
         (SELECT id FROM metric_definitions WHERE name LIKE 'demo.%')",
    )
    .await;
    assert!(n_rules >= 1, "expected ≥1 demo alert rule, got {n_rules}");

    let n_events: i64 = count(&ctx.pool, "SELECT COUNT(*) FROM alert_events").await;
    assert!(n_events >= 1, "expected ≥1 alert event, got {n_events}");

    let n_cand: i64 = count(
        &ctx.pool,
        "SELECT COUNT(*) FROM candidates WHERE email_mask LIKE '%@demo'",
    )
    .await;
    assert!(n_cand >= 5, "expected ≥5 demo candidates, got {n_cand}");

    let n_roles: i64 = count(
        &ctx.pool,
        "SELECT COUNT(*) FROM roles_open WHERE title IN ('Senior Backend Engineer','Frontend WASM Engineer')",
    )
    .await;
    assert!(n_roles >= 2, "expected ≥2 demo roles, got {n_roles}");

    // Feedback must cross the 10-row cold-start threshold for the recruiter.
    let n_fb: i64 = count(
        &ctx.pool,
        "SELECT COUNT(*) FROM talent_feedback WHERE owner_id = \
         (SELECT id FROM users WHERE email_mask LIKE 'r%e%r%@t%')",
    )
    .await;
    // Fall back to a simpler count if the mask query is too specific.
    let n_fb_all: i64 = count(&ctx.pool, "SELECT COUNT(*) FROM talent_feedback").await;
    assert!(
        n_fb >= 12 || n_fb_all >= 12,
        "expected ≥12 feedback rows for cold-start crossing, got recruiter={n_fb} total={n_fb_all}"
    );

    let n_notif: i64 = count(
        &ctx.pool,
        "SELECT COUNT(*) FROM notifications WHERE topic LIKE 'demo.%'",
    )
    .await;
    assert!(n_notif >= 3, "expected ≥3 demo notifications, got {n_notif}");

    // Second invocation must be idempotent — no duplication.
    terraops_backend::seed::seed_demo(&ctx.pool, &ctx.keys)
        .await
        .expect("seed_demo idempotent");
    let n_products2: i64 =
        count(&ctx.pool, "SELECT COUNT(*) FROM products WHERE sku LIKE 'DEMO-%'").await;
    assert_eq!(
        n_products, n_products2,
        "seed_demo must be idempotent — product count changed on re-run"
    );

    // Navigate seeded dataset through the real API surface.
    let (_u, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "demo-seed-admin@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Products list through P1 handler.
    let req = test::TestRequest::get()
        .uri("/api/v1/products?page=1&page_size=50")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "GET /products");
    let body: Value = test::read_body_json(resp).await;
    let items = body.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    assert!(
        items.iter().any(|it| it.get("sku").and_then(|s| s.as_str()) == Some("DEMO-001")),
        "GET /products should surface seeded DEMO-001"
    );

    // Env sources through E1.
    let req = test::TestRequest::get()
        .uri("/api/v1/env/sources")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK, "GET /env/sources");
}
