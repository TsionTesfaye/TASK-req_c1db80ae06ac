//! Products HTTP handlers (P1–P7) + Export (P14) + Import mount (I1–I7).
//!
//! Route registration lives here. Delegate sub-handlers to dedicated modules.

use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::{AppError, AppResult},
    products::{history, import, images, repo, service, tax_rates, export},
    state::AppState,
};
use terraops_shared::{
    dto::product::{
        CreateProductRequest, ProductHistoryEntry, SetOnShelfRequest, UpdateProductRequest,
    },
    pagination::{Page, PageQuery},
};
use serde::Deserialize;

pub fn configure(cfg: &mut web::ServiceConfig) {
    // Products
    cfg.service(
        web::scope("/products")
            .route("", web::get().to(list_products))
            .route("", web::post().to(create_product))
            .route("/export", web::post().to(export::export_products))
            .route("/{id}", web::get().to(get_product))
            .route("/{id}", web::patch().to(update_product))
            .route("/{id}", web::delete().to(delete_product))
            .route("/{id}/status", web::post().to(set_status))
            .route("/{id}/history", web::get().to(get_history))
            .route("/{id}/tax-rates", web::post().to(tax_rates::add_tax_rate))
            .route("/{id}/tax-rates/{rid}", web::patch().to(tax_rates::update_tax_rate))
            .route("/{id}/tax-rates/{rid}", web::delete().to(tax_rates::delete_tax_rate))
            .route("/{id}/images", web::post().to(images::upload_image))
            .route("/{id}/images/{imgid}", web::delete().to(images::delete_image)),
    );

    // Images serve (signed URL) — outside /products scope
    cfg.route("/images/{imgid}", web::get().to(images::serve_image));

    // Imports
    cfg.service(
        web::scope("/imports")
            .route("", web::post().to(import::upload_import))
            .route("", web::get().to(import::list_imports))
            .route("/{id}", web::get().to(import::get_import))
            .route("/{id}/rows", web::get().to(import::list_import_rows))
            .route("/{id}/validate", web::post().to(import::validate_import))
            .route("/{id}/commit", web::post().to(import::commit_import))
            .route("/{id}/cancel", web::post().to(import::cancel_import)),
    );
}

// ---------------------------------------------------------------------------
// P1 GET /products
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ProductListQuery {
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    category_id: Option<Uuid>,
    brand_id: Option<Uuid>,
    on_shelf: Option<bool>,
    q: Option<String>,
    sort: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn list_products(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<ProductListQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.read")?;
    let q = q.into_inner();
    let pq = PageQuery { page: q.page, page_size: q.page_size }.resolved();

    let filter = terraops_shared::dto::product::ProductFilter {
        site_id: q.site_id,
        department_id: q.department_id,
        category_id: q.category_id,
        brand_id: q.brand_id,
        on_shelf: q.on_shelf,
        q: q.q,
    };

    let (items, total) = repo::list_products(
        &state.pool,
        &filter,
        pq.page,
        pq.page_size,
        q.sort.as_deref(),
    )
    .await?;

    let page = Page {
        items,
        page: pq.page,
        page_size: pq.page_size,
        total: total as u64,
    };
    Ok(HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.to_string()))
        .json(page))
}

// ---------------------------------------------------------------------------
// P2 POST /products
// ---------------------------------------------------------------------------

async fn create_product(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateProductRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.write")?;
    let req = body.into_inner();
    if req.sku.trim().is_empty() {
        return Err(AppError::Validation("sku required".into()));
    }
    if req.name.trim().is_empty() {
        return Err(AppError::Validation("name required".into()));
    }

    // Optional: validate shelf_life_days is non-negative when provided.
    if let Some(n) = req.shelf_life_days {
        if n < 0 {
            return Err(AppError::Validation("shelf_life_days must be >= 0".into()));
        }
    }
    // Audit #9 issue 3: validate price_cents >= 0 in the handler so we
    // return a user-safe 422 rather than leaking the DB CHECK-constraint
    // failure as a generic 500.
    if let Some(p) = req.price_cents {
        if p < 0 {
            return Err(AppError::Validation("price_cents must be >= 0".into()));
        }
    }

    let product_id = repo::insert_product(
        &state.pool,
        req.sku.trim(),
        req.spu.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        req.barcode.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        req.shelf_life_days,
        req.name.trim(),
        req.description.as_deref(),
        req.category_id,
        req.brand_id,
        req.unit_id,
        req.site_id,
        req.department_id,
        req.on_shelf.unwrap_or(true),
        req.price_cents.unwrap_or(0),
        req.currency.as_deref().unwrap_or("USD"),
        user.0.user_id,
    )
    .await?;

    let snap = repo::product_snapshot(&state.pool, product_id).await?;
    history::record(
        &state.pool,
        product_id,
        "create",
        user.0.user_id,
        None,
        Some(snap),
    )
    .await?;

    Ok(HttpResponse::Created().json(serde_json::json!({"id": product_id})))
}

// ---------------------------------------------------------------------------
// P3 GET /products/{id}
// ---------------------------------------------------------------------------

async fn get_product(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.read")?;
    let id = path.into_inner();
    let detail = repo::get_product_detail(&state.pool, id, &state.keys.image_hmac)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(HttpResponse::Ok().json(detail))
}

// ---------------------------------------------------------------------------
// P4 PATCH /products/{id}
// ---------------------------------------------------------------------------

async fn update_product(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateProductRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.write")?;
    let id = path.into_inner();
    if !repo::product_exists(&state.pool, id).await? {
        return Err(AppError::NotFound);
    }

    let before = repo::product_snapshot(&state.pool, id).await?;
    let req = body.into_inner();

    // Audit #9 issue 3: shelf_life_days / price_cents are validated in the
    // handler so negative values return a user-safe 422 instead of a
    // generic 500 from the DB CHECK.
    if let Some(Some(n)) = req.shelf_life_days {
        if n < 0 {
            return Err(AppError::Validation("shelf_life_days must be >= 0".into()));
        }
    }
    if let Some(p) = req.price_cents {
        if p < 0 {
            return Err(AppError::Validation("price_cents must be >= 0".into()));
        }
    }

    // Audit #9 issue 2: tri-state PATCH semantics for optional master-data
    // fields. For string-valued fields, an incoming `""` is treated as a
    // clear (same normalization as create) to avoid accidental empty
    // strings; explicit `null` also clears.
    let norm_str = |s: &String| -> Option<String> {
        let t = s.trim();
        if t.is_empty() { None } else { Some(t.to_string()) }
    };
    let spu_arg: Option<Option<String>> = req.spu.map(|v| v.and_then(|s| norm_str(&s)));
    let barcode_arg: Option<Option<String>> = req.barcode.map(|v| v.and_then(|s| norm_str(&s)));
    let description_arg: Option<Option<String>> =
        req.description.map(|v| v.and_then(|s| norm_str(&s)));

    let updated = repo::update_product_fields(
        &state.pool,
        id,
        req.sku.as_deref(),
        spu_arg.as_ref().map(|v| v.as_deref()),
        barcode_arg.as_ref().map(|v| v.as_deref()),
        req.shelf_life_days,
        req.name.as_deref(),
        description_arg.as_ref().map(|v| v.as_deref()),
        req.category_id,
        req.brand_id,
        req.unit_id,
        req.site_id,
        req.department_id,
        req.price_cents,
        req.currency.as_deref(),
        user.0.user_id,
    )
    .await?;

    if !updated {
        return Err(AppError::NotFound);
    }

    let after = repo::product_snapshot(&state.pool, id).await?;
    history::record(
        &state.pool,
        id,
        "update",
        user.0.user_id,
        Some(before),
        Some(after),
    )
    .await?;

    Ok(HttpResponse::NoContent().finish())
}

// ---------------------------------------------------------------------------
// P5 DELETE /products/{id}
// ---------------------------------------------------------------------------

async fn delete_product(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.write")?;
    let id = path.into_inner();
    let before = repo::product_snapshot(&state.pool, id).await?;
    let deleted = repo::soft_delete_product(&state.pool, id).await?;
    if !deleted {
        return Err(AppError::NotFound);
    }
    history::record(
        &state.pool,
        id,
        "delete",
        user.0.user_id,
        Some(before),
        None,
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

// ---------------------------------------------------------------------------
// P6 POST /products/{id}/status
// ---------------------------------------------------------------------------

async fn set_status(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<SetOnShelfRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.write")?;
    let id = path.into_inner();
    let on_shelf = body.into_inner().on_shelf;

    let updated = repo::set_on_shelf(&state.pool, id, on_shelf).await?;
    if !updated {
        return Err(AppError::NotFound);
    }

    history::record(
        &state.pool,
        id,
        "status",
        user.0.user_id,
        None,
        Some(serde_json::json!({"on_shelf": on_shelf})),
    )
    .await?;

    service::emit_status_changed(&state.pool, id, on_shelf, user.0.user_id).await?;

    Ok(HttpResponse::NoContent().finish())
}

// ---------------------------------------------------------------------------
// P7 GET /products/{id}/history
// ---------------------------------------------------------------------------

async fn get_history(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    q: web::Query<PageQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.history.read")?;
    let id = path.into_inner();
    if !repo::product_exists(&state.pool, id).await? {
        return Err(AppError::NotFound);
    }
    let r = q.into_inner().resolved();

    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        action: String,
        changed_by: Option<Uuid>,
        changed_by_name: Option<String>,
        changed_at: DateTime<Utc>,
        before_json: Option<serde_json::Value>,
        after_json: Option<serde_json::Value>,
    }

    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT h.id, h.action, h.changed_by, u.display_name AS changed_by_name,
                h.changed_at, h.before_json, h.after_json
         FROM product_history h
         LEFT JOIN users u ON u.id = h.changed_by
         WHERE h.product_id = $1
         ORDER BY h.changed_at DESC
         LIMIT $2 OFFSET $3",
    )
    .bind(id)
    .bind(r.limit() as i64)
    .bind(r.offset() as i64)
    .fetch_all(&state.pool)
    .await?;

    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::BIGINT FROM product_history WHERE product_id = $1")
            .bind(id)
            .fetch_one(&state.pool)
            .await?;

    let items: Vec<ProductHistoryEntry> = rows
        .into_iter()
        .map(|r| ProductHistoryEntry {
            id: r.id,
            product_id: id,
            action: r.action,
            changed_by: r.changed_by,
            changed_by_name: r.changed_by_name,
            changed_at: r.changed_at,
            before_json: r.before_json,
            after_json: r.after_json,
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
