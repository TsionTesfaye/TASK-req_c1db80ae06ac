//! HTTP route handlers for env sources + observations (E1–E6) and
//! metric definitions + lineage (MD1–MD7).

use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::{AppError, AppResult},
    state::AppState,
};
use terraops_shared::{
    dto::env_source::{
        BulkObservationsRequest, CreateEnvSourceRequest, UpdateEnvSourceRequest,
    },
    dto::metric::{
        CreateMetricDefinitionRequest, MetricSeriesResponse, UpdateMetricDefinitionRequest,
    },
    pagination::{Page, PageQuery},
};

use super::{definitions, lineage, sources};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/env")
            .route("/sources", web::get().to(list_sources))
            .route("/sources", web::post().to(create_source))
            .route("/sources/{id}", web::patch().to(update_source))
            .route("/sources/{id}", web::delete().to(delete_source))
            .route("/sources/{id}/observations", web::post().to(bulk_observations))
            .route("/observations", web::get().to(list_observations)),
    );
    cfg.service(
        web::scope("/metrics")
            .route("/definitions", web::get().to(list_definitions))
            .route("/definitions", web::post().to(create_definition))
            .route("/definitions/{id}", web::get().to(get_definition))
            .route("/definitions/{id}", web::patch().to(update_definition))
            .route("/definitions/{id}", web::delete().to(delete_definition))
            .route("/definitions/{id}/series", web::get().to(get_series))
            .route("/computations/{id}/lineage", web::get().to(get_lineage)),
    );
}

// ===========================================================================
// E1 — GET /api/v1/env/sources
// ===========================================================================
async fn list_sources(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<PageQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.read")?;
    let r = q.into_inner().resolved();
    let (items, total) = sources::list(&state.pool, r.limit() as i64, r.offset() as i64).await?;
    let resp = HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(Page {
            items,
            page: r.page,
            page_size: r.page_size,
            total: total as u64,
        });
    Ok(resp)
}

// ===========================================================================
// E2 — POST /api/v1/env/sources
// ===========================================================================
async fn create_source(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateEnvSourceRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.configure")?;
    let b = body.into_inner();
    let dto = sources::create(
        &state.pool,
        &b.name,
        &b.kind,
        b.site_id,
        b.department_id,
        b.unit_id,
    )
    .await?;
    Ok(HttpResponse::Created().json(dto))
}

// ===========================================================================
// E3 — PATCH /api/v1/env/sources/{id}
// ===========================================================================
async fn update_source(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateEnvSourceRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.configure")?;
    let b = body.into_inner();
    // Audit #9 issue 1: PATCH must honor true tri-state reassignment and
    // clearing of site/department/unit pointers; previously the handler
    // dropped these fields on the floor.
    let dto = sources::update(
        &state.pool,
        path.into_inner(),
        b.name.as_deref(),
        b.kind.as_deref(),
        b.site_id,
        b.department_id,
        b.unit_id,
    )
    .await?;
    Ok(HttpResponse::Ok().json(dto))
}

// ===========================================================================
// E4 — DELETE /api/v1/env/sources/{id}
// ===========================================================================
async fn delete_source(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.configure")?;
    sources::soft_delete(&state.pool, path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ===========================================================================
// E5 — POST /api/v1/env/sources/{id}/observations
// ===========================================================================
async fn bulk_observations(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<BulkObservationsRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.configure")?;
    let inserted =
        sources::bulk_insert_observations(&state.pool, path.into_inner(), &body.observations)
            .await?;
    Ok(HttpResponse::Created().json(json!({ "inserted": inserted })))
}

// ===========================================================================
// E6 — GET /api/v1/env/observations
// ===========================================================================
#[derive(Deserialize)]
struct ObsQuery {
    source_id: Option<Uuid>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn list_observations(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<ObsQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.read")?;
    use terraops_shared::pagination::{PageQuery, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE};
    let p = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    let (items, total) = sources::list_observations(
        &state.pool,
        q.source_id,
        q.from,
        q.to,
        p.limit() as i64,
        p.offset() as i64,
    )
    .await?;
    let resp = HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(Page {
            items,
            page: p.page,
            page_size: p.page_size,
            total: total as u64,
        });
    Ok(resp)
}

// ===========================================================================
// MD1 — GET /api/v1/metrics/definitions
// ===========================================================================
async fn list_definitions(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<PageQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.read")?;
    let r = q.into_inner().resolved();
    let (items, total) =
        definitions::list(&state.pool, r.limit() as i64, r.offset() as i64).await?;
    let resp = HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(Page {
            items,
            page: r.page,
            page_size: r.page_size,
            total: total as u64,
        });
    Ok(resp)
}

// ===========================================================================
// MD2 — POST /api/v1/metrics/definitions
// ===========================================================================
async fn create_definition(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateMetricDefinitionRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.configure")?;
    let b = body.into_inner();
    // Validate formula_kind
    // Audit #13 Issue #2: `sku_on_shelf_compliance` is a prompt-required
    // KPI formula kind (new). It joins the existing three kinds as a
    // first-class citizen of the metrics/KPI/alert pipeline.
    if !["moving_average", "rate_of_change", "comfort_index", "sku_on_shelf_compliance"]
        .contains(&b.formula_kind.as_str())
    {
        return Err(AppError::Validation(
            "formula_kind must be one of: moving_average, rate_of_change, comfort_index, sku_on_shelf_compliance"
                .into(),
        ));
    }
    // Audit #4 Issue #3: validate analyst-configurable alignment rules
    // + confidence labels embedded in `params`. Missing block defaults
    // are accepted; malformed block is rejected with a 422 so the
    // analyst gets an actionable error instead of silently-bad config.
    let params = b.params.unwrap_or(serde_json::json!({}));
    terraops_shared::dto::metric::FusionConfig::from_params_value(&params)
        .map_err(AppError::Validation)?;
    let dto = definitions::create(
        &state.pool,
        &b.name,
        &b.formula_kind,
        params,
        &b.source_ids,
        b.window_seconds.unwrap_or(3600),
        user.0.user_id,
    )
    .await?;
    Ok(HttpResponse::Created().json(dto))
}

// ===========================================================================
// MD3 — GET /api/v1/metrics/definitions/{id}
// ===========================================================================
async fn get_definition(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.read")?;
    let dto = definitions::get(&state.pool, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(dto))
}

// ===========================================================================
// MD4 — PATCH /api/v1/metrics/definitions/{id}
// ===========================================================================
async fn update_definition(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateMetricDefinitionRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.configure")?;
    let b = body.into_inner();
    if let Some(ref fk) = b.formula_kind {
        if !["moving_average", "rate_of_change", "comfort_index", "sku_on_shelf_compliance"]
            .contains(&fk.as_str())
        {
            return Err(AppError::Validation(
                "formula_kind must be one of: moving_average, rate_of_change, comfort_index, sku_on_shelf_compliance"
                    .into(),
            ));
        }
    }
    // Audit #4 Issue #3: validate analyst-configurable alignment/
    // confidence params on update as well (otherwise a malformed patch
    // could sneak past the create-time validator by editing later).
    if let Some(ref p) = b.params {
        terraops_shared::dto::metric::FusionConfig::from_params_value(p)
            .map_err(AppError::Validation)?;
    }
    let dto = definitions::update(
        &state.pool,
        path.into_inner(),
        b.name.as_deref(),
        b.formula_kind.as_deref(),
        b.params,
        b.source_ids.as_deref(),
        b.window_seconds,
        b.enabled,
    )
    .await?;
    Ok(HttpResponse::Ok().json(dto))
}

// ===========================================================================
// MD5 — DELETE /api/v1/metrics/definitions/{id}
// ===========================================================================
async fn delete_definition(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.configure")?;
    definitions::soft_delete(&state.pool, path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ===========================================================================
// MD6 — GET /api/v1/metrics/definitions/{id}/series
// ===========================================================================
/// Runs the formula on-demand over the stored observations window and returns
/// series points. Each "point" is the result of applying the formula at the
/// latest available `observed_at` within the definition's window.
async fn get_series(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.read")?;
    let def_id = path.into_inner();
    let def = definitions::get(&state.pool, def_id).await?;

    // Compute on-demand: we run the formula at "now" over the last
    // `window_seconds` of observations and also return stored computations
    // for historical series.
    let stored = definitions::latest_series(&state.pool, def_id, 100).await?;

    // Also run fresh computation at current timestamp
    let now = Utc::now();
    let window_start = now - chrono::Duration::seconds(def.window_seconds as i64);

    let fresh_point: Option<terraops_shared::dto::metric::SeriesPoint> =
        if !def.source_ids.is_empty() {
            match def.formula_kind.as_str() {
                "moving_average" => {
                    let sid = def.source_ids[0];
                    let samples =
                        sources::fetch_window(&state.pool, sid, window_start, now).await?;
                    let pts = sources::formula_points(&samples);
                    super::formula::moving_average(&pts, def.window_seconds as i64, now).map(|v| {
                        let cid = persist_computation_bg(
                            state.pool.clone(),
                            def_id,
                            v,
                            &samples,
                            window_start,
                            now,
                            None,
                            None,
                        );
                        terraops_shared::dto::metric::SeriesPoint {
                            at: now,
                            value: v,
                            computation_id: Some(cid),
                        }
                    })
                }
                "rate_of_change" => {
                    let sid = def.source_ids[0];
                    let samples =
                        sources::fetch_window(&state.pool, sid, window_start, now).await?;
                    let pts = sources::formula_points(&samples);
                    super::formula::rate_of_change(&pts, def.window_seconds as i64, now).map(|v| {
                        let cid = persist_computation_bg(
                            state.pool.clone(),
                            def_id,
                            v,
                            &samples,
                            window_start,
                            now,
                            None,
                            None,
                        );
                        terraops_shared::dto::metric::SeriesPoint {
                            at: now,
                            value: v,
                            computation_id: Some(cid),
                        }
                    })
                }
                "comfort_index" => {
                    if def.source_ids.len() >= 2 {
                        let temp = sources::fetch_window(
                            &state.pool,
                            def.source_ids[0],
                            window_start,
                            now,
                        )
                        .await?;
                        let hum = sources::fetch_window(
                            &state.pool,
                            def.source_ids[1],
                            window_start,
                            now,
                        )
                        .await?;
                        // Optional 3rd source: air speed (m/s). Missing is
                        // tolerated — the extended formula returns lower
                        // confidence rather than None.
                        let air = if def.source_ids.len() >= 3 {
                            Some(
                                sources::fetch_window(
                                    &state.pool,
                                    def.source_ids[2],
                                    window_start,
                                    now,
                                )
                                .await?,
                            )
                        } else {
                            None
                        };
                        let temp_pts = sources::formula_points(&temp);
                        let hum_pts = sources::formula_points(&hum);
                        let air_pts = air.as_ref().map(|a| sources::formula_points(a));
                        super::formula::comfort_index_ext(
                            &temp_pts,
                            &hum_pts,
                            air_pts.as_deref(),
                            def.window_seconds as i64,
                            now,
                        )
                        .and_then(|out| {
                            // Audit #4 Issue #3: honor the analyst-
                            // configurable alignment rules parsed from
                            // `params.alignment`. In strict mode (the
                            // default) fresh points with alignment
                            // below `min_alignment` are dropped — we
                            // neither return them to the UI nor persist
                            // them. In lenient mode they still flow
                            // through but carry the low-alignment
                            // signal in their confidence label.
                            let fusion =
                                terraops_shared::dto::metric::FusionConfig::from_params_value(
                                    &def.params,
                                )
                                .unwrap_or_default();
                            if fusion.alignment.strict
                                && out.alignment < fusion.alignment.min_alignment
                            {
                                return None;
                            }
                            let mut all_samples: Vec<_> =
                                temp.iter().chain(hum.iter()).cloned().collect();
                            if let Some(a) = air.as_ref() {
                                all_samples.extend(a.iter().cloned());
                            }
                            let cid = persist_computation_bg(
                                state.pool.clone(),
                                def_id,
                                out.value,
                                &all_samples,
                                window_start,
                                now,
                                Some(out.alignment),
                                Some(out.confidence),
                            );
                            Some(terraops_shared::dto::metric::SeriesPoint {
                                at: now,
                                value: out.value,
                                computation_id: Some(cid),
                            })
                        })
                    } else {
                        None
                    }
                }
                "sku_on_shelf_compliance" => {
                    // Audit #13 Issue #2: aggregate observations across every
                    // attached source — each source represents one tracked
                    // SKU feed; value > 0 counts as on-shelf. Compliance % is
                    // the share of non-zero observations across the window.
                    let mut all_samples: Vec<crate::metrics_env::sources::WindowSample> =
                        Vec::new();
                    for sid in &def.source_ids {
                        let s =
                            sources::fetch_window(&state.pool, *sid, window_start, now).await?;
                        all_samples.extend(s);
                    }
                    let pts = sources::formula_points(&all_samples);
                    super::formula::sku_on_shelf_compliance(
                        &pts,
                        def.window_seconds as i64,
                        now,
                    )
                    .map(|v| {
                        let cid = persist_computation_bg(
                            state.pool.clone(),
                            def_id,
                            v,
                            &all_samples,
                            window_start,
                            now,
                            None,
                            None,
                        );
                        terraops_shared::dto::metric::SeriesPoint {
                            at: now,
                            value: v,
                            computation_id: Some(cid),
                        }
                    })
                }
                _ => None,
            }
        } else {
            None
        };

    let mut points = stored;
    if let Some(p) = fresh_point {
        // Prepend fresh point (most recent first)
        points.insert(0, p);
    }

    Ok(HttpResponse::Ok().json(MetricSeriesResponse {
        definition_id: def_id,
        formula_kind: def.formula_kind,
        window_seconds: def.window_seconds,
        points,
    }))
}

/// Fire-and-forget: persist a computation result in the background.
///
/// The caller mints a fresh `Uuid::new_v4()` here so the returned
/// `computation_id` can be stamped on the live `SeriesPoint` **before** the
/// async INSERT finishes. The UI follows the same id to
/// `/metrics/computations/{id}/lineage` and reads back the complete input
/// observation list — including each `observation_id`, not just `observed_at`
/// + `value` — closing the audit #3 "why this value" lineage gap.
///
/// `alignment` and `confidence` are optional — moving_average and
/// rate_of_change pass `None`; comfort_index_ext passes the real values.
#[allow(clippy::too_many_arguments)]
fn persist_computation_bg(
    pool: sqlx::postgres::PgPool,
    definition_id: Uuid,
    result: f64,
    samples: &[sources::WindowSample],
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
    alignment: Option<f64>,
    confidence: Option<f64>,
) -> Uuid {
    let computation_id = Uuid::new_v4();
    let inputs: Value = serde_json::json!(samples
        .iter()
        .map(|s| json!({
            "observation_id": s.observation_id.to_string(),
            "observed_at": s.observed_at.to_rfc3339(),
            "value": s.value,
        }))
        .collect::<Vec<_>>());
    let ws = window_start;
    let we = window_end;
    let did = definition_id;
    let cid = computation_id;
    tokio::spawn(async move {
        let _ = definitions::save_computation(
            &pool, cid, did, result, inputs, ws, we, alignment, confidence,
        )
        .await;
    });
    computation_id
}

// ===========================================================================
// MD7 — GET /api/v1/metrics/computations/{id}/lineage
// ===========================================================================
async fn get_lineage(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "metric.read")?;
    let lin = lineage::get(&state.pool, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(lin))
}
