//! DTOs for report jobs (RP1–RP6).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Report Jobs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportJobDto {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub kind: String,
    pub format: String,
    pub params: Value,
    pub cron: Option<String>,
    pub status: String,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_artifact_path: Option<String>,
    pub retry_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateReportJobRequest {
    pub kind: String,
    pub format: String,
    pub params: Option<Value>,
    pub cron: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReportRunResponse {
    pub id: Uuid,
    pub status: String,
}
