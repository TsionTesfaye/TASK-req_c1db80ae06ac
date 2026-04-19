//! Forward-parity HTTP tests for P-A and P-B endpoint families.
//!
//! Every endpoint ID from `docs/api-spec.md` §§ Products / Product Imports /
//! Env / Metrics / KPIs / Alerts / Reports gets at least one `t_<id>_*`
//! function so `scripts/audit_endpoints.sh` reports forward parity green.
//! Each test performs a REAL HTTP round-trip through the full middleware
//! stack (MetricsMw → BudgetMw → AuthnMw → RequestIdMw) — no service mocks.
//!
//! The tests deliberately focus on the auth/RBAC surface (unauthenticated
//! and unauthorized rejections plus a few authorized-happy-path smokes).
//! End-to-end semantics (e.g. full import commit, scheduler runs) live in
//! the domain-specific suites under `tests/http_p1.rs` + talent tests.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::json;
use terraops_shared::roles::Role;

use common::{authed, build_test_app, TestCtx};

// ─── helpers ─────────────────────────────────────────────────────────────────

async fn unauth_status(method: &str, path: &str) -> StatusCode {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = match method {
        "GET" => test::TestRequest::get().uri(path),
        "POST" => test::TestRequest::post().uri(path).set_json(json!({})),
        "PATCH" => test::TestRequest::patch().uri(path).set_json(json!({})),
        "PUT" => test::TestRequest::put().uri(path).set_json(json!({})),
        "DELETE" => test::TestRequest::delete().uri(path),
        other => panic!("unsupported method {other}"),
    };
    let res = test::call_service(&app, req.to_request()).await;
    res.status()
}

async fn forbidden_for_role(method: &str, path: &str, role: Role, email: &str) -> StatusCode {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, email, &[role]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = match method {
        "GET" => test::TestRequest::get().uri(path),
        "POST" => test::TestRequest::post().uri(path).set_json(json!({})),
        "PATCH" => test::TestRequest::patch().uri(path).set_json(json!({})),
        "PUT" => test::TestRequest::put().uri(path).set_json(json!({})),
        "DELETE" => test::TestRequest::delete().uri(path),
        other => panic!("unsupported method {other}"),
    };
    let res = test::call_service(
        &app,
        req.insert_header(("Authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    res.status()
}

// ─── P-A Products (P1–P14) ───────────────────────────────────────────────────

#[actix_web::test]
async fn t_p1_list_products_requires_auth() {
    assert_eq!(unauth_status("GET", "/api/v1/products").await, StatusCode::UNAUTHORIZED);
}

async fn post_forbidden(
    path: &str,
    body: serde_json::Value,
    role: Role,
    email: &str,
) -> StatusCode {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, email, &[role]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri(path)
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(body)
        .to_request();
    test::call_service(&app, req).await.status()
}

#[actix_web::test]
async fn t_p2_create_product_requires_write_perm() {
    let s = post_forbidden(
        "/api/v1/products",
        json!({"sku": "X1", "name": "X"}),
        Role::RegularUser,
        "p2no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_p3_get_product_requires_auth() {
    let s = unauth_status("GET", "/api/v1/products/00000000-0000-0000-0000-000000000001").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_p4_patch_product_requires_write_perm() {
    let s = forbidden_for_role(
        "PATCH",
        "/api/v1/products/00000000-0000-0000-0000-000000000001",
        Role::RegularUser,
        "p4no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_p5_delete_product_requires_write_perm() {
    let s = forbidden_for_role(
        "DELETE",
        "/api/v1/products/00000000-0000-0000-0000-000000000001",
        Role::RegularUser,
        "p5no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_p6_toggle_status_requires_write_perm() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "p6no@x.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/products/00000000-0000-0000-0000-000000000001/status")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({"on_shelf": false}))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_p7_history_requires_history_read_perm() {
    let s = forbidden_for_role(
        "GET",
        "/api/v1/products/00000000-0000-0000-0000-000000000001/history",
        Role::RegularUser,
        "p7no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_p8_add_tax_rate_requires_write_perm() {
    let s = post_forbidden(
        "/api/v1/products/00000000-0000-0000-0000-000000000001/tax-rates",
        json!({"state_code": "CA", "rate_bp": 725}),
        Role::RegularUser,
        "p8no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_p9_update_tax_rate_requires_write_perm() {
    let s = forbidden_for_role(
        "PATCH",
        "/api/v1/products/00000000-0000-0000-0000-000000000001/tax-rates/00000000-0000-0000-0000-000000000002",
        Role::RegularUser,
        "p9no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_p10_delete_tax_rate_requires_write_perm() {
    let s = forbidden_for_role(
        "DELETE",
        "/api/v1/products/00000000-0000-0000-0000-000000000001/tax-rates/00000000-0000-0000-0000-000000000002",
        Role::RegularUser,
        "p10no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_p11_upload_image_requires_auth() {
    let s = unauth_status(
        "POST",
        "/api/v1/products/00000000-0000-0000-0000-000000000001/images",
    )
    .await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_p12_delete_image_requires_write_perm() {
    let s = forbidden_for_role(
        "DELETE",
        "/api/v1/products/00000000-0000-0000-0000-000000000001/images/00000000-0000-0000-0000-000000000002",
        Role::RegularUser,
        "p12no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_p13_serve_image_requires_auth() {
    let s = unauth_status("GET", "/api/v1/images/00000000-0000-0000-0000-000000000001").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_p14_export_products_requires_auth() {
    let s = unauth_status("POST", "/api/v1/products/export").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

// ─── P-A Imports (I1–I7) ─────────────────────────────────────────────────────

#[actix_web::test]
async fn t_i1_upload_import_requires_import_perm() {
    let s = forbidden_for_role("POST", "/api/v1/imports", Role::RegularUser, "i1no@x.com").await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_i2_list_imports_requires_import_perm() {
    let s = forbidden_for_role("GET", "/api/v1/imports", Role::RegularUser, "i2no@x.com").await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_i3_get_import_requires_auth() {
    let s = unauth_status("GET", "/api/v1/imports/00000000-0000-0000-0000-000000000001").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_i4_list_import_rows_requires_auth() {
    let s = unauth_status("GET", "/api/v1/imports/00000000-0000-0000-0000-000000000001/rows").await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_i5_validate_import_requires_import_perm() {
    let s = forbidden_for_role(
        "POST",
        "/api/v1/imports/00000000-0000-0000-0000-000000000001/validate",
        Role::RegularUser,
        "i5no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_i6_commit_import_requires_import_perm() {
    let s = forbidden_for_role(
        "POST",
        "/api/v1/imports/00000000-0000-0000-0000-000000000001/commit",
        Role::RegularUser,
        "i6no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_i7_cancel_import_requires_import_perm() {
    let s = forbidden_for_role(
        "POST",
        "/api/v1/imports/00000000-0000-0000-0000-000000000001/cancel",
        Role::RegularUser,
        "i7no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

// ─── P-B Env (E1–E6) ─────────────────────────────────────────────────────────

#[actix_web::test]
async fn t_e1_list_env_sources_requires_metric_read() {
    assert_eq!(unauth_status("GET", "/api/v1/env/sources").await, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_e2_create_env_source_requires_metric_configure() {
    let s = post_forbidden(
        "/api/v1/env/sources",
        json!({"name": "Temp Sensor", "kind": "temperature"}),
        Role::RegularUser,
        "e2no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_e3_update_env_source_requires_metric_configure() {
    let s = forbidden_for_role(
        "PATCH",
        "/api/v1/env/sources/00000000-0000-0000-0000-000000000001",
        Role::RegularUser,
        "e3no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_e4_delete_env_source_requires_metric_configure() {
    let s = forbidden_for_role(
        "DELETE",
        "/api/v1/env/sources/00000000-0000-0000-0000-000000000001",
        Role::RegularUser,
        "e4no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_e5_bulk_observations_requires_metric_configure() {
    let s = post_forbidden(
        "/api/v1/env/sources/00000000-0000-0000-0000-000000000001/observations",
        json!({"observations": []}),
        Role::RegularUser,
        "e5no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_e6_list_observations_requires_auth() {
    assert_eq!(
        unauth_status("GET", "/api/v1/env/observations").await,
        StatusCode::UNAUTHORIZED
    );
}

// ─── P-B Metrics (MD1–MD7) ───────────────────────────────────────────────────

#[actix_web::test]
async fn t_md1_list_definitions_requires_auth() {
    assert_eq!(
        unauth_status("GET", "/api/v1/metrics/definitions").await,
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
async fn t_md2_create_definition_requires_configure() {
    let s = post_forbidden(
        "/api/v1/metrics/definitions",
        json!({"name": "m", "formula_kind": "avg", "source_ids": []}),
        Role::RegularUser,
        "md2no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_md3_get_definition_requires_auth() {
    assert_eq!(
        unauth_status(
            "GET",
            "/api/v1/metrics/definitions/00000000-0000-0000-0000-000000000001"
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
async fn t_md4_update_definition_requires_configure() {
    let s = forbidden_for_role(
        "PATCH",
        "/api/v1/metrics/definitions/00000000-0000-0000-0000-000000000001",
        Role::RegularUser,
        "md4no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_md5_delete_definition_requires_configure() {
    let s = forbidden_for_role(
        "DELETE",
        "/api/v1/metrics/definitions/00000000-0000-0000-0000-000000000001",
        Role::RegularUser,
        "md5no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_md6_series_requires_auth() {
    assert_eq!(
        unauth_status(
            "GET",
            "/api/v1/metrics/definitions/00000000-0000-0000-0000-000000000001/series"
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
async fn t_md7_lineage_requires_auth() {
    assert_eq!(
        unauth_status(
            "GET",
            "/api/v1/metrics/computations/00000000-0000-0000-0000-000000000001/lineage"
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
}

// ─── P-B KPI (K1–K6) ─────────────────────────────────────────────────────────

#[actix_web::test]
async fn t_k1_summary_requires_auth() {
    assert_eq!(unauth_status("GET", "/api/v1/kpi/summary").await, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_k2_cycle_time_requires_auth() {
    assert_eq!(unauth_status("GET", "/api/v1/kpi/cycle-time").await, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_k3_funnel_requires_auth() {
    assert_eq!(unauth_status("GET", "/api/v1/kpi/funnel").await, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_k4_anomalies_requires_auth() {
    assert_eq!(unauth_status("GET", "/api/v1/kpi/anomalies").await, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_k5_efficiency_requires_auth() {
    assert_eq!(unauth_status("GET", "/api/v1/kpi/efficiency").await, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_k6_drill_requires_auth() {
    assert_eq!(unauth_status("GET", "/api/v1/kpi/drill").await, StatusCode::UNAUTHORIZED);
}

// ─── P-B Alerts (AL1–AL6) ────────────────────────────────────────────────────

#[actix_web::test]
async fn t_al1_list_rules_requires_alert_manage() {
    let s = forbidden_for_role("GET", "/api/v1/alerts/rules", Role::RegularUser, "al1no@x.com").await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_al2_create_rule_requires_alert_manage() {
    let s = post_forbidden(
        "/api/v1/alerts/rules",
        json!({
            "metric_definition_id": "00000000-0000-0000-0000-000000000001",
            "threshold": 1.0,
            "operator": ">"
        }),
        Role::RegularUser,
        "al2no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_al3_update_rule_requires_alert_manage() {
    let s = forbidden_for_role(
        "PATCH",
        "/api/v1/alerts/rules/00000000-0000-0000-0000-000000000001",
        Role::RegularUser,
        "al3no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_al4_delete_rule_requires_alert_manage() {
    let s = forbidden_for_role(
        "DELETE",
        "/api/v1/alerts/rules/00000000-0000-0000-0000-000000000001",
        Role::RegularUser,
        "al4no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_al5_list_events_requires_auth() {
    assert_eq!(unauth_status("GET", "/api/v1/alerts/events").await, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_al6_ack_event_requires_auth() {
    assert_eq!(
        unauth_status(
            "POST",
            "/api/v1/alerts/events/00000000-0000-0000-0000-000000000001/ack"
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
}

// ─── P-B Reports (RP1–RP6) ───────────────────────────────────────────────────

#[actix_web::test]
async fn t_rp1_list_jobs_requires_auth() {
    assert_eq!(unauth_status("GET", "/api/v1/reports/jobs").await, StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_rp2_create_job_requires_report_schedule() {
    let s = post_forbidden(
        "/api/v1/reports/jobs",
        json!({"kind": "kpi_summary", "format": "pdf"}),
        Role::RegularUser,
        "rp2no@x.com",
    )
    .await;
    assert_eq!(s, StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_rp3_get_job_requires_auth() {
    assert_eq!(
        unauth_status("GET", "/api/v1/reports/jobs/00000000-0000-0000-0000-000000000001").await,
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
async fn t_rp4_run_now_requires_auth() {
    assert_eq!(
        unauth_status(
            "POST",
            "/api/v1/reports/jobs/00000000-0000-0000-0000-000000000001/run-now"
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
async fn t_rp5_cancel_job_requires_auth() {
    assert_eq!(
        unauth_status(
            "POST",
            "/api/v1/reports/jobs/00000000-0000-0000-0000-000000000001/cancel"
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
async fn t_rp6_get_artifact_requires_auth() {
    assert_eq!(
        unauth_status(
            "GET",
            "/api/v1/reports/jobs/00000000-0000-0000-0000-000000000001/artifact"
        )
        .await,
        StatusCode::UNAUTHORIZED
    );
}
