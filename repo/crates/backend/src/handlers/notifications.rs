//! Notification center endpoints — all self-scoped (no permission check
//! beyond authentication).
//!
//!   GET    /api/v1/notifications
//!   POST   /api/v1/notifications/{id}/read
//!   POST   /api/v1/notifications/read-all
//!   GET    /api/v1/notifications/unread-count
//!   GET    /api/v1/notifications/subscriptions
//!   PUT    /api/v1/notifications/subscriptions
//!   GET    /api/v1/notifications/mailbox-exports           (list)
//!   POST   /api/v1/notifications/mailbox-export            (generate .mbox)
//!   GET    /api/v1/notifications/mailbox-exports/{id}      (download .mbox)

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
            .route("/mailbox-exports", web::get().to(list_exports))
            .route("/mailbox-export", web::post().to(create_export))
            .route("/mailbox-exports/{id}", web::get().to(download_export)),
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

// ---------------------------------------------------------------------------
// POST /notifications/mailbox-export
// ---------------------------------------------------------------------------
//
// Generates a locally-materialized `.mbox` file containing every notification
// the caller can see (self-scoped). Writes the file under
// `{runtime_dir}/mailbox/{user_id}/{export_id}.mbox`, records a row in
// `mailbox_exports`, and returns the summary (the same shape used by the
// list endpoint). The file layout follows the classic mbox format: each
// message starts with a `From ` envelope line, followed by standard RFC822
// headers (`Date`, `From`, `Subject`, `X-Terraops-Topic`), a blank line,
// and the notification body. This is the prompt-required "local mailbox
// file export" delivery path: no SMTP, no outbound network — everything
// lives on disk inside the single-node Docker deployment.
async fn create_export(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    use std::io::Write;

    // Collect every notification for this user in chronological order so
    // the generated mbox file reads top-to-bottom as a timeline.
    #[derive(FromRow)]
    struct Row {
        topic: String,
        title: String,
        body: String,
        created_at: DateTime<Utc>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT topic, title, body, created_at FROM notifications \
         WHERE user_id = $1 ORDER BY created_at ASC",
    )
    .bind(user.0.user_id)
    .fetch_all(&state.pool)
    .await?;

    // Mint a fresh export id up-front so we can name the file after it.
    let export_id = Uuid::new_v4();

    // Target path: `{runtime_dir}/mailbox/{user_id}/{export_id}.mbox`.
    let mut dir = state.runtime_dir.clone();
    dir.push("mailbox");
    dir.push(user.0.user_id.to_string());
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::Internal(format!("mailbox dir create failed: {e}")))?;
    let path = dir.join(format!("{export_id}.mbox"));

    // Build the mbox body. Use a String first so we can also record the
    // exact size that hits disk.
    let mut buf = String::with_capacity(256 + rows.len() * 128);
    let sender_addr = format!("terraops-notifications+{}@local", user.0.user_id);
    for r in &rows {
        // Envelope line — the mbox `From ` separator. The date must be in
        // asctime-ish form; `%a %b %d %H:%M:%S %Y` matches the traditional
        // `ctime` format used by historical mbox readers.
        let from_line = format!(
            "From {} {}\n",
            sender_addr,
            r.created_at.format("%a %b %d %H:%M:%S %Y")
        );
        buf.push_str(&from_line);
        buf.push_str(&format!("Date: {}\n", r.created_at.to_rfc2822()));
        buf.push_str(&format!("From: TerraOps <{}>\n", sender_addr));
        buf.push_str(&format!("To: <{}>\n", user.0.user_id));
        buf.push_str("Content-Type: text/plain; charset=UTF-8\n");
        buf.push_str("MIME-Version: 1.0\n");
        buf.push_str(&format!("X-Terraops-Topic: {}\n", sanitize_header(&r.topic)));
        buf.push_str(&format!("Subject: {}\n", sanitize_header(&r.title)));
        buf.push('\n');
        // Escape any bare `From ` lines inside the body per mbox `>From ` rule.
        for line in r.body.split('\n') {
            if line.starts_with("From ") {
                buf.push('>');
            }
            buf.push_str(line);
            buf.push('\n');
        }
        buf.push('\n');
    }

    let bytes = buf.as_bytes();
    {
        let mut f = std::fs::File::create(&path)
            .map_err(|e| AppError::Internal(format!("mailbox file create failed: {e}")))?;
        f.write_all(bytes)
            .map_err(|e| AppError::Internal(format!("mailbox file write failed: {e}")))?;
        f.flush().ok();
    }

    let path_s = path.to_string_lossy().to_string();
    let size = bytes.len() as i64;

    // Persist the mailbox_exports row using the pre-minted id so the file
    // name on disk matches the DB row. `generated_at` defaults to NOW().
    let (generated_at,): (DateTime<Utc>,) = sqlx::query_as(
        "INSERT INTO mailbox_exports (id, user_id, path, size_bytes) \
         VALUES ($1, $2, $3, $4) RETURNING generated_at",
    )
    .bind(export_id)
    .bind(user.0.user_id)
    .bind(&path_s)
    .bind(size)
    .fetch_one(&state.pool)
    .await?;

    let dto = MailboxExportSummary {
        id: export_id,
        path: path_s,
        size_bytes: size,
        generated_at,
    };
    Ok(HttpResponse::Created().json(dto))
}

/// Strip CRLF / control chars from values that will be inlined into RFC822
/// headers so a notification title/topic with newlines cannot inject rogue
/// headers into the mbox stream.
fn sanitize_header(v: &str) -> String {
    v.replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_header_replaces_lf() {
        assert_eq!(sanitize_header("hello\nworld"), "hello world");
    }

    #[test]
    fn sanitize_header_replaces_cr() {
        assert_eq!(sanitize_header("hello\rworld"), "hello world");
    }

    #[test]
    fn sanitize_header_replaces_crlf_sequence() {
        // \r\n is two chars; each becomes a space → two spaces.
        assert_eq!(sanitize_header("line1\r\nline2"), "line1  line2");
    }

    #[test]
    fn sanitize_header_passthrough_normal() {
        let s = "Normal Subject: Testing 123";
        assert_eq!(sanitize_header(s), s);
    }

    #[test]
    fn sanitize_header_empty_string() {
        assert_eq!(sanitize_header(""), "");
    }
}

// ---------------------------------------------------------------------------
// GET /notifications/mailbox-exports/{id}
// ---------------------------------------------------------------------------
//
// Streams the previously-generated `.mbox` bytes back to the caller as
// `application/mbox`. Self-scoped: the row is matched by both `id` AND
// `user_id`, so one user cannot pull another user's mailbox export.
async fn download_export(
    user: AuthUser,
    state: web::Data<AppState>,
    path: web::Path<Uuid>,
) -> AppResult<impl Responder> {
    let id = path.into_inner();
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT path FROM mailbox_exports WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user.0.user_id)
    .fetch_optional(&state.pool)
    .await?;
    let (file_path,) = row.ok_or(AppError::NotFound)?;
    let bytes = std::fs::read(&file_path)
        .map_err(|e| AppError::Internal(format!("mailbox file read failed: {e}")))?;
    Ok(HttpResponse::Ok()
        .content_type("application/mbox")
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"{id}.mbox\""),
        ))
        .body(bytes))
}
