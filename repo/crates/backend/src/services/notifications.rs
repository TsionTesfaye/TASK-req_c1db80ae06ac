//! Notification emit + listing helpers.
//!
//! `emit` is the single shared entry point everything in the backend uses
//! to publish a notification to a user — the design explicitly calls out
//! that there is no separate ad-hoc publisher path.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppResult;

pub async fn emit(
    pool: &PgPool,
    user_id: Uuid,
    topic: &str,
    title: &str,
    body: &str,
    payload: Value,
) -> AppResult<Uuid> {
    // Respect subscription preference (default ON when no row).
    let enabled: Option<(bool,)> = sqlx::query_as(
        "SELECT enabled FROM notification_subscriptions WHERE user_id = $1 AND topic = $2",
    )
    .bind(user_id)
    .bind(topic)
    .fetch_optional(pool)
    .await?;
    if let Some((false,)) = enabled {
        // User has explicitly opted out of this topic — record nothing.
        return Ok(Uuid::nil());
    }

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO notifications (user_id, topic, title, body, payload_json) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(user_id)
    .bind(topic)
    .bind(title)
    .bind(body)
    .bind(payload)
    .fetch_one(pool)
    .await?;

    // Local-only "delivery": mark it delivered successfully on attempt #1.
    // Offline environment — there is no external transport. This keeps the
    // retry table populated honestly (design §Notifications).
    let _ = sqlx::query(
        "INSERT INTO notification_delivery_attempts (notification_id, attempt_no, state) \
         VALUES ($1, 1, 'success')",
    )
    .bind(row.0)
    .execute(pool)
    .await;

    Ok(row.0)
}
