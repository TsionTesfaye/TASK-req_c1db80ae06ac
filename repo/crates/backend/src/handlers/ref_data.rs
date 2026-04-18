//! Reference-data endpoints REF1–REF9.
//!
//!   REF1 GET  /api/v1/ref/sites
//!   REF2 GET  /api/v1/ref/departments?site=<uuid>
//!   REF3 GET  /api/v1/ref/categories
//!   REF4 POST /api/v1/ref/categories          (ref.write)
//!   REF5 GET  /api/v1/ref/brands
//!   REF6 POST /api/v1/ref/brands              (ref.write)
//!   REF7 GET  /api/v1/ref/units
//!   REF8 POST /api/v1/ref/units               (ref.write)
//!   REF9 GET  /api/v1/ref/states

use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use sqlx::FromRow;
use terraops_shared::dto::ref_data::{
    BrandRef, CategoryRef, CreateBrand, CreateCategory, CreateUnit, DepartmentRef, SiteRef,
    StateRef, UnitRef,
};
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::AppResult,
    services::audit as audit_svc,
    state::AppState,
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/ref")
            .route("/sites", web::get().to(list_sites))
            .route("/departments", web::get().to(list_departments))
            .route("/categories", web::get().to(list_categories))
            .route("/categories", web::post().to(create_category))
            .route("/brands", web::get().to(list_brands))
            .route("/brands", web::post().to(create_brand))
            .route("/units", web::get().to(list_units))
            .route("/units", web::post().to(create_unit))
            .route("/states", web::get().to(list_states)),
    );
}

async fn list_sites(_user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        code: String,
        name: String,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>("SELECT id, code, name FROM sites ORDER BY code")
        .fetch_all(&state.pool)
        .await?;
    Ok(HttpResponse::Ok().json(
        rows.into_iter()
            .map(|r| SiteRef {
                id: r.id,
                code: r.code,
                name: r.name,
            })
            .collect::<Vec<_>>(),
    ))
}

#[derive(Deserialize)]
struct DeptQuery {
    site: Option<Uuid>,
}

async fn list_departments(
    _user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<DeptQuery>,
) -> AppResult<impl Responder> {
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        site_id: Uuid,
        code: String,
        name: String,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT id, site_id, code, name FROM departments \
         WHERE ($1::UUID IS NULL OR site_id = $1) ORDER BY code",
    )
    .bind(q.site)
    .fetch_all(&state.pool)
    .await?;
    Ok(HttpResponse::Ok().json(
        rows.into_iter()
            .map(|r| DepartmentRef {
                id: r.id,
                site_id: r.site_id,
                code: r.code,
                name: r.name,
            })
            .collect::<Vec<_>>(),
    ))
}

async fn list_categories(_user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        parent_id: Option<Uuid>,
        name: String,
    }
    let rows: Vec<Row> =
        sqlx::query_as::<_, Row>("SELECT id, parent_id, name FROM categories ORDER BY name")
            .fetch_all(&state.pool)
            .await?;
    Ok(HttpResponse::Ok().json(
        rows.into_iter()
            .map(|r| CategoryRef {
                id: r.id,
                parent_id: r.parent_id,
                name: r.name,
            })
            .collect::<Vec<_>>(),
    ))
}

async fn create_category(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateCategory>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "ref.write")?;
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO categories (parent_id, name) VALUES ($1, $2) RETURNING id",
    )
    .bind(body.parent_id)
    .bind(&body.name)
    .fetch_one(&state.pool)
    .await?;
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "ref.category.create",
        Some("category"),
        Some(&row.0.to_string()),
        json!({"name": body.name}),
    )
    .await?;
    Ok(HttpResponse::Created().json(json!({"id": row.0})))
}

async fn list_brands(_user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        name: String,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>("SELECT id, name FROM brands ORDER BY name")
        .fetch_all(&state.pool)
        .await?;
    Ok(HttpResponse::Ok().json(
        rows.into_iter()
            .map(|r| BrandRef {
                id: r.id,
                name: r.name,
            })
            .collect::<Vec<_>>(),
    ))
}

async fn create_brand(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateBrand>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "ref.write")?;
    let row: (Uuid,) = sqlx::query_as("INSERT INTO brands (name) VALUES ($1) RETURNING id")
        .bind(&body.name)
        .fetch_one(&state.pool)
        .await?;
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "ref.brand.create",
        Some("brand"),
        Some(&row.0.to_string()),
        json!({"name": body.name}),
    )
    .await?;
    Ok(HttpResponse::Created().json(json!({"id": row.0})))
}

async fn list_units(_user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        code: String,
        description: Option<String>,
    }
    let rows: Vec<Row> =
        sqlx::query_as::<_, Row>("SELECT id, code, description FROM units ORDER BY code")
            .fetch_all(&state.pool)
            .await?;
    Ok(HttpResponse::Ok().json(
        rows.into_iter()
            .map(|r| UnitRef {
                id: r.id,
                code: r.code,
                description: r.description,
            })
            .collect::<Vec<_>>(),
    ))
}

async fn create_unit(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateUnit>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "ref.write")?;
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO units (code, description) VALUES ($1, $2) RETURNING id",
    )
    .bind(&body.code)
    .bind(body.description.as_deref())
    .fetch_one(&state.pool)
    .await?;
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "ref.unit.create",
        Some("unit"),
        Some(&row.0.to_string()),
        json!({"code": body.code}),
    )
    .await?;
    Ok(HttpResponse::Created().json(json!({"id": row.0})))
}

async fn list_states(_user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT code, name FROM state_codes ORDER BY code")
            .fetch_all(&state.pool)
            .await?;
    Ok(HttpResponse::Ok().json(
        rows.into_iter()
            .map(|(code, name)| StateRef { code, name })
            .collect::<Vec<_>>(),
    ))
}
