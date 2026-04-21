//! User + role + permission data access.
//!
//! All lookups are by the hash-or-id path — plaintext email never hits
//! the WHERE clause; we always look up by `email_hash` (HMAC over the
//! normalized address) so the DB sees opaque bytes.

use sqlx::PgPool;
use terraops_shared::roles::Role;
use uuid::Uuid;

use crate::{
    crypto::email::{email_hash, normalize_email},
    errors::{AppError, AppResult},
    models::{UserRow, UserWithRoles},
};

pub async fn find_by_id(pool: &PgPool, user_id: Uuid) -> AppResult<Option<UserRow>> {
    let row: Option<UserRow> = sqlx::query_as::<_, UserRow>(
        "SELECT id, display_name, username, email_ciphertext, email_hash, email_mask, password_hash, \
                password_updated_at, is_active, failed_login_count, locked_until, timezone, \
                created_at, updated_at \
         FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Find a user by plaintext email → normalized HMAC lookup.
///
/// Audit #10 issue #2: this helper is **not** used by `/auth/login`
/// anymore — the sign-in contract is username-only with no email
/// fallback. It remains here for admin-only flows that may need to
/// resolve an email to a user id outside the login path.
#[allow(dead_code)]
pub async fn find_by_email(
    pool: &PgPool,
    email_plain: &str,
    hmac_key: &[u8; 32],
) -> AppResult<Option<UserRow>> {
    let normalized = normalize_email(email_plain);
    let hash = email_hash(&normalized, hmac_key).to_vec();
    let row: Option<UserRow> = sqlx::query_as::<_, UserRow>(
        "SELECT id, display_name, username, email_ciphertext, email_hash, email_mask, password_hash, \
                password_updated_at, is_active, failed_login_count, locked_until, timezone, \
                created_at, updated_at \
         FROM users WHERE email_hash = $1",
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Find a user by case-insensitive username. This is the primary
/// lookup used by the login handler (audit #4 issue 4: sign-in contract
/// is locally-validated username + password, not email).
pub async fn find_by_username(
    pool: &PgPool,
    username: &str,
) -> AppResult<Option<UserRow>> {
    let uname = username.trim().to_lowercase();
    if uname.is_empty() {
        return Ok(None);
    }
    let row: Option<UserRow> = sqlx::query_as::<_, UserRow>(
        "SELECT id, display_name, username, email_ciphertext, email_hash, email_mask, password_hash, \
                password_updated_at, is_active, failed_login_count, locked_until, timezone, \
                created_at, updated_at \
         FROM users WHERE LOWER(username) = $1",
    )
    .bind(&uname)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn roles_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<Role>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT r.name FROM roles r \
         JOIN user_roles ur ON ur.role_id = r.id \
         WHERE ur.user_id = $1 ORDER BY r.name",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let mut roles = Vec::with_capacity(rows.len());
    for (name,) in rows {
        if let Some(r) = db_name_to_role(&name) {
            roles.push(r);
        }
    }
    Ok(roles)
}

pub fn db_name_to_role(name: &str) -> Option<Role> {
    match name {
        "administrator" => Some(Role::Administrator),
        "data_steward" => Some(Role::DataSteward),
        "analyst" => Some(Role::Analyst),
        "recruiter" => Some(Role::Recruiter),
        "regular_user" => Some(Role::RegularUser),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_name_to_role_covers_all_known_names() {
        assert_eq!(db_name_to_role("administrator"), Some(Role::Administrator));
        assert_eq!(db_name_to_role("data_steward"),  Some(Role::DataSteward));
        assert_eq!(db_name_to_role("analyst"),        Some(Role::Analyst));
        assert_eq!(db_name_to_role("recruiter"),      Some(Role::Recruiter));
        assert_eq!(db_name_to_role("regular_user"),   Some(Role::RegularUser));
    }

    #[test]
    fn db_name_to_role_unknown_returns_none() {
        assert_eq!(db_name_to_role("super_admin"), None);
        assert_eq!(db_name_to_role(""),            None);
        assert_eq!(db_name_to_role("ADMINISTRATOR"), None); // case-sensitive
    }

    #[test]
    fn lockout_constants_match_design_contract() {
        // Security Decision #7: 10 failures / 15-min window.
        assert_eq!(LOCKOUT_FAILURE_THRESHOLD, 10);
        assert_eq!(LOCKOUT_DURATION_MINUTES, 15);
    }
}

pub async fn permissions_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT p.code FROM permissions p \
         JOIN role_permissions rp ON rp.permission_id = p.id \
         JOIN user_roles ur       ON ur.role_id = rp.role_id \
         WHERE ur.user_id = $1 ORDER BY p.code",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(c,)| c).collect())
}

pub async fn load_with_roles(pool: &PgPool, user_id: Uuid) -> AppResult<UserWithRoles> {
    let user = find_by_id(pool, user_id)
        .await?
        .ok_or(AppError::NotFound)?;
    let roles = roles_for_user(pool, user_id).await?;
    let permissions = permissions_for_user(pool, user_id).await?;
    Ok(UserWithRoles {
        user,
        roles,
        permissions,
    })
}

/// Lockout threshold — number of consecutive failed login attempts
/// required before the user is locked out. Aligned with
/// `docs/design.md` Security Decision #7 ("Lockout after 10 failures
/// / 15-min rolling window") and `docs/api-spec.md` A1 row
/// ("lockout after 10/15min"). Audit #13 Issue #3 closed the drift —
/// previous code used 5, which contradicted the documented contract
/// and the A1 login spec. Any change here must update both docs.
pub const LOCKOUT_FAILURE_THRESHOLD: i32 = 10;

/// Lockout duration applied when the threshold is reached.
pub const LOCKOUT_DURATION_MINUTES: i32 = 15;

/// Increment failed login counter. Returns new count.
///
/// Locks the account (`locked_until = NOW() + 15 min`) when the
/// post-increment count reaches `LOCKOUT_FAILURE_THRESHOLD`.
pub async fn note_failed_login(pool: &PgPool, user_id: Uuid) -> AppResult<i32> {
    let row: (i32,) = sqlx::query_as(
        "UPDATE users SET failed_login_count = failed_login_count + 1, \
                           locked_until = CASE WHEN failed_login_count + 1 >= $2 \
                                               THEN NOW() + ($3::int || ' minutes')::interval \
                                               ELSE locked_until END \
         WHERE id = $1 RETURNING failed_login_count",
    )
    .bind(user_id)
    .bind(LOCKOUT_FAILURE_THRESHOLD)
    .bind(LOCKOUT_DURATION_MINUTES)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

pub async fn reset_failed_login(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
    sqlx::query(
        "UPDATE users SET failed_login_count = 0, locked_until = NULL WHERE id = $1",
    )
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}
