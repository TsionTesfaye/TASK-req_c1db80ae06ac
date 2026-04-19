//! Product tax-rate handlers (P8–P10).

use actix_web::{web, HttpResponse, Responder};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::{AppError, AppResult},
    products::{history, repo},
    state::AppState,
};
use terraops_shared::dto::product::{CreateTaxRateRequest, ProductTaxRateDto, UpdateTaxRateRequest};

// ---------------------------------------------------------------------------
// P8 POST /products/{id}/tax-rates
// ---------------------------------------------------------------------------

pub async fn add_tax_rate(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<CreateTaxRateRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.write")?;
    let product_id = path.into_inner();
    if !repo::product_exists(&state.pool, product_id).await? {
        return Err(AppError::NotFound);
    }
    let req = body.into_inner();
    if req.rate_bp < 0 {
        return Err(AppError::Validation("rate_bp must be >= 0".into()));
    }
    // Verify state code is valid
    let valid: Option<(String,)> =
        sqlx::query_as("SELECT code FROM state_codes WHERE code = $1")
            .bind(&req.state_code)
            .fetch_optional(&state.pool)
            .await?;
    if valid.is_none() {
        return Err(AppError::Validation(format!(
            "unknown state_code '{}'",
            req.state_code
        )));
    }

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO product_tax_rates (product_id, state_code, rate_bp) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(product_id)
    .bind(&req.state_code)
    .bind(req.rate_bp)
    .fetch_one(&state.pool)
    .await?;

    history::record(
        &state.pool,
        product_id,
        "tax_rate",
        user.0.user_id,
        None,
        Some(serde_json::json!({"action":"add","state_code":&req.state_code,"rate_bp":req.rate_bp})),
    )
    .await?;

    Ok(HttpResponse::Created().json(serde_json::json!({"id": row.0})))
}

// ---------------------------------------------------------------------------
// P9 PATCH /products/{id}/tax-rates/{rid}
// ---------------------------------------------------------------------------

pub async fn update_tax_rate(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<(Uuid, Uuid)>,
    body: web::Json<UpdateTaxRateRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.write")?;
    let (product_id, rid) = path.into_inner();
    if !repo::product_exists(&state.pool, product_id).await? {
        return Err(AppError::NotFound);
    }
    let req = body.into_inner();
    let Some(rate_bp) = req.rate_bp else {
        return Ok(HttpResponse::NoContent().finish());
    };
    if rate_bp < 0 {
        return Err(AppError::Validation("rate_bp must be >= 0".into()));
    }
    let res = sqlx::query(
        "UPDATE product_tax_rates SET rate_bp = $3, updated_at = NOW() \
         WHERE id = $1 AND product_id = $2",
    )
    .bind(rid)
    .bind(product_id)
    .bind(rate_bp)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    history::record(
        &state.pool,
        product_id,
        "tax_rate",
        user.0.user_id,
        None,
        Some(serde_json::json!({"action":"update","rate_id":rid,"rate_bp":rate_bp})),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

// ---------------------------------------------------------------------------
// P10 DELETE /products/{id}/tax-rates/{rid}
// ---------------------------------------------------------------------------

pub async fn delete_tax_rate(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<(Uuid, Uuid)>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.write")?;
    let (product_id, rid) = path.into_inner();
    if !repo::product_exists(&state.pool, product_id).await? {
        return Err(AppError::NotFound);
    }
    let res = sqlx::query(
        "DELETE FROM product_tax_rates WHERE id = $1 AND product_id = $2",
    )
    .bind(rid)
    .bind(product_id)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    history::record(
        &state.pool,
        product_id,
        "tax_rate",
        user.0.user_id,
        None,
        Some(serde_json::json!({"action":"delete","rate_id":rid})),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

// ---------------------------------------------------------------------------
// List tax rates (used internally by get_product_detail)
// ---------------------------------------------------------------------------

pub async fn list_for_product(
    pool: &sqlx::PgPool,
    product_id: Uuid,
) -> crate::errors::AppResult<Vec<ProductTaxRateDto>> {
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        state_code: String,
        rate_bp: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT id, state_code, rate_bp, created_at, updated_at \
         FROM product_tax_rates WHERE product_id = $1 ORDER BY state_code",
    )
    .bind(product_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| ProductTaxRateDto {
            id: r.id,
            product_id,
            state_code: r.state_code,
            rate_bp: r.rate_bp,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect())
}
