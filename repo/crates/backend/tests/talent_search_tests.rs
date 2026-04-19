//! Talent search + candidates CRUD tests (T1–T3).
//!
//! Naming: `t_t1_*`, `t_t2_*`, `t_t3_*` so `audit_endpoints.sh` picks them up.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use terraops_shared::roles::Role;

use common::{authed, build_test_app, TestCtx};

// ── T1: GET /api/v1/talent/candidates ────────────────────────────────────────

#[actix_web::test]
async fn t_t1_list_candidates_requires_auth() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/candidates")
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_t1_list_candidates_requires_talent_read() {
    let ctx = TestCtx::new().await;
    // RegularUser has no talent.read
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t1noperm@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/candidates")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_t1_list_candidates_returns_empty_initially() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t1recruiter@example.com", &[Role::Recruiter]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/candidates")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert!(body.as_array().unwrap().is_empty());
}

#[actix_web::test]
async fn t_t1_list_candidates_with_search_q() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t1search@example.com", &[Role::Recruiter]).await;
    // Seed a candidate with known full_name
    sqlx::query(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Alice Rust', 'a***@x.com', 5, '{rust,postgres}', 80)",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    // Search for "alice"
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/candidates?q=alice")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["full_name"], "Alice Rust");
}

#[actix_web::test]
async fn t_t1_list_candidates_x_total_count_header() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t1total@example.com", &[Role::Recruiter]).await;
    sqlx::query(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Bob Dev', 'b***@x.com', 3, '{go}', 60)",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/candidates")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let total = res
        .headers()
        .get("x-total-count")
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    assert!(total >= 1);
}

// ── T2: POST /api/v1/talent/candidates ───────────────────────────────────────

#[actix_web::test]
async fn t_t2_create_candidate_requires_talent_manage() {
    let ctx = TestCtx::new().await;
    // RegularUser has no talent.manage (Recruiter does, per design.md §220).
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t2nopers@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/candidates")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "full_name": "X", "email_mask": "x@x.com",
            "years_experience": 1, "skills": [], "completeness_score": 50
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    // Recruiter has talent.read but not talent.manage
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_t2_create_candidate_admin_ok() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t2admin@example.com", &[Role::Administrator]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/candidates")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "full_name": "Carol Smith",
            "email_mask": "c***@example.com",
            "years_experience": 7,
            "skills": ["rust", "actix"],
            "completeness_score": 90,
            "bio": "Experienced backend developer"
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["full_name"], "Carol Smith");
    assert_eq!(body["years_experience"], 7);
    assert!(body["id"].is_string());
}

#[actix_web::test]
async fn t_t2_create_candidate_invalid_completeness() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t2inv@example.com", &[Role::Administrator]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/candidates")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "full_name": "X", "email_mask": "x@x.com",
            "years_experience": 1, "skills": [], "completeness_score": 150
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ── T3: GET /api/v1/talent/candidates/{id} ───────────────────────────────────

#[actix_web::test]
async fn t_t3_get_candidate_not_found() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t3nf@example.com", &[Role::Recruiter]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/candidates/00000000-0000-0000-0000-000000000000")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn t_t3_get_candidate_ok() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t3ok@example.com", &[Role::Recruiter]).await;
    let (id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Dave Go', 'd***@x.com', 4, '{go,postgres}', 70) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/talent/candidates/{id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["full_name"], "Dave Go");
    assert_eq!(body["id"], id.to_string());
}
