//! Talent watchlist tests (T10–T13) — SELF-scoped.
//!
//! Naming: `t_t10_*`, `t_t11_*`, `t_t12_*`, `t_t13_*`.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use terraops_shared::roles::Role;

use common::{authed, build_test_app, TestCtx};

// ── T10: GET /api/v1/talent/watchlists ───────────────────────────────────────

#[actix_web::test]
async fn t_t10_list_watchlists_requires_auth() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/watchlists")
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_t10_list_watchlists_self_scoped() {
    let ctx = TestCtx::new().await;
    let (u1, t1) = authed(&ctx.pool, &ctx.keys, "t10u1@example.com", &[Role::Recruiter]).await;
    let (_u2, t2) = authed(&ctx.pool, &ctx.keys, "t10u2@example.com", &[Role::Recruiter]).await;

    // Insert watchlist for u1
    sqlx::query(
        "INSERT INTO talent_watchlists (owner_id, name) VALUES ($1, 'u1 list')",
    )
    .bind(u1)
    .execute(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // u2 should see empty list
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/watchlists")
        .insert_header(("Authorization", format!("Bearer {t2}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert!(body.as_array().unwrap().is_empty());

    // u1 sees their own list
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/watchlists")
        .insert_header(("Authorization", format!("Bearer {t1}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["name"], "u1 list");
}

// ── T11: POST /api/v1/talent/watchlists ──────────────────────────────────────

#[actix_web::test]
async fn t_t11_create_watchlist_ok() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t11create@example.com", &[Role::Recruiter]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/watchlists")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({"name": "My Favorites"}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["name"], "My Favorites");
    assert_eq!(body["item_count"], 0);
    assert!(body["id"].is_string());
}

#[actix_web::test]
async fn t_t11_create_watchlist_requires_auth() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/watchlists")
        .set_json(json!({"name": "Test"}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

// ── T12: POST /api/v1/talent/watchlists/{id}/items ───────────────────────────

#[actix_web::test]
async fn t_t12_add_item_to_own_watchlist() {
    let ctx = TestCtx::new().await;
    let (uid, token) = authed(&ctx.pool, &ctx.keys, "t12own@example.com", &[Role::Recruiter]).await;

    let (wl_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO talent_watchlists (owner_id, name) VALUES ($1, 'My List') RETURNING id",
    )
    .bind(uid)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let (cand_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Eve', 'e***@x.com', 2, '{python}', 50) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/talent/watchlists/{wl_id}/items"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({"candidate_id": cand_id}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[actix_web::test]
async fn t_t12_add_item_forbidden_for_other_user() {
    let ctx = TestCtx::new().await;
    let (u1, _t1) = authed(&ctx.pool, &ctx.keys, "t12fo1@example.com", &[Role::Recruiter]).await;
    let (_u2, t2) = authed(&ctx.pool, &ctx.keys, "t12fo2@example.com", &[Role::Recruiter]).await;

    let (wl_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO talent_watchlists (owner_id, name) VALUES ($1, 'u1 list') RETURNING id",
    )
    .bind(u1)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let (cand_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Frank', 'f***@x.com', 2, '{java}', 40) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/talent/watchlists/{wl_id}/items"))
        .insert_header(("Authorization", format!("Bearer {t2}")))
        .set_json(json!({"candidate_id": cand_id}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

// ── T13: DELETE /api/v1/talent/watchlists/{id}/items/{cid} ───────────────────

#[actix_web::test]
async fn t_t13_remove_item_ok() {
    let ctx = TestCtx::new().await;
    let (uid, token) = authed(&ctx.pool, &ctx.keys, "t13rem@example.com", &[Role::Recruiter]).await;

    let (wl_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO talent_watchlists (owner_id, name) VALUES ($1, 'Watchlist') RETURNING id",
    )
    .bind(uid)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let (cand_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Grace', 'g***@x.com', 3, '{ts}', 60) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Add the item first
    sqlx::query(
        "INSERT INTO talent_watchlist_items (watchlist_id, candidate_id) VALUES ($1, $2)",
    )
    .bind(wl_id)
    .bind(cand_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/talent/watchlists/{wl_id}/items/{cand_id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

#[actix_web::test]
async fn t_t13_remove_item_forbidden_for_other_user() {
    let ctx = TestCtx::new().await;
    let (u1, _t1) = authed(&ctx.pool, &ctx.keys, "t13f1@example.com", &[Role::Recruiter]).await;
    let (_u2, t2) = authed(&ctx.pool, &ctx.keys, "t13f2@example.com", &[Role::Recruiter]).await;

    let (wl_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO talent_watchlists (owner_id, name) VALUES ($1, 'wl') RETURNING id",
    )
    .bind(u1)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let (cand_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Heidi', 'h***@x.com', 1, '{js}', 20) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO talent_watchlist_items (watchlist_id, candidate_id) VALUES ($1, $2)")
        .bind(wl_id)
        .bind(cand_id)
        .execute(&ctx.pool)
        .await
        .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/talent/watchlists/{wl_id}/items/{cand_id}"))
        .insert_header(("Authorization", format!("Bearer {t2}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}
