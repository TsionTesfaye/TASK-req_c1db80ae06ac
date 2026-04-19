//! Auth / session DTOs (A1–A5).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::roles::Role;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub access_expires_at: DateTime<Utc>,
    pub user: AuthUserDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefreshResponse {
    pub access_token: String,
    pub access_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthUserDto {
    pub id: Uuid,
    pub display_name: String,
    pub email: Option<String>,
    pub email_mask: String,
    pub roles: Vec<Role>,
    pub permissions: Vec<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}
