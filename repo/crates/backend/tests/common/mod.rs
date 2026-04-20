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
        authn::AuthnMw, budget::BudgetMw, metrics::MetricsMw, request_id::RequestIdMw,
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
pub fn build_test_app(
    state: AppState,
) -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse<
            EitherBody<EitherBody<EitherBody<BoxBody>>>,
        >,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    App::new()
        .app_data(web::Data::new(state))
        .wrap(MetricsMw)
        .wrap(BudgetMw)
        .wrap(AuthnMw)
        .wrap(RequestIdMw)
        .service(web::scope("/api/v1").configure(handlers::configure))
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
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (display_name, email_ciphertext, email_hash, email_mask, \
                            password_hash, timezone) \
         VALUES ($1, $2, $3, $4, $5, 'America/New_York') RETURNING id",
    )
    .bind(format!("Test {}", email))
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
