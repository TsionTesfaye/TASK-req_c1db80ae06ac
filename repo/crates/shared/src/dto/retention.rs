//! Retention policy DTOs (R1–R3).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub domain: String,
    pub ttl_days: i32,
    pub last_enforced_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRetentionPolicy {
    pub ttl_days: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionRunResult {
    pub domain: String,
    pub deleted: i64,
    pub enforced_at: DateTime<Utc>,
}
