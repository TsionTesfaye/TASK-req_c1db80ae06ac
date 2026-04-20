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
    /// Underlying metric_computations row id, so the UI can link each
    /// series point to its full lineage (`/metrics/computations/{id}/lineage`).
    /// Scalar-formula live points that are not persisted as a computation
    /// return `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computation_id: Option<Uuid>,
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
    /// Alignment quality of the contributing sources (0..1). Present for
    /// multi-source formulas such as `comfort_index`; `None` for scalar
    /// formulas that operate on a single source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<f64>,
    /// Confidence in the computed value (0..1). Combines source-count and
    /// sample-density factors. `None` for scalar formulas.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub computed_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formula_kind_str_roundtrip_for_every_variant() {
        for (k, s) in [
            (FormulaKind::MovingAverage, "moving_average"),
            (FormulaKind::RateOfChange, "rate_of_change"),
            (FormulaKind::ComfortIndex, "comfort_index"),
        ] {
            assert_eq!(k.as_str(), s);
            assert_eq!(FormulaKind::from_str(s), Some(k.clone()));
        }
        assert_eq!(FormulaKind::from_str("bogus"), None);
        assert_eq!(FormulaKind::from_str(""), None);
    }

    #[test]
    fn formula_kind_serde_uses_snake_case() {
        let s = serde_json::to_string(&FormulaKind::ComfortIndex).unwrap();
        assert_eq!(s, "\"comfort_index\"");
        let back: FormulaKind = serde_json::from_str(&s).unwrap();
        assert_eq!(back, FormulaKind::ComfortIndex);
    }

    #[test]
    fn series_point_roundtrip() {
        let at = chrono::Utc::now();
        let sp = SeriesPoint { at, value: 42.5, computation_id: None };
        let j = serde_json::to_value(&sp).unwrap();
        let back: SeriesPoint = serde_json::from_value(j).unwrap();
        assert_eq!(back.value, 42.5);
        assert_eq!(back.at, at);
        assert_eq!(back.computation_id, None);
        // When None, the field is omitted from JSON entirely.
        assert!(!serde_json::to_string(&sp).unwrap().contains("computation_id"));
    }

    #[test]
    fn series_point_carries_computation_id() {
        let at = chrono::Utc::now();
        let cid = Uuid::new_v4();
        let sp = SeriesPoint { at, value: 1.0, computation_id: Some(cid) };
        let s = serde_json::to_string(&sp).unwrap();
        assert!(s.contains("computation_id"));
        let back: SeriesPoint = serde_json::from_str(&s).unwrap();
        assert_eq!(back.computation_id, Some(cid));
    }
}
