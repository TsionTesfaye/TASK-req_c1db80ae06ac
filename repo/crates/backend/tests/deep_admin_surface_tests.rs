//! Deep HTTP coverage for the U/A/SEC/M/REF admin surfaces.
//!
//! `http_p1.rs` establishes one happy-path test per endpoint id (U1–U10,
//! A1–A5, SEC1–SEC9, M1–M4, REF1–REF9). This suite drives the *branch*
//! surface that those happy paths skip: validation failures, permission
//! denials, edge transitions, multi-row pagination, and admin-only
//! mutation paths. Every test goes through the full middleware stack
//! against a real Postgres via `TestCtx`, mirroring the gate-1 coverage
//! contract for `crates/backend/src/handlers/{users,auth,security,
//! monitoring,ref_data}.rs`.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use std::net::SocketAddr;
use terraops_shared::roles::Role;
use uuid::Uuid;

use common::{authed, build_test_app, create_user_with_roles, username_for, TestCtx};

fn bearer(tok: &str) -> String {
    format!("Bearer {tok}")
}

fn loopback_peer() -> SocketAddr {
    "127.0.0.1:50000".parse().unwrap()
}

// ===========================================================================
// users — validation + permission branches not covered by http_p1.
// ===========================================================================

#[actix_web::test]
async fn deep_users_create_validates_display_name_email_password() {
    let ctx = TestCtx::new().await;
    let (_id, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepuv-admin@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Empty display_name → 422
    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", bearer(&admin_tok)))
        .set_json(json!({
            "display_name": "   ",
            "email": "ok@example.com",
            "password": "TerraOps!2026",
            "roles": [],
            "timezone": null
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Bad email (no @) → 422
    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", bearer(&admin_tok)))
        .set_json(json!({
            "display_name": "Has Name",
            "email": "no-at-sign",
            "password": "TerraOps!2026",
            "roles": [],
            "timezone": null
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Weak password → 422
    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", bearer(&admin_tok)))
        .set_json(json!({
            "display_name": "Has Name",
            "email": "ok2@example.com",
            "password": "short",
            "roles": [],
            "timezone": null
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Multi-role create round-trip exercises ANY($3) role insert + audit.
    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", bearer(&admin_tok)))
        .set_json(json!({
            "display_name": "Multi Role",
            "email": "multi-role@example.com",
            "password": "TerraOps!2026",
            "roles": ["analyst", "data_steward"],
            "timezone": "UTC"
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(res).await;
    let new_id: Uuid = body["id"].as_str().unwrap().parse().unwrap();
    let names: Vec<(String,)> = sqlx::query_as(
        "SELECT r.name FROM user_roles ur JOIN roles r ON r.id = ur.role_id \
         WHERE ur.user_id = $1 ORDER BY r.name",
    )
    .bind(new_id)
    .fetch_all(&ctx.pool)
    .await
    .unwrap();
    let mut names: Vec<String> = names.into_iter().map(|(n,)| n).collect();
    names.sort();
    assert_eq!(names, vec!["analyst".to_string(), "data_steward".to_string()]);
}

#[actix_web::test]
async fn deep_users_get_not_found_and_other_user_forbidden() {
    let ctx = TestCtx::new().await;
    let (_uid, user_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepu-other@example.com",
        &[Role::RegularUser],
    )
    .await;
    let other = create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "deepu-victim@example.com",
        "TerraOps!2026",
        &[Role::RegularUser],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Reading another user without user.manage → 403.
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/users/{other}"))
        .insert_header(("Authorization", bearer(&user_tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );

    // Admin reading random unknown id → 404.
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepu-admin@example.com",
        &[Role::Administrator],
    )
    .await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/users/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[actix_web::test]
async fn deep_users_update_admin_email_and_active_toggle() {
    let ctx = TestCtx::new().await;
    let victim = create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "deepu-upd@example.com",
        "TerraOps!2026",
        &[Role::RegularUser],
    )
    .await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepu-upd-admin@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Admin: change email + display_name + timezone in one call.
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/users/{victim}"))
        .insert_header(("Authorization", bearer(&admin_tok)))
        .set_json(json!({
            "email": "deepu-upd-renamed@example.com",
            "display_name": "Renamed",
            "timezone": "UTC"
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
    let row: (String, String, String) = sqlx::query_as(
        "SELECT display_name, email_mask, timezone FROM users WHERE id = $1",
    )
    .bind(victim)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(row.0, "Renamed");
    // email_mask format: `<first>***@<first>***.<tld>` → domain TLD preserved.
    assert!(row.1.ends_with(".com"), "mask={}", row.1);
    assert!(row.1.starts_with("d***@"), "mask={}", row.1);
    assert_eq!(row.2, "UTC");

    // Admin: toggle is_active=false revokes all sessions for that user.
    let _ = common::issue_session_for(&ctx.pool, &ctx.keys, victim).await;
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/users/{victim}"))
        .insert_header(("Authorization", bearer(&admin_tok)))
        .set_json(json!({"is_active": false}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
    let alive: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM sessions WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(victim)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    assert_eq!(alive.0, 0);
}

#[actix_web::test]
async fn deep_users_delete_self_blocked_and_unknown_404() {
    let ctx = TestCtx::new().await;
    let (admin_id, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepu-del-admin@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Admin deactivating themselves → validation 422.
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/users/{admin_id}"))
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Deleting an unknown id → 404.
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/users/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[actix_web::test]
async fn deep_users_assign_roles_clears_when_empty() {
    let ctx = TestCtx::new().await;
    let victim = create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "deepu-clear@example.com",
        "TerraOps!2026",
        &[Role::Analyst, Role::Recruiter],
    )
    .await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepu-clear-admin@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Empty role_ids → wipe all roles.
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/users/{victim}/roles"))
        .insert_header(("Authorization", bearer(&admin_tok)))
        .set_json(json!({"role_ids": []}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM user_roles WHERE user_id = $1")
        .bind(victim)
        .fetch_one(&ctx.pool)
        .await
        .unwrap();
    assert_eq!(count.0, 0);
}

#[actix_web::test]
async fn deep_users_audit_filters_by_actor_and_action() {
    let ctx = TestCtx::new().await;
    let (admin_id, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepu-audit-admin@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Generate two distinct audit actions.
    let req = test::TestRequest::post()
        .uri("/api/v1/users")
        .insert_header(("Authorization", bearer(&admin_tok)))
        .set_json(json!({
            "display_name": "AuditTgt",
            "email": "deepu-audit-target@example.com",
            "password": "TerraOps!2026",
            "roles": [],
            "timezone": null
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::CREATED
    );

    // Filter by actor.
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/audit?actor={admin_id}&action=user.create&page=1&page_size=10"
        ))
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    let items = body["items"].as_array().unwrap();
    assert!(items.iter().all(|r| r["action"] == "user.create"));
    assert!(items.iter().all(|r| r["actor_id"] == admin_id.to_string()));

    // Empty filter result (action that nobody emitted).
    let req = test::TestRequest::get()
        .uri("/api/v1/audit?action=does.not.exist")
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    let body: Value =
        test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 0);
    assert_eq!(body["total"], 0);
}

// ===========================================================================
// auth — change_password failure path + logout idempotency.
// ===========================================================================

#[actix_web::test]
async fn deep_auth_change_password_rejects_wrong_current() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepa-cp@example.com",
        &[Role::RegularUser],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    let req = test::TestRequest::post()
        .uri("/api/v1/auth/change-password")
        .insert_header(("Authorization", bearer(&tok)))
        .set_json(json!({
            "current_password": "WrongOld!2026",
            "new_password": "NewTerraOps!2099"
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

#[actix_web::test]
async fn deep_auth_logout_idempotent_without_cookie() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // No cookie + no bearer: still 204 (logout is intentionally idempotent).
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/logout")
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
}

#[actix_web::test]
async fn deep_auth_refresh_without_cookie_unauthorized() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNAUTHORIZED
    );
}

// ===========================================================================
// security — non-admin denial + branches not in http_p1.
// ===========================================================================

#[actix_web::test]
async fn deep_security_allowlist_requires_allowlist_manage() {
    let ctx = TestCtx::new().await;
    let (_id, user_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepsec-user@example.com",
        &[Role::RegularUser],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    // GET denied
    let req = test::TestRequest::get()
        .uri("/api/v1/security/allowlist")
        .insert_header(("Authorization", bearer(&user_tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
    // POST denied
    let req = test::TestRequest::post()
        .uri("/api/v1/security/allowlist")
        .insert_header(("Authorization", bearer(&user_tok)))
        .set_json(json!({"cidr": "10.0.0.0/8"}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
    // DELETE denied
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/security/allowlist/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&user_tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::FORBIDDEN
    );
}

#[actix_web::test]
async fn deep_security_allowlist_delete_unknown_404() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepsec-del@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/security/allowlist/{}", Uuid::new_v4()))
        .peer_addr(loopback_peer())
        .insert_header(("Authorization", bearer(&tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[actix_web::test]
async fn deep_security_device_cert_bad_hex_and_short_pin_validations() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepsec-dc@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Non-hex spki → 422.
    let req = test::TestRequest::post()
        .uri("/api/v1/security/device-certs")
        .insert_header(("Authorization", bearer(&tok)))
        .set_json(json!({
            "label": "bad-hex",
            "spki_sha256_hex": "ZZZZZZZZ"
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Hex but only 16 bytes → 422.
    let req = test::TestRequest::post()
        .uri("/api/v1/security/device-certs")
        .insert_header(("Authorization", bearer(&tok)))
        .set_json(json!({
            "label": "short",
            "spki_sha256_hex": "00112233445566778899aabbccddeeff"
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    // Revoke unknown → 404.
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/security/device-certs/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[actix_web::test]
async fn deep_security_mtls_patch_toggles_and_status_reflects_certs() {
    let ctx = TestCtx::new().await;
    let (_id, tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepsec-mtls@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Toggle on
    let req = test::TestRequest::patch()
        .uri("/api/v1/security/mtls")
        .insert_header(("Authorization", bearer(&tok)))
        .set_json(json!({"enforced": true}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
    // Confirm via GET.
    let req = test::TestRequest::get()
        .uri("/api/v1/security/mtls")
        .insert_header(("Authorization", bearer(&tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body["enforced"], true);

    // Register + revoke a cert so status counts include both buckets.
    let pin = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    let req = test::TestRequest::post()
        .uri("/api/v1/security/device-certs")
        .insert_header(("Authorization", bearer(&tok)))
        .set_json(json!({"label": "alpha", "spki_sha256_hex": pin}))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    let cert_id = body["id"].as_str().unwrap().to_string();
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/security/device-certs/{cert_id}"))
        .insert_header(("Authorization", bearer(&tok)))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
    // Status now reports 0 active, 1 revoked.
    let req = test::TestRequest::get()
        .uri("/api/v1/security/mtls/status")
        .insert_header(("Authorization", bearer(&tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body["active_certs"], 0);
    assert_eq!(body["revoked_certs"], 1);
    assert_eq!(body["enforced"], true);

    // Toggle back off so we don't leave global state surprises for siblings.
    let req = test::TestRequest::patch()
        .uri("/api/v1/security/mtls")
        .insert_header(("Authorization", bearer(&tok)))
        .set_json(json!({"enforced": false}))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::NO_CONTENT
    );
}

// ===========================================================================
// monitoring — non-admin denials + crash-report surface with payload.
// ===========================================================================

#[actix_web::test]
async fn deep_monitoring_denies_non_admin_and_ingests_crash() {
    let ctx = TestCtx::new().await;
    let (_uid, user_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepm-user@example.com",
        &[Role::RegularUser],
    )
    .await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepm-admin@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Latency + errors + crash list deny non-admin.
    for path in ["/api/v1/monitoring/latency", "/api/v1/monitoring/errors", "/api/v1/monitoring/crash-reports"] {
        let req = test::TestRequest::get()
            .uri(path)
            .insert_header(("Authorization", bearer(&user_tok)))
            .to_request();
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::FORBIDDEN,
            "{path} should be admin-only"
        );
    }

    // Any auth'd user can ingest a crash with full payload.
    let req = test::TestRequest::post()
        .uri("/api/v1/monitoring/crash-report")
        .insert_header(("Authorization", bearer(&user_tok)))
        .set_json(json!({
            "page": "/dashboard",
            "agent": "Mozilla/5.0 deep-test",
            "stack": "Error: boom\n  at line 1",
            "payload": {"k": "v"}
        }))
        .to_request();
    assert_eq!(
        test::call_service(&app, req).await.status(),
        StatusCode::CREATED
    );

    // Admin sees latency+errors (empty arrays after truncate, but 200).
    for path in ["/api/v1/monitoring/latency", "/api/v1/monitoring/errors"] {
        let req = test::TestRequest::get()
            .uri(path)
            .insert_header(("Authorization", bearer(&admin_tok)))
            .to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::OK);
        let body: Value = test::read_body_json(res).await;
        assert!(body.is_array());
    }

    // Admin reads paginated crash list (now contains the one we ingested).
    let req = test::TestRequest::get()
        .uri("/api/v1/monitoring/crash-reports?page=1&page_size=10")
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert!(body["items"].as_array().unwrap().len() >= 1);
    assert!(body["total"].as_u64().unwrap() >= 1);
}

// ===========================================================================
// ref_data — site-filter on departments + write permission denial.
// ===========================================================================

#[actix_web::test]
async fn deep_ref_departments_filter_and_write_perms() {
    let ctx = TestCtx::new().await;
    let (_uid, user_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepref-user@example.com",
        &[Role::RegularUser],
    )
    .await;
    let (_sid, steward_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepref-steward@example.com",
        &[Role::DataSteward],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Seed two sites + departments.
    let (site_a,): (Uuid,) =
        sqlx::query_as("INSERT INTO sites (code,name) VALUES ('SA','Site A') RETURNING id")
            .fetch_one(&ctx.pool)
            .await
            .unwrap();
    let (site_b,): (Uuid,) =
        sqlx::query_as("INSERT INTO sites (code,name) VALUES ('SB','Site B') RETURNING id")
            .fetch_one(&ctx.pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO departments (site_id, code, name) VALUES ($1,'DA','DeptA')")
        .bind(site_a)
        .execute(&ctx.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO departments (site_id, code, name) VALUES ($1,'DB','DeptB')")
        .bind(site_b)
        .execute(&ctx.pool)
        .await
        .unwrap();

    // departments?site=A → only DA.
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/ref/departments?site={site_a}"))
        .insert_header(("Authorization", bearer(&user_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["code"], "DA");

    // Unfiltered → both.
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/departments")
        .insert_header(("Authorization", bearer(&user_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert_eq!(body.as_array().unwrap().len(), 2);

    // Regular user cannot write categories/brands/units.
    for (path, payload) in [
        ("/api/v1/ref/categories", json!({"name":"X"})),
        ("/api/v1/ref/brands", json!({"name":"Y"})),
        ("/api/v1/ref/units", json!({"code":"u","description":null})),
    ] {
        let req = test::TestRequest::post()
            .uri(path)
            .insert_header(("Authorization", bearer(&user_tok)))
            .set_json(payload)
            .to_request();
        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::FORBIDDEN
        );
    }

    // Data steward CAN write all three (ref.write granted to data_steward).
    for (path, payload) in [
        ("/api/v1/ref/categories", json!({"name":"NewCat"})),
        ("/api/v1/ref/brands", json!({"name":"NewBrand"})),
        ("/api/v1/ref/units", json!({"code":"each","description":"per unit"})),
    ] {
        let req = test::TestRequest::post()
            .uri(path)
            .insert_header(("Authorization", bearer(&steward_tok)))
            .set_json(payload)
            .to_request();
        let res = test::call_service(&app, req).await;
        assert_eq!(res.status(), StatusCode::CREATED, "steward POST {path}");
    }

    // states list is open to any auth'd user.
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/states")
        .insert_header(("Authorization", bearer(&user_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert!(body.as_array().unwrap().len() >= 1);
}

// ===========================================================================
// auth login/refresh with peer_addr — exercises the IP-parse branches in
// handlers/auth.rs (lines 100-105 on login, 161-163 on refresh) and the
// logout-with-valid-cookie audit-record path (~line 194).
// ===========================================================================

#[actix_web::test]
async fn deep_auth_login_refresh_logout_with_peer_addr_parses_ip() {
    let ctx = TestCtx::new().await;
    let uid = create_user_with_roles(
        &ctx.pool,
        &ctx.keys,
        "deepauth-ip@example.com",
        "TerraOps!2026",
        &[Role::RegularUser],
    )
    .await;
    let uname = username_for(&ctx.pool, uid).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Login with peer_addr set -> hits the IP parse branch in login().
    // Audit #10 issue #2: username-only login contract; use the
    // DB-assigned username, not the email.
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .peer_addr(loopback_peer())
        .set_json(json!({
            "username": uname,
            "password": "TerraOps!2026"
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let set_cookie = res
        .headers()
        .get(actix_web::http::header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let raw = set_cookie.split(';').next().unwrap().to_string();

    // Refresh with peer_addr -> hits the IP parse branch in refresh().
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .peer_addr(loopback_peer())
        .insert_header((actix_web::http::header::COOKIE, raw.clone()))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let new_cookie = res
        .headers()
        .get(actix_web::http::header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    // Logout with the *valid* rotated cookie -> hits the inner audit-record
    // branch that only runs when lookup_active succeeds.
    let req = test::TestRequest::post()
        .uri("/api/v1/auth/logout")
        .insert_header((actix_web::http::header::COOKIE, new_cookie))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

// ===========================================================================
// list endpoints with seeded rows — exercises the row-mapping closures in
// handlers/security.rs (list_allowlist, list_device_certs) and
// handlers/ref_data.rs (list_sites, list_categories, list_brands, list_units).
// ===========================================================================

#[actix_web::test]
async fn deep_list_endpoints_with_seeded_rows_map_row_shapes() {
    let ctx = TestCtx::new().await;
    let (_aid, admin_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "deeplist-admin@example.com",
        &[Role::Administrator],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // --- seed reference tables (sites/categories/brands/units) ---
    sqlx::query("INSERT INTO sites (code,name) VALUES ('LS1','List Site 1')")
        .execute(&ctx.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO sites (code,name) VALUES ('LS2','List Site 2')")
        .execute(&ctx.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO categories (parent_id,name) VALUES (NULL,'ListCat1')")
        .execute(&ctx.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO brands (name) VALUES ('ListBrand1')")
        .execute(&ctx.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO units (code,description) VALUES ('lu1','List Unit 1')")
        .execute(&ctx.pool)
        .await
        .unwrap();

    // --- seed allowlist + device_certs rows (so the list closures map >=1 row) ---
    // Seed a loopback-matching CIDR so the allowlist middleware still accepts
    // the in-process test requests (peer_addr=127.0.0.1).
    sqlx::query(
        "INSERT INTO endpoint_allowlist (cidr,note,enabled,created_by) \
         VALUES ('127.0.0.0/8'::inet,'list-note',true,NULL)",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO device_certs (label, issued_to_user_id, serial, spki_sha256, pem_path, notes, created_by) \
         VALUES ('list-dev', NULL, 'SN-LIST', decode('11','hex') || decode(repeat('11',31),'hex'), NULL, 'list-note', NULL)",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();

    // --- GET /ref/sites -> expect >=2 mapped rows ---
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/sites")
        .peer_addr(loopback_peer())
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    let sites = body.as_array().unwrap();
    assert!(sites.len() >= 2);
    assert!(sites.iter().any(|s| s["code"] == "LS1" && s["name"] == "List Site 1"));

    // --- GET /ref/categories ---
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/categories")
        .peer_addr(loopback_peer())
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|c| c["name"] == "ListCat1")
    );

    // --- GET /ref/brands ---
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/brands")
        .peer_addr(loopback_peer())
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|b| b["name"] == "ListBrand1")
    );

    // --- GET /ref/units ---
    let req = test::TestRequest::get()
        .uri("/api/v1/ref/units")
        .peer_addr(loopback_peer())
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|u| u["code"] == "lu1" && u["description"] == "List Unit 1")
    );

    // --- GET /security/allowlist -> maps row into AllowlistEntry ---
    let req = test::TestRequest::get()
        .uri("/api/v1/security/allowlist")
        .peer_addr(loopback_peer())
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    let arr = body.as_array().unwrap();
    assert!(arr.iter().any(|e| {
        e["cidr"].as_str().unwrap_or("").starts_with("127.0.0.0/")
            && e["note"] == "list-note"
            && e["enabled"] == true
    }));

    // --- GET /security/device-certs -> maps row into DeviceCert ---
    let req = test::TestRequest::get()
        .uri("/api/v1/security/device-certs")
        .peer_addr(loopback_peer())
        .insert_header(("Authorization", bearer(&admin_tok)))
        .to_request();
    let body: Value = test::read_body_json(test::call_service(&app, req).await).await;
    let arr = body.as_array().unwrap();
    assert!(arr.iter().any(|d| {
        d["label"] == "list-dev"
            && d["serial"] == "SN-LIST"
            && d["spki_sha256_hex"].as_str().map(|s| s.len()).unwrap_or(0) == 64
    }));
}
