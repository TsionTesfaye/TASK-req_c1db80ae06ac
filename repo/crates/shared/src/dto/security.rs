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

/// Persisted mTLS configuration plus honest live-vs-persisted delta.
///
/// Audit #12 Issue #3: the rustls `ServerConfig` is constructed once at
/// process startup from `mtls_config.enforced`, so a PATCH to change
/// the flag only takes effect on the *next* process restart. Clients
/// (admin UI, ops scripts, tests) must be told that difference
/// honestly — not left to infer it. The DTO therefore exposes both the
/// persisted-desired flag and the startup-active flag alongside a
/// `pending_restart` boolean and a human-readable contract note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MtlsConfig {
    /// Value currently persisted in `mtls_config.enforced` — i.e. the
    /// desired steady-state enforcement flag as set by admins.
    pub enforced: bool,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<Uuid>,
    /// Value of `enforced` that was read at process startup and used to
    /// build the live rustls `ServerConfig`. Device-cert SPKI pins
    /// refresh live; this top-level mode does not.
    pub active_enforced: bool,
    /// True when `enforced != active_enforced` — i.e. a restart is
    /// required for the persisted value to become the live TLS mode.
    pub pending_restart: bool,
    /// Stable contract note suitable for UI display. Always populated.
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateMtlsConfig {
    pub enforced: bool,
}
