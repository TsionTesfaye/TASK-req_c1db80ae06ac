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
use terraops_shared::{
    dto::kpi::DrillQuery,
    pagination::{Page, PageQuery},
};

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
// K2 — Cycle Time
// ===========================================================================
#[derive(Deserialize)]
struct CycleQuery {
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    #[allow(dead_code)]
    category: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn cycle_time(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<CycleQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "kpi.read")?;
    use terraops_shared::pagination::PageQuery;
    let p = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    let (items, total) = repo::cycle_time(
        &state.pool,
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

// ===========================================================================
// K3 — Funnel
// ===========================================================================
async fn funnel(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    require_permission(&user.0, "kpi.read")?;
    let resp = repo::funnel(&state.pool).await?;
    Ok(HttpResponse::Ok().json(resp))
}

// ===========================================================================
// K4 — Anomalies
// ===========================================================================
#[derive(Deserialize)]
struct AnomalyQuery {
    #[allow(dead_code)]
    site_id: Option<Uuid>,
    #[allow(dead_code)]
    department_id: Option<Uuid>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn anomalies(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<AnomalyQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "kpi.read")?;
    use terraops_shared::pagination::PageQuery;
    let p = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    let (items, total) =
        repo::anomalies(&state.pool, q.from, q.to, p.limit() as i64, p.offset() as i64).await?;
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
#[derive(Deserialize)]
struct EffQuery {
    #[allow(dead_code)]
    site_id: Option<Uuid>,
    #[allow(dead_code)]
    department_id: Option<Uuid>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn efficiency(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<EffQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "kpi.read")?;
    use terraops_shared::pagination::PageQuery;
    let p = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    let (items, total) =
        repo::efficiency(&state.pool, q.from, q.to, p.limit() as i64, p.offset() as i64).await?;
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
    #[allow(dead_code)]
    site_id: Option<Uuid>,
    #[allow(dead_code)]
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
    use terraops_shared::pagination::PageQuery;
    let p = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    let (items, total) = repo::drill(
        &state.pool,
        q.metric_kind.as_deref(),
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
