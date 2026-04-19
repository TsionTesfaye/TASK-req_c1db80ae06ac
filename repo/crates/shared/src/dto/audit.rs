//! Audit log DTOs (U10).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub actor_id: Option<Uuid>,
    pub actor_display: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub meta: serde_json::Value,
    pub at: DateTime<Utc>,
}
