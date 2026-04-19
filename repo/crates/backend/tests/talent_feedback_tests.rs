//! Talent feedback tests (T9) — PERM(talent.feedback).
//!
//! Naming: `t_t9_*`.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use terraops_shared::roles::Role;

use common::{authed, build_test_app, TestCtx};

// ── T9: POST /api/v1/talent/feedback ─────────────────────────────────────────

#[actix_web::test]
async fn t_t9_feedback_requires_auth() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/feedback")
        .set_json(json!({
            "candidate_id": "00000000-0000-0000-0000-000000000001",
            "thumb": "up"
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_t9_feedback_requires_talent_feedback_perm() {
    let ctx = TestCtx::new().await;
    // RegularUser does not have talent.feedback
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t9noperm@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let (cand_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Ivan', 'i***@x.com', 2, '{}', 30) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/feedback")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "candidate_id": cand_id,
            "thumb": "up"
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_t9_feedback_recruiter_can_submit() {
    let ctx = TestCtx::new().await;
    // Recruiter has talent.feedback
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t9rec@example.com", &[Role::Recruiter]).await;
    let (cand_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Judy', 'j***@x.com', 5, '{rust}', 75) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/feedback")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "candidate_id": cand_id,
            "thumb": "up",
            "note": "Strong Rust skills"
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["thumb"], "up");
    assert_eq!(body["note"], "Strong Rust skills");
    assert_eq!(body["candidate_id"], cand_id.to_string());
    assert!(body["id"].is_string());
}

#[actix_web::test]
async fn t_t9_feedback_invalid_thumb() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t9thumb@example.com", &[Role::Recruiter]).await;
    let (cand_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Karl', 'k***@x.com', 3, '{}', 40) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/feedback")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "candidate_id": cand_id,
            "thumb": "sideways"
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[actix_web::test]
async fn t_t9_feedback_candidate_not_found() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t9nf@example.com", &[Role::Recruiter]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/feedback")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "candidate_id": "00000000-0000-0000-0000-000000000001",
            "thumb": "down"
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn t_t9_feedback_owner_scoped_record() {
    let ctx = TestCtx::new().await;
    let (uid, token) = authed(&ctx.pool, &ctx.keys, "t9owner@example.com", &[Role::Recruiter]).await;
    let (cand_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Lena', 'l***@x.com', 4, '{python}', 65) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/feedback")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "candidate_id": cand_id,
            "thumb": "down",
            "note": "Not a fit"
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(res).await;
    // owner_id must be the calling user
    assert_eq!(body["owner_id"], uid.to_string());
}
