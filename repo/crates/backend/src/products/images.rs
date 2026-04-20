//! Image upload/serve/delete handlers (P11–P13).

use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use futures_util::TryStreamExt;
use std::io::Write;
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser},
    crypto::signed_url,
    errors::{AppError, AppResult},
    products::{history, repo},
    state::AppState,
    storage,
};

// ---------------------------------------------------------------------------
// P11 POST /products/{id}/images
// ---------------------------------------------------------------------------

pub async fn upload_image(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    mut payload: Multipart,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.write")?;
    let product_id = path.into_inner();
    if !repo::product_exists(&state.pool, product_id).await? {
        return Err(AppError::NotFound);
    }

    let img_id = Uuid::new_v4();
    let storage_path = storage::images::image_path(img_id);

    let mut content_type = "application/octet-stream".to_string();
    let mut total_bytes: usize = 0;

    // Read one field from the multipart payload
    if let Some(mut field) = payload.try_next().await.map_err(|e| {
        AppError::Validation(format!("multipart error: {e}"))
    })? {
        // Attempt to get content_type from the field header
        if let Some(ct) = field.content_type() {
            content_type = ct.to_string();
        }

        // Ensure the storage directory exists
        if let Some(parent) = std::path::Path::new(&storage_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Internal(format!("mkdir images: {e}")))?;
        }

        let mut file = std::fs::File::create(&storage_path)
            .map_err(|e| AppError::Internal(format!("create image file: {e}")))?;

        while let Some(chunk) = field.try_next().await.map_err(|e| {
            AppError::Internal(format!("read chunk: {e}"))
        })? {
            total_bytes += chunk.len();
            file.write_all(&chunk)
                .map_err(|e| AppError::Internal(format!("write chunk: {e}")))?;
        }
    } else {
        return Err(AppError::Validation("no file field in multipart".into()));
    }

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO product_images (id, product_id, storage_path, content_type, size_bytes, uploaded_by) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(img_id)
    .bind(product_id)
    .bind(&storage_path)
    .bind(&content_type)
    .bind(total_bytes as i32)
    .bind(user.0.user_id)
    .fetch_one(&state.pool)
    .await?;

    history::record(
        &state.pool,
        product_id,
        "image",
        user.0.user_id,
        None,
        Some(serde_json::json!({ "action": "upload", "image_id": row.0 })),
    )
    .await?;

    Ok(HttpResponse::Created().json(serde_json::json!({"id": row.0})))
}

// ---------------------------------------------------------------------------
// P12 DELETE /products/{id}/images/{imgid}
// ---------------------------------------------------------------------------

pub async fn delete_image(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<(Uuid, Uuid)>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "product.write")?;
    let (product_id, img_id) = path.into_inner();
    if !repo::product_exists(&state.pool, product_id).await? {
        return Err(AppError::NotFound);
    }

    // Get path before delete so we can remove the file
    let row = repo::get_image_row(&state.pool, img_id).await?;
    let Some(img) = row else {
        return Err(AppError::NotFound);
    };
    if img.product_id != product_id {
        return Err(AppError::NotFound);
    }

    sqlx::query("DELETE FROM product_images WHERE id = $1")
        .bind(img_id)
        .execute(&state.pool)
        .await?;

    // Best-effort file removal — don't fail the request if the file is gone
    let _ = std::fs::remove_file(&img.storage_path);

    history::record(
        &state.pool,
        product_id,
        "image",
        user.0.user_id,
        Some(serde_json::json!({ "image_id": img_id })),
        None,
    )
    .await?;

    Ok(HttpResponse::NoContent().finish())
}

// ---------------------------------------------------------------------------
// P13 GET /images/{imgid} — signed URL serve
// ---------------------------------------------------------------------------

pub async fn serve_image(
    req: HttpRequest,
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    let img_id = path.into_inner();
    let api_path = format!("/api/v1/images/{img_id}");

    // Extract sig + exp from query
    let qs = req.query_string();
    let mut exp_val: Option<i64> = None;
    let mut sig_val: Option<String> = None;
    for kv in qs.split('&') {
        if let Some(v) = kv.strip_prefix("exp=") {
            exp_val = v.parse::<i64>().ok();
        } else if let Some(v) = kv.strip_prefix("sig=") {
            sig_val = Some(v.to_string());
        }
    }

    let (exp, sig) = match (exp_val, sig_val) {
        (Some(e), Some(s)) => (e, s),
        _ => return Err(AppError::Forbidden("missing signed URL parameters")),
    };

    signed_url::verify(&api_path, user.0.user_id, exp, &sig, &state.keys.image_hmac)
        .map_err(|_| AppError::Forbidden("invalid or expired signed URL"))?;

    // Look up the image row
    let img = repo::get_image_row(&state.pool, img_id)
        .await?
        .ok_or(AppError::NotFound)?;

    let bytes = std::fs::read(&img.storage_path)
        .map_err(|_| AppError::NotFound)?;

    Ok(HttpResponse::Ok()
        .content_type(img.content_type.as_str())
        .body(bytes))
}
