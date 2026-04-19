//! Talent weights repository functions (T7 get, T8 put) — SELF-scoped.

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use terraops_shared::dto::talent::{TalentWeights, UpdateWeightsRequest};
use uuid::Uuid;

use crate::errors::AppError;

#[derive(FromRow)]
pub struct WeightsRow {
    pub user_id: Uuid,
    pub skills_weight: i32,
    pub experience_weight: i32,
    pub recency_weight: i32,
    pub completeness_weight: i32,
    pub updated_at: DateTime<Utc>,
}

impl From<WeightsRow> for TalentWeights {
    fn from(r: WeightsRow) -> Self {
        TalentWeights {
            user_id: r.user_id,
            skills_weight: r.skills_weight,
            experience_weight: r.experience_weight,
            recency_weight: r.recency_weight,
            completeness_weight: r.completeness_weight,
            updated_at: r.updated_at,
        }
    }
}

/// Get weights for the given user. Returns default weights if none are stored.
pub async fn get(pool: &PgPool, user_id: Uuid) -> Result<TalentWeights, AppError> {
    let maybe = sqlx::query_as::<_, WeightsRow>(
        "SELECT user_id, skills_weight, experience_weight, \
         recency_weight, completeness_weight, updated_at \
         FROM talent_weights WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(match maybe {
        Some(r) => r.into(),
        None => TalentWeights {
            user_id,
            skills_weight: 40,
            experience_weight: 30,
            recency_weight: 15,
            completeness_weight: 15,
            updated_at: Utc::now(),
        },
    })
}

/// Upsert weights for the given user.
pub async fn upsert(
    pool: &PgPool,
    user_id: Uuid,
    req: &UpdateWeightsRequest,
) -> Result<TalentWeights, AppError> {
    let sum =
        req.skills_weight + req.experience_weight + req.recency_weight + req.completeness_weight;
    if sum != 100 {
        return Err(AppError::Validation(format!(
            "weights must sum to 100, got {sum}"
        )));
    }
    if req.skills_weight < 0
        || req.experience_weight < 0
        || req.recency_weight < 0
        || req.completeness_weight < 0
    {
        return Err(AppError::Validation("all weights must be >= 0".into()));
    }

    let row = sqlx::query_as::<_, WeightsRow>(
        "INSERT INTO talent_weights \
         (user_id, skills_weight, experience_weight, recency_weight, completeness_weight, updated_at) \
         VALUES ($1, $2, $3, $4, $5, NOW()) \
         ON CONFLICT (user_id) DO UPDATE \
         SET skills_weight = EXCLUDED.skills_weight, \
             experience_weight = EXCLUDED.experience_weight, \
             recency_weight = EXCLUDED.recency_weight, \
             completeness_weight = EXCLUDED.completeness_weight, \
             updated_at = NOW() \
         RETURNING user_id, skills_weight, experience_weight, \
                   recency_weight, completeness_weight, updated_at",
    )
    .bind(user_id)
    .bind(req.skills_weight)
    .bind(req.experience_weight)
    .bind(req.recency_weight)
    .bind(req.completeness_weight)
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}
