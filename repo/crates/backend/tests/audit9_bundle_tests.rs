//! Audit #9 bugfix-1 bundle — real HTTP coverage for the five issue fixes:
//!   1. env-source PATCH threads and clears site/department/unit
//!   2. product PATCH tri-state clear semantics on master-data fields
//!   3. negative price_cents returns 422, not 500
//!   5. report self-service endpoints require `report.run` in addition to
//!      owner match (issue 4 is a docs-only fix, no test)

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use terraops_shared::roles::Role;
use uuid::Uuid;

use common::{authed, build_test_app, create_user_with_roles, issue_session_for, TestCtx};

fn bearer(tok: &str) -> String {
    format!("Bearer {tok}")
}

// =============================================================================
// Audit #9 issue 1 — env-source PATCH honors site/department/unit reassignment
// =============================================================================
#[actix_web::test]
async fn audit9_env_source_patch_threads_and_clears_scope() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "audit9-envsrc@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;

    // Seed master data
    let (site_a,): (Uuid,) = sqlx::query_as(
        "INSERT INTO sites (code, name) VALUES ('SA9','Site A9') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (site_b,): (Uuid,) = sqlx::query_as(
        "INSERT INTO sites (code, name) VALUES ('SB9','Site B9') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (dept_a,): (Uuid,) = sqlx::query_as(
        "INSERT INTO departments (site_id, code, name) VALUES ($1,'DA9','DeptA9') RETURNING id",
    )
    .bind(site_a)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (unit_a,): (Uuid,) = sqlx::query_as(
        "INSERT INTO units (code) VALUES ('ea9') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Create a source scoped to (site_a, dept_a, unit_a)
    let req = test::TestRequest::post()
        .uri("/api/v1/env/sources")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "name": "SrcScope",
            "kind": "temperature",
            "site_id": site_a,
            "department_id": dept_a,
            "unit_id": unit_a
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let sid = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    assert_eq!(body["site_id"].as_str().unwrap(), site_a.to_string());
    assert_eq!(body["department_id"].as_str().unwrap(), dept_a.to_string());
    assert_eq!(body["unit_id"].as_str().unwrap(), unit_a.to_string());

    // PATCH reassign site to site_b — before the fix this silently kept site_a
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/env/sources/{sid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"site_id": site_b}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(
        body["site_id"].as_str().unwrap(),
        site_b.to_string(),
        "site_id reassignment must be persisted"
    );

    // PATCH clear department and unit via explicit null
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/env/sources/{sid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"department_id": null, "unit_id": null}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert!(
        body["department_id"].is_null(),
        "department_id must be cleared by explicit null"
    );
    assert!(
        body["unit_id"].is_null(),
        "unit_id must be cleared by explicit null"
    );
    // Site must remain unchanged (omitted field → leave as-is).
    assert_eq!(body["site_id"].as_str().unwrap(), site_b.to_string());

    // PATCH with unrelated fields must not touch scope pointers
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/env/sources/{sid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"name": "SrcScopeRenamed"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["site_id"].as_str().unwrap(), site_b.to_string());
    assert!(body["department_id"].is_null());
    assert!(body["unit_id"].is_null());
}

// =============================================================================
// Audit #9 issue 2 — product PATCH tri-state clear semantics
// =============================================================================
#[actix_web::test]
async fn audit9_product_patch_clears_optional_master_data() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "audit9-prod-clear@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;

    // Seed master data
    let (site_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO sites (code, name) VALUES ('SP9','SitePC9') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (dept_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO departments (site_id, code, name) VALUES ($1,'DP9','DeptPC9') RETURNING id",
    )
    .bind(site_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (cat_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO categories (name) VALUES ('CatPC9') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Create product with spu + barcode + category + site + department set
    let req = test::TestRequest::post()
        .uri("/api/v1/products")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "sku": "SKU-CLR",
            "spu": "SPU-001",
            "barcode": "0123456789012",
            "name": "Clear Me",
            "category_id": cat_id,
            "site_id": site_id,
            "department_id": dept_id,
            "price_cents": 1000,
            "currency": "USD"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let pid = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // Sanity check: values landed.
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["spu"], "SPU-001");
    assert_eq!(body["barcode"], "0123456789012");
    assert_eq!(body["category_id"].as_str().unwrap(), cat_id.to_string());
    assert_eq!(body["site_id"].as_str().unwrap(), site_id.to_string());
    assert_eq!(body["department_id"].as_str().unwrap(), dept_id.to_string());

    // PATCH with explicit null on every clearable optional master-data field
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "spu": null,
            "barcode": null,
            "category_id": null,
            "site_id": null,
            "department_id": null
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Re-read — all cleared
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    // `spu` and `barcode` use skip_serializing_if, so clearing removes the key
    // entirely from the response body.
    assert!(body.get("spu").map(|v| v.is_null()).unwrap_or(true));
    assert!(body.get("barcode").map(|v| v.is_null()).unwrap_or(true));
    assert!(body["category_id"].is_null());
    assert!(body["site_id"].is_null());
    assert!(body["department_id"].is_null());

    // PATCH omitting fields must leave them untouched: set spu again, then
    // PATCH only the name and verify spu survives.
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"spu": "SPU-RESET"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"name": "Renamed"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["spu"], "SPU-RESET");
    assert_eq!(body["name"], "Renamed");
}

// =============================================================================
// Audit #9 issue 3 — negative price_cents → 422 (not 500)
// =============================================================================
#[actix_web::test]
async fn audit9_product_negative_price_cents_is_422() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "audit9-neg-price@example.com",
        &[Role::Administrator, Role::DataSteward],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Create with negative price → 422 validation error
    let req = test::TestRequest::post()
        .uri("/api/v1/products")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "sku": "SKU-NEG",
            "name": "Neg Price",
            "price_cents": -1
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "negative price_cents on create must be a validation 422, not a 500"
    );
    let body: Value = test::read_body_json(resp).await;
    let msg = body["message"]
        .as_str()
        .or_else(|| body["error"].as_str())
        .unwrap_or("");
    assert!(
        msg.to_lowercase().contains("price_cents"),
        "error body should name the offending field, got: {body}"
    );

    // Create a valid product, then PATCH to negative → 422
    let req = test::TestRequest::post()
        .uri("/api/v1/products")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"sku": "SKU-POS", "name": "Ok", "price_cents": 500}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let pid = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"price_cents": -999}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::UNPROCESSABLE_ENTITY,
        "negative price_cents on PATCH must be a validation 422, not a 500"
    );

    // Zero price is allowed (>= 0)
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"price_cents": 0}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

// =============================================================================
// Audit #9 issue 5 — report self-service endpoints require report.run
// =============================================================================
#[actix_web::test]
async fn audit9_report_self_endpoints_require_report_run_permission() {
    let ctx = TestCtx::new().await;

    // Owner user with NO roles → no `report.run`. We insert a report_jobs row
    // directly so we don't need `report.schedule` to seed the fixture.
    let owner_id = create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "audit9-report-noperm@example.com",
        "TerraOps!2026",
        &[],
    )
    .await;
    let (owner_token, _sid) = issue_session_for(&ctx.pool, &ctx.keys, owner_id).await;

    let (job_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO report_jobs (owner_id, kind, format, params, cron) \
         VALUES ($1, 'kpi_summary', 'csv', '{}'::jsonb, NULL) \
         RETURNING id",
    )
    .bind(owner_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // RP3 GET — owner, no report.run → 403
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reports/jobs/{job_id}"))
        .insert_header(("Authorization", bearer(&owner_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "owner without report.run must be forbidden from GET"
    );

    // RP4 run-now — owner, no report.run → 403
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/reports/jobs/{job_id}/run-now"))
        .insert_header(("Authorization", bearer(&owner_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // RP5 cancel — owner, no report.run → 403
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/reports/jobs/{job_id}/cancel"))
        .insert_header(("Authorization", bearer(&owner_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // RP6 artifact — owner, no report.run → 403 (note: even before file exists,
    // the permission check fires first).
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reports/jobs/{job_id}/artifact"))
        .insert_header(("Authorization", bearer(&owner_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Sanity: a regular_user (which DOES have report.run) but is NOT the
    // owner should be blocked by the owner check, not the permission check
    // → still 403 but via OwnerGuard. This proves we kept both guards.
    let (_other_id, other_token) = authed(
        &ctx.pool,
        &ctx.keys,
        "audit9-report-notowner@example.com",
        &[Role::RegularUser],
    )
    .await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/reports/jobs/{job_id}"))
        .insert_header(("Authorization", bearer(&other_token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}
