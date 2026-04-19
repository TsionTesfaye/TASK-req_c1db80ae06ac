//! P14 POST /products/export — CSV + XLSX streaming export.

use actix_web::{web, HttpResponse, Responder};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::AppResult,
    state::AppState,
};
use terraops_shared::dto::product::{ExportKind, ExportRequest};

#[derive(FromRow)]
struct ExportRow {
    id: Uuid,
    sku: String,
    name: String,
    description: Option<String>,
    on_shelf: bool,
    price_cents: i32,
    currency: String,
    category_name: Option<String>,
    brand_name: Option<String>,
    site_code: Option<String>,
    department_code: Option<String>,
    updated_at: DateTime<Utc>,
}

pub async fn export_products(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<ExportRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.read")?;
    let req = body.into_inner();

    let filter = req.filter.unwrap_or_default();
    let q_like = filter.q.as_ref().map(|v| format!("%{v}%"));

    let rows: Vec<ExportRow> = sqlx::query_as::<_, ExportRow>(
        "SELECT p.id, p.sku, p.name, p.description, p.on_shelf, p.price_cents, p.currency,
                c.name AS category_name, br.name AS brand_name,
                s.code AS site_code, d.code AS department_code, p.updated_at
         FROM products p
         LEFT JOIN categories c ON c.id = p.category_id
         LEFT JOIN brands br ON br.id = p.brand_id
         LEFT JOIN sites s ON s.id = p.site_id
         LEFT JOIN departments d ON d.id = p.department_id
         WHERE p.deleted_at IS NULL
           AND ($1::UUID IS NULL OR p.site_id = $1)
           AND ($2::UUID IS NULL OR p.department_id = $2)
           AND ($3::UUID IS NULL OR p.category_id = $3)
           AND ($4::UUID IS NULL OR p.brand_id = $4)
           AND ($5::BOOLEAN IS NULL OR p.on_shelf = $5)
           AND ($6::TEXT IS NULL OR p.name ILIKE $6 OR p.sku ILIKE $6)
         ORDER BY p.updated_at DESC",
    )
    .bind(filter.site_id)
    .bind(filter.department_id)
    .bind(filter.category_id)
    .bind(filter.brand_id)
    .bind(filter.on_shelf)
    .bind(q_like.as_deref())
    .fetch_all(&state.pool)
    .await?;

    match req.kind {
        ExportKind::Csv => export_csv(rows),
        ExportKind::Xlsx => export_xlsx(rows),
    }
}

fn export_csv(rows: Vec<ExportRow>) -> AppResult<HttpResponse> {
    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record([
        "id", "sku", "name", "description", "on_shelf", "price_cents",
        "currency", "category", "brand", "site", "department", "updated_at",
    ])
    .ok();
    for r in rows {
        wtr.write_record([
            r.id.to_string(),
            r.sku,
            r.name,
            r.description.unwrap_or_default(),
            r.on_shelf.to_string(),
            r.price_cents.to_string(),
            r.currency,
            r.category_name.unwrap_or_default(),
            r.brand_name.unwrap_or_default(),
            r.site_code.unwrap_or_default(),
            r.department_code.unwrap_or_default(),
            r.updated_at.to_rfc3339(),
        ])
        .ok();
    }
    let data = wtr.into_inner().unwrap_or_default();
    Ok(HttpResponse::Ok()
        .content_type("text/csv")
        .insert_header(("Content-Disposition", "attachment; filename=\"products.csv\""))
        .body(data))
}

fn export_xlsx(rows: Vec<ExportRow>) -> AppResult<HttpResponse> {
    use rust_xlsxwriter::Workbook;

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();

    let headers = [
        "ID", "SKU", "Name", "Description", "On Shelf", "Price (cents)",
        "Currency", "Category", "Brand", "Site", "Department", "Updated At",
    ];
    for (col, h) in headers.iter().enumerate() {
        ws.write_string(0, col as u16, *h).ok();
    }
    for (i, r) in rows.into_iter().enumerate() {
        let row = (i + 1) as u32;
        ws.write_string(row, 0, &r.id.to_string()).ok();
        ws.write_string(row, 1, &r.sku).ok();
        ws.write_string(row, 2, &r.name).ok();
        ws.write_string(row, 3, r.description.as_deref().unwrap_or("")).ok();
        ws.write_boolean(row, 4, r.on_shelf).ok();
        ws.write_number(row, 5, r.price_cents as f64).ok();
        ws.write_string(row, 6, &r.currency).ok();
        ws.write_string(row, 7, r.category_name.as_deref().unwrap_or("")).ok();
        ws.write_string(row, 8, r.brand_name.as_deref().unwrap_or("")).ok();
        ws.write_string(row, 9, r.site_code.as_deref().unwrap_or("")).ok();
        ws.write_string(row, 10, r.department_code.as_deref().unwrap_or("")).ok();
        ws.write_string(row, 11, &r.updated_at.to_rfc3339()).ok();
    }

    let data = wb.save_to_buffer().unwrap_or_default();
    Ok(HttpResponse::Ok()
        .content_type("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
        .insert_header(("Content-Disposition", "attachment; filename=\"products.xlsx\""))
        .body(data))
}
