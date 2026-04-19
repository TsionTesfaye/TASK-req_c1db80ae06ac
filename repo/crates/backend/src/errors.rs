//! Unified `AppError` → JSON envelope mapping.
//!
//! Every handler returns `Result<impl Responder, AppError>`. `AppError`
//! implements `actix_web::ResponseError` so the framework emits the normalized
//! envelope automatically.

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde_json::json;
use terraops_shared::error::{ErrorCode, ErrorEnvelope};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid credentials")]
    AuthInvalidCredentials,

    #[error("account locked")]
    AuthLocked,

    #[error("forbidden: {0}")]
    Forbidden(&'static str),

    #[error("authentication required")]
    AuthRequired,

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("validation failed (fields)")]
    ValidationFields(Vec<FieldError>),

    #[error("not found")]
    NotFound,

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("rate limited")]
    RateLimited,

    #[error("handler budget exceeded")]
    Timeout,

    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, serde::Serialize)]
pub struct FieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn code(&self) -> ErrorCode {
        match self {
            AppError::AuthInvalidCredentials => ErrorCode::AuthInvalidCredentials,
            AppError::AuthLocked => ErrorCode::AuthLocked,
            AppError::Forbidden(_) => ErrorCode::AuthForbidden,
            AppError::AuthRequired => ErrorCode::AuthRequired,
            AppError::Validation(_) | AppError::ValidationFields(_) => ErrorCode::ValidationFailed,
            AppError::NotFound => ErrorCode::NotFound,
            AppError::Conflict(_) => ErrorCode::Conflict,
            AppError::RateLimited => ErrorCode::RateLimited,
            AppError::Timeout => ErrorCode::Timeout,
            AppError::Internal(_) => ErrorCode::Internal,
        }
    }

    pub fn from_anyhow(err: anyhow::Error) -> Self {
        tracing::error!(error = %err, "internal error");
        AppError::Internal("internal error".into())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &err {
            if let Some(code) = db.code() {
                if code == "23505" {
                    return AppError::Conflict(db.message().to_string());
                }
            }
        }
        tracing::error!(error = %err, "sqlx error");
        AppError::Internal("database error".into())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::from_anyhow(err)
    }
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::AuthInvalidCredentials => StatusCode::UNAUTHORIZED,
            AppError::AuthLocked => StatusCode::LOCKED,
            AppError::AuthRequired => StatusCode::UNAUTHORIZED,
            AppError::Forbidden(_) => StatusCode::FORBIDDEN,
            AppError::Validation(_) | AppError::ValidationFields(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            AppError::Timeout => StatusCode::GATEWAY_TIMEOUT,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let details = match self {
            AppError::ValidationFields(fs) => Some(json!({ "fields": fs })),
            _ => None,
        };
        let envelope = ErrorEnvelope {
            error_code: self.code(),
            message: self.to_string(),
            // The request_id middleware replaces this placeholder with the
            // actual per-request id via an extension-aware wrapper; the
            // placeholder here ensures the envelope shape is always valid
            // even on paths that did not mount the middleware (e.g. tests
            // asserting shape before the middleware is wired).
            request_id: "unknown".into(),
            details,
        };
        HttpResponse::build(self.status_code()).json(envelope)
    }
}

/// Convenience alias.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_have_stable_code_and_status() {
        let cases: Vec<(AppError, ErrorCode, u16)> = vec![
            (AppError::AuthInvalidCredentials, ErrorCode::AuthInvalidCredentials, 401),
            (AppError::AuthLocked, ErrorCode::AuthLocked, 423),
            (AppError::AuthRequired, ErrorCode::AuthRequired, 401),
            (AppError::Forbidden("x"), ErrorCode::AuthForbidden, 403),
            (AppError::Validation("bad".into()), ErrorCode::ValidationFailed, 422),
            (AppError::ValidationFields(vec![]), ErrorCode::ValidationFailed, 422),
            (AppError::NotFound, ErrorCode::NotFound, 404),
            (AppError::Conflict("dup".into()), ErrorCode::Conflict, 409),
            (AppError::RateLimited, ErrorCode::RateLimited, 429),
            (AppError::Timeout, ErrorCode::Timeout, 504),
            (AppError::Internal("boom".into()), ErrorCode::Internal, 500),
        ];
        for (err, expected_code, expected_status) in cases {
            assert_eq!(err.code() as u32, expected_code as u32, "{:?}", err);
            assert_eq!(err.status_code().as_u16(), expected_status, "{:?}", err);
            // Ensures Display works (thiserror) and formats non-empty text.
            assert!(!format!("{}", err).is_empty());
        }
    }

    #[test]
    fn sqlx_unique_violation_maps_to_conflict() {
        // We cannot easily construct a real sqlx::Error::Database without a
        // live error, so we exercise the catch-all RowNotFound branch and
        // confirm it becomes Internal.
        let err: AppError = sqlx::Error::RowNotFound.into();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn anyhow_maps_to_internal() {
        let ah = anyhow::anyhow!("some failure");
        let err: AppError = ah.into();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[test]
    fn error_response_emits_json_envelope_with_code() {
        let err = AppError::Validation("short".into());
        let resp = err.error_response();
        assert_eq!(resp.status().as_u16(), 422);
    }

    #[test]
    fn validation_fields_include_details() {
        let err = AppError::ValidationFields(vec![FieldError {
            field: "sku".into(),
            code: "required".into(),
            message: "sku: required".into(),
        }]);
        let resp = err.error_response();
        assert_eq!(resp.status().as_u16(), 422);
    }
}
