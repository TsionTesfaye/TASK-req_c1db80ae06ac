//! User + role admin DTOs (U1–U10).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::roles::Role;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserListItem {
    pub id: Uuid,
    pub display_name: String,
    pub email_mask: String,
    pub is_active: bool,
    pub locked: bool,
    pub roles: Vec<Role>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserDetail {
    pub id: Uuid,
    pub display_name: String,
    /// Admin-only: decrypted plaintext email.
    pub email: Option<String>,
    pub email_mask: String,
    pub is_active: bool,
    pub locked: bool,
    pub failed_login_count: i32,
    pub password_updated_at: DateTime<Utc>,
    pub roles: Vec<Role>,
    pub timezone: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub display_name: String,
    pub email: String,
    pub password: String,
    pub roles: Vec<Role>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UpdateUserRequest {
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub timezone: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssignRolesRequest {
    pub role_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoleDto {
    pub id: Uuid,
    pub name: String,
    pub display: String,
    pub permissions: Vec<String>,
}
