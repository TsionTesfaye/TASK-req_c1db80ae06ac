//! CSRF contract tests (M1).
//!
//! Proves the documented mutation-request header control from
//! `docs/design.md` §CSRF and `docs/api-spec.md` §auth:
//!
//! > State-changing requests require `X-Requested-With: terraops` in
//! > addition to the bearer access token.
//!
//! These tests drive the app through `build_test_app_strict` (no
//! auto-injector) so the real `CsrfMw` behavior is exercised end-to-end.
//! They also validate that `GET` is exempt and that the SPA-style
//! `build_test_app` harness (with header auto-inject) continues to
//! pass mutations through unchanged.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use terraops_shared::roles::Role;

use common::{authed, build_test_app, build_test_app_strict, TestCtx};

fn bearer(tok: &str) -> String {
    format!("Bearer {tok}")
}

// ── Strict app: asserts the CSRF middleware actually runs ─────────────────────

#[actix_web::test]
async fn csrf_post_without_header_is_forbidden() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "csrf-post@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app_strict(ctx.state.clone())).await;

    // POST without X-Requested-With → 403.
    let req = test::TestRequest::post()
        .uri("/api/v1/alerts/rules")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"metric_definition_id": "00000000-0000-0000-0000-000000000001"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "mutation without X-Requested-With must be rejected 403"
    );

    // And the error envelope carries the normalized AUTH_FORBIDDEN code.
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["error_code"].as_str().unwrap(), "AUTH_FORBIDDEN");
}

#[actix_web::test]
async fn csrf_post_with_wrong_value_is_forbidden() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "csrf-wrong@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app_strict(ctx.state.clone())).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/alerts/rules")
        .insert_header(("Authorization", bearer(&token)))
        .insert_header(("X-Requested-With", "XMLHttpRequest")) // wrong value
        .set_json(json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "mutation with wrong X-Requested-With value must be 403"
    );
}

#[actix_web::test]
async fn csrf_patch_put_delete_all_require_header() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "csrf-methods@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app_strict(ctx.state.clone())).await;

    let uuid_path = "/api/v1/alerts/rules/00000000-0000-0000-0000-000000000001";

    for req in [
        test::TestRequest::patch()
            .uri(uuid_path)
            .insert_header(("Authorization", bearer(&token)))
            .set_json(json!({"threshold": 1.0}))
            .to_request(),
        test::TestRequest::put()
            .uri("/api/v1/talent/weights")
            .insert_header(("Authorization", bearer(&token)))
            .set_json(json!({}))
            .to_request(),
        test::TestRequest::delete()
            .uri(uuid_path)
            .insert_header(("Authorization", bearer(&token)))
            .to_request(),
    ] {
        let resp = test::call_service(&app, req).await;
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "PATCH/PUT/DELETE without X-Requested-With must be 403"
        );
    }
}

#[actix_web::test]
async fn csrf_get_does_not_require_header() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "csrf-get@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app_strict(ctx.state.clone())).await;

    // GET without X-Requested-With → must NOT be rejected by CsrfMw.
    let req = test::TestRequest::get()
        .uri("/api/v1/alerts/rules")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_ne!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "GETs are side-effect-free and must not require the CSRF header"
    );
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn csrf_unauthenticated_mutation_still_401_first() {
    // CsrfMw sits AFTER AuthnMw — unauth requests must surface as 401,
    // not 403 masked by CSRF. This preserves the clearer error signal.
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app_strict(ctx.state.clone())).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/alerts/rules")
        .set_json(json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "unauthenticated mutation must surface as 401, not 403"
    );
}

#[actix_web::test]
async fn csrf_post_with_correct_header_passes() {
    // Positive proof: with the correct header, CsrfMw passes and the
    // request reaches the handler (201 CREATED on a valid rule body).
    let ctx = TestCtx::new().await;
    let (uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "csrf-ok@example.com",
        &[Role::Administrator],
    )
    .await;

    // Seed a metric_definitions row to reference.
    let (def_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO metric_definitions (name, formula_kind) \
         VALUES ('CsrfRuleMetric','moving_average') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let _ = uid; // suppress unused-warning if any

    let app = test::init_service(build_test_app_strict(ctx.state.clone())).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/alerts/rules")
        .insert_header(("Authorization", bearer(&token)))
        .insert_header(("X-Requested-With", "terraops"))
        .set_json(json!({
            "metric_definition_id": def_id,
            "threshold": 1.0,
            "operator": ">",
            "duration_seconds": 0,
            "severity": "warning"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "mutation with valid X-Requested-With must pass CsrfMw and reach handler"
    );
}

// ── Relaxed (auto-inject) app: confirms existing tests still work ────────────

#[actix_web::test]
async fn csrf_non_strict_app_autoinjects_for_existing_tests() {
    // Sanity: the default `build_test_app` harness (used by the ~205
    // pre-existing mutation tests) auto-injects the CSRF header so those
    // tests exercise the CsrfMw-on path without manual header plumbing.
    let ctx = TestCtx::new().await;
    let (uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "csrf-autoinject@example.com",
        &[Role::Administrator],
    )
    .await;
    let (def_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO metric_definitions (name, formula_kind) \
         VALUES ('CsrfAutoInjectMetric','moving_average') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let _ = uid;

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/alerts/rules")
        .insert_header(("Authorization", bearer(&token)))
        // intentionally omit X-Requested-With — injector must supply it.
        .set_json(json!({
            "metric_definition_id": def_id,
            "threshold": 2.0,
            "operator": ">",
            "duration_seconds": 0,
            "severity": "warning"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
}
