//! Notification center endpoints N1–N7 — all self-scoped (no permission
//! check beyond authentication).
//!
//!   N1 GET    /api/v1/notifications
//!   N2 POST   /api/v1/notifications/{id}/read
//!   N3 POST   /api/v1/notifications/read-all
//!   N4 GET    /api/v1/notifications/unread-count
//!   N5 GET    /api/v1/notifications/subscriptions
//!   N6 PUT    /api/v1/notifications/subscriptions
//!   N7 GET    /api/v1/notifications/mailbox-exports

use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::FromRow;
use terraops_shared::{
    dto::notification::{
        MailboxExportSummary, NotificationItem, NotificationSubscription,
        UpsertSubscriptionsRequest,
    },
    pagination::{Page, PageQuery},
};
use uuid::Uuid;

use crate::{
    auth::extractors::AuthUser,
    errors::{AppError, AppResult},
    state::AppState,
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/notifications")
            .route("", web::get().to(list_notifications))
            .route("/{id}/read", web::post().to(mark_read))
            .route("/read-all", web::post().to(mark_all_read))
            .route("/unread-count", web::get().to(unread_count))
            .route("/subscriptions", web::get().to(list_subs))
            .route("/subscriptions", web::put().to(upsert_subs))
            .route("/mailbox-exports", web::get().to(list_exports)),
    );
}

async fn list_notifications(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<PageQuery>,
) -> AppResult<impl Responder> {
    let r = q.into_inner().resolved();
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        topic: String,
        title: String,
        body: String,
        payload_json: serde_json::Value,
        read_at: Option<DateTime<Utc>>,
        created_at: DateTime<Utc>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT id, topic, title, body, payload_json, read_at, created_at \
         FROM notifications WHERE user_id = $1 \
         ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(user.0.user_id)
    .bind(r.limit() as i64)
    .bind(r.offset() as i64)
    .fetch_all(&state.pool)
    .await?;
    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::BIGINT FROM notifications WHERE user_id = $1")
            .bind(user.0.user_id)
            .fetch_one(&state.pool)
            .await?;
    let items: Vec<NotificationItem> = rows
        .into_iter()
        .map(|r| NotificationItem {
            id: r.id,
            topic: r.topic,
            title: r.title,
            body: r.body,
            payload: r.payload_json,
            read_at: r.read_at,
            created_at: r.created_at,
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

async fn mark_read(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    let id = path.into_inner();
    let res = sqlx::query(
        "UPDATE notifications SET read_at = NOW() \
         WHERE id = $1 AND user_id = $2 AND read_at IS NULL",
    )
    .bind(id)
    .bind(user.0.user_id)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        // Either doesn't exist or belongs to another user OR already read.
        // Confirm ownership so we do not leak existence.
        let owned: Option<(bool,)> = sqlx::query_as(
            "SELECT read_at IS NOT NULL FROM notifications WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user.0.user_id)
        .fetch_optional(&state.pool)
        .await?;
        if owned.is_none() {
            return Err(AppError::NotFound);
        }
    }
    Ok(HttpResponse::NoContent().finish())
}

async fn mark_all_read(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    sqlx::query(
        "UPDATE notifications SET read_at = NOW() WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(user.0.user_id)
    .execute(&state.pool)
    .await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn unread_count(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM notifications WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(user.0.user_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(HttpResponse::Ok().json(json!({"unread": total.0})))
}

async fn list_subs(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    let rows: Vec<(String, bool)> = sqlx::query_as(
        "SELECT topic, enabled FROM notification_subscriptions WHERE user_id = $1 ORDER BY topic",
    )
    .bind(user.0.user_id)
    .fetch_all(&state.pool)
    .await?;
    let items: Vec<NotificationSubscription> = rows
        .into_iter()
        .map(|(topic, enabled)| NotificationSubscription { topic, enabled })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

async fn upsert_subs(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<UpsertSubscriptionsRequest>,
) -> AppResult<impl Responder> {
    let mut tx = state.pool.begin().await?;
    for s in &body.subscriptions {
        sqlx::query(
            "INSERT INTO notification_subscriptions (user_id, topic, enabled, updated_at) \
             VALUES ($1, $2, $3, NOW()) \
             ON CONFLICT (user_id, topic) DO UPDATE SET enabled = EXCLUDED.enabled, \
                                                        updated_at = NOW()",
        )
        .bind(user.0.user_id)
        .bind(&s.topic)
        .bind(s.enabled)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(HttpResponse::NoContent().finish())
}

async fn list_exports(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        path: String,
        size_bytes: i64,
        generated_at: DateTime<Utc>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT id, path, size_bytes, generated_at FROM mailbox_exports WHERE user_id = $1 \
         ORDER BY generated_at DESC",
    )
    .bind(user.0.user_id)
    .fetch_all(&state.pool)
    .await?;
    let items: Vec<MailboxExportSummary> = rows
        .into_iter()
        .map(|r| MailboxExportSummary {
            id: r.id,
            path: r.path,
            size_bytes: r.size_bytes,
            generated_at: r.generated_at,
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}
