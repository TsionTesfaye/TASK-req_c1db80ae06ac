//! Alert rule + event route handlers (AL1–AL6).
//!
//!   AL1 GET    /api/v1/alerts/rules
//!   AL2 POST   /api/v1/alerts/rules
//!   AL3 PATCH  /api/v1/alerts/rules/{id}
//!   AL4 DELETE /api/v1/alerts/rules/{id}
//!   AL5 GET    /api/v1/alerts/events
//!   AL6 POST   /api/v1/alerts/events/{id}/ack

use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::{AppError, AppResult},
    state::AppState,
};
use terraops_shared::{
    dto::alert::{
        AlertEventQuery, AckAlertEventResponse, CreateAlertRuleRequest, UpdateAlertRuleRequest,
    },
    pagination::{Page, PageQuery},
};

use super::rules;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/alerts")
            .route("/rules", web::get().to(list_rules))
            .route("/rules", web::post().to(create_rule))
            .route("/rules/{id}", web::patch().to(update_rule))
            .route("/rules/{id}", web::delete().to(delete_rule))
            .route("/events", web::get().to(list_events))
            .route("/events/{id}/ack", web::post().to(ack_event)),
    );
}

// ===========================================================================
// AL1 — GET /api/v1/alerts/rules
// ===========================================================================
async fn list_rules(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<PageQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "alert.manage")?;
    let r = q.into_inner().resolved();
    let (items, total) = rules::list_rules(&state.pool, r.limit() as i64, r.offset() as i64).await?;
    Ok(HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(Page {
            items,
            page: r.page,
            page_size: r.page_size,
            total: total as u64,
        }))
}

// ===========================================================================
// AL2 — POST /api/v1/alerts/rules
// ===========================================================================
async fn create_rule(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateAlertRuleRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "alert.manage")?;
    let b = body.into_inner();
    let valid_ops = [">", "<", ">=", "<=", "="];
    if !valid_ops.contains(&b.operator.as_str()) {
        return Err(AppError::Validation(
            "operator must be one of: >, <, >=, <=, =".into(),
        ));
    }
    let severity = b.severity.as_deref().unwrap_or("warning");
    if !["info", "warning", "critical"].contains(&severity) {
        return Err(AppError::Validation(
            "severity must be one of: info, warning, critical".into(),
        ));
    }
    let dto = rules::create_rule(
        &state.pool,
        b.metric_definition_id,
        b.threshold,
        &b.operator,
        b.duration_seconds.unwrap_or(0),
        severity,
        user.0.user_id,
    )
    .await?;
    Ok(HttpResponse::Created().json(dto))
}

// ===========================================================================
// AL3 — PATCH /api/v1/alerts/rules/{id}
// ===========================================================================
async fn update_rule(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateAlertRuleRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "alert.manage")?;
    let b = body.into_inner();
    if let Some(ref op) = b.operator {
        if ![">" , "<", ">=", "<=", "="].contains(&op.as_str()) {
            return Err(AppError::Validation(
                "operator must be one of: >, <, >=, <=, =".into(),
            ));
        }
    }
    let dto = rules::update_rule(
        &state.pool,
        path.into_inner(),
        b.threshold,
        b.operator.as_deref(),
        b.duration_seconds,
        b.severity.as_deref(),
        b.enabled,
    )
    .await?;
    Ok(HttpResponse::Ok().json(dto))
}

// ===========================================================================
// AL4 — DELETE /api/v1/alerts/rules/{id}
// ===========================================================================
async fn delete_rule(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "alert.manage")?;
    rules::delete_rule(&state.pool, path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ===========================================================================
// AL5 — GET /api/v1/alerts/events
// ===========================================================================
#[derive(Deserialize)]
struct EventsQuery {
    rule_id: Option<Uuid>,
    unacked_only: Option<bool>,
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn list_events(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<EventsQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "kpi.read")?;
    use terraops_shared::pagination::PageQuery;
    let p = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    let (items, total) = rules::list_events(
        &state.pool,
        q.rule_id,
        q.unacked_only.unwrap_or(false),
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
// AL6 — POST /api/v1/alerts/events/{id}/ack
// ===========================================================================
async fn ack_event(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "alert.ack")?;
    let dto = rules::ack_event(&state.pool, path.into_inner(), user.0.user_id).await?;
    Ok(HttpResponse::Ok().json(AckAlertEventResponse {
        id: dto.id,
        acked_at: dto.acked_at.unwrap_or_else(Utc::now),
    }))
}

use chrono::Utc;
