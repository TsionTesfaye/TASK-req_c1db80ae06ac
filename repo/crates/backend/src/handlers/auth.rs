//! Auth endpoints A1–A5.
//!
//! Audit #10 issue #2: `/auth/login` accepts **username + password
//! only**. There is no email fallback — passing an email value as the
//! `username` field is rejected with `AUTH_INVALID_CREDENTIALS` unless
//! the user's DB `username` happens to equal that string exactly.
//!
//!   A1 POST /api/v1/auth/login         — username + password → access_token (+ refresh cookie)
//!   A2 POST /api/v1/auth/refresh       — rotate refresh → new access_token
//!   A3 POST /api/v1/auth/logout        — revoke presented refresh
//!   A4 GET  /api/v1/auth/me            — current user + roles + permissions
//!   A5 POST /api/v1/auth/change-password — self-service password change
//!
//! The refresh token rides in an HttpOnly, Secure, SameSite=Strict cookie
//! so the SPA never has to touch raw refresh material, and an XSS bug on
//! the page cannot read it. The access token is returned in the response
//! body for the SPA to attach as `Authorization: Bearer`.

use actix_web::{
    cookie::{Cookie, SameSite},
    web, HttpRequest, HttpResponse, Responder,
};
use chrono::{Duration, Utc};
use serde_json::json;
use terraops_shared::dto::auth::{
    AuthUserDto, ChangePasswordRequest, LoginRequest, LoginResponse, RefreshResponse,
};

use crate::{
    auth::{
        extractors::AuthUser,
        password,
        sessions::{self, IDLE_TIMEOUT_DAYS},
    },
    crypto::{email as email_crypto, jwt},
    errors::{AppError, AppResult},
    services::audit,
    services::users as user_svc,
    state::AppState,
};

pub const REFRESH_COOKIE: &str = "tops_refresh";

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/login", web::post().to(login))
            .route("/refresh", web::post().to(refresh))
            .route("/logout", web::post().to(logout))
            .route("/me", web::get().to(me))
            .route("/change-password", web::post().to(change_password)),
    );
}

fn build_refresh_cookie<'a>(value: String, max_age: i64) -> Cookie<'a> {
    let mut c = Cookie::build(REFRESH_COOKIE, value)
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/api/v1/auth")
        .finish();
    c.set_max_age(actix_web::cookie::time::Duration::seconds(max_age));
    c
}

fn auth_user_dto_from(ctx: &crate::auth::extractors::AuthContext) -> AuthUserDto {
    AuthUserDto {
        id: ctx.user_id,
        display_name: ctx.display_name.clone(),
        email: None,
        email_mask: ctx.email_mask.clone(),
        roles: ctx.roles.clone(),
        permissions: ctx.permissions.clone(),
        timezone: ctx.timezone.clone(),
    }
}

async fn login(
    state: web::Data<AppState>,
    http: HttpRequest,
    body: web::Json<LoginRequest>,
) -> AppResult<HttpResponse> {
    let req = body.into_inner();
    let user = password::authenticate(
        &state.pool,
        &state.keys.email_hmac,
        &req.username,
        &req.password,
    )
    .await?;

    // Resolve roles+permissions for the access-token response body.
    let roles = user_svc::roles_for_user(&state.pool, user.id).await?;
    let permissions = user_svc::permissions_for_user(&state.pool, user.id).await?;

    let user_agent = http
        .headers()
        .get(actix_web::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let ip_net = http
        .connection_info()
        .realip_remote_addr()
        .and_then(|s| {
            let bare = s.rsplit_once(':').map(|(h, _)| h).unwrap_or(s);
            bare.trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<std::net::IpAddr>()
                .ok()
        })
        .map(ipnetwork::IpNetwork::from);

    let issued = sessions::issue(&state.pool, user.id, user_agent.as_deref(), ip_net).await?;
    let (access_token, exp) = jwt::mint(user.id, issued.session_id, &state.keys.jwt)
        .map_err(|e| AppError::Internal(format!("jwt mint: {e}")))?;

    audit::record(
        &state.pool,
        Some(user.id),
        "auth.login",
        Some("user"),
        Some(&user.id.to_string()),
        json!({"session_id": issued.session_id}),
    )
    .await?;

    let dto = AuthUserDto {
        id: user.id,
        display_name: user.display_name.clone(),
        email: None,
        email_mask: user.email_mask.clone(),
        roles,
        permissions,
        timezone: user.timezone.clone(),
    };

    let response = LoginResponse {
        access_token,
        access_expires_at: exp,
        user: dto,
    };
    let max_age = Duration::days(IDLE_TIMEOUT_DAYS).num_seconds();
    let cookie = build_refresh_cookie(issued.token_plain, max_age);
    Ok(HttpResponse::Ok().cookie(cookie).json(response))
}

async fn refresh(
    state: web::Data<AppState>,
    http: HttpRequest,
) -> AppResult<HttpResponse> {
    let token = http
        .cookie(REFRESH_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or(AppError::AuthInvalidCredentials)?;

    let session = sessions::lookup_active(&state.pool, &token).await?;
    let user_agent = http
        .headers()
        .get(actix_web::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let ip_net = http
        .connection_info()
        .realip_remote_addr()
        .and_then(|s| {
            let bare = s.rsplit_once(':').map(|(h, _)| h).unwrap_or(s);
            bare.trim_start_matches('[').trim_end_matches(']').parse::<std::net::IpAddr>().ok()
        })
        .map(ipnetwork::IpNetwork::from);

    let new_refresh = sessions::rotate(&state.pool, &session, user_agent.as_deref(), ip_net).await?;
    let (access_token, exp) = jwt::mint(session.user_id, new_refresh.session_id, &state.keys.jwt)
        .map_err(|e| AppError::Internal(format!("jwt mint: {e}")))?;

    let max_age = Duration::days(IDLE_TIMEOUT_DAYS).num_seconds();
    let cookie = build_refresh_cookie(new_refresh.token_plain, max_age);
    Ok(HttpResponse::Ok().cookie(cookie).json(RefreshResponse {
        access_token,
        access_expires_at: exp,
    }))
}

async fn logout(
    state: web::Data<AppState>,
    http: HttpRequest,
) -> AppResult<HttpResponse> {
    if let Some(c) = http.cookie(REFRESH_COOKIE) {
        if let Ok(sess) = sessions::lookup_active(&state.pool, c.value()).await {
            sessions::revoke(&state.pool, sess.id).await?;
            audit::record(
                &state.pool,
                Some(sess.user_id),
                "auth.logout",
                Some("session"),
                Some(&sess.id.to_string()),
                json!({}),
            )
            .await?;
        }
    }
    // Clear cookie regardless so the browser stops sending it.
    let mut clear = Cookie::build(REFRESH_COOKIE, "")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/api/v1/auth")
        .finish();
    clear.make_removal();
    Ok(HttpResponse::NoContent().cookie(clear).finish())
}

async fn me(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    // Always resolve fresh from DB so role/permission changes are seen
    // immediately on the next /me call even if the JWT is still valid.
    let row = user_svc::find_by_id(&state.pool, user.0.user_id)
        .await?
        .ok_or(AppError::AuthInvalidCredentials)?;
    let roles = user_svc::roles_for_user(&state.pool, user.0.user_id).await?;
    let permissions = user_svc::permissions_for_user(&state.pool, user.0.user_id).await?;
    let _ = email_crypto::decrypt_email; // kept in use: email plaintext stays admin-only
    let dto = AuthUserDto {
        id: row.id,
        display_name: row.display_name,
        email: None,
        email_mask: row.email_mask,
        roles,
        permissions,
        timezone: row.timezone,
    };
    // Align cached ctx view for sanity (not required for response).
    let _ = auth_user_dto_from;
    Ok(HttpResponse::Ok().json(dto))
}

async fn change_password(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<ChangePasswordRequest>,
) -> AppResult<HttpResponse> {
    let req = body.into_inner();
    // Verify current.
    let row = user_svc::find_by_id(&state.pool, user.0.user_id)
        .await?
        .ok_or(AppError::AuthInvalidCredentials)?;
    if !crate::crypto::argon::verify_password(&req.current_password, &row.password_hash) {
        return Err(AppError::AuthInvalidCredentials);
    }
    password::update_password(&state.pool, user.0.user_id, &req.new_password).await?;
    audit::record(
        &state.pool,
        Some(user.0.user_id),
        "auth.change_password",
        Some("user"),
        Some(&user.0.user_id.to_string()),
        json!({}),
    )
    .await?;
    let _ = Utc::now;
    Ok(HttpResponse::NoContent().finish())
}
