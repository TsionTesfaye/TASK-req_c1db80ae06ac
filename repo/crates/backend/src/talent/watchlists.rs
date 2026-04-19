//! Watchlist repository functions (T10–T13) — SELF-scoped.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use terraops_shared::dto::talent::{CandidateListItem, WatchlistEntry, WatchlistItem};
use uuid::Uuid;

use crate::errors::AppError;

#[derive(FromRow)]
pub struct WatchlistRow {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub item_count: i64,
}

impl From<WatchlistRow> for WatchlistItem {
    fn from(r: WatchlistRow) -> Self {
        WatchlistItem {
            id: r.id,
            name: r.name,
            created_at: r.created_at,
            updated_at: r.updated_at,
            item_count: r.item_count,
        }
    }
}

/// List watchlists for the calling user (SELF).
pub async fn list(pool: &PgPool, owner_id: Uuid) -> Result<Vec<WatchlistItem>, AppError> {
    let rows = sqlx::query_as::<_, WatchlistRow>(
        "SELECT tw.id, tw.name, tw.created_at, tw.updated_at, \
         COUNT(twi.candidate_id) AS item_count \
         FROM talent_watchlists tw \
         LEFT JOIN talent_watchlist_items twi ON twi.watchlist_id = tw.id \
         WHERE tw.owner_id = $1 \
         GROUP BY tw.id \
         ORDER BY tw.created_at DESC",
    )
    .bind(owner_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// Create a new watchlist for the calling user.
pub async fn create(
    pool: &PgPool,
    owner_id: Uuid,
    name: &str,
) -> Result<WatchlistItem, AppError> {
    let row = sqlx::query_as::<_, WatchlistRow>(
        "INSERT INTO talent_watchlists (owner_id, name) \
         VALUES ($1, $2) \
         RETURNING id, name, created_at, updated_at, 0::BIGINT AS item_count",
    )
    .bind(owner_id)
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(row.into())
}

/// Verify a watchlist belongs to the given owner. Returns AppError::Forbidden
/// if it belongs to someone else, or AppError::NotFound if it doesn't exist.
pub async fn assert_owner(
    pool: &PgPool,
    watchlist_id: Uuid,
    owner_id: Uuid,
) -> Result<(), AppError> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT owner_id FROM talent_watchlists WHERE id = $1",
    )
    .bind(watchlist_id)
    .fetch_optional(pool)
    .await?;

    match row {
        None => Err(AppError::NotFound),
        Some((wl_owner,)) if wl_owner != owner_id => {
            Err(AppError::Forbidden("not watchlist owner"))
        }
        _ => Ok(()),
    }
}

/// Add a candidate to a watchlist. The caller must have already verified
/// ownership via `assert_owner`.
pub async fn add_item(
    pool: &PgPool,
    watchlist_id: Uuid,
    candidate_id: Uuid,
) -> Result<(), AppError> {
    // Verify candidate exists
    let exists: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM candidates WHERE id = $1 AND deleted_at IS NULL")
            .bind(candidate_id)
            .fetch_optional(pool)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    sqlx::query(
        "INSERT INTO talent_watchlist_items (watchlist_id, candidate_id) \
         VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(watchlist_id)
    .bind(candidate_id)
    .execute(pool)
    .await?;

    // Update watchlist updated_at
    sqlx::query("UPDATE talent_watchlists SET updated_at = NOW() WHERE id = $1")
        .bind(watchlist_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Remove a candidate from a watchlist. The caller must have already verified
/// ownership via `assert_owner`.
pub async fn remove_item(
    pool: &PgPool,
    watchlist_id: Uuid,
    candidate_id: Uuid,
) -> Result<(), AppError> {
    let res = sqlx::query(
        "DELETE FROM talent_watchlist_items \
         WHERE watchlist_id = $1 AND candidate_id = $2",
    )
    .bind(watchlist_id)
    .bind(candidate_id)
    .execute(pool)
    .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    sqlx::query("UPDATE talent_watchlists SET updated_at = NOW() WHERE id = $1")
        .bind(watchlist_id)
        .execute(pool)
        .await?;

    Ok(())
}

#[derive(FromRow)]
struct WatchlistItemRow {
    // candidate columns
    id: Uuid,
    full_name: String,
    email_mask: String,
    location: Option<String>,
    years_experience: i32,
    skills: Vec<String>,
    completeness_score: i32,
    last_active_at: DateTime<Utc>,
    added_at: DateTime<Utc>,
}

/// List items in a watchlist.
pub async fn list_items(
    pool: &PgPool,
    watchlist_id: Uuid,
) -> Result<Vec<WatchlistEntry>, AppError> {
    let rows = sqlx::query_as::<_, WatchlistItemRow>(
        "SELECT c.id, c.full_name, c.email_mask, c.location, c.years_experience, \
         c.skills, c.completeness_score, c.last_active_at, twi.added_at \
         FROM talent_watchlist_items twi \
         JOIN candidates c ON c.id = twi.candidate_id \
         WHERE twi.watchlist_id = $1 AND c.deleted_at IS NULL \
         ORDER BY twi.added_at DESC",
    )
    .bind(watchlist_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| WatchlistEntry {
            candidate: CandidateListItem {
                id: r.id,
                full_name: r.full_name,
                email_mask: r.email_mask,
                location: r.location,
                years_experience: r.years_experience,
                skills: r.skills,
                completeness_score: r.completeness_score,
                last_active_at: r.last_active_at,
            },
            added_at: r.added_at,
        })
        .collect())
}
