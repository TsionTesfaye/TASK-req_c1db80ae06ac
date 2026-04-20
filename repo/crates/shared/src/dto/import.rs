//! Import batch DTOs (I1–I7).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportBatchSummary {
    pub id: Uuid,
    pub uploaded_by: Uuid,
    pub filename: String,
    pub kind: String,
    pub status: String,
    pub row_count: i32,
    pub error_count: i32,
    pub created_at: DateTime<Utc>,
    pub committed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportRowDto {
    pub id: Uuid,
    pub batch_id: Uuid,
    pub row_number: i32,
    pub raw: serde_json::Value,
    pub errors: serde_json::Value,
    pub valid: bool,
}

/// Response returned by `POST /imports/{id}/validate` (I5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportValidateResult {
    pub id: Uuid,
    pub error_count: i32,
    pub status: String,
}

/// Response returned by `POST /imports/{id}/commit` (I6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportCommitResult {
    pub id: Uuid,
    pub inserted: i32,
    pub status: String,
}

/// Response returned by `POST /imports/{id}/cancel` (I7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportCancelResult {
    pub id: Uuid,
    pub status: String,
}
