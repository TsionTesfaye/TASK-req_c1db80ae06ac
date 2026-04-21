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

/// Audit #4 Issue #3: analyst-configurable alignment rules for the
/// comfort-index fusion formula. The analyst declares how strict the
/// temporal-alignment gating is (`min_alignment` — computations with an
/// alignment score below this float are discarded, not persisted) and
/// which confidence label ranges the dashboard should paint.
///
/// Stored inside `metric_definitions.params` under the `alignment` key
/// so it round-trips through the existing JSONB column. The parser in
/// `from_params_value` tolerates an absent block by returning defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlignmentRules {
    /// Minimum alignment (0..=1) required to persist a live computation.
    /// Fresh points with a lower alignment are dropped with a
    /// `low_alignment` tag so the dashboard surfaces the gap rather
    /// than silently smoothing over drift. Default: `0.25`.
    pub min_alignment: f64,
    /// Soft threshold: computations above `warn_alignment` render as
    /// `ok`; below render as `warn`. Default: `0.75`.
    pub warn_alignment: f64,
    /// When `true` the persist path refuses to write a computation with
    /// `alignment < min_alignment` (the default, strict mode). When
    /// `false` the value is still written but flagged with a
    /// low-alignment note for operator review.
    pub strict: bool,
}

impl Default for AlignmentRules {
    fn default() -> Self {
        Self {
            min_alignment: 0.25,
            warn_alignment: 0.75,
            strict: true,
        }
    }
}

/// Audit #4 Issue #3: analyst-configurable confidence label mapping.
/// Each row says "confidence values in `[min, max]` render with `label`
/// and CSS class `css_class`". The list is ordered and the first
/// matching band wins. `min` is inclusive; `max` is inclusive for the
/// last band and exclusive otherwise so bands are contiguous.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfidenceLabel {
    pub label: String,
    pub min: f64,
    pub max: f64,
    /// Short CSS class tag such as `ok`, `warn`, `bad`. Rendered as
    /// `tx-chip tx-chip--{css_class}` in the SPA. Defaults use the
    /// existing chip palette; analyst-supplied values are validated to
    /// be ASCII `[a-z0-9_-]+`.
    pub css_class: String,
}

/// Full analyst configuration embedded in a metric definition's
/// `params` JSONB. Separate from `CreateMetricDefinitionRequest` so the
/// backend can treat it as an optional block; missing → defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FusionConfig {
    pub alignment: AlignmentRules,
    pub confidence_labels: Vec<ConfidenceLabel>,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            alignment: AlignmentRules::default(),
            confidence_labels: default_confidence_labels(),
        }
    }
}

/// Three-band defaults that match the existing dashboard palette.
pub fn default_confidence_labels() -> Vec<ConfidenceLabel> {
    vec![
        ConfidenceLabel {
            label: "high".into(),
            min: 0.80,
            max: 1.01,
            css_class: "ok".into(),
        },
        ConfidenceLabel {
            label: "medium".into(),
            min: 0.50,
            max: 0.80,
            css_class: "warn".into(),
        },
        ConfidenceLabel {
            label: "low".into(),
            min: 0.0,
            max: 0.50,
            css_class: "bad".into(),
        },
    ]
}

impl FusionConfig {
    /// Parse from a `metric_definitions.params` JSONB value. Missing
    /// keys fall back to defaults. Returns a `String` error that the
    /// backend maps to `AppError::Validation`.
    pub fn from_params_value(v: &Value) -> Result<Self, String> {
        let mut cfg = FusionConfig::default();
        let obj = match v {
            Value::Object(m) => m,
            Value::Null => return Ok(cfg),
            _ => return Err("params must be a JSON object".into()),
        };
        if let Some(a) = obj.get("alignment") {
            let ar: AlignmentRules = serde_json::from_value(a.clone())
                .map_err(|e| format!("alignment: {e}"))?;
            if !(0.0..=1.0).contains(&ar.min_alignment) {
                return Err("alignment.min_alignment must be in [0,1]".into());
            }
            if !(0.0..=1.0).contains(&ar.warn_alignment) {
                return Err("alignment.warn_alignment must be in [0,1]".into());
            }
            if ar.warn_alignment < ar.min_alignment {
                return Err("alignment.warn_alignment must be >= min_alignment".into());
            }
            cfg.alignment = ar;
        }
        if let Some(cl) = obj.get("confidence_labels") {
            let bands: Vec<ConfidenceLabel> = serde_json::from_value(cl.clone())
                .map_err(|e| format!("confidence_labels: {e}"))?;
            if bands.is_empty() {
                return Err("confidence_labels must not be empty when provided".into());
            }
            for b in &bands {
                if b.label.trim().is_empty() {
                    return Err("confidence_labels.label must be non-empty".into());
                }
                if !(0.0..=1.01).contains(&b.min) || !(0.0..=1.01).contains(&b.max) {
                    return Err("confidence_labels.min/max must be in [0,1.01]".into());
                }
                if b.max <= b.min {
                    return Err("confidence_labels.max must be greater than min".into());
                }
                if !b
                    .css_class
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                    || b.css_class.is_empty()
                {
                    return Err(
                        "confidence_labels.css_class must match [A-Za-z0-9_-]+".into()
                    );
                }
            }
            cfg.confidence_labels = bands;
        }
        Ok(cfg)
    }

    /// Resolve a numeric confidence score to the analyst-declared
    /// label, falling back to `"unknown"/"neutral"` when no band
    /// matches. Used by both server-side lineage decoration and the
    /// frontend summary surface.
    pub fn label_for(&self, confidence: f64) -> (String, String) {
        for b in &self.confidence_labels {
            if confidence >= b.min && confidence < b.max {
                return (b.label.clone(), b.css_class.clone());
            }
            if (b.max - 1.01).abs() < 1e-9 && confidence >= b.min {
                // Last band: max is treated as inclusive.
                return (b.label.clone(), b.css_class.clone());
            }
        }
        ("unknown".into(), "neutral".into())
    }
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

    // ── FusionConfig / AlignmentRules / default_confidence_labels ─────────────

    #[test]
    fn alignment_rules_default_is_sane() {
        let ar = AlignmentRules::default();
        assert_eq!(ar.min_alignment, 0.25);
        assert_eq!(ar.warn_alignment, 0.75);
        assert!(ar.strict);
    }

    #[test]
    fn default_confidence_labels_produces_three_bands() {
        let bands = default_confidence_labels();
        assert_eq!(bands.len(), 3);
        assert_eq!(bands[0].label, "high");
        assert_eq!(bands[1].label, "medium");
        assert_eq!(bands[2].label, "low");
    }

    #[test]
    fn fusion_config_default_combines_defaults() {
        let cfg = FusionConfig::default();
        assert_eq!(cfg.alignment, AlignmentRules::default());
        assert_eq!(cfg.confidence_labels.len(), 3);
    }

    #[test]
    fn from_params_null_returns_default() {
        let cfg = FusionConfig::from_params_value(&serde_json::Value::Null).unwrap();
        assert_eq!(cfg.alignment.min_alignment, 0.25);
    }

    #[test]
    fn from_params_non_object_returns_error() {
        let err = FusionConfig::from_params_value(&serde_json::json!([1, 2])).unwrap_err();
        assert!(err.contains("JSON object"), "error: {err}");
    }

    #[test]
    fn from_params_empty_object_returns_default() {
        let cfg = FusionConfig::from_params_value(&serde_json::json!({})).unwrap();
        assert_eq!(cfg.alignment.min_alignment, 0.25);
        assert_eq!(cfg.confidence_labels.len(), 3);
    }

    #[test]
    fn from_params_alignment_overrides_defaults() {
        let v = serde_json::json!({
            "alignment": { "min_alignment": 0.1, "warn_alignment": 0.6, "strict": false }
        });
        let cfg = FusionConfig::from_params_value(&v).unwrap();
        assert_eq!(cfg.alignment.min_alignment, 0.1);
        assert_eq!(cfg.alignment.warn_alignment, 0.6);
        assert!(!cfg.alignment.strict);
    }

    #[test]
    fn from_params_rejects_min_alignment_out_of_range() {
        let v = serde_json::json!({
            "alignment": { "min_alignment": 1.5, "warn_alignment": 0.8, "strict": true }
        });
        let err = FusionConfig::from_params_value(&v).unwrap_err();
        assert!(err.contains("min_alignment"), "error: {err}");
    }

    #[test]
    fn from_params_rejects_warn_alignment_out_of_range() {
        let v = serde_json::json!({
            "alignment": { "min_alignment": 0.2, "warn_alignment": -0.1, "strict": true }
        });
        let err = FusionConfig::from_params_value(&v).unwrap_err();
        assert!(err.contains("warn_alignment"), "error: {err}");
    }

    #[test]
    fn from_params_rejects_warn_below_min() {
        let v = serde_json::json!({
            "alignment": { "min_alignment": 0.8, "warn_alignment": 0.5, "strict": true }
        });
        let err = FusionConfig::from_params_value(&v).unwrap_err();
        assert!(err.contains("warn_alignment"), "error: {err}");
    }

    #[test]
    fn from_params_confidence_labels_override() {
        let v = serde_json::json!({
            "confidence_labels": [
                { "label": "good", "min": 0.7, "max": 1.01, "css_class": "ok" },
                { "label": "bad",  "min": 0.0, "max": 0.7,  "css_class": "warn" }
            ]
        });
        let cfg = FusionConfig::from_params_value(&v).unwrap();
        assert_eq!(cfg.confidence_labels.len(), 2);
        assert_eq!(cfg.confidence_labels[0].label, "good");
    }

    #[test]
    fn from_params_rejects_empty_confidence_labels() {
        let v = serde_json::json!({ "confidence_labels": [] });
        let err = FusionConfig::from_params_value(&v).unwrap_err();
        assert!(err.contains("empty"), "error: {err}");
    }

    #[test]
    fn from_params_rejects_blank_label_name() {
        let v = serde_json::json!({
            "confidence_labels": [{ "label": "  ", "min": 0.0, "max": 1.0, "css_class": "ok" }]
        });
        let err = FusionConfig::from_params_value(&v).unwrap_err();
        assert!(err.contains("non-empty"), "error: {err}");
    }

    #[test]
    fn from_params_rejects_invalid_band_range() {
        let v = serde_json::json!({
            "confidence_labels": [{ "label": "x", "min": 0.5, "max": 0.3, "css_class": "ok" }]
        });
        let err = FusionConfig::from_params_value(&v).unwrap_err();
        assert!(err.contains("max must be greater"), "error: {err}");
    }

    #[test]
    fn from_params_rejects_invalid_css_class() {
        let v = serde_json::json!({
            "confidence_labels": [{ "label": "x", "min": 0.0, "max": 1.0, "css_class": "bad class!" }]
        });
        let err = FusionConfig::from_params_value(&v).unwrap_err();
        assert!(err.contains("css_class"), "error: {err}");
    }

    #[test]
    fn from_params_rejects_out_of_range_band_min_max() {
        let v = serde_json::json!({
            "confidence_labels": [{ "label": "x", "min": -0.1, "max": 1.0, "css_class": "ok" }]
        });
        let err = FusionConfig::from_params_value(&v).unwrap_err();
        assert!(err.contains("min/max"), "error: {err}");
    }

    #[test]
    fn label_for_maps_high_band() {
        let cfg = FusionConfig::default();
        let (label, css) = cfg.label_for(0.95);
        assert_eq!(label, "high");
        assert_eq!(css, "ok");
    }

    #[test]
    fn label_for_maps_medium_band() {
        let cfg = FusionConfig::default();
        let (label, css) = cfg.label_for(0.65);
        assert_eq!(label, "medium");
        assert_eq!(css, "warn");
    }

    #[test]
    fn label_for_maps_low_band() {
        let cfg = FusionConfig::default();
        let (label, css) = cfg.label_for(0.3);
        assert_eq!(label, "low");
        assert_eq!(css, "bad");
    }

    #[test]
    fn label_for_last_band_inclusive_at_max() {
        // The high band has max=1.01; values >= 0.80 and exactly at the boundary should resolve.
        let cfg = FusionConfig::default();
        let (label, _css) = cfg.label_for(1.0);
        assert_eq!(label, "high");
    }

    #[test]
    fn label_for_returns_unknown_when_no_band_matches() {
        let cfg = FusionConfig {
            alignment: AlignmentRules::default(),
            confidence_labels: vec![],
        };
        let (label, css) = cfg.label_for(0.5);
        assert_eq!(label, "unknown");
        assert_eq!(css, "neutral");
    }
}
