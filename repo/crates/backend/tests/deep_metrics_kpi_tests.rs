//! Deep HTTP coverage for env sources (E1–E6), metric definitions (MD1–MD7),
//! and KPI endpoints (K1–K6). Drives real routes through the full middleware
//! stack against real Postgres using the shared TestCtx harness.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use terraops_shared::roles::Role;
use uuid::Uuid;

use common::{authed, build_test_app, TestCtx};

fn bearer(tok: &str) -> String {
    format!("Bearer {tok}")
}

// ── E1–E6: env sources + observations lifecycle ────────────────────────────

#[actix_web::test]
async fn deep_env_sources_full_lifecycle() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepe-src@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // E2 create
    let req = test::TestRequest::post()
        .uri("/api/v1/env/sources")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"name": "Sensor A", "kind": "temperature"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let sid = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // E1 list
    let req = test::TestRequest::get()
        .uri("/api/v1/env/sources")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 1);

    // E3 update
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/env/sources/{sid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"name": "Sensor A v2", "kind": "humidity"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // E3 update on missing
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/env/sources/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"name": "nope"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // E5 bulk observations
    let now = Utc::now();
    let observations = (0..10)
        .map(|i| {
            json!({
                "observed_at": (now - Duration::minutes(i)).to_rfc3339(),
                "value": 20.0 + i as f64,
                "unit": "C"
            })
        })
        .collect::<Vec<_>>();
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/env/sources/{sid}/observations"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"observations": observations}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["inserted"], 10);

    // E5 on missing source
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/env/sources/{}/observations", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"observations": []}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // E6 list observations — no filter
    let req = test::TestRequest::get()
        .uri("/api/v1/env/observations")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 10);

    // E6 list with source_id + from + to (URL-encode the `+` in RFC3339 tz)
    let from = (now - Duration::hours(1)).to_rfc3339().replace('+', "%2B");
    let to = (now + Duration::hours(1)).to_rfc3339().replace('+', "%2B");
    let req = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/env/observations?source_id={sid}&from={from}&to={to}"
        ))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // E4 delete
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/env/sources/{sid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // E4 again → 404
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/env/sources/{sid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── MD1–MD7: metric definitions for all three formula kinds + series ──────

#[actix_web::test]
async fn deep_metric_definitions_all_formula_kinds() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepm-md@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Seed two env sources (temp + humidity for comfort_index).
    let (temp_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO env_sources (name, kind) VALUES ('Temp','temperature') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (hum_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO env_sources (name, kind) VALUES ('Hum','humidity') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Seed observations for both
    let now = Utc::now();
    for i in 0..20 {
        sqlx::query(
            "INSERT INTO env_observations (source_id, observed_at, value, unit) \
             VALUES ($1,$2,$3,'C')",
        )
        .bind(temp_id)
        .bind(now - Duration::minutes(i))
        .bind(22.0 + (i as f64) * 0.1)
        .execute(&ctx.pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO env_observations (source_id, observed_at, value, unit) \
             VALUES ($1,$2,$3,'%')",
        )
        .bind(hum_id)
        .bind(now - Duration::minutes(i))
        .bind(50.0 + (i as f64) * 0.2)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    // MD2 create — moving_average
    let req = test::TestRequest::post()
        .uri("/api/v1/metrics/definitions")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "name": "TempMA",
            "formula_kind": "moving_average",
            "source_ids": [temp_id],
            "window_seconds": 3600
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let ma_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // MD2 rate_of_change
    let req = test::TestRequest::post()
        .uri("/api/v1/metrics/definitions")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "name": "TempROC",
            "formula_kind": "rate_of_change",
            "source_ids": [temp_id],
            "window_seconds": 3600
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let roc_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // MD2 comfort_index
    let req = test::TestRequest::post()
        .uri("/api/v1/metrics/definitions")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "name": "Comfort",
            "formula_kind": "comfort_index",
            "source_ids": [temp_id, hum_id],
            "window_seconds": 3600
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let ci_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // MD2 bad formula kind → 422
    let req = test::TestRequest::post()
        .uri("/api/v1/metrics/definitions")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "name": "Bad",
            "formula_kind": "not_a_real_formula",
            "source_ids": []
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // MD1 list
    let req = test::TestRequest::get()
        .uri("/api/v1/metrics/definitions")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert!(body["total"].as_i64().unwrap() >= 3);

    // MD3 get each
    for id in &[ma_id, roc_id, ci_id] {
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/metrics/definitions/{id}"))
            .insert_header(("Authorization", bearer(&token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // MD3 missing
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/metrics/definitions/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // MD4 update
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/metrics/definitions/{ma_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"window_seconds": 7200, "enabled": true}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // MD4 invalid formula_kind
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/metrics/definitions/{ma_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"formula_kind": "nope"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // MD6 series for each kind (exercises compute path + storage)
    for id in &[ma_id, roc_id, ci_id] {
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/metrics/definitions/{id}/series"))
            .insert_header(("Authorization", bearer(&token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // Seed a computation row directly so we can exercise MD7 lineage
    let obs_uuid = Uuid::new_v4();
    let inputs = json!([{
        "observation_id": obs_uuid.to_string(),
        "observed_at": now.to_rfc3339(),
        "value": 22.5
    }]);
    let (comp_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO metric_computations (definition_id, result, inputs, window_start, window_end) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(ma_id)
    .bind(22.5f64)
    .bind(&inputs)
    .bind(now - Duration::hours(1))
    .bind(now)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // MD7 lineage
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/metrics/computations/{comp_id}/lineage"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["formula"], "moving_average");
    assert_eq!(body["result"], 22.5);
    assert_eq!(body["input_observations"].as_array().unwrap().len(), 1);

    // MD7 missing
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/metrics/computations/{}/lineage", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // MD5 soft delete
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/metrics/definitions/{roc_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // MD5 again → 404
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/metrics/definitions/{roc_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── K1–K6 KPI handlers ─────────────────────────────────────────────────────

#[actix_web::test]
async fn deep_kpi_endpoints_with_real_seed() {
    let ctx = TestCtx::new().await;
    let (uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepk-kpi@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;

    // Seed a definition so we can create alert rules on it
    let (def_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO metric_definitions (name, formula_kind) \
         VALUES ('KPIDef','moving_average') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Seed an alert rule + several alert_events (fired, acked, resolved
    // combinations across 5 days) so every K* aggregation path has data.
    let (rule_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO alert_rules (metric_definition_id, threshold, operator, severity, created_by) \
         VALUES ($1, 10.0, '>', 'warning', $2) RETURNING id",
    )
    .bind(def_id)
    .bind(uid)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let now = Utc::now();
    for i in 0..10 {
        let fired = now - Duration::days(i % 5) - Duration::hours(i as i64);
        let resolved_at = if i % 2 == 0 {
            Some(fired + Duration::hours(2))
        } else {
            None
        };
        let acked_at = if i % 3 == 0 {
            Some(fired + Duration::minutes(30))
        } else {
            None
        };
        sqlx::query(
            "INSERT INTO alert_events (rule_id, fired_at, value, acked_at, acked_by, resolved_at) \
             VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(rule_id)
        .bind(fired)
        .bind(15.0 + i as f64)
        .bind(acked_at)
        .bind(acked_at.map(|_| uid))
        .bind(resolved_at)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    // Seed metric_computations for efficiency + drill
    for i in 0..6 {
        sqlx::query(
            "INSERT INTO metric_computations (definition_id, computed_at, result, window_start, window_end) \
             VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(def_id)
        .bind(now - Duration::hours(i))
        .bind(50.0 + i as f64)
        .bind(now - Duration::hours(i + 1))
        .bind(now - Duration::hours(i))
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // K1 summary
    let req = test::TestRequest::get()
        .uri("/api/v1/kpi/summary")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let st = resp.status();
    let body_bytes = test::read_body(resp).await;
    eprintln!("kpi/summary status={st} body={}", String::from_utf8_lossy(&body_bytes));
    assert_eq!(st, StatusCode::OK);

    // K2 cycle-time (no filters, with filters, with paging)
    for q in [
        "",
        "?page=1&page_size=10",
        "?from=2020-01-01&to=2099-12-31",
    ] {
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/kpi/cycle-time{q}"))
            .insert_header(("Authorization", bearer(&token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK, "q={q}");
    }

    // K3 funnel
    let req = test::TestRequest::get()
        .uri("/api/v1/kpi/funnel")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["stages"].as_array().unwrap().len(), 3);

    // K4 anomalies
    for q in ["", "?from=2020-01-01&to=2099-12-31"] {
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/kpi/anomalies{q}"))
            .insert_header(("Authorization", bearer(&token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // K5 efficiency
    for q in ["", "?from=2020-01-01&to=2099-12-31"] {
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/kpi/efficiency{q}"))
            .insert_header(("Authorization", bearer(&token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // K6 drill — no filter + kind filter
    for q in [
        "",
        "?metric_kind=moving_average",
        "?from=2020-01-01&to=2099-12-31",
    ] {
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/kpi/drill{q}"))
            .insert_header(("Authorization", bearer(&token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
