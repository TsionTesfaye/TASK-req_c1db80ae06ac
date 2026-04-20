//! Shared test harness used by every HTTP integration test.
//!
//! Runs everything against a REAL Postgres (from `DATABASE_URL`) with REAL
//! middleware, REAL routing, REAL crypto — no service mocks. Before each
//! test we truncate the dynamic tables (leaving the seeded roles +
//! permissions intact) so tests are order-independent.
//!
//! Authenticated clients are produced by `issue_session_for(...)`, which
//! inserts a live `sessions` row and mints a matching JWT so the bearer
//! token passes the real `authn` middleware end to end.

#![allow(dead_code)]

use std::{path::PathBuf, sync::Arc};

use actix_web::{
    body::{BoxBody, EitherBody},
    web, App,
};
use once_cell::sync::{Lazy, OnceCell};
use tokio::sync::{Mutex, MutexGuard};
use sqlx::postgres::PgPool;
use terraops_backend::{
    auth::sessions,
    crypto::{
        argon,
        email::{email_hash, email_mask, encrypt_email, normalize_email},
        jwt,
        keys::RuntimeKeys,
    },
    db,
    handlers,
    middleware::{
        authn::AuthnMw, budget::BudgetMw, csrf::CsrfMw, metrics::MetricsMw,
        request_id::RequestIdMw,
    },
    state::AppState,
};
use terraops_shared::roles::Role;
use uuid::Uuid;

static MIGRATE_ONCE: OnceCell<()> = OnceCell::new();

async fn ensure_migrated(pool: &PgPool) {
    if MIGRATE_ONCE.get().is_some() {
        return;
    }
    db::run_migrations(pool)
        .await
        .expect("run_migrations for test DB");
    let _ = MIGRATE_ONCE.set(());
}

/// Every integration test acquires this lock on startup so they all share
/// a single serialized session against the one Postgres database. Tests
/// truncate on entry and assume no concurrent writer.
static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

async fn acquire_lock() -> MutexGuard<'static, ()> {
    TEST_LOCK.lock().await
}

pub struct TestCtx {
    pub pool: PgPool,
    pub keys: Arc<RuntimeKeys>,
    pub state: AppState,
    _guard: MutexGuard<'static, ()>,
}

impl TestCtx {
    pub async fn new() -> Self {
        let guard = acquire_lock().await;
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for HTTP tests");
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(8)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&database_url)
            .await
            .expect("connect test db");
        ensure_migrated(&pool).await;
        truncate_dynamic_tables(&pool).await;
        let keys = Arc::new(RuntimeKeys::for_testing());
        let runtime_dir = PathBuf::from(format!(
            "/tmp/terraops-test-runtime-{}",
            uuid::Uuid::new_v4()
        ));
        let _ = std::fs::create_dir_all(&runtime_dir);
        let state = AppState {
            pool: pool.clone(),
            keys: keys.clone(),
            static_dir: PathBuf::from("/tmp/terraops-test-dist"),
            default_timezone: "America/New_York".into(),
            runtime_dir,
            // Tests assume the persisted-desired flag and the startup-
            // active flag start in sync (both off); cases that PATCH
            // the DB then assert `pending_restart` manually against the
            // fresh DB row versus this captured startup value.
            mtls_startup_enforced: false,
        };
        Self {
            pool,
            keys,
            state,
            _guard: guard,
        }
    }
}

pub async fn truncate_dynamic_tables(pool: &PgPool) {
    // TRUNCATE ... CASCADE across the tables that tests might mutate.
    // Careful: keep roles/permissions/role_permissions and retention_policies
    // + mtls_config + state_codes (seeded canonical rows) intact.
    sqlx::query(
        "TRUNCATE TABLE \
            notification_delivery_attempts, notifications, notification_subscriptions, \
            mailbox_exports, \
            audit_log, \
            sessions, user_roles, users, \
            endpoint_allowlist, device_certs, client_crash_reports, api_metrics, \
            talent_watchlist_items, talent_watchlists, talent_weights, \
            talent_feedback, roles_open, candidates, \
            kpi_rollup_daily, alert_events, alert_rules, report_jobs, \
            metric_computations, metric_definitions, \
            env_observations, env_sources, \
            categories, brands, units, departments, sites \
         RESTART IDENTITY CASCADE",
    )
    .execute(pool)
    .await
    .expect("truncate dynamic tables");
    // NOTE: the TRUNCATE above uses CASCADE, which will cascade-truncate every
    // table with an FK back to `users` (mtls_config.updated_by,
    // retention_policies.updated_by, etc.) even though those FKs are
    // ON DELETE SET NULL — TRUNCATE CASCADE ignores the action and still
    // wipes the referring table. So we must explicitly re-seed the
    // canonical singletons here; otherwise SEC7–9 and R1–R3 see empty tables
    // and the handlers 500 on fetch_one(RowNotFound).
    sqlx::query(
        "INSERT INTO mtls_config (id, enforced, updated_by, updated_at) \
         VALUES (1, FALSE, NULL, NOW()) \
         ON CONFLICT (id) DO UPDATE SET enforced = EXCLUDED.enforced, \
           updated_by = NULL, updated_at = NOW()",
    )
    .execute(pool)
    .await
    .expect("reseed mtls_config");
    sqlx::query(
        "INSERT INTO retention_policies (domain, ttl_days) VALUES \
            ('env_raw',  548), ('kpi', 1825), ('feedback', 730), ('audit', 0) \
         ON CONFLICT (domain) DO UPDATE SET \
            ttl_days = EXCLUDED.ttl_days, last_enforced_at = NULL, \
            updated_by = NULL, updated_at = NOW()",
    )
    .execute(pool)
    .await
    .expect("reseed retention_policies");
}

/// Build the full Actix application exactly like `app::run` does — middleware
/// stack + routers — but ready for `actix_web::test::init_service`.
///
/// The stack mirrors production (`RequestId → Authn → Csrf → Budget →
/// Metrics`) plus one test-only shim `CsrfHeaderInjectorMw` inserted
/// OUTSIDE `CsrfMw`. The shim transparently adds `X-Requested-With:
/// terraops` to every inbound test request so existing handler/RBAC tests
/// don't need to plumb the header manually — they exercise the CSRF-
/// gated app exactly like the SPA, which always sends the header.
///
/// The CSRF control itself is verified directly in `csrf_tests.rs`,
/// which builds the app via `build_test_app_strict()` (no injector) and
/// asserts 403 on mutations without the header and pass-through on GETs.
pub fn build_test_app(
    state: AppState,
) -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse<
            EitherBody<EitherBody<EitherBody<EitherBody<BoxBody>>>>,
        >,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    App::new()
        .app_data(web::Data::new(state))
        .wrap(MetricsMw)
        .wrap(BudgetMw)
        .wrap(CsrfMw)
        .wrap(csrf_injector::CsrfHeaderInjectorMw)
        .wrap(AuthnMw)
        .wrap(RequestIdMw)
        .service(web::scope("/api/v1").configure(handlers::configure))
}

/// Strict variant that mirrors production exactly — no `X-Requested-With`
/// auto-injector. Used by `csrf_tests.rs` to prove the CSRF contract.
pub fn build_test_app_strict(
    state: AppState,
) -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse<
            EitherBody<EitherBody<EitherBody<EitherBody<BoxBody>>>>,
        >,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    App::new()
        .app_data(web::Data::new(state))
        .wrap(MetricsMw)
        .wrap(BudgetMw)
        .wrap(CsrfMw)
        .wrap(AuthnMw)
        .wrap(RequestIdMw)
        .service(web::scope("/api/v1").configure(handlers::configure))
}

/// Test-only shim: injects the `X-Requested-With: terraops` header on
/// every inbound request so pre-existing handler tests don't need to
/// plumb the CSRF header through every `TestRequest::{post,patch,put,
/// delete}` call site. Runs outside `CsrfMw` in `build_test_app` so
/// `CsrfMw` sees the header and accepts the request — this keeps the
/// CSRF middleware on the request path without forcing a mass rewrite
/// of existing test files. The strict contract is asserted separately
/// in `csrf_tests.rs` against `build_test_app_strict`.
mod csrf_injector {
    use std::{
        future::{ready, Ready},
        rc::Rc,
    };

    use actix_web::{
        dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
        http::header::{HeaderName, HeaderValue},
        Error,
    };
    use futures_util::future::LocalBoxFuture;

    pub struct CsrfHeaderInjectorMw;

    impl<S, B> Transform<S, ServiceRequest> for CsrfHeaderInjectorMw
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
        B: 'static,
    {
        type Response = ServiceResponse<B>;
        type Error = Error;
        type InitError = ();
        type Transform = CsrfHeaderInjectorSvc<S>;
        type Future = Ready<Result<Self::Transform, Self::InitError>>;

        fn new_transform(&self, service: S) -> Self::Future {
            ready(Ok(CsrfHeaderInjectorSvc {
                inner: Rc::new(service),
            }))
        }
    }

    pub struct CsrfHeaderInjectorSvc<S> {
        inner: Rc<S>,
    }

    impl<S, B> Service<ServiceRequest> for CsrfHeaderInjectorSvc<S>
    where
        S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
        B: 'static,
    {
        type Response = ServiceResponse<B>;
        type Error = Error;
        type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

        forward_ready!(inner);

        fn call(&self, mut req: ServiceRequest) -> Self::Future {
            let svc = self.inner.clone();
            Box::pin(async move {
                // Only inject if the test request didn't already set it —
                // so csrf_tests.rs can still drive strict cases through
                // `build_test_app_strict` (which has no injector) without
                // this shim interfering.
                let already = req
                    .headers()
                    .get("x-requested-with")
                    .is_some();
                if !already {
                    let headers = req.headers_mut();
                    headers.insert(
                        HeaderName::from_static("x-requested-with"),
                        HeaderValue::from_static("terraops"),
                    );
                }
                svc.call(req).await
            })
        }
    }
}

/// Insert a user row, encrypt email with the test keys, and grant the
/// requested roles. Returns the user id.
pub async fn create_user_with_roles(
    pool: &PgPool,
    keys: &RuntimeKeys,
    email: &str,
    password: &str,
    roles: &[Role],
) -> Uuid {
    let normalized = normalize_email(email);
    let ct = encrypt_email(&normalized, &keys.email_enc).expect("enc");
    let hash = email_hash(&normalized, &keys.email_hmac).to_vec();
    let mask = email_mask(&normalized);
    let phc = argon::hash_password(password).expect("hash");
    // Audit #4 Issue #4: users have a first-class `username` column used
    // by the login contract; for tests we derive it from the email
    // local-part + a random suffix to avoid uniqueness collisions when
    // two tests seed the same email into the same DB.
    let uname = format!(
        "{}-{}",
        normalized.split('@').next().unwrap_or("user"),
        &Uuid::new_v4().to_string()[..8]
    );
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (display_name, username, email_ciphertext, email_hash, email_mask, \
                            password_hash, timezone) \
         VALUES ($1, $2, $3, $4, $5, $6, 'America/New_York') RETURNING id",
    )
    .bind(format!("Test {}", email))
    .bind(&uname)
    .bind(&ct)
    .bind(&hash)
    .bind(&mask)
    .bind(&phc)
    .fetch_one(pool)
    .await
    .expect("insert user");
    for r in roles {
        sqlx::query(
            "INSERT INTO user_roles (user_id, role_id) \
             SELECT $1, id FROM roles WHERE name = $2",
        )
        .bind(row.0)
        .bind(r.as_db())
        .execute(pool)
        .await
        .expect("grant role");
    }
    row.0
}

/// Fetch the DB-assigned `username` for a user created by
/// `create_user_with_roles`. The harness generates a random-suffixed
/// username per user (see audit #4 issue #4); tests that call
/// `/auth/login` must POST this exact value as `{username}` because the
/// login contract is username-only with **no** email fallback
/// (audit #10 issue #2).
pub async fn username_for(pool: &PgPool, user_id: Uuid) -> String {
    let (uname,): (String,) = sqlx::query_as("SELECT username FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("fetch username for test user");
    uname
}

/// Insert a live `sessions` row + mint a matching HS256 access token.
pub async fn issue_session_for(
    pool: &PgPool,
    keys: &RuntimeKeys,
    user_id: Uuid,
) -> (String, Uuid) {
    let issued = sessions::issue(pool, user_id, Some("test-ua"), None)
        .await
        .expect("issue");
    let (token, _) = jwt::mint(user_id, issued.session_id, &keys.jwt).expect("mint jwt");
    (token, issued.session_id)
}

/// Full one-shot: create user with roles + issue bearer token.
pub async fn authed(
    pool: &PgPool,
    keys: &RuntimeKeys,
    email: &str,
    roles: &[Role],
) -> (Uuid, String) {
    let id = create_user_with_roles(pool, keys, email, "TerraOps!2026", roles).await;
    let (token, _sid) = issue_session_for(pool, keys, id).await;
    (id, token)
}
