//! Deep HTTP coverage for alerts (AL1–AL6) and reports (RP1–RP6).
//! Real middleware stack + real Postgres via TestCtx.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use std::io::Write;
use terraops_shared::roles::Role;
use uuid::Uuid;

use common::{authed, build_test_app, TestCtx};

fn bearer(tok: &str) -> String {
    format!("Bearer {tok}")
}

// ── AL1–AL4: alert rules CRUD ──────────────────────────────────────────────

#[actix_web::test]
async fn deep_alert_rules_crud_and_validation() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepa-rules@example.com",
        &[Role::Administrator],
    )
    .await;

    // Seed a metric definition the rule can reference
    let (def_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO metric_definitions (name, formula_kind) \
         VALUES ('RuleMetric','moving_average') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // AL2 create valid
    let req = test::TestRequest::post()
        .uri("/api/v1/alerts/rules")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "metric_definition_id": def_id,
            "threshold": 25.0,
            "operator": ">",
            "duration_seconds": 60,
            "severity": "warning"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let rule_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // AL2 bad operator
    let req = test::TestRequest::post()
        .uri("/api/v1/alerts/rules")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "metric_definition_id": def_id,
            "threshold": 1.0,
            "operator": "!=",
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // AL2 bad severity
    let req = test::TestRequest::post()
        .uri("/api/v1/alerts/rules")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "metric_definition_id": def_id,
            "threshold": 1.0,
            "operator": ">",
            "severity": "catastrophic"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // AL1 list
    let req = test::TestRequest::get()
        .uri("/api/v1/alerts/rules?page=1&page_size=10")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 1);

    // AL3 update
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/alerts/rules/{rule_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"threshold": 30.0, "severity": "critical", "enabled": false}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // AL3 invalid operator
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/alerts/rules/{rule_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"operator": "!!"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // AL3 missing rule
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/alerts/rules/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"threshold": 1.0}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // AL4 delete
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/alerts/rules/{rule_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // AL4 again → 404
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/alerts/rules/{rule_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── AL5 list events + unacked_only filter ──────────────────────────────────

#[actix_web::test]
async fn deep_alert_events_list_filters() {
    let ctx = TestCtx::new().await;
    let (uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepa-events@example.com",
        &[Role::Administrator],
    )
    .await;

    // Seed def + rule
    let (def_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO metric_definitions (name, formula_kind) \
         VALUES ('EvtDef','rate_of_change') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (rule_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO alert_rules (metric_definition_id, threshold, operator, severity, created_by) \
         VALUES ($1, 5.0, '>', 'critical', $2) RETURNING id",
    )
    .bind(def_id)
    .bind(uid)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Seed 3 events — 1 acked, 2 unacked
    sqlx::query("INSERT INTO alert_events (rule_id, value, acked_at, acked_by) VALUES ($1,1.0,NOW(),$2)")
        .bind(rule_id).bind(uid).execute(&ctx.pool).await.unwrap();
    sqlx::query("INSERT INTO alert_events (rule_id, value) VALUES ($1, 2.0)")
        .bind(rule_id).execute(&ctx.pool).await.unwrap();
    sqlx::query("INSERT INTO alert_events (rule_id, value) VALUES ($1, 3.0)")
        .bind(rule_id).execute(&ctx.pool).await.unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // AL5 list all
    let req = test::TestRequest::get()
        .uri("/api/v1/alerts/events")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 3);

    // AL5 unacked only
    let req = test::TestRequest::get()
        .uri("/api/v1/alerts/events?unacked_only=true")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 2);

    // AL5 by rule_id
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/alerts/events?rule_id={rule_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── RP1–RP5: full report job lifecycle ─────────────────────────────────────

#[actix_web::test]
async fn deep_report_jobs_lifecycle() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepr-job@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // RP2 create valid kpi_summary/pdf
    let req = test::TestRequest::post()
        .uri("/api/v1/reports/jobs")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"kind": "kpi_summary", "format": "pdf"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let j1 = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // RP2 env_series/csv
    let req = test::TestRequest::post()
        .uri("/api/v1/reports/jobs")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"kind": "env_series", "format": "csv", "cron": "0 * * * *"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // RP2 alert_digest/xlsx
    let req = test::TestRequest::post()
        .uri("/api/v1/reports/jobs")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"kind": "alert_digest", "format": "xlsx"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // RP2 invalid kind
    let req = test::TestRequest::post()
        .uri("/api/v1/reports/jobs")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"kind": "nope", "format": "pdf"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // RP2 invalid format
    let req = test::TestRequest::post()
        .uri("/api/v1/reports/jobs")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"kind": "kpi_summary", "format": "html"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // RP1 list — self only
    let req = test::TestRequest::get()
        .uri("/api/v1/reports/jobs")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 3);

    // RP3 get self
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reports/jobs/{j1}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // RP3 missing
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reports/jobs/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // RP4 run-now
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/reports/jobs/{j1}/run-now"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "scheduled");

    // RP5 cancel
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/reports/jobs/{j1}/cancel"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // RP4 after cancel → 422
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/reports/jobs/{j1}/run-now"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Force status = running directly, then RP5 cancel → 409, RP4 run-now → 409
    sqlx::query("UPDATE report_jobs SET status='running' WHERE id=$1")
        .bind(j1).execute(&ctx.pool).await.unwrap();
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/reports/jobs/{j1}/cancel"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/reports/jobs/{j1}/run-now"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

// ── RP6 artifact: 404 before path, 200 after path, correct content-type ────

#[actix_web::test]
async fn deep_report_artifact_read_paths() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepr-art@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Create job
    let req = test::TestRequest::post()
        .uri("/api/v1/reports/jobs")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"kind": "kpi_summary", "format": "csv"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let jid = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // RP6 before any artifact → 404
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reports/jobs/{jid}/artifact"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Write a real file and patch last_artifact_path
    let tmp = std::env::temp_dir().join(format!("terraops-test-{jid}.csv"));
    let mut f = std::fs::File::create(&tmp).unwrap();
    f.write_all(b"col1,col2\nv1,v2\n").unwrap();
    let tmp_str = tmp.to_string_lossy().to_string();
    sqlx::query("UPDATE report_jobs SET last_artifact_path=$2 WHERE id=$1")
        .bind(jid).bind(&tmp_str).execute(&ctx.pool).await.unwrap();

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reports/jobs/{jid}/artifact"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    assert_eq!(&body[..], b"col1,col2\nv1,v2\n");

    // Cleanup
    let _ = std::fs::remove_file(&tmp);

    // RP6 after path set but file missing → 404
    sqlx::query("UPDATE report_jobs SET last_artifact_path='/nonexistent/terraops/gone.csv' WHERE id=$1")
        .bind(jid).execute(&ctx.pool).await.unwrap();
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reports/jobs/{jid}/artifact"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Report owner isolation (SELF guard) ────────────────────────────────────

#[actix_web::test]
async fn deep_report_jobs_owner_isolation() {
    let ctx = TestCtx::new().await;
    let (_a, tok_a) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepr-a@example.com",
        &[Role::Administrator],
    )
    .await;
    let (_b, tok_b) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepr-b@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // A creates a job
    let req = test::TestRequest::post()
        .uri("/api/v1/reports/jobs")
        .insert_header(("Authorization", bearer(&tok_a)))
        .set_json(json!({"kind": "kpi_summary", "format": "pdf"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let jid = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // B cannot get it
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reports/jobs/{jid}"))
        .insert_header(("Authorization", bearer(&tok_b)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // B cannot run-now
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/reports/jobs/{jid}/run-now"))
        .insert_header(("Authorization", bearer(&tok_b)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // B cannot cancel
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/reports/jobs/{jid}/cancel"))
        .insert_header(("Authorization", bearer(&tok_b)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // B cannot get artifact
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reports/jobs/{jid}/artifact"))
        .insert_header(("Authorization", bearer(&tok_b)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // B's list is empty
    let req = test::TestRequest::get()
        .uri("/api/v1/reports/jobs")
        .insert_header(("Authorization", bearer(&tok_b)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 0);
}
