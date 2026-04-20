//! DTOs for environmental sources (E1–E6) and observations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Env Sources
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnvSourceDto {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub unit_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateEnvSourceRequest {
    pub name: String,
    pub kind: String,
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub unit_id: Option<Uuid>,
}

/// PATCH body for env sources.
///
/// `site_id`, `department_id`, and `unit_id` use tri-state semantics so the
/// analyst can reassign **or clear** master-data pointers:
///   * field omitted  → `None`            → leave as-is
///   * `"field": null`→ `Some(None)`      → clear to NULL
///   * `"field": id`  → `Some(Some(id))`  → set to `id`
///
/// See `crate::tristate::double_option` for the serde glue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct UpdateEnvSourceRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub site_id: Option<Option<Uuid>>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub department_id: Option<Option<Uuid>>,
    #[serde(
        default,
        deserialize_with = "crate::tristate::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub unit_id: Option<Option<Uuid>>,
}

// ---------------------------------------------------------------------------
// Observations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObservationDto {
    pub id: Uuid,
    pub source_id: Uuid,
    pub observed_at: DateTime<Utc>,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObservationInput {
    pub observed_at: DateTime<Utc>,
    pub value: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BulkObservationsRequest {
    pub observations: Vec<ObservationInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BulkObservationsResponse {
    pub inserted: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObservationQuery {
    pub source_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}
