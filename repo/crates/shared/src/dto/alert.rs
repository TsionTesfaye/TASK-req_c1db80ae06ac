//! DTOs for alert rules, events, and acknowledgement (AL1–AL6).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Alert Rules
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertRuleDto {
    pub id: Uuid,
    pub metric_definition_id: Uuid,
    pub threshold: f64,
    pub operator: String,
    pub duration_seconds: i32,
    pub severity: String,
    pub enabled: bool,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateAlertRuleRequest {
    pub metric_definition_id: Uuid,
    pub threshold: f64,
    pub operator: String,
    pub duration_seconds: Option<i32>,
    pub severity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateAlertRuleRequest {
    pub threshold: Option<f64>,
    pub operator: Option<String>,
    pub duration_seconds: Option<i32>,
    pub severity: Option<String>,
    pub enabled: Option<bool>,
}

// ---------------------------------------------------------------------------
// Alert Events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertEventDto {
    pub id: Uuid,
    pub rule_id: Uuid,
    pub fired_at: DateTime<Utc>,
    pub value: f64,
    pub acked_at: Option<DateTime<Utc>>,
    pub acked_by: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    /// Severity from the parent rule — denormalized for display convenience.
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AckAlertEventResponse {
    pub id: Uuid,
    pub acked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertEventQuery {
    pub rule_id: Option<Uuid>,
    pub unacked_only: Option<bool>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}
