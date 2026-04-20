//! User row + role-resolved composite view.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use terraops_shared::roles::Role;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub display_name: String,
    pub username: String,
    pub email_ciphertext: Vec<u8>,
    pub email_hash: Vec<u8>,
    pub email_mask: String,
    pub password_hash: String,
    pub password_updated_at: DateTime<Utc>,
    pub is_active: bool,
    pub failed_login_count: i32,
    pub locked_until: Option<DateTime<Utc>>,
    pub timezone: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UserRow {
    pub fn is_locked_now(&self) -> bool {
        self.locked_until
            .map(|t| t > Utc::now())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub struct UserWithRoles {
    pub user: UserRow,
    pub roles: Vec<Role>,
    pub permissions: Vec<String>,
}
