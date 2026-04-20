//! Database access layer for products and related tables.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use terraops_shared::dto::product::{ProductDetail, ProductFilter, ProductListItem, ProductTaxRateDto, ProductImageDto};

// ---------------------------------------------------------------------------
// Internal DB row types
// ---------------------------------------------------------------------------

#[derive(FromRow)]
pub struct ProductRow {
    pub id: Uuid,
    pub sku: String,
    pub spu: Option<String>,
    pub barcode: Option<String>,
    pub shelf_life_days: Option<i32>,
    pub name: String,
    pub description: Option<String>,
    pub category_id: Option<Uuid>,
    pub brand_id: Option<Uuid>,
    pub unit_id: Option<Uuid>,
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub on_shelf: bool,
    pub price_cents: i32,
    pub currency: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// List products
// ---------------------------------------------------------------------------

pub async fn list_products(
    pool: &PgPool,
    filter: &ProductFilter,
    page: u32,
    page_size: u32,
    sort: Option<&str>,
) -> AppResult<(Vec<ProductListItem>, i64)> {
    let sort_col = match sort.unwrap_or("updated_at") {
        "sku" => "p.sku",
        "name" => "p.name",
        "price_cents" => "p.price_cents",
        "on_shelf" => "p.on_shelf",
        _ => "p.updated_at",
    };

    let q = format!(
        "SELECT p.id, p.sku, p.spu, p.barcode, p.shelf_life_days, p.name,
                c.name AS category_name, p.category_id,
                br.name AS brand_name, p.brand_id, p.on_shelf, p.price_cents, p.currency,
                p.site_id, p.department_id, p.updated_at
         FROM products p
         LEFT JOIN categories c ON c.id = p.category_id
         LEFT JOIN brands br ON br.id = p.brand_id
         WHERE p.deleted_at IS NULL
           AND ($1::UUID IS NULL OR p.site_id = $1)
           AND ($2::UUID IS NULL OR p.department_id = $2)
           AND ($3::UUID IS NULL OR p.category_id = $3)
           AND ($4::UUID IS NULL OR p.brand_id = $4)
           AND ($5::BOOLEAN IS NULL OR p.on_shelf = $5)
           AND ($6::TEXT IS NULL OR p.name ILIKE $6 OR p.sku ILIKE $6 OR p.barcode = $7 OR p.spu ILIKE $6)
         ORDER BY {sort_col} DESC
         LIMIT $8 OFFSET $9"
    );

    let q_like = filter.q.as_ref().map(|v| format!("%{v}%"));
    let q_exact = filter.q.as_ref().map(|v| v.trim().to_string());
    let offset = (page - 1) as i64 * page_size as i64;

    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        sku: String,
        spu: Option<String>,
        barcode: Option<String>,
        shelf_life_days: Option<i32>,
        name: String,
        category_name: Option<String>,
        category_id: Option<Uuid>,
        brand_name: Option<String>,
        brand_id: Option<Uuid>,
        on_shelf: bool,
        price_cents: i32,
        currency: String,
        site_id: Option<Uuid>,
        department_id: Option<Uuid>,
        updated_at: DateTime<Utc>,
    }

    let rows: Vec<Row> = sqlx::query_as::<_, Row>(&q)
        .bind(filter.site_id)
        .bind(filter.department_id)
        .bind(filter.category_id)
        .bind(filter.brand_id)
        .bind(filter.on_shelf)
        .bind(q_like.as_deref())
        .bind(q_exact.as_deref())
        .bind(page_size as i64)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let count_q = "SELECT COUNT(*)::BIGINT FROM products p
         WHERE p.deleted_at IS NULL
           AND ($1::UUID IS NULL OR p.site_id = $1)
           AND ($2::UUID IS NULL OR p.department_id = $2)
           AND ($3::UUID IS NULL OR p.category_id = $3)
           AND ($4::UUID IS NULL OR p.brand_id = $4)
           AND ($5::BOOLEAN IS NULL OR p.on_shelf = $5)
           AND ($6::TEXT IS NULL OR p.name ILIKE $6 OR p.sku ILIKE $6 OR p.barcode = $7 OR p.spu ILIKE $6)";

    let q_like2 = filter.q.as_ref().map(|v| format!("%{v}%"));
    let q_exact2 = filter.q.as_ref().map(|v| v.trim().to_string());
    let total: (i64,) = sqlx::query_as(count_q)
        .bind(filter.site_id)
        .bind(filter.department_id)
        .bind(filter.category_id)
        .bind(filter.brand_id)
        .bind(filter.on_shelf)
        .bind(q_like2.as_deref())
        .bind(q_exact2.as_deref())
        .fetch_one(pool)
        .await?;

    let items = rows
        .into_iter()
        .map(|r| ProductListItem {
            id: r.id,
            sku: r.sku,
            spu: r.spu,
            barcode: r.barcode,
            shelf_life_days: r.shelf_life_days,
            name: r.name,
            category_id: r.category_id,
            category_name: r.category_name,
            brand_id: r.brand_id,
            brand_name: r.brand_name,
            on_shelf: r.on_shelf,
            price_cents: r.price_cents,
            currency: r.currency,
            site_id: r.site_id,
            department_id: r.department_id,
            updated_at: r.updated_at,
        })
        .collect();

    Ok((items, total.0))
}

// ---------------------------------------------------------------------------
// Get single product (with tax rates + images)
// ---------------------------------------------------------------------------

pub async fn get_product_by_id(pool: &PgPool, id: Uuid) -> AppResult<Option<ProductRow>> {
    let row: Option<ProductRow> = sqlx::query_as::<_, ProductRow>(
        "SELECT id, sku, spu, barcode, shelf_life_days, name, description,
                category_id, brand_id, unit_id,
                site_id, department_id, on_shelf, price_cents, currency,
                created_at, updated_at, deleted_at, created_by, updated_by
         FROM products WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_product_detail(
    pool: &PgPool,
    id: Uuid,
    image_hmac_key: &[u8; 32],
) -> AppResult<Option<ProductDetail>> {
    let row = match get_product_by_id(pool, id).await? {
        Some(r) if r.deleted_at.is_none() => r,
        _ => return Ok(None),
    };

    // Joined names
    let cat_name: Option<(String,)> = if let Some(cid) = row.category_id {
        sqlx::query_as("SELECT name FROM categories WHERE id = $1")
            .bind(cid)
            .fetch_optional(pool)
            .await?
    } else {
        None
    };
    let brand_name: Option<(String,)> = if let Some(bid) = row.brand_id {
        sqlx::query_as("SELECT name FROM brands WHERE id = $1")
            .bind(bid)
            .fetch_optional(pool)
            .await?
    } else {
        None
    };
    let unit_code: Option<(String,)> = if let Some(uid) = row.unit_id {
        sqlx::query_as("SELECT code FROM units WHERE id = $1")
            .bind(uid)
            .fetch_optional(pool)
            .await?
    } else {
        None
    };
    let site_code: Option<(String,)> = if let Some(sid) = row.site_id {
        sqlx::query_as("SELECT code FROM sites WHERE id = $1")
            .bind(sid)
            .fetch_optional(pool)
            .await?
    } else {
        None
    };
    let dept_code: Option<(String,)> = if let Some(did) = row.department_id {
        sqlx::query_as("SELECT code FROM departments WHERE id = $1")
            .bind(did)
            .fetch_optional(pool)
            .await?
    } else {
        None
    };

    // Tax rates
    #[derive(FromRow)]
    struct TaxRow {
        id: Uuid,
        state_code: String,
        rate_bp: i32,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    }
    let tax_rows: Vec<TaxRow> = sqlx::query_as::<_, TaxRow>(
        "SELECT id, state_code, rate_bp, created_at, updated_at
         FROM product_tax_rates WHERE product_id = $1 ORDER BY state_code",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let tax_rates = tax_rows
        .into_iter()
        .map(|t| ProductTaxRateDto {
            id: t.id,
            product_id: id,
            state_code: t.state_code,
            rate_bp: t.rate_bp,
            created_at: t.created_at,
            updated_at: t.updated_at,
        })
        .collect();

    // Images with signed URLs
    #[derive(FromRow)]
    struct ImgRow {
        id: Uuid,
        storage_path: String,
        content_type: String,
        size_bytes: i32,
        uploaded_at: DateTime<Utc>,
        uploaded_by: Option<Uuid>,
    }
    let img_rows: Vec<ImgRow> = sqlx::query_as::<_, ImgRow>(
        "SELECT id, storage_path, content_type, size_bytes, uploaded_at, uploaded_by
         FROM product_images WHERE product_id = $1 ORDER BY uploaded_at",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    let images = img_rows
        .into_iter()
        .map(|img| {
            let path = format!("/api/v1/images/{}", img.id);
            let qs = crate::crypto::signed_url::sign(&path, 600, image_hmac_key);
            let signed_url = format!("{path}?{qs}");
            ProductImageDto {
                id: img.id,
                product_id: id,
                signed_url,
                content_type: img.content_type,
                size_bytes: img.size_bytes,
                uploaded_at: img.uploaded_at,
                uploaded_by: img.uploaded_by,
            }
        })
        .collect();

    Ok(Some(ProductDetail {
        id: row.id,
        sku: row.sku,
        spu: row.spu,
        barcode: row.barcode,
        shelf_life_days: row.shelf_life_days,
        name: row.name,
        description: row.description,
        category_id: row.category_id,
        category_name: cat_name.map(|(n,)| n),
        brand_id: row.brand_id,
        brand_name: brand_name.map(|(n,)| n),
        unit_id: row.unit_id,
        unit_code: unit_code.map(|(c,)| c),
        site_id: row.site_id,
        site_code: site_code.map(|(c,)| c),
        department_id: row.department_id,
        department_code: dept_code.map(|(c,)| c),
        on_shelf: row.on_shelf,
        price_cents: row.price_cents,
        currency: row.currency,
        tax_rates,
        images,
        created_at: row.created_at,
        updated_at: row.updated_at,
        created_by: row.created_by,
    }))
}

// ---------------------------------------------------------------------------
// Insert product
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub async fn insert_product(
    pool: &PgPool,
    sku: &str,
    spu: Option<&str>,
    barcode: Option<&str>,
    shelf_life_days: Option<i32>,
    name: &str,
    description: Option<&str>,
    category_id: Option<Uuid>,
    brand_id: Option<Uuid>,
    unit_id: Option<Uuid>,
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    on_shelf: bool,
    price_cents: i32,
    currency: &str,
    created_by: Uuid,
) -> AppResult<Uuid> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO products (sku, spu, barcode, shelf_life_days,
                               name, description, category_id, brand_id, unit_id,
                               site_id, department_id, on_shelf, price_cents, currency,
                               created_by, updated_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $15)
         RETURNING id",
    )
    .bind(sku)
    .bind(spu)
    .bind(barcode)
    .bind(shelf_life_days)
    .bind(name)
    .bind(description)
    .bind(category_id)
    .bind(brand_id)
    .bind(unit_id)
    .bind(site_id)
    .bind(department_id)
    .bind(on_shelf)
    .bind(price_cents)
    .bind(currency)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

// ---------------------------------------------------------------------------
// Update product
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub async fn update_product_fields(
    pool: &PgPool,
    id: Uuid,
    sku: Option<&str>,
    spu: Option<Option<&str>>,
    barcode: Option<Option<&str>>,
    shelf_life_days: Option<Option<i32>>,
    name: Option<&str>,
    description: Option<Option<&str>>,
    category_id: Option<Option<Uuid>>,
    brand_id: Option<Option<Uuid>>,
    unit_id: Option<Option<Uuid>>,
    site_id: Option<Option<Uuid>>,
    department_id: Option<Option<Uuid>>,
    price_cents: Option<i32>,
    currency: Option<&str>,
    updated_by: Uuid,
) -> AppResult<bool> {
    // Build set clauses dynamically
    let mut sets: Vec<String> = vec!["updated_at = NOW()".to_string(), format!("updated_by = '{updated_by}'")];

    if let Some(v) = sku { sets.push(format!("sku = '{}'", v.replace('\'', "''"))); }
    if let Some(v) = spu {
        match v {
            Some(s) => sets.push(format!("spu = '{}'", s.replace('\'', "''"))),
            None => sets.push("spu = NULL".to_string()),
        }
    }
    if let Some(v) = barcode {
        match v {
            Some(s) => sets.push(format!("barcode = '{}'", s.replace('\'', "''"))),
            None => sets.push("barcode = NULL".to_string()),
        }
    }
    if let Some(v) = shelf_life_days {
        match v {
            Some(n) => sets.push(format!("shelf_life_days = {n}")),
            None => sets.push("shelf_life_days = NULL".to_string()),
        }
    }
    if let Some(v) = name { sets.push(format!("name = '{}'", v.replace('\'', "''"))); }
    if let Some(v) = description {
        match v {
            Some(d) => sets.push(format!("description = '{}'", d.replace('\'', "''"))),
            None => sets.push("description = NULL".to_string()),
        }
    }
    if let Some(v) = category_id {
        match v {
            Some(id) => sets.push(format!("category_id = '{id}'")),
            None => sets.push("category_id = NULL".to_string()),
        }
    }
    if let Some(v) = brand_id {
        match v {
            Some(id) => sets.push(format!("brand_id = '{id}'")),
            None => sets.push("brand_id = NULL".to_string()),
        }
    }
    if let Some(v) = unit_id {
        match v {
            Some(id) => sets.push(format!("unit_id = '{id}'")),
            None => sets.push("unit_id = NULL".to_string()),
        }
    }
    if let Some(v) = site_id {
        match v {
            Some(id) => sets.push(format!("site_id = '{id}'")),
            None => sets.push("site_id = NULL".to_string()),
        }
    }
    if let Some(v) = department_id {
        match v {
            Some(id) => sets.push(format!("department_id = '{id}'")),
            None => sets.push("department_id = NULL".to_string()),
        }
    }
    if let Some(v) = price_cents { sets.push(format!("price_cents = {v}")); }
    if let Some(v) = currency { sets.push(format!("currency = '{}'", v.replace('\'', "''"))); }

    let sql = format!(
        "UPDATE products SET {} WHERE id = $1 AND deleted_at IS NULL",
        sets.join(", ")
    );
    let res = sqlx::query(&sql).bind(id).execute(pool).await?;
    Ok(res.rows_affected() > 0)
}

// ---------------------------------------------------------------------------
// Soft delete
// ---------------------------------------------------------------------------

pub async fn soft_delete_product(pool: &PgPool, id: Uuid) -> AppResult<bool> {
    let res = sqlx::query(
        "UPDATE products SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

// ---------------------------------------------------------------------------
// On-shelf toggle
// ---------------------------------------------------------------------------

pub async fn set_on_shelf(pool: &PgPool, id: Uuid, on_shelf: bool) -> AppResult<bool> {
    let res = sqlx::query(
        "UPDATE products SET on_shelf = $2, updated_at = NOW() \
         WHERE id = $1 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(on_shelf)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

// ---------------------------------------------------------------------------
// Get image row by ID (for serving)
// ---------------------------------------------------------------------------

#[derive(FromRow)]
pub struct ImageRow {
    pub id: Uuid,
    pub product_id: Uuid,
    pub storage_path: String,
    pub content_type: String,
    pub size_bytes: i32,
}

pub async fn get_image_row(pool: &PgPool, img_id: Uuid) -> AppResult<Option<ImageRow>> {
    let row: Option<ImageRow> = sqlx::query_as::<_, ImageRow>(
        "SELECT id, product_id, storage_path, content_type, size_bytes \
         FROM product_images WHERE id = $1",
    )
    .bind(img_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

// ---------------------------------------------------------------------------
// Check product exists (not deleted)
// ---------------------------------------------------------------------------

pub async fn product_exists(pool: &PgPool, id: Uuid) -> AppResult<bool> {
    let row: Option<(bool,)> =
        sqlx::query_as("SELECT TRUE FROM products WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row.is_some())
}

// ---------------------------------------------------------------------------
// Snapshot for history (before/after)
// ---------------------------------------------------------------------------

pub async fn product_snapshot(pool: &PgPool, id: Uuid) -> AppResult<serde_json::Value> {
    let row: Option<ProductRow> = get_product_by_id(pool, id).await?;
    match row {
        Some(r) => Ok(serde_json::json!({
            "sku": r.sku, "spu": r.spu, "barcode": r.barcode,
            "shelf_life_days": r.shelf_life_days,
            "name": r.name, "description": r.description,
            "on_shelf": r.on_shelf, "price_cents": r.price_cents,
            "currency": r.currency, "category_id": r.category_id,
            "brand_id": r.brand_id, "site_id": r.site_id,
            "department_id": r.department_id
        })),
        None => Err(AppError::NotFound),
    }
}
