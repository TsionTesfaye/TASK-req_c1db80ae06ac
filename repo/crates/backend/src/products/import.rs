//! Import pipeline handlers (I1–I7).
//!
//!   I1 POST /imports                       — upload + parse
//!   I2 GET  /imports                       — list batches
//!   I3 GET  /imports/{id}                  — batch summary
//!   I4 GET  /imports/{id}/rows             — paginated rows
//!   I5 POST /imports/{id}/validate         — re-run validation
//!   I6 POST /imports/{id}/commit           — commit (0-error gate)
//!   I7 POST /imports/{id}/cancel           — cancel

use actix_multipart::Multipart;
use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;
use sqlx::FromRow;
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::{AppError, AppResult},
    products::{import_validator, service},
    state::AppState,
};
use terraops_shared::{
    dto::import::{ImportBatchSummary, ImportRowDto},
    pagination::{Page, PageQuery},
};

// ---------------------------------------------------------------------------
// I1 POST /imports — upload + parse
// ---------------------------------------------------------------------------

pub async fn upload_import(
    user: AuthUser,
    state: web::Data<AppState>,
    mut payload: Multipart,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.import")?;

    let mut filename = String::from("upload");
    let mut raw_bytes: Vec<u8> = Vec::new();

    if let Some(mut field) = payload.try_next().await.map_err(|e| {
        AppError::Validation(format!("multipart error: {e}"))
    })? {
        if let Some(cd) = field.content_disposition() {
            if let Some(name) = cd.get_filename() {
                filename = name.to_string();
            }
        }
        while let Some(chunk) = field.try_next().await.map_err(|e| {
            AppError::Internal(format!("read chunk: {e}"))
        })? {
            raw_bytes.extend_from_slice(&chunk);
        }
    } else {
        return Err(AppError::Validation("no file field".into()));
    }

    let kind = if filename.to_lowercase().ends_with(".xlsx") { "xlsx" } else { "csv" };

    // Parse rows into JSON objects
    let parsed = if kind == "xlsx" {
        parse_xlsx(&raw_bytes)?
    } else {
        parse_csv(&raw_bytes)?
    };

    let row_count = parsed.len() as i32;

    // Create batch
    let batch_id: (Uuid,) = sqlx::query_as(
        "INSERT INTO import_batches (uploaded_by, filename, kind, row_count) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(user.0.user_id)
    .bind(&filename)
    .bind(kind)
    .bind(row_count)
    .fetch_one(&state.pool)
    .await?;
    let batch_id = batch_id.0;

    // Insert rows and validate
    let mut error_count = 0i32;
    for (idx, raw) in parsed.iter().enumerate() {
        let errors = import_validator::validate_row(raw);
        let valid = errors.is_empty();
        if !valid {
            error_count += 1;
        }
        let errors_json = serde_json::Value::Array(
            errors.iter().map(|e| serde_json::Value::String(e.clone())).collect(),
        );
        sqlx::query(
            "INSERT INTO import_rows (batch_id, row_number, raw, errors, valid) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(batch_id)
        .bind((idx + 1) as i32)
        .bind(raw)
        .bind(&errors_json)
        .bind(valid)
        .execute(&state.pool)
        .await?;
    }

    // Update error_count + status
    let status = if error_count == 0 { "validated" } else { "uploaded" };
    sqlx::query(
        "UPDATE import_batches SET error_count = $2, status = $3 WHERE id = $1",
    )
    .bind(batch_id)
    .bind(error_count)
    .bind(status)
    .execute(&state.pool)
    .await?;

    Ok(HttpResponse::Created().json(serde_json::json!({
        "id": batch_id,
        "row_count": row_count,
        "error_count": error_count,
        "status": status,
    })))
}

// ---------------------------------------------------------------------------
// I2 GET /imports — list batches (scoped to caller; admin sees all)
// ---------------------------------------------------------------------------

pub async fn list_imports(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<PageQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.import")?;
    let r = q.into_inner().resolved();

    let is_admin = user.0.has_permission("user.manage");

    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        uploaded_by: Uuid,
        filename: String,
        kind: String,
        status: String,
        row_count: i32,
        error_count: i32,
        created_at: DateTime<Utc>,
        committed_at: Option<DateTime<Utc>>,
        cancelled_at: Option<DateTime<Utc>>,
    }

    let rows: Vec<Row> = if is_admin {
        sqlx::query_as::<_, Row>(
            "SELECT id, uploaded_by, filename, kind, status, row_count, error_count,
                    created_at, committed_at, cancelled_at
             FROM import_batches
             ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        )
        .bind(r.limit() as i64)
        .bind(r.offset() as i64)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, Row>(
            "SELECT id, uploaded_by, filename, kind, status, row_count, error_count,
                    created_at, committed_at, cancelled_at
             FROM import_batches
             WHERE uploaded_by = $1
             ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user.0.user_id)
        .bind(r.limit() as i64)
        .bind(r.offset() as i64)
        .fetch_all(&state.pool)
        .await?
    };

    let total: (i64,) = if is_admin {
        sqlx::query_as("SELECT COUNT(*)::BIGINT FROM import_batches")
            .fetch_one(&state.pool)
            .await?
    } else {
        sqlx::query_as("SELECT COUNT(*)::BIGINT FROM import_batches WHERE uploaded_by = $1")
            .bind(user.0.user_id)
            .fetch_one(&state.pool)
            .await?
    };

    let items: Vec<ImportBatchSummary> = rows
        .into_iter()
        .map(|r| ImportBatchSummary {
            id: r.id,
            uploaded_by: r.uploaded_by,
            filename: r.filename,
            kind: r.kind,
            status: r.status,
            row_count: r.row_count,
            error_count: r.error_count,
            created_at: r.created_at,
            committed_at: r.committed_at,
            cancelled_at: r.cancelled_at,
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

// ---------------------------------------------------------------------------
// I3 GET /imports/{id} — batch summary
// ---------------------------------------------------------------------------

pub async fn get_import(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.import")?;
    let id = path.into_inner();
    let batch = get_batch_scoped(&state, id, &user).await?;
    Ok(HttpResponse::Ok().json(batch))
}

// ---------------------------------------------------------------------------
// I4 GET /imports/{id}/rows — paginated rows
// ---------------------------------------------------------------------------

pub async fn list_import_rows(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    q: web::Query<PageQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.import")?;
    let id = path.into_inner();
    // Ownership check
    get_batch_scoped(&state, id, &user).await?;

    let r = q.into_inner().resolved();

    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        batch_id: Uuid,
        row_number: i32,
        raw: serde_json::Value,
        errors: serde_json::Value,
        valid: bool,
    }

    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT id, batch_id, row_number, raw, errors, valid
         FROM import_rows WHERE batch_id = $1
         ORDER BY row_number LIMIT $2 OFFSET $3",
    )
    .bind(id)
    .bind(r.limit() as i64)
    .bind(r.offset() as i64)
    .fetch_all(&state.pool)
    .await?;

    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::BIGINT FROM import_rows WHERE batch_id = $1")
            .bind(id)
            .fetch_one(&state.pool)
            .await?;

    let items: Vec<ImportRowDto> = rows
        .into_iter()
        .map(|r| ImportRowDto {
            id: r.id,
            batch_id: r.batch_id,
            row_number: r.row_number,
            raw: r.raw,
            errors: r.errors,
            valid: r.valid,
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

// ---------------------------------------------------------------------------
// I5 POST /imports/{id}/validate — re-run validation
// ---------------------------------------------------------------------------

pub async fn validate_import(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.import")?;
    let id = path.into_inner();
    let batch = get_batch_scoped(&state, id, &user).await?;
    if batch.status == "committed" || batch.status == "cancelled" {
        return Err(AppError::Validation(format!(
            "cannot validate a {} batch",
            batch.status
        )));
    }

    // Re-fetch rows and re-validate
    #[derive(FromRow)]
    struct RowFetch {
        id: Uuid,
        raw: serde_json::Value,
    }
    let rows: Vec<RowFetch> = sqlx::query_as::<_, RowFetch>(
        "SELECT id, raw FROM import_rows WHERE batch_id = $1 ORDER BY row_number",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    let mut error_count = 0i32;
    for row in &rows {
        let errors = import_validator::validate_row(&row.raw);
        let valid = errors.is_empty();
        if !valid { error_count += 1; }
        let errors_json = serde_json::Value::Array(
            errors.iter().map(|e| serde_json::Value::String(e.clone())).collect(),
        );
        sqlx::query(
            "UPDATE import_rows SET errors = $2, valid = $3 WHERE id = $1",
        )
        .bind(row.id)
        .bind(&errors_json)
        .bind(valid)
        .execute(&state.pool)
        .await?;
    }

    let status = if error_count == 0 { "validated" } else { "uploaded" };
    sqlx::query(
        "UPDATE import_batches SET error_count = $2, status = $3 WHERE id = $1",
    )
    .bind(id)
    .bind(error_count)
    .bind(status)
    .execute(&state.pool)
    .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "id": id,
        "error_count": error_count,
        "status": status,
    })))
}

// ---------------------------------------------------------------------------
// I6 POST /imports/{id}/commit — single-tx commit
// ---------------------------------------------------------------------------

pub async fn commit_import(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.import")?;
    let id = path.into_inner();
    let batch = get_batch_scoped(&state, id, &user).await?;

    if batch.status != "validated" {
        return Err(AppError::Validation(format!(
            "batch must be in 'validated' status to commit (current: {})",
            batch.status
        )));
    }
    if batch.error_count > 0 {
        return Err(AppError::Validation(format!(
            "batch has {} validation errors; fix them before committing",
            batch.error_count
        )));
    }

    // Fetch all rows
    #[derive(FromRow)]
    struct RowFetch {
        raw: serde_json::Value,
    }
    let rows: Vec<RowFetch> = sqlx::query_as::<_, RowFetch>(
        "SELECT raw FROM import_rows WHERE batch_id = $1 ORDER BY row_number",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    let mut tx = state.pool.begin().await?;
    let mut inserted = 0i32;

    for row in &rows {
        let (sku, spu, barcode, shelf_life_days, name, on_shelf, price_cents, currency) =
            import_validator::to_product_fields(&row.raw);

        let product_id: (Uuid,) = sqlx::query_as(
            "INSERT INTO products (sku, spu, barcode, shelf_life_days,
                                   name, on_shelf, price_cents, currency,
                                   created_by, updated_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
             ON CONFLICT (sku) DO UPDATE SET
                spu = EXCLUDED.spu,
                barcode = EXCLUDED.barcode,
                shelf_life_days = EXCLUDED.shelf_life_days,
                name = EXCLUDED.name, on_shelf = EXCLUDED.on_shelf,
                price_cents = EXCLUDED.price_cents, currency = EXCLUDED.currency,
                updated_by = EXCLUDED.updated_by, updated_at = NOW()
             RETURNING id",
        )
        .bind(&sku)
        .bind(spu.as_deref())
        .bind(barcode.as_deref())
        .bind(shelf_life_days)
        .bind(&name)
        .bind(on_shelf)
        .bind(price_cents)
        .bind(&currency)
        .bind(user.0.user_id)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO product_history (product_id, action, changed_by, after_json)
             VALUES ($1, 'create', $2, $3)",
        )
        .bind(product_id.0)
        .bind(user.0.user_id)
        .bind(&row.raw)
        .execute(&mut *tx)
        .await?;

        inserted += 1;
    }

    sqlx::query(
        "UPDATE import_batches SET status = 'committed', committed_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Emit notification (best-effort)
    let _ = service::emit_import_committed(&state.pool, id, inserted, batch.uploaded_by).await;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "id": id,
        "inserted": inserted,
        "status": "committed",
    })))
}

// ---------------------------------------------------------------------------
// I7 POST /imports/{id}/cancel
// ---------------------------------------------------------------------------

pub async fn cancel_import(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.import")?;
    let id = path.into_inner();
    let batch = get_batch_scoped(&state, id, &user).await?;

    if batch.status == "committed" || batch.status == "cancelled" {
        return Err(AppError::Validation(format!(
            "cannot cancel a {} batch",
            batch.status
        )));
    }

    sqlx::query(
        "UPDATE import_batches SET status = 'cancelled', cancelled_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({"id": id, "status": "cancelled"})))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn get_batch_scoped(
    state: &AppState,
    id: Uuid,
    user: &AuthUser,
) -> AppResult<ImportBatchSummary> {
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        uploaded_by: Uuid,
        filename: String,
        kind: String,
        status: String,
        row_count: i32,
        error_count: i32,
        created_at: DateTime<Utc>,
        committed_at: Option<DateTime<Utc>>,
        cancelled_at: Option<DateTime<Utc>>,
    }
    let row: Option<Row> = sqlx::query_as::<_, Row>(
        "SELECT id, uploaded_by, filename, kind, status, row_count, error_count,
                created_at, committed_at, cancelled_at
         FROM import_batches WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    let row = row.ok_or(AppError::NotFound)?;

    // Object-level auth: only the uploader or a user.manage user may access
    if row.uploaded_by != user.0.user_id && !user.0.has_permission("user.manage") {
        return Err(AppError::Forbidden("not batch owner"));
    }

    Ok(ImportBatchSummary {
        id: row.id,
        uploaded_by: row.uploaded_by,
        filename: row.filename,
        kind: row.kind,
        status: row.status,
        row_count: row.row_count,
        error_count: row.error_count,
        created_at: row.created_at,
        committed_at: row.committed_at,
        cancelled_at: row.cancelled_at,
    })
}

// ---------------------------------------------------------------------------
// CSV parser
// ---------------------------------------------------------------------------

fn parse_csv(data: &[u8]) -> AppResult<Vec<serde_json::Value>> {
    let mut rdr = csv::Reader::from_reader(data);
    let headers = rdr
        .headers()
        .map_err(|e| AppError::Validation(format!("CSV headers: {e}")))?
        .clone();
    let mut out = Vec::new();
    for result in rdr.records() {
        let record = result.map_err(|e| AppError::Validation(format!("CSV row: {e}")))?;
        let mut map = serde_json::Map::new();
        for (h, v) in headers.iter().zip(record.iter()) {
            map.insert(h.to_string(), serde_json::Value::String(v.to_string()));
        }
        out.push(serde_json::Value::Object(map));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// XLSX parser
// ---------------------------------------------------------------------------

fn parse_xlsx(data: &[u8]) -> AppResult<Vec<serde_json::Value>> {
    use calamine::{open_workbook_from_rs, Reader, Xlsx};
    use std::io::Cursor;

    let cursor = Cursor::new(data);
    let mut wb: Xlsx<_> = open_workbook_from_rs(cursor)
        .map_err(|e| AppError::Validation(format!("XLSX open: {e}")))?;

    let sheet_names = wb.sheet_names().to_vec();
    let sheet_name = sheet_names.first().ok_or_else(|| {
        AppError::Validation("XLSX has no sheets".into())
    })?;

    let range = wb
        .worksheet_range(sheet_name)
        .map_err(|e| AppError::Validation(format!("XLSX sheet: {e}")))?;

    let mut rows = range.rows();
    let header_row = match rows.next() {
        Some(r) => r,
        None => return Ok(vec![]),
    };

    let headers: Vec<String> = header_row
        .iter()
        .map(|c| c.to_string())
        .collect();

    let mut out = Vec::new();
    for row in rows {
        let mut map = serde_json::Map::new();
        for (h, v) in headers.iter().zip(row.iter()) {
            let val = match v {
                calamine::Data::Empty => serde_json::Value::Null,
                calamine::Data::String(s) => serde_json::Value::String(s.clone()),
                calamine::Data::Float(f) => serde_json::json!(*f as i64),
                calamine::Data::Int(i) => serde_json::json!(*i),
                calamine::Data::Bool(b) => serde_json::Value::Bool(*b),
                calamine::Data::Error(_) => serde_json::Value::Null,
                calamine::Data::DateTime(dt) => serde_json::Value::String(dt.to_string()),
                calamine::Data::DateTimeIso(s) => serde_json::Value::String(s.clone()),
                calamine::Data::DurationIso(s) => serde_json::Value::String(s.clone()),
            };
            map.insert(h.clone(), val);
        }
        out.push(serde_json::Value::Object(map));
    }
    Ok(out)
}
