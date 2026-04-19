//! Notification center DTOs (N1–N7).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationItem {
    pub id: Uuid,
    pub topic: String,
    pub title: String,
    pub body: String,
    pub payload: serde_json::Value,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationSubscription {
    pub topic: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpsertSubscriptionsRequest {
    pub subscriptions: Vec<NotificationSubscription>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MailboxExportSummary {
    pub id: Uuid,
    pub path: String,
    pub size_bytes: i64,
    pub generated_at: DateTime<Utc>,
}
