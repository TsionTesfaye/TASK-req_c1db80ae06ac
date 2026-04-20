//! Talent weights tests (T7–T8) — SELF-scoped.
//!
//! Naming: `t_t7_*`, `t_t8_*`.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use terraops_shared::roles::Role;

use common::{authed, build_test_app, TestCtx};

// ── T7: GET /api/v1/talent/weights ───────────────────────────────────────────

#[actix_web::test]
async fn t_t7_get_weights_requires_auth() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/weights")
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_t7_get_weights_returns_defaults_when_none_stored() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t7def@example.com", &[Role::Recruiter]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/weights")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["skills_weight"], 40);
    assert_eq!(body["experience_weight"], 30);
    assert_eq!(body["recency_weight"], 15);
    assert_eq!(body["completeness_weight"], 15);
}

#[actix_web::test]
async fn t_t7_get_weights_returns_own_only() {
    let ctx = TestCtx::new().await;
    let (u1, t1) = authed(&ctx.pool, &ctx.keys, "t7u1@example.com", &[Role::Recruiter]).await;
    let (_u2, t2) = authed(&ctx.pool, &ctx.keys, "t7u2@example.com", &[Role::Recruiter]).await;

    // Insert custom weights for u1
    sqlx::query(
        "INSERT INTO talent_weights (user_id, skills_weight, experience_weight, recency_weight, completeness_weight) \
         VALUES ($1, 50, 30, 10, 10)",
    )
    .bind(u1)
    .execute(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // u2 should still get defaults
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/weights")
        .insert_header(("Authorization", format!("Bearer {t2}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["skills_weight"], 40); // default, not u1's 50
}

// ── T8: PUT /api/v1/talent/weights ───────────────────────────────────────────

#[actix_web::test]
async fn t_t8_put_weights_requires_auth() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::put()
        .uri("/api/v1/talent/weights")
        .set_json(json!({
            "skills_weight": 40, "experience_weight": 30,
            "recency_weight": 15, "completeness_weight": 15
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_t8_put_weights_updates_own() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t8own@example.com", &[Role::Recruiter]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::put()
        .uri("/api/v1/talent/weights")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "skills_weight": 50,
            "experience_weight": 25,
            "recency_weight": 15,
            "completeness_weight": 10
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["skills_weight"], 50);
    assert_eq!(body["experience_weight"], 25);
}

#[actix_web::test]
async fn t_t8_put_weights_rejects_sum_not_100() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t8sum@example.com", &[Role::Recruiter]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::put()
        .uri("/api/v1/talent/weights")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "skills_weight": 50,
            "experience_weight": 50,
            "recency_weight": 50,
            "completeness_weight": 50
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[actix_web::test]
async fn t_t8_put_weights_scoped_self_only() {
    let ctx = TestCtx::new().await;
    let (u1, _t1) = authed(&ctx.pool, &ctx.keys, "t8s1@example.com", &[Role::Recruiter]).await;
    let (_u2, t2) = authed(&ctx.pool, &ctx.keys, "t8s2@example.com", &[Role::Recruiter]).await;

    // u2 updates their own weights
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::put()
        .uri("/api/v1/talent/weights")
        .insert_header(("Authorization", format!("Bearer {t2}")))
        .set_json(json!({
            "skills_weight": 60,
            "experience_weight": 20,
            "recency_weight": 10,
            "completeness_weight": 10
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);

    // u1 should still have defaults (unaffected).
    // Note: `issue_session_for` returns `(token, session_id)` — the token is
    // the first tuple element. Earlier versions of this test destructured in
    // reverse order, so the "bearer" was actually a session UUID, the request
    // was rejected at auth, and the response body was an error envelope whose
    // `skills_weight` key was missing (Null).
    let (t1_new, _sid) = common::issue_session_for(&ctx.pool, &ctx.keys, u1).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/weights")
        .insert_header(("Authorization", format!("Bearer {t1_new}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["skills_weight"], 40); // default
}

// ── Audit #6 Issue #1: T7/T8 must require `talent.read` ──────────────────────
// Before the fix, any authenticated user — including a bare RegularUser —
// could read and mutate talent weights. After the fix, callers without
// `talent.read` (the permission held by Recruiter + Administrator) are
// refused with 403 at the handler layer.

#[actix_web::test]
async fn t_t7_get_weights_forbidden_without_talent_read() {
    let ctx = TestCtx::new().await;
    let (_uid, token) =
        authed(&ctx.pool, &ctx.keys, "t7rbac@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/weights")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_t8_put_weights_forbidden_without_talent_read() {
    let ctx = TestCtx::new().await;
    let (_uid, token) =
        authed(&ctx.pool, &ctx.keys, "t8rbac@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::put()
        .uri("/api/v1/talent/weights")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "skills_weight": 40, "experience_weight": 30,
            "recency_weight": 15, "completeness_weight": 15
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_t10_list_watchlists_forbidden_without_talent_read() {
    let ctx = TestCtx::new().await;
    let (_uid, token) =
        authed(&ctx.pool, &ctx.keys, "t10rbac@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/watchlists")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_t11_create_watchlist_forbidden_without_talent_read() {
    let ctx = TestCtx::new().await;
    let (_uid, token) =
        authed(&ctx.pool, &ctx.keys, "t11rbac@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/watchlists")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({ "name": "denied" }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
