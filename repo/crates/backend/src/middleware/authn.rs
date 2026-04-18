//! Bearer-token authentication + CIDR allowlist enforcement.
//!
//! Combined middleware: if the `Authorization: Bearer <jwt>` header is
//! present, we validate the JWT, confirm the session row is still active,
//! resolve roles + permissions, and attach an `AuthContext` to the
//! request. We also enforce the CIDR allowlist: if any enabled row exists
//! in `endpoint_allowlist`, the client IP must match at least one. An
//! empty allowlist means "no restriction" so a fresh install is usable.
//!
//! Routes that require auth do so via the `AuthUser` extractor. Public
//! routes (login, health, ready) simply never ask for it and so are not
//! affected if the token is missing.

use std::{
    future::{ready, Ready},
    net::IpAddr,
    rc::Rc,
};

use actix_web::{
    body::{BoxBody, EitherBody},
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    web, Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;

use crate::{
    auth::{extractors::AuthContext, sessions as sess},
    crypto::jwt,
    errors::AppError,
    services::users as user_svc,
    state::AppState,
};

pub struct AuthnMw;

impl<S, B> Transform<S, ServiceRequest> for AuthnMw
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthnSvc<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthnSvc {
            inner: Rc::new(service),
        }))
    }
}

pub struct AuthnSvc<S> {
    inner: Rc<S>,
}

fn parse_bearer(header: &str) -> Option<&str> {
    let (scheme, value) = header.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return None;
    }
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

async fn resolve_auth(
    state: &AppState,
    token: &str,
) -> Result<AuthContext, AppError> {
    let claims = jwt::parse(token, &state.keys.jwt)
        .map_err(|_| AppError::AuthInvalidCredentials)?;
    if !sess::is_session_active(&state.pool, claims.sid).await? {
        return Err(AppError::AuthInvalidCredentials);
    }
    let user = user_svc::find_by_id(&state.pool, claims.sub)
        .await?
        .ok_or(AppError::AuthInvalidCredentials)?;
    if !user.is_active {
        return Err(AppError::AuthInvalidCredentials);
    }
    let roles = user_svc::roles_for_user(&state.pool, user.id).await?;
    let permissions = user_svc::permissions_for_user(&state.pool, user.id).await?;
    Ok(AuthContext {
        user_id: user.id,
        session_id: claims.sid,
        roles,
        permissions,
        display_name: user.display_name,
        email_mask: user.email_mask,
        timezone: user.timezone,
    })
}

async fn allowlist_allows(
    state: &AppState,
    ip: Option<IpAddr>,
) -> Result<bool, AppError> {
    use ipnetwork::IpNetwork;
    let rows: Vec<(IpNetwork,)> = sqlx::query_as(
        "SELECT cidr FROM endpoint_allowlist WHERE enabled = TRUE",
    )
    .fetch_all(&state.pool)
    .await?;
    if rows.is_empty() {
        return Ok(true);
    }
    let Some(ip) = ip else {
        return Ok(false);
    };
    Ok(rows.iter().any(|(net,)| net.contains(ip)))
}

impl<S, B> Service<ServiceRequest> for AuthnSvc<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(inner);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.inner.clone();
        Box::pin(async move {
            let state = req
                .app_data::<web::Data<AppState>>()
                .cloned();
            let bearer = req
                .headers()
                .get(actix_web::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(parse_bearer)
                .map(|s| s.to_string());
            let peer_ip = req.connection_info().realip_remote_addr()
                .and_then(|s| {
                    // actix returns either "ip:port" or "ip"
                    let bare = s.rsplit_once(':').map(|(h, _)| h).unwrap_or(s);
                    bare.trim_start_matches('[').trim_end_matches(']').parse::<IpAddr>().ok()
                });

            if let Some(state) = state.as_ref() {
                // Enforce allowlist uniformly. Empty allowlist = permissive.
                let allowed = allowlist_allows(state, peer_ip).await;
                if let Ok(false) = allowed {
                    // Do NOT clone the inner HttpRequest here — it would bump
                    // the underlying `Rc<HttpRequestInner>` strong_count and
                    // cause the router's `Rc::get_mut` to panic on subsequent
                    // requests in the same test process. Return the error
                    // through actix's Error path; the framework will render
                    // the normalized envelope for us.
                    return Err(actix_web::Error::from(AppError::Forbidden(
                        "ip not in allowlist",
                    )));
                }
            }

            if let (Some(state), Some(token)) = (state, bearer) {
                match resolve_auth(&state, &token).await {
                    Ok(ctx) => {
                        req.extensions_mut().insert(ctx);
                    }
                    Err(_) => {
                        // Do not leak reason to unauth'd clients — simply
                        // skip attaching a context; downstream extractors
                        // will 401 for protected routes.
                    }
                }
            }

            let res = svc.call(req).await?;
            Ok(res.map_into_left_body())
        })
    }
}
