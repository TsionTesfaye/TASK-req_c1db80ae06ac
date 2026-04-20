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
    alert::{AckAlertEventResponse, AlertEventDto, AlertRuleDto, CreateAlertRuleRequest,
        UpdateAlertRuleRequest},
    audit::AuditEntry,
    auth::{AuthUserDto, ChangePasswordRequest, LoginRequest, LoginResponse, RefreshResponse},
    env_source::{BulkObservationsRequest, BulkObservationsResponse, CreateEnvSourceRequest,
        EnvSourceDto, ObservationDto, UpdateEnvSourceRequest},
    import::{ImportBatchSummary, ImportCancelResult, ImportCommitResult, ImportRowDto,
        ImportValidateResult},
    kpi::{AnomalyRow, CycleTimeRow, DrillRow, EfficiencyRow, FunnelResponse, KpiSummary},
    metric::{ComputationLineage, CreateMetricDefinitionRequest, MetricDefinitionDto,
        MetricSeriesResponse, UpdateMetricDefinitionRequest},
    monitoring::{CrashReport, ErrorBucket, IngestCrashReport, LatencyBucket},
    notification::{MailboxExportSummary, NotificationItem, NotificationSubscription,
        UpsertSubscriptionsRequest},
    product::{CreateProductRequest, CreateTaxRateRequest, ProductDetail, ProductHistoryEntry,
        ProductListItem, SetOnShelfRequest, UpdateProductRequest, UpdateTaxRateRequest},
    ref_data::{BrandRef, CategoryRef, DepartmentRef, SiteRef, StateRef, UnitRef},
    report::{CreateReportJobRequest, ReportJobDto, ReportRunResponse},
    retention::{RetentionPolicy, RetentionRunResult, UpdateRetentionPolicy},
    security::{AllowlistEntry, CreateAllowlistEntry, DeviceCert, MtlsConfig, UpdateMtlsConfig},
    talent::{AddWatchlistItemRequest, CandidateDetail, CandidateListItem, CreateFeedbackRequest,
        CreateRoleRequest, CreateWatchlistRequest, FeedbackRecord, RecommendationResult,
        RoleOpenItem, TalentWeights, UpdateWeightsRequest, UpsertCandidateRequest,
        WatchlistEntry, WatchlistItem},
    user::{AssignRolesRequest, CreateUserRequest, RoleDto, UpdateUserRequest, UserDetail,
        UserListItem},
};
use terraops_shared::error::{ErrorCode, ErrorEnvelope};
use terraops_shared::pagination::Page;
use uuid::Uuid;

/// Request timeout per design §Budget rules.
pub const REQUEST_TIMEOUT_MS: u32 = 3_000;

/// Single-retry-on-GET policy: the first GET failure (network or 5xx) is
/// retried exactly once. Non-GET verbs are never retried.
pub const GET_RETRIES: u32 = 1;

/// Base URL for the REST API. The SPA and API share a single TLS origin
/// (`:8443`) so a relative prefix is correct in every deployment.
pub const API_BASE: &str = "/api/v1";

/// Minimal application/x-www-form-urlencoded escaper for query params.
/// We encode anything outside the unreserved ASCII set. We avoid pulling
/// in a heavier dep here since the SPA already compiles to wasm and the
/// escaper is only used for a handful of user-supplied query fragments.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        let c = *b;
        let unreserved = (b'A'..=b'Z').contains(&c)
            || (b'a'..=b'z').contains(&c)
            || (b'0'..=b'9').contains(&c)
            || matches!(c, b'-' | b'_' | b'.' | b'~');
        if unreserved {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{:02X}", c));
        }
    }
    out
}

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
                ErrorCode::AuthInvalidCredentials => "Incorrect username or password.".into(),
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

    /// GET variant that additionally parses the `X-Total-Count` response
    /// header (if present). Used by server-paginated list endpoints whose
    /// body is a bare `Vec<T>` rather than a `Page<T>` envelope — the
    /// backend always includes `X-Total-Count` on those handlers, and the
    /// SPA needs that total to render the server pager.
    async fn get_with_total<T: DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<(T, Option<u64>), ApiError> {
        let builder = RequestBuilder::new(&self.endpoint(path)).method(Method::GET);
        let builder = self.attach_auth(builder);
        let req = builder
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let res = self.send_once(req).await?;
        let total = res
            .headers()
            .get("x-total-count")
            .and_then(|s| s.parse::<u64>().ok());
        let body = Self::decode_json::<T>(res).await?;
        Ok((body, total))
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
    //
    // The backend wraps paginated list endpoints (`/users`, `/audit`,
    // `/monitoring/crashes`) in the shared `Page<T>` envelope
    // (`items/page/page_size/total`). This client exposes both the raw
    // page and a thin `*_items` convenience accessor that unwraps the
    // `items` vector for callers that don't render pagination controls.

    pub async fn list_users_page(&self) -> Result<Page<UserListItem>, ApiError> {
        self.get_with_retry::<Page<UserListItem>>("/users").await
    }
    pub async fn list_users(&self) -> Result<Vec<UserListItem>, ApiError> {
        Ok(self.list_users_page().await?.items)
    }
    pub async fn get_user(&self, id: Uuid) -> Result<UserDetail, ApiError> {
        self.get_with_retry::<UserDetail>(&format!("/users/{id}")).await
    }
    /// POST /api/v1/users. Backend (`handlers/users.rs::create_user`)
    /// returns `201 Created` with body `{ "id": <uuid> }`, not a full
    /// `UserDetail`. This method honors the real contract; the UI then
    /// either refreshes the user list or, when needed, calls
    /// `get_user(id)` to render the detail.
    pub async fn create_user(&self, req: &CreateUserRequest) -> Result<Uuid, ApiError> {
        #[derive(serde::Deserialize)]
        struct IdEnvelope {
            id: Uuid,
        }
        let env: IdEnvelope = self.mutate(Method::POST, "/users", Some(req)).await?;
        Ok(env.id)
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
        // Backend route is POST /users/{id}/roles (U7) — see
        // crates/backend/src/handlers/users.rs:48 and http_p1.rs t_u7.
        self.mutate_no_body(Method::POST, &format!("/users/{id}/roles"), Some(req))
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
    pub async fn list_audit_page(&self) -> Result<Page<AuditEntry>, ApiError> {
        self.get_with_retry::<Page<AuditEntry>>("/audit").await
    }
    pub async fn list_audit(&self) -> Result<Vec<AuditEntry>, ApiError> {
        Ok(self.list_audit_page().await?.items)
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
    // Backend path shapes (see crates/backend/src/handlers/monitoring.rs:24-30):
    //   GET  /monitoring/crash-reports  — paginated list (Page<CrashReport>)
    //   POST /monitoring/crash-report   — authenticated client crash ingest
    pub async fn list_crashes_page(&self) -> Result<Page<CrashReport>, ApiError> {
        self.get_with_retry::<Page<CrashReport>>("/monitoring/crash-reports").await
    }
    pub async fn list_crashes(&self) -> Result<Vec<CrashReport>, ApiError> {
        Ok(self.list_crashes_page().await?.items)
    }
    pub async fn ingest_crash(&self, req: &IngestCrashReport) -> Result<(), ApiError> {
        self.mutate_no_body(Method::POST, "/monitoring/crash-report", Some(req)).await
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

    pub async fn list_notifications_page(&self) -> Result<Page<NotificationItem>, ApiError> {
        self.get_with_retry::<Page<NotificationItem>>("/notifications").await
    }
    pub async fn list_notifications(&self) -> Result<Vec<NotificationItem>, ApiError> {
        Ok(self.list_notifications_page().await?.items)
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

    // ---- P-A Catalog: Products (P1–P14) -------------------------------------

    pub async fn list_products_page(&self) -> Result<Page<ProductListItem>, ApiError> {
        self.list_products_page_query("").await
    }
    /// Paginated product listing with raw querystring (e.g. `q=foo&page=2`).
    pub async fn list_products_page_query(
        &self,
        query: &str,
    ) -> Result<Page<ProductListItem>, ApiError> {
        let path = if query.is_empty() {
            "/products".to_string()
        } else {
            format!("/products?{query}")
        };
        self.get_with_retry::<Page<ProductListItem>>(&path).await
    }
    pub async fn list_products(&self) -> Result<Vec<ProductListItem>, ApiError> {
        Ok(self.list_products_page().await?.items)
    }
    pub async fn get_product(&self, id: Uuid) -> Result<ProductDetail, ApiError> {
        self.get_with_retry(&format!("/products/{id}")).await
    }
    /// Backend returns `{ "id": <uuid> }` (HTTP 201). The caller can re-fetch
    /// the full `ProductDetail` via `get_product(id)` after creation.
    pub async fn create_product(
        &self,
        req: &CreateProductRequest,
    ) -> Result<Uuid, ApiError> {
        #[derive(serde::Deserialize)]
        struct Wrap { id: Uuid }
        let w: Wrap = self.mutate(Method::POST, "/products", Some(req)).await?;
        Ok(w.id)
    }
    /// Backend returns 204 No Content on success.
    pub async fn update_product(
        &self,
        id: Uuid,
        req: &UpdateProductRequest,
    ) -> Result<(), ApiError> {
        self.mutate_no_body(Method::PATCH, &format!("/products/{id}"), Some(req)).await
    }
    pub async fn delete_product(&self, id: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::DELETE, &format!("/products/{id}"), None).await
    }
    /// Backend returns 204 No Content on success.
    pub async fn set_product_status(
        &self,
        id: Uuid,
        req: &SetOnShelfRequest,
    ) -> Result<(), ApiError> {
        self.mutate_no_body(Method::POST, &format!("/products/{id}/status"), Some(req)).await
    }
    pub async fn product_history_page(
        &self,
        id: Uuid,
    ) -> Result<Page<ProductHistoryEntry>, ApiError> {
        self.get_with_retry::<Page<ProductHistoryEntry>>(&format!("/products/{id}/history"))
            .await
    }
    pub async fn product_history(
        &self,
        id: Uuid,
    ) -> Result<Vec<ProductHistoryEntry>, ApiError> {
        Ok(self.product_history_page(id).await?.items)
    }
    pub async fn add_tax_rate(&self, id: Uuid, req: &CreateTaxRateRequest) -> Result<serde_json::Value, ApiError> {
        self.mutate(Method::POST, &format!("/products/{id}/tax-rates"), Some(req)).await
    }
    pub async fn update_tax_rate(&self, id: Uuid, rid: Uuid, req: &UpdateTaxRateRequest) -> Result<serde_json::Value, ApiError> {
        self.mutate(Method::PATCH, &format!("/products/{id}/tax-rates/{rid}"), Some(req)).await
    }
    pub async fn delete_tax_rate(&self, id: Uuid, rid: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::DELETE, &format!("/products/{id}/tax-rates/{rid}"), None).await
    }
    pub async fn delete_product_image(&self, id: Uuid, imgid: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::DELETE, &format!("/products/{id}/images/{imgid}"), None).await
    }
    pub async fn export_products(&self, body: &serde_json::Value) -> Result<serde_json::Value, ApiError> {
        self.mutate(Method::POST, "/products/export", Some(body)).await
    }

    // ---- P-A Imports (I1–I7) ------------------------------------------------

    pub async fn list_imports_page(&self) -> Result<Page<ImportBatchSummary>, ApiError> {
        self.get_with_retry::<Page<ImportBatchSummary>>("/imports").await
    }
    pub async fn list_imports(&self) -> Result<Vec<ImportBatchSummary>, ApiError> {
        Ok(self.list_imports_page().await?.items)
    }
    pub async fn get_import(&self, id: Uuid) -> Result<ImportBatchSummary, ApiError> {
        self.get_with_retry(&format!("/imports/{id}")).await
    }
    pub async fn list_import_rows_page(
        &self,
        id: Uuid,
    ) -> Result<Page<ImportRowDto>, ApiError> {
        self.get_with_retry::<Page<ImportRowDto>>(&format!("/imports/{id}/rows")).await
    }
    pub async fn list_import_rows(&self, id: Uuid) -> Result<Vec<ImportRowDto>, ApiError> {
        Ok(self.list_import_rows_page(id).await?.items)
    }
    /// Backend returns `{id, error_count, status}`.
    pub async fn validate_import(&self, id: Uuid) -> Result<ImportValidateResult, ApiError> {
        self.mutate::<(), _>(Method::POST, &format!("/imports/{id}/validate"), None).await
    }
    /// Backend returns `{id, inserted, status}`.
    pub async fn commit_import(&self, id: Uuid) -> Result<ImportCommitResult, ApiError> {
        self.mutate::<(), _>(Method::POST, &format!("/imports/{id}/commit"), None).await
    }
    /// Backend returns `{id, status}`.
    pub async fn cancel_import(&self, id: Uuid) -> Result<ImportCancelResult, ApiError> {
        self.mutate::<(), _>(Method::POST, &format!("/imports/{id}/cancel"), None).await
    }

    // ---- P-B Env sources + observations (E1–E6) -----------------------------

    pub async fn list_env_sources_page(&self) -> Result<Page<EnvSourceDto>, ApiError> {
        self.get_with_retry::<Page<EnvSourceDto>>("/env/sources").await
    }
    pub async fn list_env_sources(&self) -> Result<Vec<EnvSourceDto>, ApiError> {
        Ok(self.list_env_sources_page().await?.items)
    }
    pub async fn create_env_source(&self, req: &CreateEnvSourceRequest) -> Result<EnvSourceDto, ApiError> {
        self.mutate(Method::POST, "/env/sources", Some(req)).await
    }
    pub async fn update_env_source(&self, id: Uuid, req: &UpdateEnvSourceRequest) -> Result<EnvSourceDto, ApiError> {
        self.mutate(Method::PATCH, &format!("/env/sources/{id}"), Some(req)).await
    }
    pub async fn delete_env_source(&self, id: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::DELETE, &format!("/env/sources/{id}"), None).await
    }
    pub async fn bulk_observations(&self, id: Uuid, req: &BulkObservationsRequest) -> Result<BulkObservationsResponse, ApiError> {
        self.mutate(Method::POST, &format!("/env/sources/{id}/observations"), Some(req)).await
    }
    pub async fn list_observations_page(
        &self,
        query: &str,
    ) -> Result<Page<ObservationDto>, ApiError> {
        let path = if query.is_empty() { "/env/observations".to_string() }
                   else { format!("/env/observations?{query}") };
        self.get_with_retry::<Page<ObservationDto>>(&path).await
    }
    pub async fn list_observations(&self, query: &str) -> Result<Vec<ObservationDto>, ApiError> {
        Ok(self.list_observations_page(query).await?.items)
    }

    // ---- P-B Metric definitions + series + lineage (MD1–MD7) ----------------

    pub async fn list_metric_definitions_page(
        &self,
    ) -> Result<Page<MetricDefinitionDto>, ApiError> {
        self.get_with_retry::<Page<MetricDefinitionDto>>("/metrics/definitions").await
    }
    pub async fn list_metric_definitions(&self) -> Result<Vec<MetricDefinitionDto>, ApiError> {
        Ok(self.list_metric_definitions_page().await?.items)
    }
    pub async fn get_metric_definition(&self, id: Uuid) -> Result<MetricDefinitionDto, ApiError> {
        self.get_with_retry(&format!("/metrics/definitions/{id}")).await
    }
    pub async fn create_metric_definition(&self, req: &CreateMetricDefinitionRequest) -> Result<MetricDefinitionDto, ApiError> {
        self.mutate(Method::POST, "/metrics/definitions", Some(req)).await
    }
    pub async fn update_metric_definition(&self, id: Uuid, req: &UpdateMetricDefinitionRequest) -> Result<MetricDefinitionDto, ApiError> {
        self.mutate(Method::PATCH, &format!("/metrics/definitions/{id}"), Some(req)).await
    }
    pub async fn delete_metric_definition(&self, id: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::DELETE, &format!("/metrics/definitions/{id}"), None).await
    }
    pub async fn metric_series(&self, id: Uuid) -> Result<MetricSeriesResponse, ApiError> {
        self.get_with_retry(&format!("/metrics/definitions/{id}/series")).await
    }
    pub async fn metric_lineage(&self, computation_id: Uuid) -> Result<ComputationLineage, ApiError> {
        self.get_with_retry(&format!("/metrics/computations/{computation_id}/lineage")).await
    }

    // ---- P-B KPI (K1–K6) ----------------------------------------------------

    pub async fn kpi_summary(&self) -> Result<KpiSummary, ApiError> {
        self.get_with_retry("/kpi/summary").await
    }
    pub async fn kpi_cycle_time_page(
        &self,
        query: &str,
    ) -> Result<Page<CycleTimeRow>, ApiError> {
        let path = if query.is_empty() { "/kpi/cycle-time".to_string() }
                   else { format!("/kpi/cycle-time?{query}") };
        self.get_with_retry::<Page<CycleTimeRow>>(&path).await
    }
    pub async fn kpi_cycle_time(&self) -> Result<Vec<CycleTimeRow>, ApiError> {
        Ok(self.kpi_cycle_time_page("").await?.items)
    }
    pub async fn kpi_funnel(&self) -> Result<FunnelResponse, ApiError> {
        self.get_with_retry("/kpi/funnel").await
    }
    pub async fn kpi_anomalies_page(
        &self,
        query: &str,
    ) -> Result<Page<AnomalyRow>, ApiError> {
        let path = if query.is_empty() { "/kpi/anomalies".to_string() }
                   else { format!("/kpi/anomalies?{query}") };
        self.get_with_retry::<Page<AnomalyRow>>(&path).await
    }
    pub async fn kpi_anomalies(&self) -> Result<Vec<AnomalyRow>, ApiError> {
        Ok(self.kpi_anomalies_page("").await?.items)
    }
    pub async fn kpi_efficiency_page(
        &self,
        query: &str,
    ) -> Result<Page<EfficiencyRow>, ApiError> {
        let path = if query.is_empty() { "/kpi/efficiency".to_string() }
                   else { format!("/kpi/efficiency?{query}") };
        self.get_with_retry::<Page<EfficiencyRow>>(&path).await
    }
    pub async fn kpi_efficiency(&self) -> Result<Vec<EfficiencyRow>, ApiError> {
        Ok(self.kpi_efficiency_page("").await?.items)
    }
    pub async fn kpi_drill_page(
        &self,
        query: &str,
    ) -> Result<Page<DrillRow>, ApiError> {
        let path = if query.is_empty() { "/kpi/drill".to_string() }
                   else { format!("/kpi/drill?{query}") };
        self.get_with_retry::<Page<DrillRow>>(&path).await
    }
    pub async fn kpi_drill(&self) -> Result<Vec<DrillRow>, ApiError> {
        Ok(self.kpi_drill_page("").await?.items)
    }

    // ---- P-B Alerts (AL1–AL6) -----------------------------------------------

    pub async fn list_alert_rules_page(&self) -> Result<Page<AlertRuleDto>, ApiError> {
        self.get_with_retry::<Page<AlertRuleDto>>("/alerts/rules").await
    }
    pub async fn list_alert_rules(&self) -> Result<Vec<AlertRuleDto>, ApiError> {
        Ok(self.list_alert_rules_page().await?.items)
    }
    pub async fn create_alert_rule(&self, req: &CreateAlertRuleRequest) -> Result<AlertRuleDto, ApiError> {
        self.mutate(Method::POST, "/alerts/rules", Some(req)).await
    }
    pub async fn update_alert_rule(&self, id: Uuid, req: &UpdateAlertRuleRequest) -> Result<AlertRuleDto, ApiError> {
        self.mutate(Method::PATCH, &format!("/alerts/rules/{id}"), Some(req)).await
    }
    pub async fn delete_alert_rule(&self, id: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::DELETE, &format!("/alerts/rules/{id}"), None).await
    }
    pub async fn list_alert_events_page(&self) -> Result<Page<AlertEventDto>, ApiError> {
        self.get_with_retry::<Page<AlertEventDto>>("/alerts/events").await
    }
    pub async fn list_alert_events_page_query(
        &self,
        query: &str,
    ) -> Result<Page<AlertEventDto>, ApiError> {
        let path = if query.is_empty() { "/alerts/events".to_string() }
                   else { format!("/alerts/events?{query}") };
        self.get_with_retry::<Page<AlertEventDto>>(&path).await
    }
    pub async fn list_alert_events(&self) -> Result<Vec<AlertEventDto>, ApiError> {
        Ok(self.list_alert_events_page().await?.items)
    }
    pub async fn ack_alert_event(&self, id: Uuid) -> Result<AckAlertEventResponse, ApiError> {
        self.mutate::<(), _>(Method::POST, &format!("/alerts/events/{id}/ack"), None).await
    }

    // ---- P-B Reports (RP1–RP6) ----------------------------------------------

    pub async fn list_report_jobs_page(&self) -> Result<Page<ReportJobDto>, ApiError> {
        self.get_with_retry::<Page<ReportJobDto>>("/reports/jobs").await
    }
    pub async fn list_report_jobs(&self) -> Result<Vec<ReportJobDto>, ApiError> {
        Ok(self.list_report_jobs_page().await?.items)
    }
    pub async fn get_report_job(&self, id: Uuid) -> Result<ReportJobDto, ApiError> {
        self.get_with_retry(&format!("/reports/jobs/{id}")).await
    }
    pub async fn create_report_job(&self, req: &CreateReportJobRequest) -> Result<ReportJobDto, ApiError> {
        self.mutate(Method::POST, "/reports/jobs", Some(req)).await
    }
    pub async fn run_report_now(&self, id: Uuid) -> Result<ReportRunResponse, ApiError> {
        self.mutate::<(), _>(Method::POST, &format!("/reports/jobs/{id}/run-now"), None).await
    }
    pub async fn cancel_report_job(&self, id: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(Method::POST, &format!("/reports/jobs/{id}/cancel"), None).await
    }
    /// Download the last artifact for a report job as raw bytes. The SPA
    /// can wrap this in a Blob + object URL to trigger a browser download.
    pub async fn download_report_artifact(&self, id: Uuid) -> Result<Vec<u8>, ApiError> {
        let builder = RequestBuilder::new(&self.endpoint(&format!(
            "/reports/jobs/{id}/artifact"
        )))
        .method(Method::GET);
        let builder = self.attach_auth(builder);
        let req = builder
            .build()
            .map_err(|e| ApiError::Network(e.to_string()))?;
        let res = self.send_once(req).await?;
        let status = res.status();
        if status >= 200 && status < 300 {
            res.binary()
                .await
                .map_err(|e| ApiError::Decode(e.to_string()))
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

    // ---- P-C Talent Intelligence (T1–T13) -----------------------------------

    pub async fn list_candidates(&self) -> Result<Vec<CandidateListItem>, ApiError> {
        self.list_candidates_query("").await
    }
    /// List candidates with a raw querystring (e.g.
    /// `q=jane&skills=rust,sql&min_years=3&location=NYC&availability=immediate`).
    /// Supported keys: `q`, `skills` (CSV), `min_years`, `location`, `major`,
    /// `min_education`, `availability`, `page`, `page_size`.
    pub async fn list_candidates_query(
        &self,
        query: &str,
    ) -> Result<Vec<CandidateListItem>, ApiError> {
        let path = if query.is_empty() {
            "/talent/candidates".to_string()
        } else {
            format!("/talent/candidates?{query}")
        };
        self.get_with_retry(&path).await
    }
    /// Server-paginated candidate listing. Returns `(items, total)` where
    /// `total` is the `X-Total-Count` header parsed as u64. Used by the
    /// recruiter SPA to drive the Prev/Next pager (Audit #6 Issue #3).
    pub async fn list_candidates_query_paged(
        &self,
        query: &str,
    ) -> Result<(Vec<CandidateListItem>, Option<u64>), ApiError> {
        let path = if query.is_empty() {
            "/talent/candidates".to_string()
        } else {
            format!("/talent/candidates?{query}")
        };
        self.get_with_total::<Vec<CandidateListItem>>(&path).await
    }
    /// Server-paginated role listing with free-form query string. Mirrors
    /// `list_candidates_query_paged`; backend handler returns a bare
    /// `Vec<RoleOpenItem>` plus `X-Total-Count`.
    pub async fn list_talent_roles_paged(
        &self,
        query: &str,
    ) -> Result<(Vec<RoleOpenItem>, Option<u64>), ApiError> {
        let path = if query.is_empty() {
            "/talent/roles".to_string()
        } else {
            format!("/talent/roles?{query}")
        };
        self.get_with_total::<Vec<RoleOpenItem>>(&path).await
    }
    pub async fn get_candidate(&self, id: Uuid) -> Result<CandidateDetail, ApiError> {
        self.get_with_retry(&format!("/talent/candidates/{id}")).await
    }
    pub async fn create_candidate(&self, req: &UpsertCandidateRequest) -> Result<CandidateDetail, ApiError> {
        self.mutate(Method::POST, "/talent/candidates", Some(req)).await
    }
    pub async fn list_talent_roles(&self) -> Result<Vec<RoleOpenItem>, ApiError> {
        self.get_with_retry("/talent/roles").await
    }
    /// Audit #4 Issue #5: recruiter-side role search/filter. All query
    /// fragments are optional; an all-`None` call is equivalent to
    /// `list_talent_roles`.
    pub async fn search_talent_roles(
        &self,
        q: Option<&str>,
        status: Option<&str>,
        min_years: Option<i32>,
        skills_csv: Option<&str>,
    ) -> Result<Vec<RoleOpenItem>, ApiError> {
        let mut parts: Vec<String> = Vec::new();
        if let Some(v) = q.map(str::trim).filter(|s| !s.is_empty()) {
            parts.push(format!("q={}", urlencode(v)));
        }
        if let Some(v) = status.map(str::trim).filter(|s| !s.is_empty()) {
            parts.push(format!("status={}", urlencode(v)));
        }
        if let Some(y) = min_years {
            parts.push(format!("min_years={y}"));
        }
        if let Some(v) = skills_csv.map(str::trim).filter(|s| !s.is_empty()) {
            parts.push(format!("skills={}", urlencode(v)));
        }
        let path = if parts.is_empty() {
            "/talent/roles".to_string()
        } else {
            format!("/talent/roles?{}", parts.join("&"))
        };
        self.get_with_retry(&path).await
    }
    pub async fn create_talent_role(&self, req: &CreateRoleRequest) -> Result<RoleOpenItem, ApiError> {
        self.mutate(Method::POST, "/talent/roles", Some(req)).await
    }
    pub async fn get_recommendations(&self, role_id: Uuid) -> Result<RecommendationResult, ApiError> {
        self.get_with_retry(&format!("/talent/recommendations?role_id={role_id}")).await
    }
    pub async fn get_talent_weights(&self) -> Result<TalentWeights, ApiError> {
        self.get_with_retry("/talent/weights").await
    }
    pub async fn put_talent_weights(&self, req: &UpdateWeightsRequest) -> Result<TalentWeights, ApiError> {
        self.mutate(Method::PUT, "/talent/weights", Some(req)).await
    }
    pub async fn post_talent_feedback(&self, req: &CreateFeedbackRequest) -> Result<FeedbackRecord, ApiError> {
        self.mutate(Method::POST, "/talent/feedback", Some(req)).await
    }
    pub async fn list_watchlists(&self) -> Result<Vec<WatchlistItem>, ApiError> {
        self.get_with_retry("/talent/watchlists").await
    }
    pub async fn create_watchlist(&self, req: &CreateWatchlistRequest) -> Result<WatchlistItem, ApiError> {
        self.mutate(Method::POST, "/talent/watchlists", Some(req)).await
    }
    pub async fn list_watchlist_items(&self, id: Uuid) -> Result<Vec<WatchlistEntry>, ApiError> {
        self.get_with_retry(&format!("/talent/watchlists/{id}/items")).await
    }
    pub async fn add_watchlist_item(&self, id: Uuid, req: &AddWatchlistItemRequest) -> Result<(), ApiError> {
        self.mutate_no_body(Method::POST, &format!("/talent/watchlists/{id}/items"), Some(req)).await
    }
    pub async fn remove_watchlist_item(&self, id: Uuid, candidate_id: Uuid) -> Result<(), ApiError> {
        self.mutate_no_body::<()>(
            Method::DELETE,
            &format!("/talent/watchlists/{id}/items/{candidate_id}"),
            None,
        ).await
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
