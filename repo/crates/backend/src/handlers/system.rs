//! S1 /api/v1/health, S2 /api/v1/ready — unauthenticated probes.

use actix_web::{web, HttpResponse, Responder};
use terraops_shared::dto::health::{HealthResponse, ReadyResponse};

use crate::state::AppState;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health));
    cfg.route("/ready", web::get().to(ready));
}

async fn health() -> impl Responder {
    HttpResponse::Ok().json(HealthResponse { status: "ok" })
}

async fn ready(state: web::Data<AppState>) -> impl Responder {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(_) => HttpResponse::Ok().json(ReadyResponse {
            status: "ready",
            db: true,
        }),
        Err(_) => HttpResponse::ServiceUnavailable().json(ReadyResponse {
            status: "not_ready",
            db: false,
        }),
    }
}
