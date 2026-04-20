//! Password verification + lockout + change-password flow.
//!
//! Login rules (design §Security):
//!   * 10 consecutive failed attempts → 15 min lock (per docs/design.md
//!     Security Decision #7 and docs/api-spec.md A1 "lockout after
//!     10/15min"). Audit #13 Issue #3 closed the previous 5-attempt drift.
//!   * Successful login resets the counter.
//!   * Lockout is wall-clock (via `users.locked_until`).
//!
//! Change-password rules:
//!   * Caller must know the current password (unless they hold
//!     `user.manage` and are updating someone else — handled in the user
//!     admin handler, not here).
//!   * Successful change → hash updated, all sessions revoked.

use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::sessions as sess,
    crypto::argon,
    errors::{AppError, AppResult},
    models::UserRow,
    services::users as user_svc,
};

/// Attempt login with a locally-validated username + password (audit
/// #4 issue 4; audit #10 issue 2 — username-only, no email fallback).
/// Returns the authenticated user row when credentials match and the
/// account is usable.
///
/// The prompt-required login contract is **username + password**. Email
/// values are not accepted as the login identifier on `/auth/login`;
/// callers that pass an email-form value will get
/// `AuthInvalidCredentials` the same as any other unknown identifier.
/// The dummy verify preserves a uniform timing profile.
pub async fn authenticate(
    pool: &PgPool,
    _email_hmac_key: &[u8; 32],
    username: &str,
    password: &str,
) -> AppResult<UserRow> {
    let candidate = user_svc::find_by_username(pool, username).await?;
    let Some(user) = candidate else {
        // Even on "no such user" we run a dummy argon2 verify to keep the
        // timing profile uniform. This is cheap and standard.
        let _ = argon::verify_password(password, "$argon2id$v=19$m=19456,t=2,p=1$\
            c2FsdHNhbHRzYWx0$wCqv8vMCRL3FQc9KfTz9c6lN5zXjxj8oVQsBTmQ1Vzw");
        return Err(AppError::AuthInvalidCredentials);
    };

    if !user.is_active {
        return Err(AppError::AuthInvalidCredentials);
    }
    if user.is_locked_now() {
        return Err(AppError::AuthLocked);
    }

    if !argon::verify_password(password, &user.password_hash) {
        let _ = user_svc::note_failed_login(pool, user.id).await?;
        return Err(AppError::AuthInvalidCredentials);
    }

    user_svc::reset_failed_login(pool, user.id).await?;
    Ok(user)
}

/// Admin/self flow: update password hash + revoke sessions.
pub async fn update_password(
    pool: &PgPool,
    user_id: Uuid,
    new_password: &str,
) -> AppResult<()> {
    validate_password_complexity(new_password)?;
    let phc = argon::hash_password(new_password)
        .map_err(|e| AppError::Internal(format!("hash: {e}")))?;
    sqlx::query(
        "UPDATE users SET password_hash = $1, password_updated_at = NOW(), \
                           failed_login_count = 0, locked_until = NULL, updated_at = NOW() \
         WHERE id = $2",
    )
    .bind(&phc)
    .bind(user_id)
    .execute(pool)
    .await?;
    sess::revoke_all_for_user(pool, user_id).await?;
    Ok(())
}

pub fn validate_password_complexity(pw: &str) -> AppResult<()> {
    if pw.chars().count() < 12 {
        return Err(AppError::Validation(
            "password must be at least 12 characters".into(),
        ));
    }
    let has_upper = pw.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = pw.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = pw.chars().any(|c| c.is_ascii_digit());
    if !(has_upper && has_lower && has_digit) {
        return Err(AppError::Validation(
            "password must contain upper, lower, and digit".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complexity_rejects_short() {
        assert!(validate_password_complexity("Short1").is_err());
    }

    #[test]
    fn complexity_rejects_missing_classes() {
        assert!(validate_password_complexity("alllowercase123").is_err());
        assert!(validate_password_complexity("ALLUPPERCASE123").is_err());
        assert!(validate_password_complexity("MixedCaseNoDigit").is_err());
    }

    #[test]
    fn complexity_accepts_ok() {
        assert!(validate_password_complexity("TerraOps!2026").is_ok());
    }
}
