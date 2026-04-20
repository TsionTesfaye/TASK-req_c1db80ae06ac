//! KPI data access — computes KPI metrics on-demand from existing tables.
//!
//! Design choice: KPI values are computed on-demand from the `alert_events`
//! and `metric_computations` tables rather than requiring a pre-populated
//! `kpi_rollup_daily`. This keeps P-B self-contained. The rollup job would
//! write to `kpi_rollup_daily` as an optimisation; the handlers fall back to
//! on-demand queries when no cached row exists.
//!
//! Filter semantics (audit #3 Issue 1):
//!   * `site_id` / `department_id` — a metric definition "belongs to" a site
//!     or department when any of its `source_ids` resolves to an
//!     `env_sources` row whose `site_id` / `department_id` matches the
//!     filter. `alert_events` inherit the site/dept of their rule's metric
//!     definition. `metric_computations` inherit from their own
//!     `definition_id`.
//!   * `category` — matches `metric_definitions.formula_kind` (the honest
//!     KPI-level category axis: `moving_average`, `rate_of_change`,
//!     `comfort_index`). Free-text category values that do not match any
//!     known formula_kind simply return no rows, which is the correct
//!     behavior for an unknown slice.

use chrono::{NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppResult;
use terraops_shared::dto::kpi::{
    AnomalyRow, CycleTimeRow, DrillRow, EfficiencyRow, FunnelResponse, FunnelStage, KpiSummary,
};

// ---------------------------------------------------------------------------
// K1 — KPI Summary
// ---------------------------------------------------------------------------

pub async fn summary(pool: &PgPool) -> AppResult<KpiSummary> {
    // cycle_time_avg: average number of hours between alert_events.fired_at
    // and alert_events.resolved_at across all events in the past 30 days.
    let cycle_hours: (Option<f64>,) = sqlx::query_as(
        "SELECT AVG(EXTRACT(EPOCH FROM (resolved_at - fired_at)) / 3600.0)::FLOAT8 \
         FROM alert_events \
         WHERE resolved_at IS NOT NULL \
           AND fired_at >= NOW() - INTERVAL '30 days'",
    )
    .fetch_one(pool)
    .await?;

    // funnel_conversion: ratio of acked events to total events in past 30 days.
    let (total, acked): (i64, i64) = {
        let (t,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*)::BIGINT FROM alert_events \
             WHERE fired_at >= NOW() - INTERVAL '30 days'",
        )
        .fetch_one(pool)
        .await?;
        let (a,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*)::BIGINT FROM alert_events \
             WHERE fired_at >= NOW() - INTERVAL '30 days' AND acked_at IS NOT NULL",
        )
        .fetch_one(pool)
        .await?;
        (t, a)
    };
    let funnel_pct = if total > 0 {
        (acked as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    // anomaly_count: unresolved alert events in the past 30 days.
    let (anomaly_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM alert_events \
         WHERE resolved_at IS NULL AND fired_at >= NOW() - INTERVAL '30 days'",
    )
    .fetch_one(pool)
    .await?;

    // efficiency_index: average metric computation result (as a proxy for
    // operational efficiency) from the past 24 hours. Falls back to 0.
    let efficiency: (Option<f64>,) = sqlx::query_as(
        "SELECT AVG(result) FROM metric_computations \
         WHERE computed_at >= NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(pool)
    .await?;

    // Audit #13 Issue #2: sku_on_shelf_compliance_pct — average of
    // metric_computations.result for metric definitions whose formula_kind
    // is 'sku_on_shelf_compliance', over the past 24 h. Falls back to 0.
    let sku_compliance: (Option<f64>,) = sqlx::query_as(
        "SELECT AVG(mc.result) FROM metric_computations mc \
         JOIN metric_definitions md ON md.id = mc.definition_id \
         WHERE md.formula_kind = 'sku_on_shelf_compliance' \
           AND mc.computed_at >= NOW() - INTERVAL '24 hours'",
    )
    .fetch_one(pool)
    .await?;

    Ok(KpiSummary {
        cycle_time_avg_hours: cycle_hours.0.unwrap_or(0.0),
        funnel_conversion_pct: funnel_pct,
        anomaly_count,
        efficiency_index: efficiency.0.unwrap_or(0.0),
        sku_on_shelf_compliance_pct: sku_compliance.0.unwrap_or(0.0),
        generated_at: Utc::now(),
    })
}

// ---------------------------------------------------------------------------
// Shared slice filter — alert_events / metric_computations joined through
// metric_definitions.source_ids → env_sources.
// ---------------------------------------------------------------------------
//
// The three site/department/category predicates below are attached with
// boolean short-circuit: when the filter is NULL the predicate is TRUE
// (no filtering). When present, the filter is applied as an EXISTS join.
//
// `ae_def_id_expr` is the metric_definition_id for alert_events — reached
// via `alert_rules.metric_definition_id`. `mc_def_id_expr` is
// `metric_computations.definition_id`.

/// Filter fragment for alert-events queries. Expects the following bind
/// order in the outer SQL: `$site, $dept, $category`. Must be wrapped with
/// a rule-id-to-definition-id join via the `def_ids_for_alert_events` CTE
/// declared in every query using it.
const ALERT_DEF_FILTER: &str = "\
    AND ($1::uuid IS NULL OR EXISTS (\
        SELECT 1 FROM metric_definitions md \
        JOIN alert_rules ar ON ar.metric_definition_id = md.id \
        JOIN env_sources es ON es.id = ANY(md.source_ids) \
        WHERE ar.id = ae.rule_id AND es.site_id = $1)) \
    AND ($2::uuid IS NULL OR EXISTS (\
        SELECT 1 FROM metric_definitions md \
        JOIN alert_rules ar ON ar.metric_definition_id = md.id \
        JOIN env_sources es ON es.id = ANY(md.source_ids) \
        WHERE ar.id = ae.rule_id AND es.department_id = $2)) \
    AND ($3::text IS NULL OR EXISTS (\
        SELECT 1 FROM metric_definitions md \
        JOIN alert_rules ar ON ar.metric_definition_id = md.id \
        WHERE ar.id = ae.rule_id AND md.formula_kind = $3))";

/// Filter fragment for metric_computations queries. Same bind order
/// `$site, $dept, $category`.
const MC_DEF_FILTER: &str = "\
    AND ($1::uuid IS NULL OR EXISTS (\
        SELECT 1 FROM metric_definitions md \
        JOIN env_sources es ON es.id = ANY(md.source_ids) \
        WHERE md.id = mc.definition_id AND es.site_id = $1)) \
    AND ($2::uuid IS NULL OR EXISTS (\
        SELECT 1 FROM metric_definitions md \
        JOIN env_sources es ON es.id = ANY(md.source_ids) \
        WHERE md.id = mc.definition_id AND es.department_id = $2)) \
    AND ($3::text IS NULL OR EXISTS (\
        SELECT 1 FROM metric_definitions md \
        WHERE md.id = mc.definition_id AND md.formula_kind = $3))";

// ---------------------------------------------------------------------------
// K2 — Cycle Time
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub async fn cycle_time(
    pool: &PgPool,
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    category: Option<&str>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<CycleTimeRow>, i64)> {
    #[derive(sqlx::FromRow)]
    struct Row {
        day: NaiveDate,
        avg_hours: Option<f64>,
        cnt: i64,
    }

    let from_ts = from.map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    let to_ts = to.map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());

    let sql = format!(
        "SELECT DATE(ae.fired_at AT TIME ZONE 'UTC') AS day, \
                (AVG(EXTRACT(EPOCH FROM (ae.resolved_at - ae.fired_at)) / 3600.0))::FLOAT8 AS avg_hours, \
                COUNT(*)::BIGINT AS cnt \
         FROM alert_events ae \
         WHERE ae.resolved_at IS NOT NULL \
           AND ($4::timestamptz IS NULL OR ae.fired_at >= $4) \
           AND ($5::timestamptz IS NULL OR ae.fired_at <= $5) \
           {filter} \
         GROUP BY day \
         ORDER BY day DESC LIMIT $6 OFFSET $7",
        filter = ALERT_DEF_FILTER
    );
    let rows: Vec<Row> = sqlx::query_as(&sql)
        .bind(site_id)
        .bind(department_id)
        .bind(category)
        .bind(from_ts)
        .bind(to_ts)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let total_sql = format!(
        "SELECT COUNT(DISTINCT DATE(ae.fired_at AT TIME ZONE 'UTC'))::BIGINT \
         FROM alert_events ae \
         WHERE ae.resolved_at IS NOT NULL \
           AND ($4::timestamptz IS NULL OR ae.fired_at >= $4) \
           AND ($5::timestamptz IS NULL OR ae.fired_at <= $5) \
           {filter}",
        filter = ALERT_DEF_FILTER
    );
    let total: (i64,) = sqlx::query_as(&total_sql)
        .bind(site_id)
        .bind(department_id)
        .bind(category)
        .bind(from_ts)
        .bind(to_ts)
        .fetch_one(pool)
        .await?;

    let out = rows
        .into_iter()
        .map(|r| CycleTimeRow {
            day: r.day,
            site_id,
            department_id,
            avg_hours: r.avg_hours.unwrap_or(0.0),
            count: r.cnt,
        })
        .collect();
    Ok((out, total.0))
}

// ---------------------------------------------------------------------------
// K3 — Funnel
// ---------------------------------------------------------------------------

pub async fn funnel(pool: &PgPool) -> AppResult<FunnelResponse> {
    funnel_sliced(pool, None, None, None, None, None).await
}

/// Funnel with real slice axes — site/department (correlated through
/// alert_rules.metric_definition_id → metric_definitions.source_ids →
/// env_sources), time window (fired_at range), and severity. Audit #8
/// Issue #2.
pub async fn funnel_sliced(
    pool: &PgPool,
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    from_ts: Option<chrono::DateTime<chrono::Utc>>,
    to_ts: Option<chrono::DateTime<chrono::Utc>>,
    severity: Option<&str>,
) -> AppResult<FunnelResponse> {
    // All three counts share the same slice predicates; we parameterise
    // the `acked_at`/`resolved_at` stage filter inline so the plan can
    // be reused across stages without rebuilding the SQL.
    let build_sql = |extra_predicate: &str| -> String {
        format!(
            "SELECT COUNT(*)::BIGINT FROM alert_events ae \
             JOIN alert_rules ar ON ar.id = ae.rule_id \
             WHERE ($1::TIMESTAMPTZ IS NULL OR ae.fired_at >= $1) \
               AND ($2::TIMESTAMPTZ IS NULL OR ae.fired_at <= $2) \
               AND ($3::TEXT IS NULL OR ar.severity = $3) \
               AND ($4::UUID IS NULL OR EXISTS ( \
                    SELECT 1 FROM metric_definitions md \
                    JOIN env_sources es ON es.id = ANY(md.source_ids) \
                    WHERE md.id = ar.metric_definition_id AND es.site_id = $4 \
               )) \
               AND ($5::UUID IS NULL OR EXISTS ( \
                    SELECT 1 FROM metric_definitions md \
                    JOIN env_sources es ON es.id = ANY(md.source_ids) \
                    WHERE md.id = ar.metric_definition_id AND es.department_id = $5 \
               )) \
               {}",
            extra_predicate
        )
    };

    let sev_owned = severity.map(|s| s.to_string());
    let sql_total = build_sql("");
    let sql_acked = build_sql("AND ae.acked_at IS NOT NULL");
    let sql_resolved = build_sql("AND ae.resolved_at IS NOT NULL");

    let (total,): (i64,) = sqlx::query_as(&sql_total)
        .bind(from_ts)
        .bind(to_ts)
        .bind(sev_owned.as_deref())
        .bind(site_id)
        .bind(department_id)
        .fetch_one(pool)
        .await?;
    let (acked,): (i64,) = sqlx::query_as(&sql_acked)
        .bind(from_ts)
        .bind(to_ts)
        .bind(sev_owned.as_deref())
        .bind(site_id)
        .bind(department_id)
        .fetch_one(pool)
        .await?;
    let (resolved,): (i64,) = sqlx::query_as(&sql_resolved)
        .bind(from_ts)
        .bind(to_ts)
        .bind(sev_owned.as_deref())
        .bind(site_id)
        .bind(department_id)
        .fetch_one(pool)
        .await?;

    let pct = |n: i64| -> f64 {
        if total > 0 {
            (n as f64 / total as f64) * 100.0
        } else {
            0.0
        }
    };

    let stages = vec![
        FunnelStage {
            stage: "fired".into(),
            count: total,
            conversion_pct: 100.0,
        },
        FunnelStage {
            stage: "acknowledged".into(),
            count: acked,
            conversion_pct: pct(acked),
        },
        FunnelStage {
            stage: "resolved".into(),
            count: resolved,
            conversion_pct: pct(resolved),
        },
    ];
    let overall = pct(resolved);
    Ok(FunnelResponse {
        stages,
        overall_conversion_pct: overall,
    })
}

// ---------------------------------------------------------------------------
// K4 — Anomalies
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub async fn anomalies(
    pool: &PgPool,
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    category: Option<&str>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<AnomalyRow>, i64)> {
    #[derive(sqlx::FromRow)]
    struct Row {
        day: NaiveDate,
        cnt: i64,
    }

    let from_ts = from.map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    let to_ts = to.map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());

    let sql = format!(
        "SELECT DATE(ae.fired_at AT TIME ZONE 'UTC') AS day, COUNT(*)::BIGINT AS cnt \
         FROM alert_events ae \
         WHERE ae.resolved_at IS NULL \
           AND ($4::timestamptz IS NULL OR ae.fired_at >= $4) \
           AND ($5::timestamptz IS NULL OR ae.fired_at <= $5) \
           {filter} \
         GROUP BY day ORDER BY day DESC LIMIT $6 OFFSET $7",
        filter = ALERT_DEF_FILTER
    );
    let rows: Vec<Row> = sqlx::query_as(&sql)
        .bind(site_id)
        .bind(department_id)
        .bind(category)
        .bind(from_ts)
        .bind(to_ts)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let total_sql = format!(
        "SELECT COUNT(DISTINCT DATE(ae.fired_at AT TIME ZONE 'UTC'))::BIGINT \
         FROM alert_events ae WHERE ae.resolved_at IS NULL \
           AND ($4::timestamptz IS NULL OR ae.fired_at >= $4) \
           AND ($5::timestamptz IS NULL OR ae.fired_at <= $5) \
           {filter}",
        filter = ALERT_DEF_FILTER
    );
    let total: (i64,) = sqlx::query_as(&total_sql)
        .bind(site_id)
        .bind(department_id)
        .bind(category)
        .bind(from_ts)
        .bind(to_ts)
        .fetch_one(pool)
        .await?;

    let out = rows
        .into_iter()
        .map(|r| AnomalyRow {
            day: r.day,
            site_id,
            department_id,
            count: r.cnt,
        })
        .collect();
    Ok((out, total.0))
}

// ---------------------------------------------------------------------------
// K5 — Efficiency
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub async fn efficiency(
    pool: &PgPool,
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    category: Option<&str>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<EfficiencyRow>, i64)> {
    #[derive(sqlx::FromRow)]
    struct Row {
        day: NaiveDate,
        idx: f64,
    }

    let from_ts = from.map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    let to_ts = to.map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());

    let sql = format!(
        "SELECT DATE(mc.computed_at AT TIME ZONE 'UTC') AS day, \
                AVG(mc.result) AS idx \
         FROM metric_computations mc \
         WHERE ($4::timestamptz IS NULL OR mc.computed_at >= $4) \
           AND ($5::timestamptz IS NULL OR mc.computed_at <= $5) \
           {filter} \
         GROUP BY day ORDER BY day DESC LIMIT $6 OFFSET $7",
        filter = MC_DEF_FILTER
    );
    let rows: Vec<Row> = sqlx::query_as(&sql)
        .bind(site_id)
        .bind(department_id)
        .bind(category)
        .bind(from_ts)
        .bind(to_ts)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let total_sql = format!(
        "SELECT COUNT(DISTINCT DATE(mc.computed_at AT TIME ZONE 'UTC'))::BIGINT \
         FROM metric_computations mc \
         WHERE ($4::timestamptz IS NULL OR mc.computed_at >= $4) \
           AND ($5::timestamptz IS NULL OR mc.computed_at <= $5) \
           {filter}",
        filter = MC_DEF_FILTER
    );
    let total: (i64,) = sqlx::query_as(&total_sql)
        .bind(site_id)
        .bind(department_id)
        .bind(category)
        .bind(from_ts)
        .bind(to_ts)
        .fetch_one(pool)
        .await?;

    let out = rows
        .into_iter()
        .map(|r| EfficiencyRow {
            day: r.day,
            site_id,
            department_id,
            index: r.idx,
        })
        .collect();
    Ok((out, total.0))
}

// ---------------------------------------------------------------------------
// K6 — Drill
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub async fn drill(
    pool: &PgPool,
    metric_kind: Option<&str>,
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<DrillRow>, i64)> {
    // Generic drill: aggregate metric_computations by definition name.
    // Note: `metric_kind` is the native drill axis; `category` here is
    // folded into `metric_kind` because the two express the same concept
    // in the DTO contract. The handler forwards whichever is provided.
    #[derive(sqlx::FromRow)]
    struct Row {
        def_name: String,
        avg_result: f64,
        formula_kind: String,
    }

    let from_ts = from.map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    let to_ts = to.map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());

    // Reuse MC_DEF_FILTER bind slots $1,$2,$3 for site, dept, category(=metric_kind).
    let sql = format!(
        "SELECT md.name AS def_name, AVG(mc.result) AS avg_result, md.formula_kind \
         FROM metric_computations mc \
         JOIN metric_definitions md ON md.id = mc.definition_id \
         WHERE ($4::timestamptz IS NULL OR mc.computed_at >= $4) \
           AND ($5::timestamptz IS NULL OR mc.computed_at <= $5) \
           {filter} \
         GROUP BY md.name, md.formula_kind \
         ORDER BY avg_result DESC LIMIT $6 OFFSET $7",
        filter = MC_DEF_FILTER
    );
    let rows: Vec<Row> = sqlx::query_as(&sql)
        .bind(site_id)
        .bind(department_id)
        .bind(metric_kind)
        .bind(from_ts)
        .bind(to_ts)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let total_sql = format!(
        "SELECT COUNT(DISTINCT md.name)::BIGINT \
         FROM metric_computations mc \
         JOIN metric_definitions md ON md.id = mc.definition_id \
         WHERE ($4::timestamptz IS NULL OR mc.computed_at >= $4) \
           AND ($5::timestamptz IS NULL OR mc.computed_at <= $5) \
           {filter}",
        filter = MC_DEF_FILTER
    );
    let total: (i64,) = sqlx::query_as(&total_sql)
        .bind(site_id)
        .bind(department_id)
        .bind(metric_kind)
        .bind(from_ts)
        .bind(to_ts)
        .fetch_one(pool)
        .await?;

    let out = rows
        .into_iter()
        .map(|r| DrillRow {
            dimension: "metric_definition".into(),
            label: r.def_name,
            value: r.avg_result,
            metric_kind: r.formula_kind,
        })
        .collect();
    Ok((out, total.0))
}
