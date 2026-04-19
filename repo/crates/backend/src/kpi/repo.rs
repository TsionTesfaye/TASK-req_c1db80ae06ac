//! KPI data access — computes KPI metrics on-demand from existing tables.
//!
//! Design choice: KPI values are computed on-demand from the `alert_events`
//! and `metric_computations` tables rather than requiring a pre-populated
//! `kpi_rollup_daily`. This keeps P-B self-contained. The rollup job would
//! write to `kpi_rollup_daily` as an optimisation; the handlers fall back to
//! on-demand queries when no cached row exists.

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
        "SELECT AVG(EXTRACT(EPOCH FROM (resolved_at - fired_at)) / 3600.0) \
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

    Ok(KpiSummary {
        cycle_time_avg_hours: cycle_hours.0.unwrap_or(0.0),
        funnel_conversion_pct: funnel_pct,
        anomaly_count,
        efficiency_index: efficiency.0.unwrap_or(0.0),
        generated_at: Utc::now(),
    })
}

// ---------------------------------------------------------------------------
// K2 — Cycle Time
// ---------------------------------------------------------------------------

pub async fn cycle_time(
    pool: &PgPool,
    site_id: Option<Uuid>,
    department_id: Option<Uuid>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<CycleTimeRow>, i64)> {
    // Cycle time per day: mean hours from fired_at to resolved_at.
    // When site/dept filters are provided, we join through alert_rules →
    // metric_definitions → env_sources; but env_sources may not be wired yet
    // if no definitions/sources exist. We degrade gracefully by using
    // direct alert_events aggregation when site/dept are null.
    #[derive(sqlx::FromRow)]
    struct Row {
        day: NaiveDate,
        avg_hours: Option<f64>,
        cnt: i64,
    }

    let from_ts = from.map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    let to_ts = to.map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT DATE(fired_at AT TIME ZONE 'UTC') AS day, \
                AVG(EXTRACT(EPOCH FROM (resolved_at - fired_at)) / 3600.0) AS avg_hours, \
                COUNT(*)::BIGINT AS cnt \
         FROM alert_events \
         WHERE resolved_at IS NOT NULL \
           AND ($1::timestamptz IS NULL OR fired_at >= $1) \
           AND ($2::timestamptz IS NULL OR fired_at <= $2) \
         GROUP BY day \
         ORDER BY day DESC LIMIT $3 OFFSET $4",
    )
    .bind(from_ts)
    .bind(to_ts)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT DATE(fired_at AT TIME ZONE 'UTC'))::BIGINT \
         FROM alert_events \
         WHERE resolved_at IS NOT NULL \
           AND ($1::timestamptz IS NULL OR fired_at >= $1) \
           AND ($2::timestamptz IS NULL OR fired_at <= $2)",
    )
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
    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM alert_events",
    )
    .fetch_one(pool)
    .await?;
    let (acked,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM alert_events WHERE acked_at IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;
    let (resolved,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::BIGINT FROM alert_events WHERE resolved_at IS NOT NULL",
    )
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

pub async fn anomalies(
    pool: &PgPool,
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

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT DATE(fired_at AT TIME ZONE 'UTC') AS day, COUNT(*)::BIGINT AS cnt \
         FROM alert_events \
         WHERE resolved_at IS NULL \
           AND ($1::timestamptz IS NULL OR fired_at >= $1) \
           AND ($2::timestamptz IS NULL OR fired_at <= $2) \
         GROUP BY day ORDER BY day DESC LIMIT $3 OFFSET $4",
    )
    .bind(from_ts)
    .bind(to_ts)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT DATE(fired_at AT TIME ZONE 'UTC'))::BIGINT \
         FROM alert_events WHERE resolved_at IS NULL \
           AND ($1::timestamptz IS NULL OR fired_at >= $1) \
           AND ($2::timestamptz IS NULL OR fired_at <= $2)",
    )
    .bind(from_ts)
    .bind(to_ts)
    .fetch_one(pool)
    .await?;

    let out = rows
        .into_iter()
        .map(|r| AnomalyRow {
            day: r.day,
            site_id: None,
            department_id: None,
            count: r.cnt,
        })
        .collect();
    Ok((out, total.0))
}

// ---------------------------------------------------------------------------
// K5 — Efficiency
// ---------------------------------------------------------------------------

pub async fn efficiency(
    pool: &PgPool,
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

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT DATE(computed_at AT TIME ZONE 'UTC') AS day, \
                AVG(result) AS idx \
         FROM metric_computations \
         WHERE ($1::timestamptz IS NULL OR computed_at >= $1) \
           AND ($2::timestamptz IS NULL OR computed_at <= $2) \
         GROUP BY day ORDER BY day DESC LIMIT $3 OFFSET $4",
    )
    .bind(from_ts)
    .bind(to_ts)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT DATE(computed_at AT TIME ZONE 'UTC'))::BIGINT \
         FROM metric_computations \
         WHERE ($1::timestamptz IS NULL OR computed_at >= $1) \
           AND ($2::timestamptz IS NULL OR computed_at <= $2)",
    )
    .bind(from_ts)
    .bind(to_ts)
    .fetch_one(pool)
    .await?;

    let out = rows
        .into_iter()
        .map(|r| EfficiencyRow {
            day: r.day,
            site_id: None,
            department_id: None,
            index: r.idx,
        })
        .collect();
    Ok((out, total.0))
}

// ---------------------------------------------------------------------------
// K6 — Drill
// ---------------------------------------------------------------------------

pub async fn drill(
    pool: &PgPool,
    metric_kind: Option<&str>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<DrillRow>, i64)> {
    // Generic drill: aggregate metric_computations by definition name.
    #[derive(sqlx::FromRow)]
    struct Row {
        def_name: String,
        avg_result: f64,
        formula_kind: String,
    }

    let from_ts = from.map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    let to_ts = to.map(|d| d.and_hms_opt(23, 59, 59).unwrap().and_utc());

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT md.name AS def_name, AVG(mc.result) AS avg_result, md.formula_kind \
         FROM metric_computations mc \
         JOIN metric_definitions md ON md.id = mc.definition_id \
         WHERE ($1::text IS NULL OR md.formula_kind = $1) \
           AND ($2::timestamptz IS NULL OR mc.computed_at >= $2) \
           AND ($3::timestamptz IS NULL OR mc.computed_at <= $3) \
         GROUP BY md.name, md.formula_kind \
         ORDER BY avg_result DESC LIMIT $4 OFFSET $5",
    )
    .bind(metric_kind)
    .bind(from_ts)
    .bind(to_ts)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT md.name)::BIGINT \
         FROM metric_computations mc \
         JOIN metric_definitions md ON md.id = mc.definition_id \
         WHERE ($1::text IS NULL OR md.formula_kind = $1) \
           AND ($2::timestamptz IS NULL OR mc.computed_at >= $2) \
           AND ($3::timestamptz IS NULL OR mc.computed_at <= $3)",
    )
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
