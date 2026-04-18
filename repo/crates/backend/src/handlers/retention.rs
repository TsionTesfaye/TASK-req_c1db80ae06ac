//! Retention endpoints R1–R3.
//!
//!   R1 GET   /api/v1/retention
//!   R2 PATCH /api/v1/retention/{domain}
//!   R3 POST  /api/v1/retention/{domain}/run   (trigger enforcement)

use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::FromRow;
use terraops_shared::dto::retention::{RetentionPolicy, RetentionRunResult, UpdateRetentionPolicy};

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::{AppError, AppResult},
    services::audit as audit_svc,
    state::AppState,
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/retention")
            .route("", web::get().to(list_retention))
            .route("/{domain}", web::patch().to(patch_retention))
            .route("/{domain}/run", web::post().to(run_retention)),
    );
}

#[derive(FromRow)]
struct RetentionRow {
    domain: String,
    ttl_days: i32,
    last_enforced_at: Option<DateTime<Utc>>,
    updated_at: DateTime<Utc>,
}

async fn list_retention(
    user: AuthUser,
    state: web::Data<AppState>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "retention.manage")?;
    let rows: Vec<RetentionRow> = sqlx::query_as::<_, RetentionRow>(
        "SELECT domain, ttl_days, last_enforced_at, updated_at \
         FROM retention_policies ORDER BY domain",
    )
    .fetch_all(&state.pool)
    .await?;
    let items: Vec<RetentionPolicy> = rows
        .into_iter()
        .map(|r| RetentionPolicy {
            domain: r.domain,
            ttl_days: r.ttl_days,
            last_enforced_at: r.last_enforced_at,
            updated_at: r.updated_at,
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

async fn patch_retention(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateRetentionPolicy>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "retention.manage")?;
    let domain = path.into_inner();
    let req = body.into_inner();
    if req.ttl_days < 0 {
        return Err(AppError::Validation("ttl_days must be >= 0".into()));
    }
    let res = sqlx::query(
        "UPDATE retention_policies SET ttl_days = $1, updated_by = $2, updated_at = NOW() \
         WHERE domain = $3",
    )
    .bind(req.ttl_days)
    .bind(user.0.user_id)
    .bind(&domain)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "retention.update",
        Some("retention"),
        Some(&domain),
        json!({"ttl_days": req.ttl_days}),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Enforce a retention policy. Deletion targets per design §Retention:
///   env_raw  → `env_observations` (created later; no-op until P-B)
///   kpi      → `kpi_snapshots`    (created later; no-op until P-B)
///   feedback → `talent_feedback`  (created later; no-op until P-C)
///   audit    → `audit_log`        (0 = indefinite, never deletes)
///
/// In P1 we set `last_enforced_at` and report `deleted = 0` for the domains
/// whose target tables do not yet exist, so the admin can verify the flow
/// end-to-end without waiting on the catalog/env/talent packages.
async fn run_retention(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "retention.manage")?;
    let domain = path.into_inner();
    let ttl: Option<(i32,)> = sqlx::query_as(
        "SELECT ttl_days FROM retention_policies WHERE domain = $1",
    )
    .bind(&domain)
    .fetch_optional(&state.pool)
    .await?;
    let ttl = ttl.ok_or(AppError::NotFound)?.0;

    let deleted: i64 = match domain.as_str() {
        "audit" => 0, // `audit` is indefinite by policy; 0 is the correct answer.
        _ if ttl == 0 => 0,
        _ => {
            // Target tables for env_raw/kpi/feedback land in P-B/P-C. Until
            // then we have no rows to remove — report 0 honestly.
            0
        }
    };

    sqlx::query(
        "UPDATE retention_policies SET last_enforced_at = NOW(), updated_at = NOW() \
         WHERE domain = $1",
    )
    .bind(&domain)
    .execute(&state.pool)
    .await?;

    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "retention.run",
        Some("retention"),
        Some(&domain),
        json!({"deleted": deleted, "ttl_days": ttl}),
    )
    .await?;

    Ok(HttpResponse::Ok().json(RetentionRunResult {
        domain,
        deleted,
        enforced_at: Utc::now(),
    }))
}
