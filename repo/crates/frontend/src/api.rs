//! Typed API client for every P1 backend surface.
//!
//! Design requirements (from `../docs/design.md §Budget rules`):
//!   * every request has a hard 3-second timeout,
//!   * idempotent GETs are retried exactly once on network error or 5xx,
//!   * non-idempotent verbs (POST/PATCH/PUT/DELETE) are never retried,
//!   * every error path returns a unified `ApiError` so pages can render
//!     localized messages without touching `gloo-net` details.
//!
//! The access token is attached as `Authorization: Bearer <jwt>` when set
//! on the client. The refresh cookie is managed by the browser (HttpOnly,
//! Secure, SameSite=Strict) so the SPA never reads it directly.

use std::time::Duration;

use gloo_net::http::{Method, Request, RequestBuilder, Response};
use gloo_timers::future::TimeoutFuture;
use serde::{de::DeserializeOwned, Serialize};
use terraops_shared::dto::{
    audit::AuditEntry,
    auth::{AuthUserDto, ChangePasswordRequest, LoginRequest, LoginResponse, RefreshResponse},
    monitoring::{CrashReport, ErrorBucket, IngestCrashReport, LatencyBucket},
    notification::{MailboxExportSummary, NotificationItem, NotificationSubscription,
        UpsertSubscriptionsRequest},
    ref_data::{BrandRef, CategoryRef, DepartmentRef, SiteRef, StateRef, UnitRef},
    retention::{RetentionPolicy, RetentionRunResult, UpdateRetentionPolicy},
    security::{AllowlistEntry, CreateAllowlistEntry, DeviceCert, MtlsConfig, UpdateMtlsConfig},
    user::{AssignRolesRequest, CreateUserRequest, RoleDto, UpdateUserRequest, UserDetail,
        UserListItem},
};
use terraops_shared::error::{ErrorCode, ErrorEnvelope};
use uuid::Uuid;

/// Request timeout per design §Budget rules.
pub const REQUEST_TIMEOUT_MS: u32 = 3_000;

/// Single-retry-on-GET policy: the first GET failure (network or 5xx) is
/// retried exactly once. Non-GET verbs are never retried.
pub const GET_RETRIES: u32 = 1;

/// Base URL for the REST API. The SPA and API share a single TLS origin
/// (`:8443`) so a relative prefix is correct in every deployment.
pub const API_BASE: &str = "/api/v1";

// ---------------------------------------------------------------------------
// Client + error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ApiError {
    /// Hit the 3-second budget. Maps to `ErrorCode::Timeout`.
    Timeout,
    /// Network failure (DNS, TLS, connection refused, etc.).
    Network(String),
    /// Backend returned a normalized error envelope.
    Api {
        status: u16,
        code: ErrorCode,
        message: String,
        request_id: String,
    },
    /// Non-JSON error body (e.g. bare 502 proxy page).
    Http { status: u16, body: String },
    /// JSON deserialization failure on an otherwise 2xx response.
    Decode(String),
}

impl ApiError {
    pub fn user_facing(&self) -> String {
        match self {
            ApiError::Timeout => "The request took too long. Please try again.".into(),
            ApiError::Network(_) => "Network unavailable. Please try again.".into(),
            ApiError::Api { code, message, .. } => match code {
                ErrorCode::AuthInvalidCredentials => "Incorrect email or password.".into(),
                ErrorCode::AuthLocked => "Account temporarily locked. Try again later.".into(),
                ErrorCode::AuthForbidden => "You don't have permission to do that.".into(),
                ErrorCode::AuthRequired => "Please sign in to continue.".into(),
                ErrorCode::ValidationFailed => message.clone(),
                ErrorCode::NotFound => "That item could not be found.".into(),
                ErrorCode::Conflict => message.clone(),
                ErrorCode::RateLimited => "Too many attempts. Please slow down.".into(),
                ErrorCode::Timeout => "The request took too long. Please try again.".into(),
                ErrorCode::Internal => "Something went wrong on our side.".into(),
            },
            ApiError::Http { status, .. } => format!("Unexpected HTTP {status}."),
            ApiError::Decode(_) => "We couldn't read the server response.".into(),
        }
    }

    pub fn is_unauthenticated(&self) -> bool {
        matches!(
            self,
            ApiError::Api {
                code: ErrorCode::AuthRequired | ErrorCode::AuthInvalidCredentials,
                ..
            }
        )
    }
}

/// Typed API client. Cheap to clone (holds only a short token string).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ApiClient {
    access_token: Option<String>,
}

impl ApiClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_token(token: Option<String>) -> Self {
        Self {
            access_token: token,
        }
    }

    pub fn token(&self) -> Option<&str> {
        self.access_token.as_deref()
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", API_BASE, path)
    }

    fn attach_auth(&self, mut builder: RequestBuilder) -> RequestBuilder {
        if let Some(tok) = &self.access_token {
            builder = builder.header("Authorization", &format!("Bearer {tok}"));
        }
        builder = builder.header("Accept", "application/json");
        builder
    }

    // ---- core request helpers ----------------------------------------------

    async fn send_once(&self, req: Request) -> Result<Response, ApiError> {
        let fut = req.send();
        match select_timeout(fut, Duration::from_millis(REQUEST_TIMEOUT_MS as u64)).await {
            Some(Ok(res)) => Ok(res),
            Some(Err(e)) => Err(ApiError::Network(e.to_string())),
            None => Err(ApiError::Timeout),
        }
    }

    async fn decode_json<T: DeserializeOwned>(res: Response) -> Result<T, ApiError> {
        let status = res.status();
        if status >= 200 && status < 300 {
            res.json::<T>()
                .await
                .map_err(|e| ApiError::Decode(e.to_string()))
        } else {
            // Try to read a normalized envelope first; fall back to raw text.
            let text = res
                .text()
                .await
                .map_err(|e| ApiError::Decode(e.to_string()))?;
            if let Ok(env) = serde_json::from_str::<ErrorEnvelope>(&text) {
                Err(ApiError::Api {
                    status,
                    code: env.error_code,
                    message: env.message,
                    request_id: env.request_id,
                })
            } else {
                Err(ApiError::Http { status, body: text })
            }
        }
    }

    async fn get_with_retry<T: DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let mut last_err = None;
        for attempt in 0..=GET_RETRIES {
            let builder = RequestBuilder::new(&self.endpoint(path)).method(Method::GET);
            let builder = self.attach_auth(builder);
            let req = builder
                .build()
                .map_err(|e| ApiError::Network(e.to_string()))?;
            match self.send_once(req).await {
                Ok(res) => {
                    let status = res.status();
                    if status >= 500 && attempt < GET_RETRIES {
                        continue;
                    }
                    return Self::decode_json::<T>(res).await;
                }
                Err(ApiError::Timeout) => {
                    // Timeout counts as a retryable network failure on GETs.
                    last_err = Some(ApiError::Timeout);
                    continue;
                }
                Err(e @ ApiError::Network(_)) => {
                    last_err = Some(e);
                    continue;
                }
                Err(other) => return Err(other),
            }
        }
        Err(last_err.unwrap_or(ApiError::Timeout))
    }

    async fn mutate<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<T, ApiError> {
        let mut builder = RequestBuilder::new(&self.endpoint(path)).method(method);
        builder = self.attach_auth(builder);
        let req = if let Some(b) = body {
            builder
                .header("Content-Type", "application/json")
                .json(b)
                .map_err(|e| ApiError::Network(e.to_string()))?
        } else {
            builder
                .build()
                .map_err(|e| ApiError::Network(e.to_string()))?
        };
        let res = self.send_once(req).await?;
        Self::decode_json::<T>(res).await
    }

    async fn mutate_no_body<B: Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<(), ApiError> {
        let mut builder = RequestBuilder::new(&self.endpoint(path)).method(method);
        builder = self.attach_auth(builder);
        let req = if let Some(b) = body {
            builder
                .header("Content-Type", "application/json")
                .json(b)
                .map_err(|e| ApiError::Network(e.to_string()))?
        } else {
            builder
                .build()
                .map_err(|e| ApiError::Network(e.to_string()))?
        };
        let res = self.send_once(req).await?;
        let status = res.status();
        if status >= 200 && status < 300 {
            Ok(())
        } else {
            let text = res
                .text()
                .await
                .map_err(|e| ApiError::Decode(e.to_string()))?;
            if let Ok(env) = serde_json::from_str::<ErrorEnvelope>(&text) {
                Err(ApiError::Api {
                    status,
                    code: env.error_code,
                    message: env.message,
                    request_id: env.request_id,
                })
            } else {
                Err(ApiError::Http { status, body: text })
            }
        }
    }

    // ---- Auth (A1–A5) ------------------------------------------------------

    pub async fn login(&self, req: &LoginRequest) -> Result<LoginResponse, ApiError> {
        self.mutate(Method::POST, "/auth/login", Some(req)).await
    }

    pub async fn refresh(&self) -> Result<RefreshResponse, ApiError> {
        self.mutate::<(), _>(Method::POST, "/auth/refresh", None).await
    }

    pub async fn logout(&self) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::POST, "/auth/logout", None).await
    }

    pub async fn me(&self) -> Result<AuthUserDto, ApiError> {
        self.get_with_retry::<AuthUserDto>("/auth/me").await
    }

    pub async fn change_password(&self, req: &ChangePasswordRequest) -> Result<(), ApiError> {
        self.mutate_no_body(Method::POST, "/auth/change-password", Some(req))
            .await
    }

    // ---- Users (U1–U10) ----------------------------------------------------

    pub async fn list_users(&self) -> Result<Vec<UserListItem>, ApiError> {
        self.get_with_retry::<Vec<UserListItem>>("/users").await
    }
    pub async fn get_user(&self, id: Uuid) -> Result<UserDetail, ApiError> {
        self.get_with_retry::<UserDetail>(&format!("/users/{id}")).await
    }
    pub async fn create_user(&self, req: &CreateUserRequest) -> Result<UserDetail, ApiError> {
        self.mutate(Method::POST, "/users", Some(req)).await
    }
    pub async fn update_user(&self, id: Uuid, req: &UpdateUserRequest) -> Result<(), ApiError> {
        self.mutate_no_body(Method::PATCH, &format!("/users/{id}"), Some(req))
            .await
    }
    pub async fn delete_user(&self, id: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::DELETE, &format!("/users/{id}"), None).await
    }
    pub async fn unlock_user(&self, id: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::POST, &format!("/users/{id}/unlock"), None).await
    }
    pub async fn assign_roles(&self, id: Uuid, req: &AssignRolesRequest) -> Result<(), ApiError> {
        self.mutate_no_body(Method::PUT, &format!("/users/{id}/roles"), Some(req))
            .await
    }
    pub async fn reset_password(&self, id: Uuid, new_password: &str) -> Result<(), ApiError> {
        let body = serde_json::json!({"new_password": new_password});
        self.mutate_no_body(Method::POST, &format!("/users/{id}/reset-password"), Some(&body))
            .await
    }
    pub async fn list_roles(&self) -> Result<Vec<RoleDto>, ApiError> {
        self.get_with_retry::<Vec<RoleDto>>("/roles").await
    }
    pub async fn list_audit(&self) -> Result<Vec<AuditEntry>, ApiError> {
        self.get_with_retry::<Vec<AuditEntry>>("/audit").await
    }

    // ---- Security (SEC1–SEC9) ----------------------------------------------

    pub async fn list_allowlist(&self) -> Result<Vec<AllowlistEntry>, ApiError> {
        self.get_with_retry::<Vec<AllowlistEntry>>("/security/allowlist").await
    }
    pub async fn create_allowlist(
        &self,
        req: &CreateAllowlistEntry,
    ) -> Result<serde_json::Value, ApiError> {
        self.mutate(Method::POST, "/security/allowlist", Some(req)).await
    }
    pub async fn delete_allowlist(&self, id: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::DELETE, &format!("/security/allowlist/{id}"), None)
            .await
    }
    pub async fn list_device_certs(&self) -> Result<Vec<DeviceCert>, ApiError> {
        self.get_with_retry::<Vec<DeviceCert>>("/security/device-certs").await
    }
    pub async fn revoke_device_cert(&self, id: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::DELETE, &format!("/security/device-certs/{id}"), None)
            .await
    }
    pub async fn get_mtls(&self) -> Result<MtlsConfig, ApiError> {
        self.get_with_retry::<MtlsConfig>("/security/mtls").await
    }
    pub async fn patch_mtls(&self, req: &UpdateMtlsConfig) -> Result<(), ApiError> {
        self.mutate_no_body(Method::PATCH, "/security/mtls", Some(req)).await
    }
    pub async fn mtls_status(&self) -> Result<serde_json::Value, ApiError> {
        self.get_with_retry::<serde_json::Value>("/security/mtls/status").await
    }

    // ---- Retention (R1–R3) -------------------------------------------------

    pub async fn list_retention(&self) -> Result<Vec<RetentionPolicy>, ApiError> {
        self.get_with_retry::<Vec<RetentionPolicy>>("/retention").await
    }
    pub async fn patch_retention(
        &self,
        domain: &str,
        req: &UpdateRetentionPolicy,
    ) -> Result<(), ApiError> {
        self.mutate_no_body(Method::PATCH, &format!("/retention/{domain}"), Some(req))
            .await
    }
    pub async fn run_retention(&self, domain: &str) -> Result<RetentionRunResult, ApiError> {
        self.mutate::<(), _>(Method::POST, &format!("/retention/{domain}/run"), None)
            .await
    }

    // ---- Monitoring (M1–M4) ------------------------------------------------

    pub async fn list_latency(&self) -> Result<Vec<LatencyBucket>, ApiError> {
        self.get_with_retry::<Vec<LatencyBucket>>("/monitoring/latency").await
    }
    pub async fn list_errors(&self) -> Result<Vec<ErrorBucket>, ApiError> {
        self.get_with_retry::<Vec<ErrorBucket>>("/monitoring/errors").await
    }
    pub async fn list_crashes(&self) -> Result<Vec<CrashReport>, ApiError> {
        self.get_with_retry::<Vec<CrashReport>>("/monitoring/crashes").await
    }
    pub async fn ingest_crash(&self, req: &IngestCrashReport) -> Result<(), ApiError> {
        self.mutate_no_body(Method::POST, "/monitoring/crashes", Some(req)).await
    }

    // ---- Reference data (REF1–REF9) ---------------------------------------

    pub async fn list_sites(&self) -> Result<Vec<SiteRef>, ApiError> {
        self.get_with_retry::<Vec<SiteRef>>("/ref/sites").await
    }
    pub async fn list_departments(&self) -> Result<Vec<DepartmentRef>, ApiError> {
        self.get_with_retry::<Vec<DepartmentRef>>("/ref/departments").await
    }
    pub async fn list_categories(&self) -> Result<Vec<CategoryRef>, ApiError> {
        self.get_with_retry::<Vec<CategoryRef>>("/ref/categories").await
    }
    pub async fn list_brands(&self) -> Result<Vec<BrandRef>, ApiError> {
        self.get_with_retry::<Vec<BrandRef>>("/ref/brands").await
    }
    pub async fn list_units(&self) -> Result<Vec<UnitRef>, ApiError> {
        self.get_with_retry::<Vec<UnitRef>>("/ref/units").await
    }
    pub async fn list_states(&self) -> Result<Vec<StateRef>, ApiError> {
        self.get_with_retry::<Vec<StateRef>>("/ref/states").await
    }

    // ---- Notifications (N1–N7) --------------------------------------------

    pub async fn list_notifications(&self) -> Result<Vec<NotificationItem>, ApiError> {
        self.get_with_retry::<Vec<NotificationItem>>("/notifications").await
    }
    pub async fn mark_notification_read(&self, id: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::POST, &format!("/notifications/{id}/read"), None)
            .await
    }
    pub async fn mark_all_notifications_read(&self) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::POST, "/notifications/read-all", None).await
    }
    pub async fn unread_count(&self) -> Result<i64, ApiError> {
        #[derive(serde::Deserialize)]
        struct Wrapper {
            unread: i64,
        }
        let w: Wrapper = self.get_with_retry("/notifications/unread-count").await?;
        Ok(w.unread)
    }
    pub async fn list_subscriptions(&self) -> Result<Vec<NotificationSubscription>, ApiError> {
        self.get_with_retry("/notifications/subscriptions").await
    }
    pub async fn upsert_subscriptions(
        &self,
        req: &UpsertSubscriptionsRequest,
    ) -> Result<(), ApiError> {
        self.mutate_no_body(Method::PUT, "/notifications/subscriptions", Some(req))
            .await
    }
    pub async fn list_mailbox_exports(&self) -> Result<Vec<MailboxExportSummary>, ApiError> {
        self.get_with_retry("/notifications/mailbox-exports").await
    }
}

// ---------------------------------------------------------------------------
// Timeout helper (wasm-friendly)
// ---------------------------------------------------------------------------

/// Race a future against a timeout. Returns `Some(output)` if the future
/// completes first, or `None` if the timeout elapses first. Wasm-friendly —
/// uses `gloo-timers`, not `tokio`.
pub async fn select_timeout<F: std::future::Future>(
    fut: F,
    timeout: Duration,
) -> Option<F::Output> {
    use futures::future::{select, Either};
    use futures::FutureExt;
    let timer = TimeoutFuture::new(timeout.as_millis() as u32);
    match select(Box::pin(fut.fuse()), Box::pin(timer.fuse())).await {
        Either::Left((out, _)) => Some(out),
        Either::Right((_, _)) => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    // Tests are browser-agnostic primitives (timeout race, error mapping,
    // token attachment) and do not hit the DOM or `fetch`. With no
    // `wasm_bindgen_test_configure!(run_in_browser)` directive,
    // wasm-bindgen-test-runner executes them under Node.js by default,
    // which keeps Gate 2 toolchain-light (Node only; no pinned Chromium
    // needed).

    #[wasm_bindgen_test]
    async fn timeout_fires_for_slow_future() {
        let fut = async {
            TimeoutFuture::new(5_000).await;
            42
        };
        let out = select_timeout(fut, Duration::from_millis(20)).await;
        assert!(out.is_none(), "expected timeout, got {:?}", out);
    }

    #[wasm_bindgen_test]
    async fn timeout_does_not_fire_for_fast_future() {
        let fut = async { 7 };
        let out = select_timeout(fut, Duration::from_millis(500)).await;
        assert_eq!(out, Some(7));
    }

    #[wasm_bindgen_test]
    fn api_error_user_facing_maps_codes() {
        let e = ApiError::Api {
            status: 401,
            code: ErrorCode::AuthInvalidCredentials,
            message: "bad".into(),
            request_id: "r".into(),
        };
        assert!(e.user_facing().to_lowercase().contains("incorrect"));
        assert!(ApiError::Timeout.user_facing().to_lowercase().contains("too long"));
        let nw = ApiError::Network("x".into());
        assert!(nw.user_facing().to_lowercase().contains("network"));
    }

    #[wasm_bindgen_test]
    fn api_error_unauthenticated_detects_both_codes() {
        let a = ApiError::Api {
            status: 401,
            code: ErrorCode::AuthRequired,
            message: "".into(),
            request_id: "r".into(),
        };
        assert!(a.is_unauthenticated());
        let b = ApiError::Api {
            status: 401,
            code: ErrorCode::AuthInvalidCredentials,
            message: "".into(),
            request_id: "r".into(),
        };
        assert!(b.is_unauthenticated());
        assert!(!ApiError::Timeout.is_unauthenticated());
    }

    #[wasm_bindgen_test]
    fn client_attaches_token() {
        let c = ApiClient::with_token(Some("abc".into()));
        assert_eq!(c.token(), Some("abc"));
        let c2 = ApiClient::new();
        assert_eq!(c2.token(), None);
    }
}
