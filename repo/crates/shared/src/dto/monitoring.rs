//! Admin monitoring DTOs (M1–M4).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBucket {
    pub route: String,
    pub method: String,
    pub count: i64,
    pub p50_ms: i64,
    pub p95_ms: i64,
    pub p99_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBucket {
    pub route: String,
    pub method: String,
    pub total: i64,
    pub errors: i64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashReport {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub page: Option<String>,
    pub agent: Option<String>,
    pub stack: Option<String>,
    pub payload: serde_json::Value,
    pub reported_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestCrashReport {
    pub page: Option<String>,
    pub agent: Option<String>,
    pub stack: Option<String>,
    pub payload: Option<serde_json::Value>,
}
