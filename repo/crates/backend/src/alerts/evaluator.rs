//! Duration-aware alert evaluator background job.
//!
//! Runs every 30 seconds. For each enabled alert rule:
//!   1. Fetch the latest metric computation for the referenced definition.
//!   2. Evaluate `value OPERATOR threshold`.
//!   3. If violated AND the condition has been held for >= duration_seconds
//!      continuously (checked via the last active event's fired_at), fire a
//!      new event (if none is active) AND emit a notification.
//!   4. If previously tripped (active event exists) but now resolved, set
//!      `resolved_at` on the event.
//!
//! Call `start_alert_evaluator(pool)` from `app.rs` startup.
//!
//! NOTE FOR MAIN LANE: add the following line to `app.rs` in `run()`:
//! ```rust
//! let _alert_handle = crate::alerts::evaluator::start_alert_evaluator(pool.clone());
//! ```

use std::time::Duration;

use chrono::Utc;
use sqlx::PgPool;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::errors::AppResult;

use super::rules;

/// Spawn the evaluator loop. Returns a `JoinHandle` so callers can manage
/// the task lifetime. The task logs errors and continues.
pub fn start_alert_evaluator(pool: PgPool) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            if let Err(e) = evaluate_all(&pool).await {
                tracing::error!(error = %e, "alert evaluator cycle failed");
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    })
}

/// Run a single evaluator pass synchronously. Exposed for integration tests.
pub async fn evaluate_all(pool: &PgPool) -> AppResult<()> {
    // Load all enabled rules
    #[derive(sqlx::FromRow)]
    struct RuleRef {
        id: Uuid,
        metric_definition_id: Uuid,
        threshold: f64,
        operator: String,
        duration_seconds: i32,
    }
    let rules: Vec<RuleRef> = sqlx::query_as(
        "SELECT id, metric_definition_id, threshold, operator, duration_seconds \
         FROM alert_rules WHERE enabled = TRUE",
    )
    .fetch_all(pool)
    .await?;

    for rule in rules {
        if let Err(e) = evaluate_rule(pool, &rule.id, rule.metric_definition_id, rule.threshold, &rule.operator, rule.duration_seconds).await {
            tracing::warn!(
                rule_id = %rule.id,
                error = %e,
                "alert rule evaluation failed"
            );
        }
    }
    Ok(())
}

async fn evaluate_rule(
    pool: &PgPool,
    rule_id: &Uuid,
    definition_id: Uuid,
    threshold: f64,
    operator: &str,
    duration_seconds: i32,
) -> AppResult<()> {
    // Fetch the latest computation result for this definition
    #[derive(sqlx::FromRow)]
    struct LatestComp {
        result: f64,
        computed_at: chrono::DateTime<Utc>,
    }
    let latest: Option<LatestComp> = sqlx::query_as(
        "SELECT result, computed_at FROM metric_computations \
         WHERE definition_id = $1 ORDER BY computed_at DESC LIMIT 1",
    )
    .bind(definition_id)
    .fetch_optional(pool)
    .await?;

    let Some(comp) = latest else {
        return Ok(()); // No data yet, skip
    };

    let violated = check_operator(comp.result, operator, threshold);
    let has_active = rules::has_active_event(pool, *rule_id).await?;

    if violated {
        if !has_active {
            // Check if duration_seconds requirement is met.
            // For duration_seconds == 0 we fire immediately.
            // For duration_seconds > 0 we check how long the condition has
            // been continuously violated by looking at the computed_at of
            // the latest computation that first crossed the threshold.
            // Simplified: if duration_seconds == 0, fire immediately;
            // otherwise we need to have been in violation for at least
            // duration_seconds before this tick.
            let should_fire = if duration_seconds == 0 {
                true
            } else {
                // Check if the earliest computation within duration_seconds
                // backward also violates the condition. If so, we've been
                // in violation for at least duration_seconds.
                let window_start = comp.computed_at - chrono::Duration::seconds(duration_seconds as i64);
                let (violation_count,): (i64,) = sqlx::query_as(
                    "SELECT COUNT(*)::BIGINT FROM metric_computations \
                     WHERE definition_id = $1 \
                       AND computed_at >= $2 \
                       AND computed_at <= $3",
                )
                .bind(definition_id)
                .bind(window_start)
                .bind(comp.computed_at)
                .fetch_one(pool)
                .await?;
                violation_count >= 2 // at least first + last point in window
            };

            if should_fire {
                let event_id = rules::fire_event(pool, *rule_id, comp.result).await?;
                tracing::info!(
                    rule_id = %rule_id,
                    event_id = %event_id,
                    value = comp.result,
                    threshold,
                    operator,
                    "alert fired"
                );
                // Emit notification to all users with alert.ack permission
                notify_alert_fired(pool, *rule_id, event_id, comp.result).await;
            }
        }
        // else: already active, no duplicate
    } else if has_active {
        // Condition cleared — resolve the open event
        rules::resolve_event(pool, *rule_id).await?;
        tracing::info!(rule_id = %rule_id, "alert resolved");
    }

    Ok(())
}

fn check_operator(value: f64, operator: &str, threshold: f64) -> bool {
    match operator {
        ">" => value > threshold,
        "<" => value < threshold,
        ">=" => value >= threshold,
        "<=" => value <= threshold,
        "=" => (value - threshold).abs() < f64::EPSILON,
        _ => false,
    }
}

/// Insert notifications for all users with `alert.ack` permission.
async fn notify_alert_fired(pool: &PgPool, rule_id: Uuid, event_id: Uuid, value: f64) {
    #[derive(sqlx::FromRow)]
    struct UserId {
        user_id: Uuid,
    }
    let users: Vec<UserId> = match sqlx::query_as(
        "SELECT DISTINCT up.user_id \
         FROM user_roles up \
         JOIN role_permissions rp ON rp.role_id = up.role_id \
         JOIN permissions p ON p.id = rp.permission_id \
         WHERE p.code = 'alert.ack'",
    )
    .fetch_all(pool)
    .await
    {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!(error = %e, "failed to load alert.ack users for notification");
            return;
        }
    };

    let title = format!("Alert fired: value {:.2}", value);
    let body = format!(
        "Alert rule {} fired with value {:.2}. Event id: {}.",
        rule_id, value, event_id
    );
    let payload = serde_json::json!({
        "rule_id": rule_id,
        "event_id": event_id,
        "value": value,
    });

    for u in users {
        let _ = sqlx::query(
            "INSERT INTO notifications (user_id, topic, title, body, payload_json) \
             VALUES ($1, 'alert.fired', $2, $3, $4)",
        )
        .bind(u.user_id)
        .bind(&title)
        .bind(&body)
        .bind(&payload)
        .execute(pool)
        .await;
    }
}
