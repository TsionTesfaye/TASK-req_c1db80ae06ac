//! Success-path and payload-contract tests for P-A and P-B endpoint families.
//!
//! Companion to `parity_tests.rs`, which covers the 401/403 rejection surface.
//! These tests drive the *authorized happy path* through the full middleware
//! stack (MetricsMw → BudgetMw → CsrfMw → AuthnMw → RequestIdMw) against a
//! real Postgres, asserting:
//!
//!   1. **Response status** — 200 OK or 201 CREATED.
//!   2. **Page-envelope shape** — `{items: [...], total: N, page: 1, page_size: M}`
//!      on every list endpoint.
//!   3. **Create-response shape** — `{id: <uuid>}` on every create endpoint.
//!   4. **Error-envelope shape** — `{error_code, message, request_id}` on 401/403
//!      so the API observability contract (method + path + response body) is pinned.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use terraops_shared::roles::Role;
use uuid::Uuid;

use common::{authed, build_test_app, TestCtx};

fn bearer(tok: &str) -> String {
    format!("Bearer {tok}")
}

// ---------------------------------------------------------------------------
// Error-envelope shape contract
// ---------------------------------------------------------------------------

/// Unauthenticated → 401 + body carries error_code + message + request_id.
#[actix_web::test]
async fn error_envelope_401_has_required_fields() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/products")
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("error_code").is_some(), "401 body missing error_code: {body}");
    assert!(body.get("message").is_some(), "401 body missing message: {body}");
    assert!(body.get("request_id").is_some(), "401 body missing request_id: {body}");
    let code = body["error_code"].as_str().unwrap_or("").to_ascii_lowercase();
    assert!(code.contains("auth_required"), "expected auth_required for 401, got: {code}");
}

/// Forbidden (wrong perm) → 403 + body carries AUTH_FORBIDDEN code.
#[actix_web::test]
async fn error_envelope_403_has_auth_forbidden_code() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "env403@x.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/env/sources")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"name": "Sensor", "kind": "temperature"}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("error_code").is_some(), "403 body missing error_code: {body}");
    assert!(body.get("request_id").is_some(), "403 body missing request_id: {body}");
    let code = body["error_code"].as_str().unwrap_or("").to_ascii_lowercase();
    assert!(code.contains("auth_forbidden"), "expected auth_forbidden for 403, got: {code}");
}

// ---------------------------------------------------------------------------
// P-A Products — P1 / P2 / P3 success path + payload shape
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn p1_list_products_returns_page_envelope() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "p1ok@x.com", &[Role::Administrator]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/products")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "P1 list expected 200");
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("items").is_some(), "P1 missing 'items': {body}");
    assert!(body.get("total").is_some(), "P1 missing 'total': {body}");
    assert!(body.get("page").is_some(), "P1 missing 'page': {body}");
    assert_eq!(body["total"], 0);
}

#[actix_web::test]
async fn p2_create_product_returns_id_and_p3_read_back() {
    let ctx = TestCtx::new().await;
    // DataSteward has product.write (Administrator does not).
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "p2ok@x.com", &[Role::DataSteward]).await;
    let (cat_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO categories (name) VALUES ('TestCat') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // P2 create → 201 + {id}.
    let req = test::TestRequest::post()
        .uri("/api/v1/products")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"sku": "PARITY-001", "name": "Parity Widget", "category_id": cat_id}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED, "P2 create expected 201");
    let body: Value = test::read_body_json(res).await;
    let id_str = body["id"].as_str().expect("P2 must return {id}");
    let pid = Uuid::parse_str(id_str).expect("id must be valid UUID");

    // P3 read-back → 200 + key fields.
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "P3 get expected 200");
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["sku"], "PARITY-001");
    assert_eq!(body["name"], "Parity Widget");
    assert!(body.get("on_shelf").is_some(), "P3 body must have 'on_shelf'");
    assert!(body.get("created_at").is_some(), "P3 body must have 'created_at'");
}

#[actix_web::test]
async fn p1_list_products_after_create_shows_item_in_page() {
    let ctx = TestCtx::new().await;
    // DataSteward has product.write; product.read is also granted to DataSteward.
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "p1list@x.com", &[Role::DataSteward]).await;
    let (cat_id,): (Uuid,) =
        sqlx::query_as("INSERT INTO categories (name) VALUES ('ListCat') RETURNING id")
            .fetch_one(&ctx.pool)
            .await
            .unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/products")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"sku": "LIST-001", "name": "List Widget", "category_id": cat_id}))
        .to_request();
    test::call_service(&app, req).await;

    let req = test::TestRequest::get()
        .uri("/api/v1/products")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["total"], 1, "expected 1 product");
    let items = body["items"].as_array().unwrap();
    assert_eq!(items[0]["sku"], "LIST-001");
}

// ---------------------------------------------------------------------------
// P-A Imports — I2 list
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn i2_list_imports_returns_page_envelope() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "i2ok@x.com", &[Role::DataSteward]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/imports")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "I2 list expected 200");
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("items").is_some(), "I2 missing 'items': {body}");
    assert!(body.get("total").is_some(), "I2 missing 'total': {body}");
    assert_eq!(body["total"], 0);
}

// ---------------------------------------------------------------------------
// P-B Env sources — E1 / E2 lifecycle
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn e1_list_env_sources_returns_page_envelope() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "e1ok@x.com", &[Role::Administrator]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/env/sources")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "E1 list expected 200");
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("items").is_some(), "E1 missing 'items'");
    assert!(body.get("total").is_some(), "E1 missing 'total'");
    assert_eq!(body["total"], 0);
}

#[actix_web::test]
async fn e2_create_env_source_returns_id_and_e1_list_reflects_it() {
    let ctx = TestCtx::new().await;
    // Analyst has metric.configure (Administrator and DataSteward do not).
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "e2ok@x.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/env/sources")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"name": "Parity Sensor", "kind": "temperature"}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED, "E2 expected 201");
    let body: Value = test::read_body_json(res).await;
    let src_id = body["id"].as_str().expect("E2 must return id");
    let _ = Uuid::parse_str(src_id).expect("E2 id must be UUID");

    let req = test::TestRequest::get()
        .uri("/api/v1/env/sources")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["total"], 1, "E1 total should be 1 after create");
}

// ---------------------------------------------------------------------------
// P-B Metric Definitions — MD1 / MD2 / MD3 lifecycle
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn md1_list_definitions_returns_page_envelope() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "md1ok@x.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/metrics/definitions")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "MD1 list expected 200");
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("items").is_some(), "MD1 missing 'items'");
    assert!(body.get("total").is_some(), "MD1 missing 'total'");
    // total ≥ 0; seeded default may be present
    assert!(body["total"].as_u64().is_some(), "MD1 total must be a number");
}

#[actix_web::test]
async fn md2_create_definition_returns_id_and_md3_read_back() {
    let ctx = TestCtx::new().await;
    // Analyst has metric.configure (Administrator does not).
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "md2ok@x.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/metrics/definitions")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "name": "Parity Temp Avg",
            "formula_kind": "moving_average",
            "source_ids": [],
            "window_seconds": 3600
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED, "MD2 create expected 201");
    let body: Value = test::read_body_json(res).await;
    let def_id = body["id"].as_str().expect("MD2 must return id");
    let did = Uuid::parse_str(def_id).expect("MD2 id must be UUID");

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/metrics/definitions/{did}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "MD3 get expected 200");
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["name"], "Parity Temp Avg");
    assert!(body.get("formula_kind").is_some(), "MD3 must have formula_kind");
}

// ---------------------------------------------------------------------------
// P-B KPI — K1 shape + K2 / K4 page envelopes
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn k1_kpi_summary_returns_expected_shape() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "k1ok@x.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/kpi/summary")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "K1 summary expected 200");
    let body: Value = test::read_body_json(res).await;
    for key in &[
        "cycle_time_avg_hours",
        "funnel_conversion_pct",
        "anomaly_count",
        "efficiency_index",
        "sku_on_shelf_compliance_pct",
    ] {
        assert!(body.get(*key).is_some(), "K1 summary missing '{key}': {body}");
    }
}

#[actix_web::test]
async fn k2_cycle_time_returns_page_envelope() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "k2ok@x.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/kpi/cycle-time")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "K2 cycle-time expected 200");
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("items").is_some(), "K2 missing 'items'");
    assert!(body.get("total").is_some(), "K2 missing 'total'");
}

#[actix_web::test]
async fn k4_anomalies_returns_page_envelope() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "k4ok@x.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/kpi/anomalies")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "K4 anomalies expected 200");
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("items").is_some(), "K4 missing 'items'");
    assert!(body.get("total").is_some(), "K4 missing 'total'");
}

// ---------------------------------------------------------------------------
// P-B Alerts — AL1 list + AL2 create lifecycle
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn al1_list_alert_rules_returns_page_envelope() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "al1ok@x.com", &[Role::Administrator]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/alerts/rules")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "AL1 list expected 200");
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("items").is_some(), "AL1 missing 'items'");
    assert!(body["total"].as_u64().is_some(), "AL1 total must be a number");
}

#[actix_web::test]
async fn al2_create_alert_rule_returns_id() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "al2ok@x.com", &[Role::Administrator]).await;
    let (def_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO metric_definitions (name, formula_kind) \
         VALUES ('AlertMetric','moving_average') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/alerts/rules")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "metric_definition_id": def_id,
            "threshold": 30.0,
            "operator": ">",
            "severity": "warning"
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED, "AL2 create expected 201");
    let body: Value = test::read_body_json(res).await;
    let rule_id = body["id"].as_str().expect("AL2 must return id");
    let _ = Uuid::parse_str(rule_id).expect("AL2 id must be UUID");
}

// ---------------------------------------------------------------------------
// P-B Alerts — AL5 events list page
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn al5_list_alert_events_returns_page_envelope() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "al5ok@x.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/alerts/events")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "AL5 events expected 200");
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("items").is_some(), "AL5 missing 'items'");
    assert!(body.get("total").is_some(), "AL5 missing 'total'");
    assert_eq!(body["total"], 0);
}

// ---------------------------------------------------------------------------
// P-B Reports — RP1 list + RP2/RP3/RP5 lifecycle
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn rp1_list_report_jobs_returns_page_envelope() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "rp1ok@x.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/reports/jobs")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "RP1 list expected 200");
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("items").is_some(), "RP1 missing 'items'");
    assert!(body.get("total").is_some(), "RP1 missing 'total'");
    assert_eq!(body["total"], 0);
}

#[actix_web::test]
async fn rp2_create_report_job_returns_id_and_rp3_reflects_status() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "rp2ok@x.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // RP2 create.
    let req = test::TestRequest::post()
        .uri("/api/v1/reports/jobs")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"kind": "kpi_summary", "format": "csv"}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED, "RP2 create expected 201");
    let body: Value = test::read_body_json(res).await;
    let job_id = body["id"].as_str().expect("RP2 must return id");
    let jid = Uuid::parse_str(job_id).expect("RP2 id must be UUID");

    // RP3 get.
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reports/jobs/{jid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "RP3 get expected 200");
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("id").is_some(), "RP3 must have 'id'");
    assert!(body.get("status").is_some(), "RP3 must have 'status'");
    assert!(body.get("kind").is_some(), "RP3 must have 'kind'");
    assert_eq!(body["kind"], "kpi_summary");

    // RP5 cancel → 2xx.
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/reports/jobs/{jid}/cancel"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert!(res.status().is_success(), "RP5 cancel expected 2xx, got {}", res.status());
}

// ---------------------------------------------------------------------------
// P-B Env observations — E6 list
// ---------------------------------------------------------------------------

#[actix_web::test]
async fn e6_list_observations_returns_page_envelope() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "e6ok@x.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/env/observations")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK, "E6 observations expected 200");
    let body: Value = test::read_body_json(res).await;
    assert!(body.get("items").is_some(), "E6 missing 'items'");
    assert!(body.get("total").is_some(), "E6 missing 'total'");
    assert_eq!(body["total"], 0);
}
