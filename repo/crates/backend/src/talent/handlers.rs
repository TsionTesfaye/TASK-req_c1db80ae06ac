//! Talent Intelligence HTTP handlers (T1–T13).
//!
//!   T1  GET    /api/v1/talent/candidates         PERM(talent.read)
//!   T2  POST   /api/v1/talent/candidates         PERM(talent.manage)
//!   T3  GET    /api/v1/talent/candidates/{id}    PERM(talent.read)
//!   T4  GET    /api/v1/talent/roles              PERM(talent.read)
//!   T5  POST   /api/v1/talent/roles              PERM(talent.manage)
//!   T6  GET    /api/v1/talent/recommendations    PERM(talent.read) ?role_id=
//!   T7  GET    /api/v1/talent/weights            PERM(talent.read) + SELF
//!   T8  PUT    /api/v1/talent/weights            PERM(talent.read) + SELF
//!   T9  POST   /api/v1/talent/feedback           PERM(talent.feedback)
//!   T10 GET    /api/v1/talent/watchlists         PERM(talent.read) + SELF
//!   T11 POST   /api/v1/talent/watchlists         PERM(talent.read) + SELF
//!   T12 POST   /api/v1/talent/watchlists/{id}/items   PERM(talent.read) + SELF
//!   T13 DELETE /api/v1/talent/watchlists/{id}/items/{cid}  PERM(talent.read) + SELF
//!
//! Audit #6 Issue #1: weights + watchlists were previously only
//! authentication-gated. Every ordinary authenticated user could read
//! and mutate talent weights/watchlists even without the recruiter
//! permission bundle. T7/T8/T10-T13 now require `talent.read` before
//! the SELF-ownership check runs, which matches the recruiter role
//! boundary documented in `docs/design.md`.

use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use serde::Deserialize;
use terraops_shared::{
    dto::talent::{
        AddWatchlistItemRequest, CreateFeedbackRequest, CreateRoleRequest, CreateWatchlistRequest,
        UpdateWeightsRequest, UpsertCandidateRequest,
    },
    pagination::PageQuery,
};
use uuid::Uuid;

use crate::{
    auth::extractors::{require_any_permission, require_permission, AuthUser},
    errors::AppResult,
    state::AppState,
    talent::{
        candidates, feedback, roles_open,
        scoring::{
            score_blended, score_cold_start, BlendWeights, CandidateInputs, RoleInputs,
            COLD_START_THRESHOLD,
        },
        watchlists, weights,
    },
};

use terraops_shared::dto::talent::RankedCandidate;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/talent")
            // Candidates
            .route("/candidates", web::get().to(list_candidates))
            .route("/candidates", web::post().to(create_candidate))
            .route("/candidates/{id}", web::get().to(get_candidate))
            // Roles
            .route("/roles", web::get().to(list_roles))
            .route("/roles", web::post().to(create_role))
            // Recommendations
            .route("/recommendations", web::get().to(get_recommendations))
            // Weights (SELF)
            .route("/weights", web::get().to(get_weights))
            .route("/weights", web::put().to(put_weights))
            // Feedback (PERM talent.feedback)
            .route("/feedback", web::post().to(post_feedback))
            // Watchlists (SELF)
            .route("/watchlists", web::get().to(list_watchlists))
            .route("/watchlists", web::post().to(create_watchlist))
            .route("/watchlists/{id}/items", web::get().to(list_watchlist_items))
            .route("/watchlists/{id}/items", web::post().to(add_watchlist_item))
            .route(
                "/watchlists/{id}/items/{cid}",
                web::delete().to(remove_watchlist_item),
            ),
    );
}

// ── T1: GET /talent/candidates ───────────────────────────────────────────────

async fn list_candidates(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<crate::talent::search::CandidateQuery>,
) -> AppResult<impl Responder> {
    // Audit #8 Issue #1: `talent.manage` is a strict superset of
    // `talent.read` — Administrators hold manage but not read in the RBAC
    // seed, so accept either code here.
    require_any_permission(&user.0, &["talent.read", "talent.manage"])?;

    let query = q.into_inner();
    let (page, page_size) = query.resolved_page();
    let skills_filter = query.parsed_skills();
    let offset = ((page - 1) as i64) * (page_size as i64);
    let limit = page_size as i64;

    let (rows, total) = candidates::list(
        &state.pool,
        query.q.as_deref(),
        &skills_filter,
        query.min_years,
        query.location.as_deref(),
        query.major.as_deref(),
        query.min_education.as_deref(),
        query.availability.as_deref(),
        limit,
        offset,
    )
    .await?;

    let items: Vec<_> = rows
        .into_iter()
        .map(|r| terraops_shared::dto::talent::CandidateListItem::from(r))
        .collect();

    Ok(HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(items))
}

// ── T2: POST /talent/candidates ──────────────────────────────────────────────

async fn create_candidate(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<UpsertCandidateRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "talent.manage")?;
    let row = candidates::create(&state.pool, &body.into_inner()).await?;
    Ok(HttpResponse::Created()
        .json(terraops_shared::dto::talent::CandidateDetail::from(row)))
}

// ── T3: GET /talent/candidates/{id} ─────────────────────────────────────────

async fn get_candidate(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    // Audit #8 Issue #1: `talent.manage` is a strict superset of
    // `talent.read` — Administrators hold manage but not read in the RBAC
    // seed, so accept either code here.
    require_any_permission(&user.0, &["talent.read", "talent.manage"])?;
    let row = candidates::get_by_id(&state.pool, path.into_inner()).await?;
    Ok(HttpResponse::Ok()
        .json(terraops_shared::dto::talent::CandidateDetail::from(row)))
}

// ── T4: GET /talent/roles ────────────────────────────────────────────────────

/// Query parameters for T4 list/search/filter (audit #4 issue #5).
#[derive(Debug, Deserialize, Default)]
struct RoleListQuery {
    pub q: Option<String>,
    pub status: Option<String>,
    pub department_id: Option<Uuid>,
    pub site_id: Option<Uuid>,
    pub min_years: Option<i32>,
    /// Comma-separated list of skill tokens; match is "any-of".
    pub skills: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

async fn list_roles(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<RoleListQuery>,
) -> AppResult<impl Responder> {
    // Audit #8 Issue #1: `talent.manage` is a strict superset of
    // `talent.read` — Administrators hold manage but not read in the RBAC
    // seed, so accept either code here.
    require_any_permission(&user.0, &["talent.read", "talent.manage"])?;
    let q = q.into_inner();
    let r = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    let skills: Vec<String> = q
        .skills
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if let Some(ref s) = q.status {
        if !matches!(s.as_str(), "open" | "closed" | "filled") {
            return Err(crate::errors::AppError::Validation(
                "status must be 'open', 'closed', or 'filled'".into(),
            ));
        }
    }
    let filter = crate::talent::roles_open::RoleFilter {
        q: q.q,
        status: q.status,
        department_id: q.department_id,
        site_id: q.site_id,
        min_years: q.min_years,
        skills_any: skills,
    };
    let (rows, total) =
        roles_open::list_filtered(&state.pool, &filter, r.limit() as i64, r.offset() as i64)
            .await?;
    let items: Vec<_> = rows
        .into_iter()
        .map(terraops_shared::dto::talent::RoleOpenItem::from)
        .collect();
    Ok(HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(items))
}

// ── T5: POST /talent/roles ───────────────────────────────────────────────────

async fn create_role(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateRoleRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "talent.manage")?;
    let row = roles_open::create(&state.pool, &body.into_inner(), user.0.user_id).await?;
    Ok(HttpResponse::Created()
        .json(terraops_shared::dto::talent::RoleOpenItem::from(row)))
}

// ── T6: GET /talent/recommendations ─────────────────────────────────────────

#[derive(Deserialize)]
struct RecoQuery {
    role_id: Uuid,
}

async fn get_recommendations(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<RecoQuery>,
) -> AppResult<impl Responder> {
    // Audit #8 Issue #1: `talent.manage` is a strict superset of
    // `talent.read` — Administrators hold manage but not read in the RBAC
    // seed, so accept either code here.
    require_any_permission(&user.0, &["talent.read", "talent.manage"])?;

    let role_id = q.into_inner().role_id;
    let role = roles_open::get_by_id(&state.pool, role_id).await?;
    let total_feedback = feedback::count_total(&state.pool).await?;
    let cold_start = total_feedback < COLD_START_THRESHOLD;

    // Load ALL non-deleted candidates for scoring (bounded by 200 for perf).
    let (all_candidates, _) = candidates::list(
        &state.pool,
        None,
        &[],
        None,
        None,
        None,
        None,
        None,
        200,
        0,
    )
    .await?;

    // Load caller weights (falls back to defaults if none stored).
    let w = weights::get(&state.pool, user.0.user_id).await?;
    let blend_weights = BlendWeights {
        skills: w.skills_weight,
        experience: w.experience_weight,
        recency: w.recency_weight,
        completeness: w.completeness_weight,
    };

    let now = Utc::now();

    let mut ranked: Vec<RankedCandidate> = all_candidates
        .into_iter()
        .map(|c| {
            let days = (now - c.last_active_at).num_seconds() as f64 / 86400.0;
            let inp = CandidateInputs {
                skills: &c.skills,
                years_experience: c.years_experience,
                days_since_last_active: days.max(0.0),
                completeness_raw: c.completeness_score,
                major: c.major.as_deref(),
                education: c.education.as_deref(),
                availability: c.availability.as_deref(),
            };

            let scored = if cold_start {
                score_cold_start(&inp, total_feedback)
            } else {
                let ri = RoleInputs {
                    required_skills: &role.required_skills,
                    min_years: role.min_years,
                    required_major: role.required_major.as_deref(),
                    min_education: role.min_education.as_deref(),
                    required_availability: role.required_availability.as_deref(),
                };
                score_blended(&inp, &ri, &blend_weights)
            };

            let candidate_dto = terraops_shared::dto::talent::CandidateListItem::from(c);
            RankedCandidate {
                candidate: candidate_dto,
                score: scored.score,
                reasons: scored.reasons,
            }
        })
        .collect();

    // Sort descending by score.
    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    let result = terraops_shared::dto::talent::RecommendationResult {
        cold_start,
        total_feedback,
        role_id,
        candidates: ranked,
    };

    Ok(HttpResponse::Ok().json(result))
}

// ── T7: GET /talent/weights ──────────────────────────────────────────────────

async fn get_weights(
    user: AuthUser,
    state: web::Data<AppState>,
) -> AppResult<impl Responder> {
    // Audit #8 Issue #1: `talent.manage` is a strict superset of
    // `talent.read` — Administrators hold manage but not read in the RBAC
    // seed, so accept either code here.
    require_any_permission(&user.0, &["talent.read", "talent.manage"])?;
    let w = weights::get(&state.pool, user.0.user_id).await?;
    Ok(HttpResponse::Ok().json(w))
}

// ── T8: PUT /talent/weights ──────────────────────────────────────────────────

async fn put_weights(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<UpdateWeightsRequest>,
) -> AppResult<impl Responder> {
    // Audit #8 Issue #1: `talent.manage` is a strict superset of
    // `talent.read` — Administrators hold manage but not read in the RBAC
    // seed, so accept either code here.
    require_any_permission(&user.0, &["talent.read", "talent.manage"])?;
    let w = weights::upsert(&state.pool, user.0.user_id, &body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(w))
}

// ── T9: POST /talent/feedback ────────────────────────────────────────────────

async fn post_feedback(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateFeedbackRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "talent.feedback")?;
    let rec = feedback::create(&state.pool, &body.into_inner(), user.0.user_id).await?;
    Ok(HttpResponse::Created().json(rec))
}

// ── T10: GET /talent/watchlists ──────────────────────────────────────────────

async fn list_watchlists(
    user: AuthUser,
    state: web::Data<AppState>,
) -> AppResult<impl Responder> {
    // Audit #8 Issue #1: `talent.manage` is a strict superset of
    // `talent.read` — Administrators hold manage but not read in the RBAC
    // seed, so accept either code here.
    require_any_permission(&user.0, &["talent.read", "talent.manage"])?;
    let items = watchlists::list(&state.pool, user.0.user_id).await?;
    Ok(HttpResponse::Ok().json(items))
}

// ── T11: POST /talent/watchlists ─────────────────────────────────────────────

async fn create_watchlist(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateWatchlistRequest>,
) -> AppResult<impl Responder> {
    // Audit #8 Issue #1: `talent.manage` is a strict superset of
    // `talent.read` — Administrators hold manage but not read in the RBAC
    // seed, so accept either code here.
    require_any_permission(&user.0, &["talent.read", "talent.manage"])?;
    let item = watchlists::create(&state.pool, user.0.user_id, &body.into_inner().name).await?;
    Ok(HttpResponse::Created().json(item))
}

// ── GET /talent/watchlists/{id}/items (helper for T12 page load) ─────────────

async fn list_watchlist_items(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    // Audit #8 Issue #1: `talent.manage` is a strict superset of
    // `talent.read` — Administrators hold manage but not read in the RBAC
    // seed, so accept either code here.
    require_any_permission(&user.0, &["talent.read", "talent.manage"])?;
    let watchlist_id = path.into_inner();
    watchlists::assert_owner(&state.pool, watchlist_id, user.0.user_id).await?;
    let entries = watchlists::list_items(&state.pool, watchlist_id).await?;
    Ok(HttpResponse::Ok().json(entries))
}

// ── T12: POST /talent/watchlists/{id}/items ──────────────────────────────────

async fn add_watchlist_item(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<AddWatchlistItemRequest>,
) -> AppResult<impl Responder> {
    // Audit #8 Issue #1: `talent.manage` is a strict superset of
    // `talent.read` — Administrators hold manage but not read in the RBAC
    // seed, so accept either code here.
    require_any_permission(&user.0, &["talent.read", "talent.manage"])?;
    let watchlist_id = path.into_inner();
    watchlists::assert_owner(&state.pool, watchlist_id, user.0.user_id).await?;
    watchlists::add_item(&state.pool, watchlist_id, body.into_inner().candidate_id).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ── T13: DELETE /talent/watchlists/{id}/items/{cid} ──────────────────────────

async fn remove_watchlist_item(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<(Uuid, Uuid)>,
) -> AppResult<impl Responder> {
    // Audit #8 Issue #1: `talent.manage` is a strict superset of
    // `talent.read` — Administrators hold manage but not read in the RBAC
    // seed, so accept either code here.
    require_any_permission(&user.0, &["talent.read", "talent.manage"])?;
    let (watchlist_id, candidate_id) = path.into_inner();
    watchlists::assert_owner(&state.pool, watchlist_id, user.0.user_id).await?;
    watchlists::remove_item(&state.pool, watchlist_id, candidate_id).await?;
    Ok(HttpResponse::NoContent().finish())
}
