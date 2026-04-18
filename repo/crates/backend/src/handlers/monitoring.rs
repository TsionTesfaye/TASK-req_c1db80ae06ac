//! Admin monitoring endpoints M1–M4.
//!
//!   M1 GET  /api/v1/monitoring/latency
//!   M2 GET  /api/v1/monitoring/errors
//!   M3 POST /api/v1/monitoring/crash-report     (auth'd; any user)
//!   M4 GET  /api/v1/monitoring/crash-reports    (admin)

use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use terraops_shared::{
    dto::monitoring::{CrashReport, ErrorBucket, IngestCrashReport, LatencyBucket},
    pagination::{Page, PageQuery},
};
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::AppResult,
    state::AppState,
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/monitoring")
            .route("/latency", web::get().to(latency))
            .route("/errors", web::get().to(errors))
            .route("/crash-report", web::post().to(ingest_crash))
            .route("/crash-reports", web::get().to(list_crashes)),
    );
}

async fn latency(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    require_permission(&user.0, "monitoring.read")?;
    #[derive(FromRow)]
    struct Row {
        route: String,
        method: String,
        count: i64,
        p50_ms: Option<f64>,
        p95_ms: Option<f64>,
        p99_ms: Option<f64>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT route, method, COUNT(*)::BIGINT AS count, \
                percentile_cont(0.50) WITHIN GROUP (ORDER BY latency_ms) AS p50_ms, \
                percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms) AS p95_ms, \
                percentile_cont(0.99) WITHIN GROUP (ORDER BY latency_ms) AS p99_ms \
         FROM api_metrics WHERE at > NOW() - INTERVAL '1 hour' \
         GROUP BY route, method ORDER BY count DESC LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;
    let items: Vec<LatencyBucket> = rows
        .into_iter()
        .map(|r| LatencyBucket {
            route: r.route,
            method: r.method,
            count: r.count,
            p50_ms: r.p50_ms.unwrap_or(0.0) as i64,
            p95_ms: r.p95_ms.unwrap_or(0.0) as i64,
            p99_ms: r.p99_ms.unwrap_or(0.0) as i64,
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

async fn errors(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    require_permission(&user.0, "monitoring.read")?;
    #[derive(FromRow)]
    struct Row {
        route: String,
        method: String,
        total: i64,
        errors: i64,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT route, method, COUNT(*)::BIGINT AS total, \
                SUM(CASE WHEN status >= 500 THEN 1 ELSE 0 END)::BIGINT AS errors \
         FROM api_metrics WHERE at > NOW() - INTERVAL '1 hour' \
         GROUP BY route, method ORDER BY errors DESC LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;
    let items: Vec<ErrorBucket> = rows
        .into_iter()
        .map(|r| {
            let rate = if r.total > 0 {
                r.errors as f64 / r.total as f64
            } else {
                0.0
            };
            ErrorBucket {
                route: r.route,
                method: r.method,
                total: r.total,
                errors: r.errors,
                error_rate: rate,
            }
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

async fn ingest_crash(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<IngestCrashReport>,
) -> AppResult<impl Responder> {
    let req = body.into_inner();
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO client_crash_reports (user_id, page, agent, stack, payload_json) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(user.0.user_id)
    .bind(req.page.as_deref())
    .bind(req.agent.as_deref())
    .bind(req.stack.as_deref())
    .bind(req.payload.unwrap_or_else(|| serde_json::json!({})))
    .fetch_one(&state.pool)
    .await?;
    Ok(HttpResponse::Created().json(serde_json::json!({"id": row.0})))
}

async fn list_crashes(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<PageQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "monitoring.read")?;
    let r = q.into_inner().resolved();
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        user_id: Option<Uuid>,
        page: Option<String>,
        agent: Option<String>,
        stack: Option<String>,
        payload_json: serde_json::Value,
        reported_at: DateTime<Utc>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT id, user_id, page, agent, stack, payload_json, reported_at \
         FROM client_crash_reports ORDER BY reported_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(r.limit() as i64)
    .bind(r.offset() as i64)
    .fetch_all(&state.pool)
    .await?;
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM client_crash_reports")
        .fetch_one(&state.pool)
        .await?;
    let items: Vec<CrashReport> = rows
        .into_iter()
        .map(|r| CrashReport {
            id: r.id,
            user_id: r.user_id,
            page: r.page,
            agent: r.agent,
            stack: r.stack,
            payload: r.payload_json,
            reported_at: r.reported_at,
        })
        .collect();
    let page = Page {
        items,
        page: r.page,
        page_size: r.page_size,
        total: total.0 as u64,
    };
    Ok(HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.0.to_string()))
        .json(page))
}
