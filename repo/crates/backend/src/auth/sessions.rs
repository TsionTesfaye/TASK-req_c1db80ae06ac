//! Opaque refresh-token session management.
//!
//! Design §Session & Refresh:
//!   * Refresh token is a random 32-byte value, base64url-encoded.
//!   * At rest we store **only** the SHA-256 of the token (so an
//!     uncontrolled backup of the DB cannot be used to impersonate).
//!   * Idle timeout: 30 days — `expires_at` moves forward on each rotate.
//!   * Absolute max: 90 days from `issued_at`; after that we refuse to
//!     rotate even if the idle window would still allow it.
//!   * On every `/refresh`, the presented token is single-use: we revoke
//!     the row and insert a fresh one (rotation).

use chrono::{DateTime, Duration, Utc};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};

pub const IDLE_TIMEOUT_DAYS: i64 = 30;
pub const ABSOLUTE_LIFETIME_DAYS: i64 = 90;

#[derive(Debug, Clone, FromRow)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub struct IssuedRefresh {
    pub session_id: Uuid,
    pub token_plain: String,
    pub expires_at: DateTime<Utc>,
}

fn new_opaque_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn hash_token(token: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    h.finalize().to_vec()
}

pub async fn issue(
    pool: &PgPool,
    user_id: Uuid,
    user_agent: Option<&str>,
    ip: Option<ipnetwork::IpNetwork>,
) -> AppResult<IssuedRefresh> {
    let token_plain = new_opaque_token();
    let token_hash = hash_token(&token_plain);
    let now = Utc::now();
    let expires_at = now + Duration::days(IDLE_TIMEOUT_DAYS);

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO sessions (user_id, refresh_token_hash, issued_at, expires_at, user_agent, ip_addr) \
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(now)
    .bind(expires_at)
    .bind(user_agent)
    .bind(ip)
    .fetch_one(pool)
    .await?;

    Ok(IssuedRefresh {
        session_id: row.0,
        token_plain,
        expires_at,
    })
}

/// Look up and validate the presented refresh token.
pub async fn lookup_active(pool: &PgPool, token_plain: &str) -> AppResult<Session> {
    let token_hash = hash_token(token_plain);
    let row: Option<Session> = sqlx::query_as::<_, Session>(
        "SELECT id, user_id, issued_at, expires_at, revoked_at \
         FROM sessions WHERE refresh_token_hash = $1",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?;

    let session = row.ok_or(AppError::AuthInvalidCredentials)?;
    let now = Utc::now();
    if session.revoked_at.is_some() {
        return Err(AppError::AuthInvalidCredentials);
    }
    if session.expires_at <= now {
        return Err(AppError::AuthInvalidCredentials);
    }
    let absolute_cutoff = session.issued_at + Duration::days(ABSOLUTE_LIFETIME_DAYS);
    if now >= absolute_cutoff {
        return Err(AppError::AuthInvalidCredentials);
    }
    Ok(session)
}

pub async fn revoke(pool: &PgPool, session_id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn revoke_all_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn rotate(
    pool: &PgPool,
    presented: &Session,
    user_agent: Option<&str>,
    ip: Option<ipnetwork::IpNetwork>,
) -> AppResult<IssuedRefresh> {
    revoke(pool, presented.id).await?;
    issue(pool, presented.user_id, user_agent, ip).await
}

pub async fn is_session_active(pool: &PgPool, session_id: Uuid) -> AppResult<bool> {
    let row: Option<(Option<DateTime<Utc>>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT revoked_at, expires_at FROM sessions WHERE id = $1",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;
    Ok(match row {
        Some((revoked_at, expires_at)) => revoked_at.is_none() && expires_at > Utc::now(),
        None => false,
    })
}
