//! Actix-web application builder.
//!
//! Wires:
//!   * HTTPS listener (rustls 0.22)
//!   * AppState (pool + runtime keys + static dir + default tz)
//!   * Middleware stack: request_id → authn+allowlist → budget → metrics
//!   * `/api/v1/**` routers from `handlers::configure`
//!   * SPA served from `static_dir` with HTML5 fallback

use std::sync::Arc;

use actix_web::{web, App, HttpServer};

use crate::{
    config::Config,
    crypto::keys::RuntimeKeys,
    db, handlers,
    middleware::{
        authn::AuthnMw, budget::BudgetMw, csrf::CsrfMw, metrics::MetricsMw,
        request_id::RequestIdMw,
    },
    spa,
    state::AppState,
    tls,
};

pub async fn run(cfg: Config) -> anyhow::Result<()> {
    let pool = db::connect(&cfg).await?;

    // Read the admin-controlled mTLS enforcement flag from the database
    // at startup. When `mtls_config.enforced = true`, we load the internal
    // CA bundle from the runtime volume and build the rustls server with
    // a `WebPkiClientVerifier` so any unpinned client is refused at the
    // TLS handshake. The transport-layer proof lives in
    // `crates/backend/tests/mtls_handshake_tests.rs`.
    //
    // Audit #12 Issue #3 — restart-gated contract: the flag value read
    // here is the value that ends up baked into the live
    // `rustls::ServerConfig` below. A subsequent admin PATCH to
    // `/security/mtls` updates `mtls_config.enforced` in the DB but
    // does not rewrite the live TLS server — the backend must be
    // restarted for the persisted flag to become active. We stash the
    // startup value in `AppState.mtls_startup_enforced` so the admin
    // endpoints can report `active_enforced` and `pending_restart`
    // honestly instead of implying the PATCH is live. Device-cert SPKI
    // pins still refresh live via the background refresher below.
    let mtls_enforced: bool = sqlx::query_scalar::<_, bool>(
        "SELECT enforced FROM mtls_config WHERE id = 1",
    )
    .fetch_optional(&pool)
    .await?
    .unwrap_or(false);

    let ca_path = cfg.runtime_dir.join("internal_ca").join("ca.crt");
    let tls_cfg = if mtls_enforced {
        // Build a live SPKI pin set seeded from `device_certs WHERE
        // revoked_at IS NULL` and install a background refresher. The
        // rustls `ClientCertVerifier` holds an `Arc<RwLock<_>>` into this
        // pin set, so admin revocations propagate on the next refresh
        // tick (30s) without a server restart — any handshake after that
        // is refused at the transport layer.
        let pins = tls::new_pin_set();
        let n0 = tls::refresh_pins(&pool, &pins).await?;
        tracing::info!(
            ca_path = %ca_path.display(),
            active_pins = n0,
            "mTLS enforcement is ON — binding rustls with CA-chain verifier + live device-cert SPKI pin set"
        );
        tls::spawn_pin_refresher(pool.clone(), pins.clone(), std::time::Duration::from_secs(30));
        tls::load_server_config_with_pinned_mtls(
            &cfg.tls_cert_path,
            &cfg.tls_key_path,
            &ca_path,
            pins,
        )?
    } else {
        tracing::info!("mTLS enforcement is OFF — binding rustls in one-way TLS mode");
        tls::load_server_config(&cfg.tls_cert_path, &cfg.tls_key_path)?
    };

    let keys = Arc::new(RuntimeKeys::load_or_init(&cfg.runtime_dir)?);

    let state = AppState {
        pool: pool.clone(),
        keys,
        static_dir: cfg.static_dir.clone(),
        default_timezone: cfg.default_timezone.clone(),
        runtime_dir: cfg.runtime_dir.clone(),
        // Audit #12 Issue #3: record the enforcement flag actually used
        // to construct the rustls `ServerConfig` so admin endpoints can
        // report an honest live-vs-persisted delta.
        mtls_startup_enforced: mtls_enforced,
    };

    // Start background jobs (alert evaluator, report scheduler, retention
    // sweep, metric rollup, notification retry). Handles are intentionally
    // dropped: the Tokio runtime owns them until process shutdown.
    let _job_handles = crate::jobs::start_all(pool, cfg.runtime_dir.clone());

    tracing::info!(bind = %cfg.bind_addr, "terraops-backend listening");

    let bind_addr = cfg.bind_addr.clone();
    HttpServer::new(move || {
        let state = state.clone();
        let static_dir = state.static_dir.clone();
        App::new()
            .app_data(web::Data::new(state))
            .wrap(MetricsMw)
            .wrap(BudgetMw)
            .wrap(CsrfMw)
            .wrap(AuthnMw)
            .wrap(RequestIdMw)
            .service(web::scope("/api/v1").configure(handlers::configure))
            .configure(move |c| spa::configure(c, &static_dir))
    })
    .bind_rustls_0_22(bind_addr, tls_cfg)?
    .run()
    .await?;

    Ok(())
}

/// Register every middleware + `/api/v1` handler on the provided Actix
/// `ServiceConfig`. Integration tests use this (rather than `run`) so the
/// HTTP suite can mount the real app against a test DB without binding a
/// TLS socket.
pub fn configure_api(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/api/v1").configure(handlers::configure));
}
