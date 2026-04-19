//! Business logic sitting above repo + notifications emit.

use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppResult;
use crate::services::notifications;

/// Emit a product status-changed notification to the product creator.
pub async fn emit_status_changed(
    pool: &PgPool,
    product_id: Uuid,
    on_shelf: bool,
    actor_id: Uuid,
) -> AppResult<()> {
    // Notify the actor themselves (in a real system this would fan out to
    // subscribers; for P-A we notify the actor as a lightweight demonstration).
    let status = if on_shelf { "on shelf" } else { "off shelf" };
    let _ = notifications::emit(
        pool,
        actor_id,
        "product.status.changed",
        "Product status changed",
        &format!("Product {product_id} is now {status}"),
        serde_json::json!({ "product_id": product_id, "on_shelf": on_shelf }),
    )
    .await;
    Ok(())
}

/// Emit an import-committed notification to the batch uploader.
pub async fn emit_import_committed(
    pool: &PgPool,
    batch_id: Uuid,
    row_count: i32,
    uploaded_by: Uuid,
) -> AppResult<()> {
    let _ = notifications::emit(
        pool,
        uploaded_by,
        "import.committed",
        "Import committed",
        &format!("Batch {batch_id} committed {row_count} products"),
        serde_json::json!({ "batch_id": batch_id, "row_count": row_count }),
    )
    .await;
    Ok(())
}
