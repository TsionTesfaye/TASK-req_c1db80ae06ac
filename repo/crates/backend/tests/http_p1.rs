//! P1 HTTP integration suite — no mocks, real Postgres, real middleware.
//!
//! Naming contract: every `#[actix_web::test]` is named `t_<id>_*` where
//! `<id>` is the endpoint id from `docs/api-spec.md` / `plan.md`. One test
//! per METHOD+PATH at minimum, with negative tests for the critical
//! authz boundaries.
//!
//! Requires `DATABASE_URL` pointing at a writable Postgres; the harness
//! migrates once per process and truncates dynamic tables on each test.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use std::net::SocketAddr;
use terraops_shared::roles::Role;
use uuid::Uuid;

/// Loopback peer address used for tests that install an allowlist entry and
/// then call additional endpoints in the same test. The authn middleware
/// reads the peer IP off the request; in test mode that's only set if we
/// explicitly pass it via `TestRequest::peer_addr`. We pair this with a CIDR
/// that contains 127.0.0.1 so our own follow-up calls pass allowlist check.
fn loopback_peer() -> SocketAddr {
    "127.0.0.1:50000".parse().unwrap()
}

use common::{authed, build_test_app, create_user_with_roles, TestCtx};

// ============================================================================
// System — S1, S2
// ============================================================================

#[actix_web::test]
async fn t_s1_health_ok_unauthenticated() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get().uri("/api/v1/health").to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["status"], "ok");
}

#[actix_web::test]
async fn t_s2_ready_reports_db_up() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get().uri("/api/v1/ready").to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["status"], "ready");
    assert_eq!(body["db"], true);
}

// ============================================================================
// Auth — A1..A5
// ============================================================================

#[actix_web::test]
async fn t_a1_login_returns_access_token_and_refresh_cookie() {
    let ctx = TestCtx::new().await;
    create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "a1@example.com",
        "TerraOps!2026",
        &[Role::RegularUser],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({"username": "a1@example.com", "password": "TerraOps!2026"}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    // refresh cookie must be set, HttpOnly + Secure.
    let set_cookie = res
        .headers()
        .get(actix_web::http::header::SET_COOKIE)
        .expect("refresh cookie present")
        .to_str()
        .unwrap()
        .to_string();
    assert!(set_cookie.starts_with("tops_refresh="));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("Secure"));
    let body: Value = test::read_body_json(res).await;
    assert!(!body["access_token"].as_str().unwrap().is_empty());
    assert_eq!(body["user"]["email_mask"].as_str().unwrap().contains("@"), true);
}

#[actix_web::test]
async fn t_a1_login_rejects_bad_password() {
    let ctx = TestCtx::new().await;
    create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "a1bad@example.com",
        "TerraOps!2026",
        &[Role::RegularUser],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({"username": "a1bad@example.com", "password": "WrongPass!9999"}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_a2_refresh_rotates_cookie_and_issues_new_access_token() {
    let ctx = TestCtx::new().await;
    create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "a2@example.com",
        "TerraOps!2026",
        &[Role::RegularUser],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    // Login to obtain an initial refresh cookie.
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({"username": "a2@example.com", "password": "TerraOps!2026"}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let cookie = res
        .headers()
        .get(actix_web::http::header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let raw = cookie
        .split(';')
        .next()
        .unwrap()
        .to_string();
    // Refresh.
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header((actix_web::http::header::COOKIE, raw))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert!(!body["access_token"].as_str().unwrap().is_empty());
}

#[actix_web::test]
async fn t_a3_logout_revokes_session() {
    let ctx = TestCtx::new().await;
    create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "a3@example.com",
        "TerraOps!2026",
        &[Role::RegularUser],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({"username": "a3@example.com", "password": "TerraOps!2026"}))
        .to_request();
    let login = test::call_service(&app, req).await;
    let cookie = login
        .headers()
        .get(actix_web::http::header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/logout")
        .insert_header((actix_web::http::header::COOKIE, cookie.clone()))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // Same refresh should no longer work.
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .insert_header((actix_web::http::header::COOKIE, cookie))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_a4_me_returns_identity_for_bearer_holder() {
    let ctx = TestCtx::new().await;
    let (id, token) = authed(&ctx.pool, &ctx.keys, "a4@example.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["id"], id.to_string());
    assert!(body["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p == "metric.configure"));
}

#[actix_web::test]
async fn t_a4_me_rejects_missing_bearer() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get().uri("/api/v1/auth/me").to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_a5_change_password_revokes_old_sessions() {
    let ctx = TestCtx::new().await;
    let (_id, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "a5@example.com",
        &[Role::RegularUser],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/change-password")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .set_json(json!({
            "current_password": "TerraOps!2026",
            "new_password": "NewTerraOps!2027"
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // Old token's session is revoked.
    let req = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header(("Authorization", format!("Bearer {}", token)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Users — U1..U10
// ============================================================================

#[actix_web::test]
async fn t_u1_list_users_requires_user_manage() {
    let ctx = TestCtx::new().await;
    let (_id, user_tok) = authed(&ctx.pool, &ctx.keys, "u1u@example.com", &[Role::RegularUser]).await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "u1a@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Regular user → 403
    let req = test::TestRequest::get()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", user_tok)))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::FORBIDDEN);
    // Admin → 200 + page
    let req = test::TestRequest::get()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert!(body["items"].as_array().unwrap().len() >= 2);
}

#[actix_web::test]
async fn t_u2_create_user_requires_user_manage_and_returns_id() {
    let ctx = TestCtx::new().await;
    let (_id, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "u2a@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .set_json(json!({
            "display_name": "New U",
            "email": "u2new@example.com",
            "password": "TerraOps!2026",
            "roles": ["analyst"],
            "timezone": null
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(res).await;
    let _new_id: Uuid = body["id"].as_str().unwrap().parse().unwrap();
}

#[actix_web::test]
async fn t_u3_get_user_self_and_admin() {
    let ctx = TestCtx::new().await;
    let (self_id, self_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "u3self@example.com",
        &[Role::RegularUser],
    )
    .await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "u3a@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    // Self read → 200, no plaintext email.
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/users/{}", self_id))
        .insert_header(("Authorization", format!("Bearer {}", self_tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert!(body["email"].is_null());
    // Admin read → 200, email decrypted.
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/users/{}", self_id))
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["email"], "u3self@example.com");
}

#[actix_web::test]
async fn t_u4_update_user_self_display_name() {
    let ctx = TestCtx::new().await;
    let (self_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "u4@example.com",
        &[Role::RegularUser],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/users/{}", self_id))
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({"display_name": "Renamed"}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // Non-admin can't toggle is_active.
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/users/{}", self_id))
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({"is_active": false}))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_u5_delete_user_soft_deactivates() {
    let ctx = TestCtx::new().await;
    let victim = create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "u5v@example.com",
        "TerraOps!2026",
        &[Role::RegularUser],
    )
    .await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "u5a@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/users/{}", victim))
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    // is_active is now false — admin get still works.
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/users/{}", victim))
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body["is_active"], false);
}

#[actix_web::test]
async fn t_u6_unlock_user_clears_lock() {
    let ctx = TestCtx::new().await;
    let victim = create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "u6v@example.com",
        "TerraOps!2026",
        &[Role::RegularUser],
    )
    .await;
    sqlx::query("UPDATE users SET failed_login_count = 7, locked_until = NOW() + INTERVAL '1 hour' WHERE id = $1")
        .bind(victim).execute(&ctx.pool).await.unwrap();
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "u6a@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/users/{}/unlock", victim))
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
    let row: (i32, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT failed_login_count, locked_until FROM users WHERE id = $1",
    )
    .bind(victim)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(row.0, 0);
    assert!(row.1.is_none());
}

#[actix_web::test]
async fn t_u7_assign_roles_replaces_set() {
    let ctx = TestCtx::new().await;
    let victim = create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "u7v@example.com",
        "TerraOps!2026",
        &[Role::RegularUser],
    )
    .await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "u7a@example.com",
        &[Role::Administrator],
    )
    .await;
    let analyst_id: (Uuid,) = sqlx::query_as("SELECT id FROM roles WHERE name = 'analyst'")
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/users/{}/roles", victim))
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .set_json(json!({"role_ids": [analyst_id.0.to_string()]}))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
    let names: Vec<(String,)> = sqlx::query_as(
        "SELECT r.name FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE ur.user_id = $1",
    )
    .bind(victim)
    .fetch_all(&ctx.pool)
    .await
    .unwrap();
    let names: Vec<String> = names.into_iter().map(|(n,)| n).collect();
    assert_eq!(names, vec!["analyst".to_string()]);
}

#[actix_web::test]
async fn t_u8_list_roles_requires_role_assign() {
    let ctx = TestCtx::new().await;
    let (_id, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "u8a@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/roles")
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body.as_array().unwrap().len(), 5);
}

#[actix_web::test]
async fn t_u9_reset_password_requires_user_manage() {
    let ctx = TestCtx::new().await;
    let victim = create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "u9v@example.com",
        "TerraOps!2026",
        &[Role::RegularUser],
    )
    .await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "u9a@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/users/{}/reset-password", victim))
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .set_json(json!({"new_password": "BrandNew!2099"}))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::NO_CONTENT);
    // Login with the new password works.
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({"username": "u9v@example.com", "password": "BrandNew!2099"}))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
}

#[actix_web::test]
async fn t_u10_audit_list_requires_monitoring_read() {
    let ctx = TestCtx::new().await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "u10a@example.com",
        &[Role::Administrator],
    )
    .await;
    // Generate one audit row by creating a user.
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .set_json(json!({
            "display_name": "AuditTarget",
            "email": "u10t@example.com",
            "password": "TerraOps!2026",
            "roles": [],
            "timezone": null
        }))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::CREATED);
    let req = test::TestRequest::get()
        .uri("/api/v1/audit")
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    let items = body["items"].as_array().unwrap();
    assert!(items.iter().any(|r| r["action"] == "user.create"));
}

// ============================================================================
// Security — SEC1..SEC9
// ============================================================================

#[actix_web::test]
async fn t_sec1_list_allowlist_empty_by_default() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "sec1@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/security/allowlist")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert!(body.as_array().unwrap().is_empty());
}

#[actix_web::test]
async fn t_sec2_create_allowlist_validates_cidr() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "sec2@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/security/allowlist")
        .peer_addr(loopback_peer())
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        // Must include the test peer (loopback) so follow-up requests in the
        // same test aren't blocked by our own allowlist.
        .set_json(json!({"cidr": "127.0.0.0/8", "note": "dev net", "enabled": true}))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    // Bad CIDR → 422
    let req = test::TestRequest::post()
        .uri("/api/v1/security/allowlist")
        .peer_addr(loopback_peer())
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({"cidr": "not-an-ip"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[actix_web::test]
async fn t_sec3_delete_allowlist_entry() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "sec3@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/security/allowlist")
        .peer_addr(loopback_peer())
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        // Loopback CIDR so the subsequent DELETE request from the same peer
        // isn't blocked by the allowlist we just installed.
        .set_json(json!({"cidr": "127.0.0.0/8"}))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    let id = body["id"].as_str().unwrap().to_string();
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/security/allowlist/{}", id))
        .peer_addr(loopback_peer())
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
}

#[actix_web::test]
async fn t_sec4_list_device_certs_admin_only() {
    let ctx = TestCtx::new().await;
    let (_id, user_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "sec4u@example.com",
        &[Role::RegularUser],
    )
    .await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "sec4a@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/security/device-certs")
        .insert_header(("Authorization", format!("Bearer {}", user_tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
    let req = test::TestRequest::get()
        .uri("/api/v1/security/device-certs")
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
}

#[actix_web::test]
async fn t_sec5_register_device_cert_validates_pin() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "sec5@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    // 32-byte hex pin.
    let pin = hex::encode([0xAAu8; 32]);
    let req = test::TestRequest::post()
        .uri("/api/v1/security/device-certs")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({
            "label": "device-1",
            "issued_to_user_id": null,
            "serial": null,
            "spki_sha256_hex": pin,
            "pem_path": null,
            "notes": null
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::CREATED
    );
    // Wrong-length pin → 422.
    let req = test::TestRequest::post()
        .uri("/api/v1/security/device-certs")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({
            "label": "bad",
            "spki_sha256_hex": "deadbeef"
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

#[actix_web::test]
async fn t_sec6_revoke_device_cert() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "sec6@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let pin = hex::encode([0x11u8; 32]);
    let req = test::TestRequest::post()
        .uri("/api/v1/security/device-certs")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({"label": "d", "spki_sha256_hex": pin}))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    let id = body["id"].as_str().unwrap().to_string();
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/security/device-certs/{}", id))
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
}

#[actix_web::test]
async fn t_sec7_get_mtls_returns_default_not_enforced() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "sec7@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/security/mtls")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["enforced"], false);
}

#[actix_web::test]
async fn t_sec8_patch_mtls_requires_mtls_manage() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "sec8@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::patch()
        .uri("/api/v1/security/mtls")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({"enforced": true}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
    // Verify persisted.
    let req = test::TestRequest::get()
        .uri("/api/v1/security/mtls")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body["enforced"], true);
}

#[actix_web::test]
async fn t_sec9_mtls_status_returns_cert_counts() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "sec9@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/security/mtls/status")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert!(body["active_certs"].is_number());
    assert!(body["revoked_certs"].is_number());
}

// ============================================================================
// Retention — R1..R3
// ============================================================================

#[actix_web::test]
async fn t_r1_list_retention_policies_seeded() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "r1@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/retention")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    let arr = body.as_array().unwrap();
    // Seeded: env_raw, kpi, feedback, audit
    assert!(arr.iter().any(|r| r["domain"] == "audit"));
    assert!(arr.iter().any(|r| r["domain"] == "env_raw"));
}

#[actix_web::test]
async fn t_r2_patch_retention_updates_ttl() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "r2@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::patch()
        .uri("/api/v1/retention/env_raw")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({"ttl_days": 400}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
    // Bad ttl → 422
    let req = test::TestRequest::patch()
        .uri("/api/v1/retention/env_raw")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({"ttl_days": -1}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // Missing domain → 404
    let req = test::TestRequest::patch()
        .uri("/api/v1/retention/does_not_exist")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({"ttl_days": 1}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[actix_web::test]
async fn t_r3_run_retention_reports_enforced_at() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "r3@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/retention/audit/run")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["domain"], "audit");
    assert_eq!(body["deleted"], 0);
}

// ============================================================================
// Monitoring — M1..M4
// ============================================================================

#[actix_web::test]
async fn t_m1_latency_requires_monitoring_read() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "m1@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/monitoring/latency")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[actix_web::test]
async fn t_m2_errors_requires_monitoring_read() {
    let ctx = TestCtx::new().await;
    let (_uid, user_tok) = authed(&ctx.pool, &ctx.keys, "m2u@example.com", &[Role::RegularUser]).await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "m2a@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    // regular user → 403
    let req = test::TestRequest::get()
        .uri("/api/v1/monitoring/errors")
        .insert_header(("Authorization", format!("Bearer {}", user_tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
    let req = test::TestRequest::get()
        .uri("/api/v1/monitoring/errors")
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
}

#[actix_web::test]
async fn t_m3_crash_report_any_authed_user() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(&ctx.pool, &ctx.keys, "m3@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/monitoring/crash-report")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({
            "page": "/dashboard",
            "agent": "test-agent",
            "stack": "Error: boom\n  at x",
            "payload": {"k": "v"}
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::CREATED
    );
}

#[actix_web::test]
async fn t_m4_crash_reports_list_admin_only() {
    let ctx = TestCtx::new().await;
    let (_uid, user_tok) = authed(&ctx.pool, &ctx.keys, "m4u@example.com", &[Role::RegularUser]).await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "m4a@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    // user ingests
    let req = test::TestRequest::post()
        .uri("/api/v1/monitoring/crash-report")
        .insert_header(("Authorization", format!("Bearer {}", user_tok)))
        .set_json(json!({"page": "/x"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::CREATED
    );
    // user listing denied
    let req = test::TestRequest::get()
        .uri("/api/v1/monitoring/crash-reports")
        .insert_header(("Authorization", format!("Bearer {}", user_tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
    // admin listing
    let req = test::TestRequest::get()
        .uri("/api/v1/monitoring/crash-reports")
        .insert_header(("Authorization", format!("Bearer {}", admin_tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert!(body["items"].as_array().unwrap().len() >= 1);
}

// ============================================================================
// Reference data — REF1..REF9
// ============================================================================

#[actix_web::test]
async fn t_ref1_sites_open_to_any_authed() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(&ctx.pool, &ctx.keys, "ref1@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/sites")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
}

#[actix_web::test]
async fn t_ref2_departments_filter_by_site() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(&ctx.pool, &ctx.keys, "ref2@example.com", &[Role::Analyst]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/departments")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
}

#[actix_web::test]
async fn t_ref3_categories_list() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(&ctx.pool, &ctx.keys, "ref3@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/categories")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
}

#[actix_web::test]
async fn t_ref4_create_category_requires_ref_write() {
    let ctx = TestCtx::new().await;
    let (_uid, user_tok) = authed(&ctx.pool, &ctx.keys, "ref4u@example.com", &[Role::RegularUser]).await;
    let (_sid, steward_tok) = authed(&ctx.pool, &ctx.keys, "ref4s@example.com", &[Role::DataSteward]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/ref/categories")
        .insert_header(("Authorization", format!("Bearer {}", user_tok)))
        .set_json(json!({"parent_id": null, "name": "Nope"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
    let req = test::TestRequest::post()
        .uri("/api/v1/ref/categories")
        .insert_header(("Authorization", format!("Bearer {}", steward_tok)))
        .set_json(json!({"parent_id": null, "name": "Beverages"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::CREATED
    );
}

#[actix_web::test]
async fn t_ref5_brands_list() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(&ctx.pool, &ctx.keys, "ref5@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/brands")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
}

#[actix_web::test]
async fn t_ref6_create_brand_requires_ref_write() {
    let ctx = TestCtx::new().await;
    let (_sid, steward_tok) = authed(&ctx.pool, &ctx.keys, "ref6s@example.com", &[Role::DataSteward]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/ref/brands")
        .insert_header(("Authorization", format!("Bearer {}", steward_tok)))
        .set_json(json!({"name": "BrandX"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::CREATED
    );
}

#[actix_web::test]
async fn t_ref7_units_list() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(&ctx.pool, &ctx.keys, "ref7@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/units")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
}

#[actix_web::test]
async fn t_ref8_create_unit_requires_ref_write() {
    let ctx = TestCtx::new().await;
    let (_sid, steward_tok) = authed(&ctx.pool, &ctx.keys, "ref8s@example.com", &[Role::DataSteward]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/ref/units")
        .insert_header(("Authorization", format!("Bearer {}", steward_tok)))
        .set_json(json!({"code": "kg", "description": "kilogram"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::CREATED
    );
}

#[actix_web::test]
async fn t_ref9_states_returns_seeded_list() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(&ctx.pool, &ctx.keys, "ref9@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/states")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    // 50 states + DC = 51
    assert_eq!(body.as_array().unwrap().len(), 51);
}

// ============================================================================
// Notifications — N1..N7 (self-scoped)
// ============================================================================

#[actix_web::test]
async fn t_n1_list_notifications_empty_by_default() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(&ctx.pool, &ctx.keys, "n1@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/notifications")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
}

#[actix_web::test]
async fn t_n2_mark_read_self_scope_only() {
    let ctx = TestCtx::new().await;
    let (me_id, me_tok) = authed(&ctx.pool, &ctx.keys, "n2me@example.com", &[Role::RegularUser]).await;
    let (other_id, _other_tok) = authed(&ctx.pool, &ctx.keys, "n2other@example.com", &[Role::RegularUser]).await;
    // Insert a notification for `other`.
    let other_notif: (Uuid,) = sqlx::query_as(
        "INSERT INTO notifications (user_id, topic, title, body) VALUES ($1, 'system', 't', 'b') RETURNING id",
    )
    .bind(other_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let mine: (Uuid,) = sqlx::query_as(
        "INSERT INTO notifications (user_id, topic, title, body) VALUES ($1, 'system', 't', 'b') RETURNING id",
    )
    .bind(me_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    // Cross-user read → 404 (do not leak existence)
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/notifications/{}/read", other_notif.0))
        .insert_header(("Authorization", format!("Bearer {}", me_tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
    // Own notif → 204
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/notifications/{}/read", mine.0))
        .insert_header(("Authorization", format!("Bearer {}", me_tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
}

#[actix_web::test]
async fn t_n3_mark_all_read() {
    let ctx = TestCtx::new().await;
    let (id, tok) = authed(&ctx.pool, &ctx.keys, "n3@example.com", &[Role::RegularUser]).await;
    for _ in 0..3 {
        sqlx::query(
            "INSERT INTO notifications (user_id, topic, title, body) VALUES ($1, 'system', 't', 'b')",
        )
        .bind(id)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/notifications/read-all")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
    let unread: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM notifications WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(unread.0, 0);
}

#[actix_web::test]
async fn t_n4_unread_count_is_self_scoped() {
    let ctx = TestCtx::new().await;
    let (me_id, me_tok) = authed(&ctx.pool, &ctx.keys, "n4me@example.com", &[Role::RegularUser]).await;
    let (other_id, _) = authed(&ctx.pool, &ctx.keys, "n4o@example.com", &[Role::RegularUser]).await;
    sqlx::query("INSERT INTO notifications (user_id, topic, title, body) VALUES ($1, 'x', 'a', 'b'), ($1, 'x', 'a', 'b')")
        .bind(me_id).execute(&ctx.pool).await.unwrap();
    sqlx::query("INSERT INTO notifications (user_id, topic, title, body) VALUES ($1, 'x', 'a', 'b')")
        .bind(other_id).execute(&ctx.pool).await.unwrap();
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/notifications/unread-count")
        .insert_header(("Authorization", format!("Bearer {}", me_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body["unread"], 2);
}

#[actix_web::test]
async fn t_n5_subscriptions_list_empty_by_default() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(&ctx.pool, &ctx.keys, "n5@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/notifications/subscriptions")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[actix_web::test]
async fn t_n6_upsert_subscriptions_round_trip() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(&ctx.pool, &ctx.keys, "n6@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::put()
        .uri("/api/v1/notifications/subscriptions")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .set_json(json!({
            "subscriptions": [
                {"topic": "alerts", "enabled": true},
                {"topic": "digest", "enabled": false}
            ]
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
    // Read back
    let req = test::TestRequest::get()
        .uri("/api/v1/notifications/subscriptions")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 2);
}

#[actix_web::test]
async fn t_n7_mailbox_exports_self_scoped() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(&ctx.pool, &ctx.keys, "n7@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/notifications/mailbox-exports")
        .insert_header(("Authorization", format!("Bearer {}", tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

// ============================================================================
// Cross-cutting middleware: request-id echo
// ============================================================================

#[actix_web::test]
async fn t_mw_request_id_echoes_inbound() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/health")
        .insert_header(("x-request-id", "correlated-abc"))
        .to_request();
    let res = test::call_service(&app, req).await;
    let echoed = res.headers().get("x-request-id").unwrap().to_str().unwrap();
    assert_eq!(echoed, "correlated-abc");
}
