//! Product change history writer.
//!
//! All mutations MUST call one of the `record_*` helpers so the immutable
//! append-only `product_history` table stays complete.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppResult;

pub async fn record(
    pool: &PgPool,
    product_id: Uuid,
    action: &str,
    changed_by: Uuid,
    before: Option<Value>,
    after: Option<Value>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO product_history (product_id, action, changed_by, before_json, after_json) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(product_id)
    .bind(action)
    .bind(changed_by)
    .bind(before)
    .bind(after)
    .execute(pool)
    .await?;
    Ok(())
}
