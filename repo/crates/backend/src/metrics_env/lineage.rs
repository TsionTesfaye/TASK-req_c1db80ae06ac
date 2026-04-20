//! Lineage retrieval for metric computations (MD7).
//!
//! The lineage record links a computation back to the raw observations that
//! produced it, including the formula kind, parameters, and result.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use terraops_shared::dto::metric::{ComputationLineage, LineageObservation};

pub async fn get(pool: &PgPool, computation_id: Uuid) -> AppResult<ComputationLineage> {
    #[derive(sqlx::FromRow)]
    struct CompRow {
        id: Uuid,
        definition_id: Uuid,
        computed_at: DateTime<Utc>,
        result: f64,
        inputs: Value,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
        alignment: Option<f64>,
        confidence: Option<f64>,
    }

    let comp: CompRow = sqlx::query_as(
        "SELECT id, definition_id, computed_at, result, inputs, window_start, window_end, \
                alignment, confidence \
         FROM metric_computations WHERE id = $1",
    )
    .bind(computation_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    // Fetch the parent definition for formula kind + params
    #[derive(sqlx::FromRow)]
    struct DefRow {
        formula_kind: String,
        params: Value,
    }
    let def: DefRow = sqlx::query_as(
        "SELECT formula_kind, params FROM metric_definitions WHERE id = $1",
    )
    .bind(comp.definition_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    // Parse the stored inputs array
    let input_obs: Vec<LineageObservation> = parse_inputs(&comp.inputs);

    Ok(ComputationLineage {
        computation_id: comp.id,
        definition_id: comp.definition_id,
        formula: def.formula_kind,
        params: def.params,
        input_observations: input_obs,
        result: comp.result,
        alignment: comp.alignment,
        confidence: comp.confidence,
        window_start: comp.window_start,
        window_end: comp.window_end,
        computed_at: comp.computed_at,
    })
}

/// Parse the `inputs` JSONB array stored in `metric_computations`.
/// Each element is expected to be:
/// `{"observation_id": "...", "observed_at": "...", "value": ...}`
pub(crate) fn parse_inputs(inputs: &Value) -> Vec<LineageObservation> {
    let arr = match inputs.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|item| {
            let oid = item["observation_id"]
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok())?;
            let at_str = item["observed_at"].as_str()?;
            let at: DateTime<Utc> = at_str.parse().ok()?;
            let val = item["value"].as_f64()?;
            Some(LineageObservation {
                observation_id: oid,
                observed_at: at,
                value: val,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_inputs;
    use serde_json::json;

    #[test]
    fn empty_non_array_returns_empty() {
        assert!(parse_inputs(&json!({})).is_empty());
        assert!(parse_inputs(&json!(null)).is_empty());
        assert!(parse_inputs(&json!("nope")).is_empty());
    }

    #[test]
    fn empty_array_returns_empty() {
        assert!(parse_inputs(&json!([])).is_empty());
    }

    #[test]
    fn valid_single_entry() {
        let v = json!([{
            "observation_id": "11111111-2222-3333-4444-555555555555",
            "observed_at": "2025-01-01T00:00:00Z",
            "value": 3.14
        }]);
        let out = parse_inputs(&v);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].value, 3.14);
    }

    #[test]
    fn skips_bad_entries() {
        let v = json!([
            {"observation_id":"not-a-uuid","observed_at":"2025-01-01T00:00:00Z","value":1.0},
            {"observation_id":"11111111-2222-3333-4444-555555555555","observed_at":"bogus","value":1.0},
            {"observation_id":"11111111-2222-3333-4444-555555555555","observed_at":"2025-01-01T00:00:00Z"},
            {"observation_id":"11111111-2222-3333-4444-555555555555","observed_at":"2025-01-01T00:00:00Z","value":2.5}
        ]);
        let out = parse_inputs(&v);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].value, 2.5);
    }
}
