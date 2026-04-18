//! Append-only audit log writer.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppResult;

pub async fn record(
    pool: &PgPool,
    actor_id: Option<Uuid>,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<&str>,
    meta: Value,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO audit_log (actor_id, action, target_type, target_id, meta_json) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(actor_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(meta)
    .execute(pool)
    .await?;
    Ok(())
}
