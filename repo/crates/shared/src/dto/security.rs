//! Admin security DTOs (SEC1–SEC9): IP allowlist, device certs, mTLS config.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllowlistEntry {
    pub id: Uuid,
    pub cidr: String,
    pub note: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateAllowlistEntry {
    pub cidr: String,
    pub note: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceCert {
    pub id: Uuid,
    pub label: String,
    pub issued_to_user_id: Option<Uuid>,
    pub issued_to_display: Option<String>,
    pub serial: Option<String>,
    /// Hex-encoded SHA-256 SPKI pin (lowercase, no separators).
    pub spki_sha256_hex: String,
    pub pem_path: Option<String>,
    pub notes: Option<String>,
    pub issued_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegisterDeviceCert {
    pub label: String,
    pub issued_to_user_id: Option<Uuid>,
    pub serial: Option<String>,
    /// Hex-encoded SHA-256 SPKI pin.
    pub spki_sha256_hex: String,
    pub pem_path: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MtlsConfig {
    pub enforced: bool,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateMtlsConfig {
    pub enforced: bool,
}
