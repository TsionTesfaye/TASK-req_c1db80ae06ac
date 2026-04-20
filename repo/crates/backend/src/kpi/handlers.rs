//! KPI route handlers (K1–K6).
//!
//!   K1 GET /api/v1/kpi/summary
//!   K2 GET /api/v1/kpi/cycle-time
//!   K3 GET /api/v1/kpi/funnel
//!   K4 GET /api/v1/kpi/anomalies
//!   K5 GET /api/v1/kpi/efficiency
//!   K6 GET /api/v1/kpi/drill

use actix_web::{web, HttpResponse, Responder};
use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::AppResult,
    state::AppState,
};
use terraops_shared::pagination::{Page, PageQuery};

use super::repo;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/kpi")
            .route("/summary", web::get().to(summary))
            .route("/cycle-time", web::get().to(cycle_time))
            .route("/funnel", web::get().to(funnel))
            .route("/anomalies", web::get().to(anomalies))
            .route("/efficiency", web::get().to(efficiency))
            .route("/drill", web::get().to(drill)),
    );
}

// ===========================================================================
// K1 — Summary
// ===========================================================================
async fn summary(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    require_permission(&user.0, "kpi.read")?;
    let s = repo::summary(&state.pool).await?;
    Ok(HttpResponse::Ok().json(s))
}

// ===========================================================================
// Shared slice query — K2, K4, K5 honor site_id / department_id / category.
// ===========================================================================
#[derive(Deserialize)]
struct SliceQuery {
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    category: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

// ===========================================================================
// K2 — Cycle Time
// ===========================================================================
async fn cycle_time(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<SliceQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "kpi.read")?;
    let p = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    let (items, total) = repo::cycle_time(
        &state.pool,
        q.site_id,
        q.department_id,
        q.category.as_deref(),
        q.from,
        q.to,
        p.limit() as i64,
        p.offset() as i64,
    )
    .await?;
    Ok(HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(Page {
            items,
            page: p.page,
            page_size: p.page_size,
            total: total as u64,
        }))
}

// ===========================================================================
// K3 — Funnel
// ===========================================================================
/// Funnel slice query. Audit #8 Issue #2: the funnel is now a real
/// slice-and-drill surface honoring time (`from`/`to`), spatial
/// (`site_id`/`department_id` — correlated through
/// `alert_rules.metric_definition_id → metric_definitions.source_ids →
/// env_sources`), and categorical (`severity`, accepted as `category` too)
/// dimensions. All parameters are optional; omitting them returns the
/// unsliced pipeline, which preserves the legacy contract.
#[derive(Deserialize)]
struct FunnelQuery {
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    /// Alert severity (`info|warning|critical`). Also accepted as
    /// `category` for symmetry with the other KPI slice axes.
    severity: Option<String>,
    category: Option<String>,
}

async fn funnel(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<FunnelQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "kpi.read")?;
    let from_ts = q.from.map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    let to_ts = q.to.map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());
    let sev = q
        .severity
        .as_deref()
        .or(q.category.as_deref())
        .filter(|s| ["info", "warning", "critical"].contains(s));
    let resp = repo::funnel_sliced(
        &state.pool,
        q.site_id,
        q.department_id,
        from_ts,
        to_ts,
        sev,
    )
    .await?;
    Ok(HttpResponse::Ok().json(resp))
}

// ===========================================================================
// K4 — Anomalies
// ===========================================================================
async fn anomalies(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<SliceQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "kpi.read")?;
    let p = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    let (items, total) = repo::anomalies(
        &state.pool,
        q.site_id,
        q.department_id,
        q.category.as_deref(),
        q.from,
        q.to,
        p.limit() as i64,
        p.offset() as i64,
    )
    .await?;
    Ok(HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(Page {
            items,
            page: p.page,
            page_size: p.page_size,
            total: total as u64,
        }))
}

// ===========================================================================
// K5 — Efficiency
// ===========================================================================
async fn efficiency(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<SliceQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "kpi.read")?;
    let p = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    let (items, total) = repo::efficiency(
        &state.pool,
        q.site_id,
        q.department_id,
        q.category.as_deref(),
        q.from,
        q.to,
        p.limit() as i64,
        p.offset() as i64,
    )
    .await?;
    Ok(HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(Page {
            items,
            page: p.page,
            page_size: p.page_size,
            total: total as u64,
        }))
}

// ===========================================================================
// K6 — Drill
// ===========================================================================
#[derive(Deserialize)]
struct DrillQ {
    metric_kind: Option<String>,
    /// Alias for `metric_kind` — the generic KPI "category" axis. When both
    /// are present, `metric_kind` wins.
    category: Option<String>,
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn drill(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<DrillQ>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "kpi.read")?;
    let p = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    let kind = q.metric_kind.as_deref().or(q.category.as_deref());
    let (items, total) = repo::drill(
        &state.pool,
        kind,
        q.site_id,
        q.department_id,
        q.from,
        q.to,
        p.limit() as i64,
        p.offset() as i64,
    )
    .await?;
    Ok(HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(Page {
            items,
            page: p.page,
            page_size: p.page_size,
            total: total as u64,
        }))
}
