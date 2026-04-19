//! Deep HTTP coverage for the P1–P14 + I1–I7 product surface.
//!
//! Every test drives real routes through the full middleware stack
//! (MetricsMw → BudgetMw → AuthnMw → RequestIdMw) against a real
//! Postgres using the shared `TestCtx` harness. These tests exist to
//! close the Gate 1 90% coverage contract for `products/**`.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use terraops_shared::roles::Role;
use uuid::Uuid;

use common::{authed, build_test_app, TestCtx};

fn bearer(tok: &str) -> String {
    format!("Bearer {tok}")
}

// ── P2/P3/P4/P5/P6 full CRUD round-trip ────────────────────────────────────

#[actix_web::test]
async fn deep_products_full_crud_roundtrip() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepp-crud@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Seed reference data so we can exercise joined fields.
    let (site_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO sites (code, name) VALUES ('S1','Site One') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (dept_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO departments (site_id, code, name) VALUES ($1,'D1','Dept') RETURNING id",
    )
    .bind(site_id)
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (cat_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO categories (name) VALUES ('Cat') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (brand_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO brands (name) VALUES ('BrandX') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (unit_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO units (code) VALUES ('ea') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // P2 POST /products — create with all fields
    let req = test::TestRequest::post()
        .uri("/api/v1/products")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({
            "sku": "SKU-001",
            "name": "Widget",
            "description": "A real widget",
            "category_id": cat_id,
            "brand_id": brand_id,
            "unit_id": unit_id,
            "site_id": site_id,
            "department_id": dept_id,
            "on_shelf": true,
            "price_cents": 1999,
            "currency": "USD"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let pid = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // P2 validation: empty sku
    let req = test::TestRequest::post()
        .uri("/api/v1/products")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"sku": "  ", "name": "x"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // P2 validation: empty name
    let req = test::TestRequest::post()
        .uri("/api/v1/products")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"sku": "SKU-002", "name": ""}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // P3 GET /products/{id} — real detail with joined names
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["sku"], "SKU-001");
    assert_eq!(body["category_name"], "Cat");
    assert_eq!(body["brand_name"], "BrandX");
    assert_eq!(body["unit_code"], "ea");
    assert_eq!(body["site_code"], "S1");
    assert_eq!(body["department_code"], "D1");

    // P3 NotFound
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // P4 PATCH — update fields including price + name
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"name": "Widget v2", "price_cents": 2599, "currency": "USD"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // P4 update on missing id
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/products/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"name": "nope"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // P6 POST /status — toggle on_shelf
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/products/{pid}/status"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"on_shelf": false}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // P6 on missing
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/products/{}/status", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"on_shelf": false}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // P7 GET history — should have at least create + update + status entries
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products/{pid}/history"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    let items = body["items"].as_array().unwrap();
    assert!(items.len() >= 3);

    // P7 history of missing product
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products/{}/history", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // P5 DELETE — soft delete
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // P5 again → NotFound (already soft-deleted)
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // P3 on soft-deleted → NotFound
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── P1 list filters + sort + pagination ────────────────────────────────────

#[actix_web::test]
async fn deep_products_list_filters_sort_pagination() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepp-list@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;
    let (site_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO sites (code, name) VALUES ('SA','A') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();
    let (brand_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO brands (name) VALUES ('BrandList') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    for i in 0..15u32 {
        sqlx::query(
            "INSERT INTO products (sku, name, on_shelf, price_cents, currency, site_id, brand_id) \
             VALUES ($1,$2,$3,$4,'USD',$5,$6)",
        )
        .bind(format!("L-{i:03}"))
        .bind(format!("List Item {i}"))
        .bind(i % 2 == 0)
        .bind(1000 + i as i32)
        .bind(site_id)
        .bind(brand_id)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Default list
    let req = test::TestRequest::get()
        .uri("/api/v1/products")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 15);

    // Paginate
    let req = test::TestRequest::get()
        .uri("/api/v1/products?page=2&page_size=5")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 5);
    assert_eq!(body["page"], 2);

    // Filter on_shelf=true (8 of 15)
    let req = test::TestRequest::get()
        .uri("/api/v1/products?on_shelf=true")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 8);

    // Filter by site_id
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products?site_id={site_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 15);

    // Filter by brand_id
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products?brand_id={brand_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Text search
    let req = test::TestRequest::get()
        .uri("/api/v1/products?q=List%20Item%207")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 1);

    // Sort variants — exercise each branch
    for sort in ["sku", "name", "price_cents", "on_shelf", "other"] {
        let req = test::TestRequest::get()
            .uri(&format!("/api/v1/products?sort={sort}"))
            .insert_header(("Authorization", bearer(&token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK, "sort={sort}");
    }
}

// ── P8/P9/P10 tax rates CRUD ───────────────────────────────────────────────

#[actix_web::test]
async fn deep_products_tax_rates_crud() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepp-tax@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Create product
    let (pid,): (Uuid,) = sqlx::query_as(
        "INSERT INTO products (sku, name, on_shelf, price_cents, currency) \
         VALUES ('TAX-1','Tax Product',true,100,'USD') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // P8 add valid
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/products/{pid}/tax-rates"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"state_code": "CA", "rate_bp": 750}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let rid = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // P8 unknown state code
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/products/{pid}/tax-rates"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"state_code": "ZZ", "rate_bp": 1}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // P8 negative rate
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/products/{pid}/tax-rates"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"state_code": "NY", "rate_bp": -1}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // P8 on missing product
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/products/{}/tax-rates", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"state_code": "CA", "rate_bp": 1}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // P9 update valid
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/products/{pid}/tax-rates/{rid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"rate_bp": 900}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // P9 noop (no rate_bp)
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/products/{pid}/tax-rates/{rid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // P9 negative
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/products/{pid}/tax-rates/{rid}"))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"rate_bp": -5}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // P9 missing rate id
    let req = test::TestRequest::patch()
        .uri(&format!("/api/v1/products/{pid}/tax-rates/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"rate_bp": 1}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // P10 delete
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/products/{pid}/tax-rates/{rid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // P10 again → NotFound
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/products/{pid}/tax-rates/{rid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // P10 on missing product id
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/products/{}/tax-rates/{rid}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── P11/P12/P13 images upload, serve (signed), delete ──────────────────────

#[actix_web::test]
async fn deep_products_images_upload_serve_delete() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepp-img@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;

    let (pid,): (Uuid,) = sqlx::query_as(
        "INSERT INTO products (sku, name, on_shelf, price_cents, currency) \
         VALUES ('IMG-1','Img Product',true,100,'USD') RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Upload an image (fake PNG bytes)
    let boundary = "----TerraOpsImg";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"a.png\"\r\n\
          Content-Type: image/png\r\n\r\n",
    );
    body.extend_from_slice(b"\x89PNG\r\n\x1a\nFAKEIMGBYTES");
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/products/{pid}/images"))
        .insert_header(("Authorization", bearer(&token)))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let img_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // P11 on missing product
    let boundary2 = "----TerraOpsImg2";
    let mut body2: Vec<u8> = Vec::new();
    body2.extend_from_slice(format!("--{boundary2}\r\n").as_bytes());
    body2.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"b.png\"\r\n\
          Content-Type: image/png\r\n\r\nX",
    );
    body2.extend_from_slice(format!("\r\n--{boundary2}--\r\n").as_bytes());
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/products/{}/images", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&token)))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary2}"),
        ))
        .set_payload(body2)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // P3 detail should include this image with a signed URL
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/products/{pid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    let signed_url = body["images"][0]["signed_url"].as_str().unwrap().to_string();
    assert!(signed_url.contains("sig="));
    assert!(signed_url.contains("exp="));

    // P13 serve image — use full signed URL path
    let req = test::TestRequest::get()
        .uri(&signed_url)
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // P13 without signature → forbidden
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/images/{img_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // P13 bad signature → forbidden
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/images/{img_id}?exp=9999999999&sig=deadbeef"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // P12 delete
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/products/{pid}/images/{img_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // P12 again → NotFound
    let req = test::TestRequest::delete()
        .uri(&format!("/api/v1/products/{pid}/images/{img_id}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // P12 on missing product
    let req = test::TestRequest::delete()
        .uri(&format!(
            "/api/v1/products/{}/images/{}",
            Uuid::new_v4(),
            Uuid::new_v4()
        ))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── P14 export (CSV + XLSX) + filter path ──────────────────────────────────

#[actix_web::test]
async fn deep_products_export_csv_and_xlsx() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepp-export@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;

    for i in 0..5u32 {
        sqlx::query(
            "INSERT INTO products (sku, name, on_shelf, price_cents, currency) \
             VALUES ($1,$2,true,$3,'USD')",
        )
        .bind(format!("EXP-{i}"))
        .bind(format!("Export {i}"))
        .bind(500 + i as i32)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // CSV export
    let req = test::TestRequest::post()
        .uri("/api/v1/products/export")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"kind": "csv"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("sku"));
    assert!(text.contains("EXP-0"));

    // XLSX export
    let req = test::TestRequest::post()
        .uri("/api/v1/products/export")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"kind": "xlsx"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    // XLSX is a zip — starts with PK
    assert_eq!(&body[..2], b"PK");

    // CSV with filter
    let req = test::TestRequest::post()
        .uri("/api/v1/products/export")
        .insert_header(("Authorization", bearer(&token)))
        .set_json(json!({"kind": "csv", "filter": {"q": "EXP-2"}}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("EXP-2"));
    assert!(!text.contains("EXP-0\n"));
}

// ── I1–I7 full import lifecycle: upload → validate → commit + errors ───────

#[actix_web::test]
async fn deep_imports_full_lifecycle_commit() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepi-commit@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Build valid CSV
    let csv = "sku,name,price_cents,currency,on_shelf\n\
               IMP-001,Good One,100,USD,true\n\
               IMP-002,Good Two,200,USD,false\n";
    let boundary = "----TerraOpsIC";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"good.csv\"\r\n\
          Content-Type: text/csv\r\n\r\n",
    );
    body.extend_from_slice(csv.as_bytes());
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    // I1 upload
    let req = test::TestRequest::post()
        .uri("/api/v1/imports")
        .insert_header(("Authorization", bearer(&token)))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let bid = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    assert_eq!(body["row_count"], 2);
    assert_eq!(body["error_count"], 0);
    assert_eq!(body["status"], "validated");

    // I2 list
    let req = test::TestRequest::get()
        .uri("/api/v1/imports")
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // I3 get
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/imports/{bid}"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // I4 rows
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/imports/{bid}/rows"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 2);

    // I5 revalidate
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/imports/{bid}/validate"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // I6 commit
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/imports/{bid}/commit"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["inserted"], 2);
    assert_eq!(body["status"], "committed");

    // I5 on committed → 422
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/imports/{bid}/validate"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // I6 on committed → 422
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/imports/{bid}/commit"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // I7 cancel on committed → 422
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/imports/{bid}/cancel"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[actix_web::test]
async fn deep_imports_error_rows_block_commit_then_cancel() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepi-err@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Mix valid + invalid rows (bad price, missing sku)
    let csv = "sku,name,price_cents,currency,on_shelf\n\
               GOOD-1,Okay,50,USD,true\n\
               ,Missing SKU,50,USD,true\n\
               BAD-2,Bad Price,notanumber,USD,true\n";
    let boundary = "----TerraOpsIE";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"bad.csv\"\r\n\
          Content-Type: text/csv\r\n\r\n",
    );
    body.extend_from_slice(csv.as_bytes());
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let req = test::TestRequest::post()
        .uri("/api/v1/imports")
        .insert_header(("Authorization", bearer(&token)))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let bid = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();
    assert!(body["error_count"].as_i64().unwrap() >= 2);
    assert_eq!(body["status"], "uploaded");

    // Commit should be 422 (not in validated status OR has errors)
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/imports/{bid}/commit"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Cancel succeeds
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/imports/{bid}/cancel"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Cancel again → 422
    let req = test::TestRequest::post()
        .uri(&format!("/api/v1/imports/{bid}/cancel"))
        .insert_header(("Authorization", bearer(&token)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[actix_web::test]
async fn deep_imports_object_level_auth() {
    let ctx = TestCtx::new().await;
    let (_uid_a, tok_a) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepi-a@example.com",
        &[Role::DataSteward],
    )
    .await;
    let (_uid_b, tok_b) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepi-b@example.com",
        &[Role::DataSteward],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // A uploads
    let csv = "sku,name,price_cents,currency,on_shelf\nX1,A,1,USD,true\n";
    let boundary = "----TerraOpsAuth";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"x.csv\"\r\n\
          Content-Type: text/csv\r\n\r\n",
    );
    body.extend_from_slice(csv.as_bytes());
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let req = test::TestRequest::post()
        .uri("/api/v1/imports")
        .insert_header(("Authorization", bearer(&tok_a)))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(resp).await;
    let bid = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // B tries to read A's batch → 403
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/imports/{bid}"))
        .insert_header(("Authorization", bearer(&tok_b)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // B's list excludes A's
    let req = test::TestRequest::get()
        .uri("/api/v1/imports")
        .insert_header(("Authorization", bearer(&tok_b)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = test::read_body_json(resp).await;
    assert_eq!(body["total"], 0);

    // Missing batch id → 404
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/imports/{}", Uuid::new_v4()))
        .insert_header(("Authorization", bearer(&tok_a)))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn deep_imports_upload_missing_file_field() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(
        &ctx.pool,
        &ctx.keys,
        "deepi-empty@example.com",
        &[Role::Administrator, Role::DataSteward, Role::Analyst],
    )
    .await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    let boundary = "----TerraOpsEmpty";
    let body = format!("--{boundary}--\r\n");
    let req = test::TestRequest::post()
        .uri("/api/v1/imports")
        .insert_header(("Authorization", bearer(&token)))
        .insert_header((
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ))
        .set_payload(body)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
