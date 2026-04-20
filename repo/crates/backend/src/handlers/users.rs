//! User admin endpoints U1–U10.
//!
//!   U1 GET    /api/v1/users
//!   U2 POST   /api/v1/users
//!   U3 GET    /api/v1/users/{id}
//!   U4 PATCH  /api/v1/users/{id}
//!   U5 DELETE /api/v1/users/{id}           (soft: is_active=false)
//!   U6 POST   /api/v1/users/{id}/unlock
//!   U7 POST   /api/v1/users/{id}/roles     (replace role set)
//!   U8 GET    /api/v1/roles
//!   U9 POST   /api/v1/users/{id}/reset-password
//!   U10 GET   /api/v1/audit?actor=&action= (admin audit trail)

use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use sqlx::FromRow;
use terraops_shared::{
    dto::{
        audit::AuditEntry,
        user::{
            AssignRolesRequest, CreateUserRequest, RoleDto, UpdateUserRequest, UserDetail,
            UserListItem,
        },
    },
    pagination::{Page, PageQuery},
};
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser},
    crypto::{argon, email as email_crypto},
    errors::{AppError, AppResult},
    services::{audit as audit_svc, users as user_svc},
    state::AppState,
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/users")
            .route("", web::get().to(list_users))
            .route("", web::post().to(create_user))
            .route("/{id}", web::get().to(get_user))
            .route("/{id}", web::patch().to(update_user))
            .route("/{id}", web::delete().to(delete_user))
            .route("/{id}/unlock", web::post().to(unlock_user))
            .route("/{id}/roles", web::post().to(assign_roles))
            .route("/{id}/reset-password", web::post().to(reset_password)),
    );
    cfg.route("/roles", web::get().to(list_roles));
    cfg.route("/audit", web::get().to(list_audit));
}

async fn list_users(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<PageQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "user.manage")?;
    let r = q.into_inner().resolved();

    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        display_name: String,
        email_mask: String,
        is_active: bool,
        locked_until: Option<DateTime<Utc>>,
        created_at: DateTime<Utc>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT id, display_name, email_mask, is_active, locked_until, created_at \
         FROM users ORDER BY created_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(r.limit() as i64)
    .bind(r.offset() as i64)
    .fetch_all(&state.pool)
    .await?;
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM users")
        .fetch_one(&state.pool)
        .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let roles = user_svc::roles_for_user(&state.pool, row.id).await?;
        items.push(UserListItem {
            id: row.id,
            display_name: row.display_name,
            email_mask: row.email_mask,
            is_active: row.is_active,
            locked: row.locked_until.map(|t| t > Utc::now()).unwrap_or(false),
            roles,
            created_at: row.created_at,
        });
    }
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

async fn get_user(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    let id = path.into_inner();
    // Self-OR-manage (admins can read anyone; users can read themselves).
    if user.0.user_id != id {
        require_permission(&user.0, "user.manage")?;
    }
    let row = user_svc::find_by_id(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let roles = user_svc::roles_for_user(&state.pool, id).await?;
    // Decrypt email only when caller holds user.manage (admin surface).
    let email = if user.0.has_permission("user.manage") {
        email_crypto::decrypt_email(&row.email_ciphertext, &state.keys.email_enc).ok()
    } else {
        None
    };
    let detail = UserDetail {
        id: row.id,
        display_name: row.display_name,
        email,
        email_mask: row.email_mask,
        is_active: row.is_active,
        locked: row.locked_until.map(|t| t > Utc::now()).unwrap_or(false),
        failed_login_count: row.failed_login_count,
        password_updated_at: row.password_updated_at,
        roles,
        timezone: row.timezone,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };
    Ok(HttpResponse::Ok().json(detail))
}

async fn create_user(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<CreateUserRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "user.manage")?;
    let req = body.into_inner();
    if req.display_name.trim().is_empty() {
        return Err(AppError::Validation("display_name required".into()));
    }
    crate::auth::password::validate_password_complexity(&req.password)?;

    let normalized = email_crypto::normalize_email(&req.email);
    if !normalized.contains('@') {
        return Err(AppError::Validation("invalid email".into()));
    }
    let email_ct = email_crypto::encrypt_email(&normalized, &state.keys.email_enc)
        .map_err(|e| AppError::Internal(format!("email enc: {e}")))?;
    let email_hash = email_crypto::email_hash(&normalized, &state.keys.email_hmac).to_vec();
    let email_mask = email_crypto::email_mask(&normalized);
    let password_hash = argon::hash_password(&req.password)
        .map_err(|e| AppError::Internal(format!("argon: {e}")))?;

    // Audit #4 Issue #4: username is the primary login identifier.
    // Accept an explicit `username` in the request, or derive it from
    // the email local-part. Lowercased for a case-insensitive contract.
    let username_raw = req
        .username
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| {
            normalized
                .split('@')
                .next()
                .unwrap_or(&normalized)
                .to_string()
        });
    if !username_raw
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
        || username_raw.is_empty()
    {
        return Err(AppError::Validation(
            "username must be non-empty and contain only letters, digits, '.', '_', or '-'".into(),
        ));
    }

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO users (display_name, username, email_ciphertext, email_hash, email_mask, \
                            password_hash, timezone) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(&req.display_name)
    .bind(&username_raw)
    .bind(&email_ct)
    .bind(&email_hash)
    .bind(&email_mask)
    .bind(&password_hash)
    .bind(req.timezone.clone())
    .fetch_one(&state.pool)
    .await?;
    let new_id = row.0;

    // Assign role set.
    if !req.roles.is_empty() {
        let role_names: Vec<String> = req.roles.iter().map(|r| r.as_db().to_string()).collect();
        sqlx::query(
            "INSERT INTO user_roles (user_id, role_id, granted_by) \
             SELECT $1, r.id, $2 FROM roles r WHERE r.name = ANY($3)",
        )
        .bind(new_id)
        .bind(user.0.user_id)
        .bind(&role_names)
        .execute(&state.pool)
        .await?;
    }

    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "user.create",
        Some("user"),
        Some(&new_id.to_string()),
        json!({"display_name": req.display_name, "roles": req.roles}),
    )
    .await?;
    Ok(HttpResponse::Created().json(json!({"id": new_id})))
}

async fn update_user(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateUserRequest>,
) -> AppResult<impl Responder> {
    let id = path.into_inner();
    // Self can edit display_name + timezone; user.manage can edit everything.
    let is_admin = user.0.has_permission("user.manage");
    if !is_admin && user.0.user_id != id {
        return Err(AppError::Forbidden("cannot edit another user"));
    }
    let req = body.into_inner();
    if let Some(ref email) = req.email {
        if !is_admin {
            return Err(AppError::Forbidden("email change requires user.manage"));
        }
        let normalized = email_crypto::normalize_email(email);
        let ct = email_crypto::encrypt_email(&normalized, &state.keys.email_enc)
            .map_err(|e| AppError::Internal(format!("email enc: {e}")))?;
        let h = email_crypto::email_hash(&normalized, &state.keys.email_hmac).to_vec();
        let mask = email_crypto::email_mask(&normalized);
        sqlx::query(
            "UPDATE users SET email_ciphertext = $1, email_hash = $2, email_mask = $3, \
                              updated_at = NOW() WHERE id = $4",
        )
        .bind(&ct)
        .bind(&h)
        .bind(&mask)
        .bind(id)
        .execute(&state.pool)
        .await?;
    }
    if let Some(ref name) = req.display_name {
        sqlx::query("UPDATE users SET display_name = $1, updated_at = NOW() WHERE id = $2")
            .bind(name)
            .bind(id)
            .execute(&state.pool)
            .await?;
    }
    if let Some(ref tz) = req.timezone {
        sqlx::query("UPDATE users SET timezone = $1, updated_at = NOW() WHERE id = $2")
            .bind(tz)
            .bind(id)
            .execute(&state.pool)
            .await?;
    }
    if let Some(active) = req.is_active {
        if !is_admin {
            return Err(AppError::Forbidden("activation toggle requires user.manage"));
        }
        sqlx::query("UPDATE users SET is_active = $1, updated_at = NOW() WHERE id = $2")
            .bind(active)
            .bind(id)
            .execute(&state.pool)
            .await?;
        if !active {
            crate::auth::sessions::revoke_all_for_user(&state.pool, id).await?;
        }
    }
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "user.update",
        Some("user"),
        Some(&id.to_string()),
        json!({"fields": {
            "display_name": req.display_name.is_some(),
            "email": req.email.is_some(),
            "timezone": req.timezone.is_some(),
            "is_active": req.is_active
        }}),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn delete_user(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "user.manage")?;
    let id = path.into_inner();
    if id == user.0.user_id {
        return Err(AppError::Validation("cannot deactivate self".into()));
    }
    let res = sqlx::query("UPDATE users SET is_active = FALSE, updated_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    crate::auth::sessions::revoke_all_for_user(&state.pool, id).await?;
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "user.deactivate",
        Some("user"),
        Some(&id.to_string()),
        json!({}),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn unlock_user(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "user.manage")?;
    let id = path.into_inner();
    sqlx::query(
        "UPDATE users SET failed_login_count = 0, locked_until = NULL, updated_at = NOW() \
         WHERE id = $1",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "user.unlock",
        Some("user"),
        Some(&id.to_string()),
        json!({}),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn assign_roles(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<AssignRolesRequest>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "role.assign")?;
    let id = path.into_inner();
    let req = body.into_inner();
    let mut tx = state.pool.begin().await?;
    sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for role_id in &req.role_ids {
        sqlx::query(
            "INSERT INTO user_roles (user_id, role_id, granted_by) VALUES ($1, $2, $3) \
             ON CONFLICT DO NOTHING",
        )
        .bind(id)
        .bind(role_id)
        .bind(user.0.user_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "user.roles_assign",
        Some("user"),
        Some(&id.to_string()),
        json!({"role_ids": req.role_ids}),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize)]
struct ResetPasswordBody {
    new_password: String,
}

async fn reset_password(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
    body: web::Json<ResetPasswordBody>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "user.manage")?;
    let id = path.into_inner();
    crate::auth::password::update_password(&state.pool, id, &body.new_password).await?;
    audit_svc::record(
        &state.pool,
        Some(user.0.user_id),
        "user.reset_password",
        Some("user"),
        Some(&id.to_string()),
        json!({}),
    )
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn list_roles(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    require_permission(&user.0, "role.assign")?;
    #[derive(FromRow)]
    struct RoleRow {
        id: Uuid,
        name: String,
        display: String,
    }
    let rows: Vec<RoleRow> = sqlx::query_as::<_, RoleRow>(
        "SELECT id, name, display FROM roles ORDER BY name",
    )
    .fetch_all(&state.pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let perms: Vec<(String,)> = sqlx::query_as(
            "SELECT p.code FROM permissions p \
             JOIN role_permissions rp ON rp.permission_id = p.id \
             WHERE rp.role_id = $1 ORDER BY p.code",
        )
        .bind(r.id)
        .fetch_all(&state.pool)
        .await?;
        out.push(RoleDto {
            id: r.id,
            name: r.name,
            display: r.display,
            permissions: perms.into_iter().map(|(c,)| c).collect(),
        });
    }
    Ok(HttpResponse::Ok().json(out))
}

#[derive(Deserialize)]
struct AuditQuery {
    actor: Option<Uuid>,
    action: Option<String>,
    page: Option<u32>,
    page_size: Option<u32>,
}

async fn list_audit(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<AuditQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "monitoring.read")?;
    let q = q.into_inner();
    let r = PageQuery {
        page: q.page,
        page_size: q.page_size,
    }
    .resolved();
    #[derive(FromRow)]
    struct Row {
        id: i64,
        actor_id: Option<Uuid>,
        actor_display: Option<String>,
        action: String,
        target_type: Option<String>,
        target_id: Option<String>,
        meta_json: serde_json::Value,
        at: DateTime<Utc>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT a.id, a.actor_id, u.display_name AS actor_display, a.action, \
                a.target_type, a.target_id, a.meta_json, a.at \
         FROM audit_log a LEFT JOIN users u ON u.id = a.actor_id \
         WHERE ($1::UUID IS NULL OR a.actor_id = $1) \
           AND ($2::TEXT IS NULL OR a.action = $2) \
         ORDER BY a.at DESC LIMIT $3 OFFSET $4",
    )
    .bind(q.actor)
    .bind(q.action.as_deref())
    .bind(r.limit() as i64)
    .bind(r.offset() as i64)
    .fetch_all(&state.pool)
    .await?;
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM audit_log \
         WHERE ($1::UUID IS NULL OR actor_id = $1) \
           AND ($2::TEXT IS NULL OR action = $2)",
    )
    .bind(q.actor)
    .bind(q.action.as_deref())
    .fetch_one(&state.pool)
    .await?;
    let items: Vec<AuditEntry> = rows
        .into_iter()
        .map(|r| AuditEntry {
            id: r.id,
            actor_id: r.actor_id,
            actor_display: r.actor_display,
            action: r.action,
            target_type: r.target_type,
            target_id: r.target_id,
            meta: r.meta_json,
            at: r.at,
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
