//! Talent recommendations tests (T4–T6).
//!
//! Naming: `t_t4_*`, `t_t5_*`, `t_t6_*`.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use terraops_shared::roles::Role;

use common::{authed, build_test_app, TestCtx};

// ── T4: GET /api/v1/talent/roles ─────────────────────────────────────────────

#[actix_web::test]
async fn t_t4_list_roles_requires_auth() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/roles")
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_t4_list_roles_returns_empty_initially() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t4empty@example.com", &[Role::Recruiter]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/roles")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert!(body.as_array().unwrap().is_empty());
}

// ── T5: POST /api/v1/talent/roles ────────────────────────────────────────────

#[actix_web::test]
async fn t_t5_create_role_requires_talent_manage() {
    let ctx = TestCtx::new().await;
    // RegularUser has no talent.manage (Recruiter does, per design.md §220).
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t5noperm@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/roles")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "title": "Rust Engineer",
            "required_skills": ["rust"],
            "min_years": 3
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_t5_create_role_admin_ok() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t5admin@example.com", &[Role::Administrator]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/roles")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "title": "Backend Engineer",
            "required_skills": ["rust", "postgres"],
            "min_years": 3
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["title"], "Backend Engineer");
    assert_eq!(body["status"], "open");
    assert!(body["id"].is_string());
}

// ── T6: GET /api/v1/talent/recommendations ───────────────────────────────────

#[actix_web::test]
async fn t_t6_recommendations_requires_auth() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/recommendations?role_id=00000000-0000-0000-0000-000000000001")
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_t6_recommendations_role_not_found() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t6nf@example.com", &[Role::Recruiter]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/recommendations?role_id=00000000-0000-0000-0000-000000000001")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn t_t6_recommendations_cold_start() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t6cs@example.com", &[Role::Recruiter]).await;

    // Create a role
    let (role_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO roles_open (title, required_skills, min_years) \
         VALUES ('SRE', '{rust,kubernetes}', 3) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Create 2 candidates (less than 10 feedback → cold start)
    sqlx::query(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('High Completeness', 'h***@x.com', 5, '{rust}', 90), \
                ('Low Completeness', 'l***@x.com', 5, '{rust}', 20)",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/talent/recommendations?role_id={role_id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;

    assert_eq!(body["cold_start"], true, "expected cold_start=true with no feedback");
    let candidates = body["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 2);
    // Higher completeness should rank first
    assert_eq!(candidates[0]["candidate"]["full_name"], "High Completeness");
    // Reasons must mention cold-start
    assert!(candidates[0]["reasons"][0]
        .as_str()
        .unwrap()
        .contains("Cold-start"));
}

#[actix_web::test]
async fn t_t6_recommendations_blended_after_10_feedback() {
    let ctx = TestCtx::new().await;
    let (recruiter_id, token) = authed(&ctx.pool, &ctx.keys, "t6blend@example.com", &[Role::Recruiter]).await;

    // Create candidates
    let (c1_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Rust Expert', 'r***@x.com', 8, '{rust,postgres,kubernetes}', 95) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let (c2_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('No Skills', 'n***@x.com', 1, '{python}', 30) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Create a role matching c1 better
    let (role_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO roles_open (title, required_skills, min_years) \
         VALUES ('Senior Rust Dev', '{rust,postgres}', 5) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Insert 10 feedback records to flip to blended scoring
    for i in 0..10 {
        let (fuid,): (uuid::Uuid,) = sqlx::query_as(
            "INSERT INTO users (display_name, email_ciphertext, email_hash, email_mask, password_hash) \
             VALUES ($1, '\\x00'::bytea, $2, 'u***@x.com', '$argon2id$v=19$m=19456,t=2,p=1$aaaa$bbbb') \
             RETURNING id",
        )
        .bind(format!("FbUser {i}"))
        .bind(format!("hash{i}").as_bytes().to_vec())
        .fetch_one(&ctx.pool)
        .await
        .unwrap();

        let cid = if i % 2 == 0 { c1_id } else { c2_id };
        sqlx::query(
            "INSERT INTO talent_feedback (candidate_id, role_id, owner_id, thumb) \
             VALUES ($1, $2, $3, 'up')",
        )
        .bind(cid)
        .bind(role_id)
        .bind(fuid)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/talent/recommendations?role_id={role_id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;

    assert_eq!(body["cold_start"], false, "expected blended after 10 feedback");
    let candidates = body["candidates"].as_array().unwrap();
    assert!(candidates.len() >= 2);
    // Rust Expert should score higher due to better skill match
    assert_eq!(candidates[0]["candidate"]["full_name"], "Rust Expert");

    let reasons = candidates[0]["reasons"].as_array().unwrap();
    assert!(reasons.iter().any(|r| r.as_str().unwrap().contains("Skill match")));
    assert!(reasons.iter().any(|r| r.as_str().unwrap().contains("Experience")));
}
