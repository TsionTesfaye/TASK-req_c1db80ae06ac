//! DTOs for metric definitions, computations, and lineage (MD1–MD7).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Metric Definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FormulaKind {
    MovingAverage,
    RateOfChange,
    ComfortIndex,
}

impl FormulaKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            FormulaKind::MovingAverage => "moving_average",
            FormulaKind::RateOfChange => "rate_of_change",
            FormulaKind::ComfortIndex => "comfort_index",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "moving_average" => Some(FormulaKind::MovingAverage),
            "rate_of_change" => Some(FormulaKind::RateOfChange),
            "comfort_index" => Some(FormulaKind::ComfortIndex),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricDefinitionDto {
    pub id: Uuid,
    pub name: String,
    pub formula_kind: String,
    pub params: Value,
    pub source_ids: Vec<Uuid>,
    pub window_seconds: i32,
    pub enabled: bool,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateMetricDefinitionRequest {
    pub name: String,
    pub formula_kind: String,
    pub params: Option<Value>,
    pub source_ids: Vec<Uuid>,
    pub window_seconds: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateMetricDefinitionRequest {
    pub name: Option<String>,
    pub formula_kind: Option<String>,
    pub params: Option<Value>,
    pub source_ids: Option<Vec<Uuid>>,
    pub window_seconds: Option<i32>,
    pub enabled: Option<bool>,
}

// ---------------------------------------------------------------------------
// Computations + Series
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeriesPoint {
    pub at: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricSeriesResponse {
    pub definition_id: Uuid,
    pub formula_kind: String,
    pub window_seconds: i32,
    pub points: Vec<SeriesPoint>,
}

// ---------------------------------------------------------------------------
// Lineage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LineageObservation {
    pub observation_id: Uuid,
    pub observed_at: DateTime<Utc>,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputationLineage {
    pub computation_id: Uuid,
    pub definition_id: Uuid,
    pub formula: String,
    pub params: Value,
    pub input_observations: Vec<LineageObservation>,
    pub result: f64,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub computed_at: DateTime<Utc>,
}
