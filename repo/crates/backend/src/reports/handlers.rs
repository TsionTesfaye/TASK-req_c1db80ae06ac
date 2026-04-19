//! Report job route handlers (RP1–RP6).
//!
//!   RP1 GET  /api/v1/reports/jobs            — PERM(report.run)
//!   RP2 POST /api/v1/reports/jobs            — PERM(report.schedule)
//!   RP3 GET  /api/v1/reports/jobs/{id}       — SELF (owner_id == caller)
//!   RP4 POST /api/v1/reports/jobs/{id}/run-now — SELF
//!   RP5 POST /api/v1/reports/jobs/{id}/cancel  — SELF
//!   RP6 GET  /api/v1/reports/jobs/{id}/artifact — SELF, streams last artifact

use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser, OwnerGuard},
    errors::{AppError, AppResult},
    state::AppState,
};
use terraops_shared::{
    dto::report::{CreateReportJobRequest, ReportJobDto, ReportRunResponse},
    pagination::{Page, PageQuery},
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/reports")
            .route("/jobs", web::get().to(list_jobs))
            .route("/jobs", web::post().to(create_job))
            .route("/jobs/{id}", web::get().to(get_job))
            .route("/jobs/{id}/run-now", web::post().to(run_now))
            .route("/jobs/{id}/cancel", web::post().to(cancel_job))
            .route("/jobs/{id}/artifact", web::get().to(get_artifact)),
    );
}

#[derive(FromRow)]
struct JobRow {
    id: Uuid,
    owner_id: Uuid,
    kind: String,
    format: String,
    params: Value,
    cron: Option<String>,
    status: String,
    last_run_at: Option<DateTime<Utc>>,
    last_artifact_path: Option<String>,
    retry_count: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<JobRow> for ReportJobDto {
    fn from(r: JobRow) -> Self {
        ReportJobDto {
            id: r.id,
            owner_id: r.owner_id,
            kind: r.kind,
            format: r.format,
            params: r.params,
            cron: r.cron,
            status: r.status,
            last_run_at: r.last_run_at,
            last_artifact_path: r.last_artifact_path,
            retry_count: r.retry_count,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

async fn fetch_job(state: &AppState, id: Uuid) -> AppResult<JobRow> {
    sqlx::query_as::<_, JobRow>(
        "SELECT id, owner_id, kind, format, params, cron, status, \
                last_run_at, last_artifact_path, retry_count, created_at, updated_at \
         FROM report_jobs WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)
}

// ===========================================================================
// RP1 — GET /api/v1/reports/jobs
// ===========================================================================
async fn list_jobs(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<PageQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "report.run")?;
    let r = q.into_inner().resolved();
    let rows: Vec<JobRow> = sqlx::query_as(
        "SELECT id, owner_id, kind, format, params, cron, status, \
                last_run_at, last_artifact_path, retry_count, created_at, updated_at \
         FROM report_jobs WHERE owner_id = $1 \
         ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(user.0.user_id)
    .bind(r.limit() as i64)
    .bind(r.offset() as i64)
    .fetch_all(&state.pool)
    .await?;
    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM report_jobs WHERE owner_id = $1",
    )
    .bind(user.0.user_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(Page {
            items: rows.into_iter().map(ReportJobDto::from).collect::<Vec<_>>(),
            page: r.page,
            page_size: r.page_size,
            total: total as u64,
        }))
}

// ===========================================================================
// RP2 — POST /api/v1/reports/jobs
// ===========================================================================
async fn create_job(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateReportJobRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "report.schedule")?;
    let b = body.into_inner();
    if !["kpi_summary", "env_series", "alert_digest"].contains(&b.kind.as_str()) {
        return Err(AppError::Validation("kind must be kpi_summary|env_series|alert_digest".into()));
    }
    if !["pdf", "csv", "xlsx"].contains(&b.format.as_str()) {
        return Err(AppError::Validation("format must be pdf|csv|xlsx".into()));
    }
    let row: JobRow = sqlx::query_as(
        "INSERT INTO report_jobs (owner_id, kind, format, params, cron) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, owner_id, kind, format, params, cron, status, \
                   last_run_at, last_artifact_path, retry_count, created_at, updated_at",
    )
    .bind(user.0.user_id)
    .bind(&b.kind)
    .bind(&b.format)
    .bind(b.params.unwrap_or(serde_json::json!({})))
    .bind(b.cron)
    .fetch_one(&state.pool)
    .await?;
    Ok(HttpResponse::Created().json(ReportJobDto::from(row)))
}

// ===========================================================================
// RP3 — GET /api/v1/reports/jobs/{id}   (SELF)
// ===========================================================================
async fn get_job(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    let job = fetch_job(&state, path.into_inner()).await?;
    OwnerGuard::allow_self(&user.0, job.owner_id)?;
    Ok(HttpResponse::Ok().json(ReportJobDto::from(job)))
}

// ===========================================================================
// RP4 — POST /api/v1/reports/jobs/{id}/run-now   (SELF)
// ===========================================================================
async fn run_now(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    let id = path.into_inner();
    let job = fetch_job(&state, id).await?;
    OwnerGuard::allow_self(&user.0, job.owner_id)?;
    if job.status == "running" {
        return Err(AppError::Conflict("job is already running".into()));
    }
    if job.status == "cancelled" {
        return Err(AppError::Validation("cannot run a cancelled job".into()));
    }
    // Reset to scheduled so the scheduler picks it up on next tick
    sqlx::query(
        "UPDATE report_jobs SET status='scheduled', retry_count=0 WHERE id=$1",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;
    Ok(HttpResponse::Ok().json(ReportRunResponse {
        id,
        status: "scheduled".into(),
    }))
}

// ===========================================================================
// RP5 — POST /api/v1/reports/jobs/{id}/cancel   (SELF)
// ===========================================================================
async fn cancel_job(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    let id = path.into_inner();
    let job = fetch_job(&state, id).await?;
    OwnerGuard::allow_self(&user.0, job.owner_id)?;
    if job.status == "running" {
        return Err(AppError::Conflict("cannot cancel a running job".into()));
    }
    sqlx::query("UPDATE report_jobs SET status='cancelled' WHERE id=$1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(HttpResponse::Ok().json(ReportRunResponse {
        id,
        status: "cancelled".into(),
    }))
}

// ===========================================================================
// RP6 — GET /api/v1/reports/jobs/{id}/artifact   (SELF, streams file)
// ===========================================================================
async fn get_artifact(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    let id = path.into_inner();
    let job = fetch_job(&state, id).await?;
    OwnerGuard::allow_self(&user.0, job.owner_id)?;
    let path_str = job
        .last_artifact_path
        .ok_or_else(|| AppError::NotFound)?;
    let bytes = std::fs::read(&path_str)
        .map_err(|_| AppError::NotFound)?;
    let content_type = match job.format.as_str() {
        "pdf" => "application/pdf",
        "csv" => "text/csv",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        _ => "application/octet-stream",
    };
    Ok(HttpResponse::Ok()
        .content_type(content_type)
        .body(bytes))
}
