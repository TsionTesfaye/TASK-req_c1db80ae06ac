//! Admin security endpoints SEC1–SEC9.
//!
//!   SEC1 GET    /api/v1/security/allowlist
//!   SEC2 POST   /api/v1/security/allowlist
//!   SEC3 DELETE /api/v1/security/allowlist/{id}
//!   SEC4 GET    /api/v1/security/device-certs
//!   SEC5 POST   /api/v1/security/device-certs
//!   SEC6 DELETE /api/v1/security/device-certs/{id}    (revoke)
//!   SEC7 GET    /api/v1/security/mtls
//!   SEC8 PATCH  /api/v1/security/mtls
//!   SEC9 GET    /api/v1/security/mtls/status          (snapshot for admin dashboard)

use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use ipnetwork::IpNetwork;
use serde_json::json;
use sqlx::FromRow;
use terraops_shared::dto::security::{
    AllowlistEntry, CreateAllowlistEntry, DeviceCert, MtlsConfig, RegisterDeviceCert,
    UpdateMtlsConfig,
};
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::{AppError, AppResult},
    services::audit as audit_svc,
    state::AppState,
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/security")
            .route("/allowlist", web::get().to(list_allowlist))
            .route("/allowlist", web::post().to(create_allowlist))
            .route("/allowlist/{id}", web::delete().to(delete_allowlist))
            .route("/device-certs", web::get().to(list_device_certs))
            .route("/device-certs", web::post().to(register_device_cert))
            .route("/device-certs/{id}", web::delete().to(revoke_device_cert))
            .route("/mtls", web::get().to(get_mtls))
            .route("/mtls", web::patch().to(patch_mtls))
            .route("/mtls/status", web::get().to(mtls_status)),
    );
}

async fn list_allowlist(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    require_permission(&user.0, "allowlist.manage")?;
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        cidr: IpNetwork,
        note: Option<String>,
        enabled: bool,
        created_at: DateTime<Utc>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT id, cidr, note, enabled, created_at FROM endpoint_allowlist ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    let items: Vec<AllowlistEntry> = rows
        .into_iter()
        .map(|r| AllowlistEntry {
            id: r.id,
            cidr: r.cidr.to_string(),
            note: r.note,
            enabled: r.enabled,
            created_at: r.created_at,
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

async fn create_allowlist(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateAllowlistEntry>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "allowlist.manage")?;
    let req = body.into_inner();
    let cidr: IpNetwork = req
        .cidr
        .parse()
        .map_err(|_| AppError::Validation("invalid CIDR".into()))?;
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO endpoint_allowlist (cidr, note, enabled, created_by) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(cidr)
    .bind(req.note.as_deref())
    .bind(req.enabled.unwrap_or(true))
    .bind(user.0.user_id)
    .fetch_one(&state.pool)
    .await?;
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "allowlist.create",
        Some("allowlist"),
        Some(&row.0.to_string()),
        json!({"cidr": req.cidr}),
    )
    .await?;
    Ok(HttpResponse::Created().json(json!({"id": row.0})))
}

async fn delete_allowlist(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "allowlist.manage")?;
    let id = path.into_inner();
    let res = sqlx::query("DELETE FROM endpoint_allowlist WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "allowlist.delete",
        Some("allowlist"),
        Some(&id.to_string()),
        json!({}),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn list_device_certs(
    user: AuthUser,
    state: web::Data<AppState>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "mtls.manage")?;
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        label: String,
        issued_to_user_id: Option<Uuid>,
        issued_to_display: Option<String>,
        serial: Option<String>,
        spki_sha256: Vec<u8>,
        pem_path: Option<String>,
        notes: Option<String>,
        issued_at: DateTime<Utc>,
        revoked_at: Option<DateTime<Utc>>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT d.id, d.label, d.issued_to_user_id, u.display_name AS issued_to_display, \
                d.serial, d.spki_sha256, d.pem_path, d.notes, d.issued_at, d.revoked_at \
         FROM device_certs d LEFT JOIN users u ON u.id = d.issued_to_user_id \
         ORDER BY d.issued_at DESC",
    )
    .fetch_all(&state.pool)
    .await?;
    let items: Vec<DeviceCert> = rows
        .into_iter()
        .map(|r| DeviceCert {
            id: r.id,
            label: r.label,
            issued_to_user_id: r.issued_to_user_id,
            issued_to_display: r.issued_to_display,
            serial: r.serial,
            spki_sha256_hex: hex::encode(r.spki_sha256),
            pem_path: r.pem_path,
            notes: r.notes,
            issued_at: r.issued_at,
            revoked_at: r.revoked_at,
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

async fn register_device_cert(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<RegisterDeviceCert>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "mtls.manage")?;
    let req = body.into_inner();
    let pin = hex::decode(&req.spki_sha256_hex)
        .map_err(|_| AppError::Validation("spki_sha256_hex must be hex".into()))?;
    if pin.len() != 32 {
        return Err(AppError::Validation("SPKI pin must be 32 bytes".into()));
    }
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO device_certs (label, issued_to_user_id, serial, spki_sha256, pem_path, \
                                    notes, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(&req.label)
    .bind(req.issued_to_user_id)
    .bind(req.serial.as_deref())
    .bind(&pin)
    .bind(req.pem_path.as_deref())
    .bind(req.notes.as_deref())
    .bind(user.0.user_id)
    .fetch_one(&state.pool)
    .await?;
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "device_cert.register",
        Some("device_cert"),
        Some(&row.0.to_string()),
        json!({"label": req.label, "spki_sha256_hex": req.spki_sha256_hex}),
    )
    .await?;
    Ok(HttpResponse::Created().json(json!({"id": row.0})))
}

async fn revoke_device_cert(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "mtls.manage")?;
    let id = path.into_inner();
    let res = sqlx::query(
        "UPDATE device_certs SET revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "device_cert.revoke",
        Some("device_cert"),
        Some(&id.to_string()),
        json!({}),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn get_mtls(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    require_permission(&user.0, "mtls.manage")?;
    #[derive(FromRow)]
    struct Row {
        enforced: bool,
        updated_by: Option<Uuid>,
        updated_at: DateTime<Utc>,
    }
    let row: Row = sqlx::query_as::<_, Row>(
        "SELECT enforced, updated_by, updated_at FROM mtls_config WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(HttpResponse::Ok().json(MtlsConfig {
        enforced: row.enforced,
        updated_at: row.updated_at,
        updated_by: row.updated_by,
    }))
}

async fn patch_mtls(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<UpdateMtlsConfig>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "mtls.manage")?;
    let req = body.into_inner();
    sqlx::query(
        "UPDATE mtls_config SET enforced = $1, updated_by = $2, updated_at = NOW() WHERE id = 1",
    )
    .bind(req.enforced)
    .bind(user.0.user_id)
    .execute(&state.pool)
    .await?;
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "mtls.update",
        Some("mtls_config"),
        Some("1"),
        json!({"enforced": req.enforced}),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn mtls_status(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    require_permission(&user.0, "mtls.manage")?;
    #[derive(FromRow)]
    struct Row {
        enforced: bool,
        updated_at: DateTime<Utc>,
    }
    let cfg: Row = sqlx::query_as::<_, Row>(
        "SELECT enforced, updated_at FROM mtls_config WHERE id = 1",
    )
    .fetch_one(&state.pool)
    .await?;
    let active: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM device_certs WHERE revoked_at IS NULL",
    )
    .fetch_one(&state.pool)
    .await?;
    let revoked: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM device_certs WHERE revoked_at IS NOT NULL",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(HttpResponse::Ok().json(json!({
        "enforced": cfg.enforced,
        "updated_at": cfg.updated_at,
        "active_certs": active.0,
        "revoked_certs": revoked.0
    })))
}
