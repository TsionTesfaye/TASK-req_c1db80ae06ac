//! Unified error envelope shared by backend and frontend.

use serde::{Deserialize, Serialize};

/// Canonical error codes emitted in the response envelope. The backend maps
/// these to HTTP status codes; the frontend renders them to localized
/// messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    AuthInvalidCredentials,
    AuthLocked,
    AuthForbidden,
    AuthRequired,
    ValidationFailed,
    NotFound,
    Conflict,
    RateLimited,
    Timeout,
    Internal,
}

/// Envelope body returned for every non-success JSON response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub error_code: ErrorCode,
    pub message: String,
    pub request_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}
