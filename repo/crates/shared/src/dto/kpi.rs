//! DTOs for KPI summary and drilldown endpoints (K1–K6).

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// KPI Summary (K1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KpiSummary {
    pub cycle_time_avg_hours: f64,
    pub funnel_conversion_pct: f64,
    pub anomaly_count: i64,
    pub efficiency_index: f64,
    /// Audit #13 Issue #2: % of tracked SKUs observed on-shelf in the last
    /// 24 h (averaged across all `sku_on_shelf_compliance` metric rows).
    /// 0.0 when no such metric has produced computations yet.
    pub sku_on_shelf_compliance_pct: f64,
    pub generated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Filters shared across K2–K6
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KpiSliceQuery {
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub category: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

// ---------------------------------------------------------------------------
// Cycle Time (K2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CycleTimeRow {
    pub day: NaiveDate,
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub avg_hours: f64,
    pub count: i64,
}

// ---------------------------------------------------------------------------
// Funnel (K3)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunnelStage {
    pub stage: String,
    pub count: i64,
    pub conversion_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunnelResponse {
    pub stages: Vec<FunnelStage>,
    pub overall_conversion_pct: f64,
}

// ---------------------------------------------------------------------------
// Anomalies (K4)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnomalyRow {
    pub day: NaiveDate,
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub count: i64,
}

// ---------------------------------------------------------------------------
// Efficiency (K5)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EfficiencyRow {
    pub day: NaiveDate,
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub index: f64,
}

// ---------------------------------------------------------------------------
// Drill (K6)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DrillRow {
    pub dimension: String,
    pub label: String,
    pub value: f64,
    pub metric_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DrillQuery {
    pub metric_kind: Option<String>,
    pub site_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}
