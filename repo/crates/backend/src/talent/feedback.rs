//! Feedback repository functions (T9) — PERM(talent.feedback), owner-scoped.

use sqlx::PgPool;
use terraops_shared::dto::talent::{CreateFeedbackRequest, FeedbackRecord};
use uuid::Uuid;

use crate::errors::AppError;

/// Create a feedback record. The `owner_id` is always the authenticated caller.
pub async fn create(
    pool: &PgPool,
    req: &CreateFeedbackRequest,
    owner_id: Uuid,
) -> Result<FeedbackRecord, AppError> {
    if !matches!(req.thumb.as_str(), "up" | "down") {
        return Err(AppError::Validation(
            "thumb must be 'up' or 'down'".into(),
        ));
    }

    // Verify candidate exists
    let exists: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM candidates WHERE id = $1 AND deleted_at IS NULL")
            .bind(req.candidate_id)
            .fetch_optional(pool)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    // Verify role exists if provided
    if let Some(role_id) = req.role_id {
        let role_exists: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM roles_open WHERE id = $1")
                .bind(role_id)
                .fetch_optional(pool)
                .await?;
        if role_exists.is_none() {
            return Err(AppError::NotFound);
        }
    }

    let row = sqlx::query_as::<_, crate::talent::feedback::FeedbackRow>(
        "INSERT INTO talent_feedback (candidate_id, role_id, owner_id, thumb, note) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, candidate_id, role_id, owner_id, thumb, note, created_at",
    )
    .bind(req.candidate_id)
    .bind(req.role_id)
    .bind(owner_id)
    .bind(&req.thumb)
    .bind(&req.note)
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}

/// Count feedback rows scoped to a single `(owner_id, role_id)` pair —
/// the authoritative cold-start signal per `docs/design.md` Design
/// Decision #13 ("`feedback_count(user, role_scope) < 10` → cold
/// start"). Audit HIGH H2 removed the prior `count_total` global
/// count, which let feedback authored by any user for any role
/// move every other user+role pair out of cold-start.
pub async fn count_scoped(
    pool: &PgPool,
    owner_id: Uuid,
    role_id: Uuid,
) -> Result<i64, AppError> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM talent_feedback \
         WHERE owner_id = $1 AND role_id = $2",
    )
    .bind(owner_id)
    .bind(role_id)
    .fetch_one(pool)
    .await?;
    Ok(count)
}

use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(FromRow)]
pub(crate) struct FeedbackRow {
    pub id: Uuid,
    pub candidate_id: Uuid,
    pub role_id: Option<Uuid>,
    pub owner_id: Uuid,
    pub thumb: String,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<FeedbackRow> for FeedbackRecord {
    fn from(r: FeedbackRow) -> Self {
        FeedbackRecord {
            id: r.id,
            candidate_id: r.candidate_id,
            role_id: r.role_id,
            owner_id: r.owner_id,
            thumb: r.thumb,
            note: r.note,
            created_at: r.created_at,
        }
    }
}
