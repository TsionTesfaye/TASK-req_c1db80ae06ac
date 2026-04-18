//! Actix-web application builder.
//!
//! Wires the HTTPS listener, loads runtime key material, initializes the
//! shared `AppState`, mounts the system routes (`/api/v1/health`,
//! `/api/v1/ready`), and serves the Yew SPA with HTML5 fallback. Feature
//! route families mount through `handlers::configure` as they land in P1
//! and later packages.

use std::sync::Arc;

use actix_web::{web, App, HttpResponse, HttpServer};
use terraops_shared::dto::health::{HealthResponse, ReadyResponse};

use crate::{
    config::Config,
    crypto::keys::RuntimeKeys,
    db, spa,
    state::AppState,
    tls,
};

pub async fn run(cfg: Config) -> anyhow::Result<()> {
    let pool = db::connect(&cfg).await?;
    let tls_cfg = tls::load_server_config(&cfg.tls_cert_path, &cfg.tls_key_path)?;
    let keys = Arc::new(RuntimeKeys::load_or_init(&cfg.runtime_dir)?);

    let state = AppState {
        pool,
        keys,
        static_dir: cfg.static_dir.clone(),
        default_timezone: cfg.default_timezone.clone(),
    };

    tracing::info!(bind = %cfg.bind_addr, "terraops-backend listening");

    let bind_addr = cfg.bind_addr.clone();
    HttpServer::new(move || {
        let state = state.clone();
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(
                web::scope("/api/v1")
                    .route("/health", web::get().to(health))
                    .route("/ready", web::get().to(ready)),
            )
            .configure(|c| spa::configure(c, &state.static_dir))
    })
    .bind_rustls_0_22(bind_addr, tls_cfg)?
    .run()
    .await?;

    Ok(())
}

async fn health() -> HttpResponse {
    HttpResponse::Ok().json(HealthResponse { status: "ok" })
}

async fn ready(state: web::Data<AppState>) -> HttpResponse {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(_) => HttpResponse::Ok().json(ReadyResponse {
            status: "ready",
            db: true,
        }),
        Err(err) => {
            tracing::warn!(error = %err, "readiness probe: database not reachable");
            HttpResponse::ServiceUnavailable().json(ReadyResponse {
                status: "not_ready",
                db: false,
            })
        }
    }
}

/// Re-export for callers that imported `crate::app::AppState` before the
/// state was centralized; keeps the path stable during the P1 rewrite.
pub use crate::state::AppState as _AppState;
