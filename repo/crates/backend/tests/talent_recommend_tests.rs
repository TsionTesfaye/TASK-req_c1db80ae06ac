//! Talent recommendations tests (T4–T6).
//!
//! Naming: `t_t4_*`, `t_t5_*`, `t_t6_*`.

#[path = "common/mod.rs"]
mod common;

use actix_web::{http::StatusCode, test};
use serde_json::{json, Value};
use terraops_shared::roles::Role;

use common::{authed, build_test_app, TestCtx};

// ── T4: GET /api/v1/talent/roles ─────────────────────────────────────────────

#[actix_web::test]
async fn t_t4_list_roles_requires_auth() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/roles")
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_t4_list_roles_returns_empty_initially() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t4empty@example.com", &[Role::Recruiter]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/roles")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert!(body.as_array().unwrap().is_empty());
}

// ── T5: POST /api/v1/talent/roles ────────────────────────────────────────────

#[actix_web::test]
async fn t_t5_create_role_requires_talent_manage() {
    let ctx = TestCtx::new().await;
    // RegularUser has no talent.manage (Recruiter does, per design.md §220).
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t5noperm@example.com", &[Role::RegularUser]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/roles")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "title": "Rust Engineer",
            "required_skills": ["rust"],
            "min_years": 3
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[actix_web::test]
async fn t_t5_create_role_admin_ok() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t5admin@example.com", &[Role::Administrator]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::post()
        .uri("/api/v1/talent/roles")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .set_json(json!({
            "title": "Backend Engineer",
            "required_skills": ["rust", "postgres"],
            "min_years": 3
        }))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::CREATED);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(body["title"], "Backend Engineer");
    assert_eq!(body["status"], "open");
    assert!(body["id"].is_string());
}

// ── T6: GET /api/v1/talent/recommendations ───────────────────────────────────

#[actix_web::test]
async fn t_t6_recommendations_requires_auth() {
    let ctx = TestCtx::new().await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/recommendations?role_id=00000000-0000-0000-0000-000000000001")
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn t_t6_recommendations_role_not_found() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t6nf@example.com", &[Role::Recruiter]).await;
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri("/api/v1/talent/recommendations?role_id=00000000-0000-0000-0000-000000000001")
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[actix_web::test]
async fn t_t6_recommendations_cold_start() {
    let ctx = TestCtx::new().await;
    let (_uid, token) = authed(&ctx.pool, &ctx.keys, "t6cs@example.com", &[Role::Recruiter]).await;

    // Create a role
    let (role_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO roles_open (title, required_skills, min_years) \
         VALUES ('SRE', '{rust,kubernetes}', 3) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Create 2 candidates (less than 10 feedback → cold start)
    sqlx::query(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('High Completeness', 'h***@x.com', 5, '{rust}', 90), \
                ('Low Completeness', 'l***@x.com', 5, '{rust}', 20)",
    )
    .execute(&ctx.pool)
    .await
    .unwrap();

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/talent/recommendations?role_id={role_id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;

    assert_eq!(body["cold_start"], true, "expected cold_start=true with no feedback");
    let candidates = body["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 2);
    // Higher completeness should rank first
    assert_eq!(candidates[0]["candidate"]["full_name"], "High Completeness");
    // Reasons must mention cold-start
    assert!(candidates[0]["reasons"][0]
        .as_str()
        .unwrap()
        .contains("Cold-start"));
}

#[actix_web::test]
async fn t_t6_recommendations_blended_after_10_feedback() {
    let ctx = TestCtx::new().await;
    let (recruiter_id, token) = authed(&ctx.pool, &ctx.keys, "t6blend@example.com", &[Role::Recruiter]).await;

    // Create candidates
    let (c1_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('Rust Expert', 'r***@x.com', 8, '{rust,postgres,kubernetes}', 95) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    let (c2_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ('No Skills', 'n***@x.com', 1, '{python}', 30) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Create a role matching c1 better
    let (role_id,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO roles_open (title, required_skills, min_years) \
         VALUES ('Senior Rust Dev', '{rust,postgres}', 5) RETURNING id",
    )
    .fetch_one(&ctx.pool)
    .await
    .unwrap();

    // Audit HIGH H2: cold-start is scoped by (owner, role) per
    // docs/design.md Design Decision #13 — so the 10 feedback rows
    // that flip this recruiter out of cold-start must be authored by
    // THIS recruiter AND bound to THIS role. Feedback authored by
    // anyone else, or bound to a different role, no longer counts.
    for i in 0..10 {
        let cid = if i % 2 == 0 { c1_id } else { c2_id };
        sqlx::query(
            "INSERT INTO talent_feedback (candidate_id, role_id, owner_id, thumb) \
             VALUES ($1, $2, $3, 'up')",
        )
        .bind(cid)
        .bind(role_id)
        .bind(recruiter_id)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/talent/recommendations?role_id={role_id}"))
        .insert_header(("Authorization", format!("Bearer {token}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;

    assert_eq!(body["cold_start"], false, "expected blended after 10 feedback");
    let candidates = body["candidates"].as_array().unwrap();
    assert!(candidates.len() >= 2);
    // Rust Expert should score higher due to better skill match
    assert_eq!(candidates[0]["candidate"]["full_name"], "Rust Expert");

    let reasons = candidates[0]["reasons"].as_array().unwrap();
    assert!(reasons.iter().any(|r| r.as_str().unwrap().contains("Skill match")));
    assert!(reasons.iter().any(|r| r.as_str().unwrap().contains("Experience")));
}

// ─── Audit HIGH H2 — cold-start scope isolation (user + role) ────────────────
//
// docs/design.md Design Decision #13 specifies
// `feedback_count(user, role_scope) < 10` → cold start.
// These tests prove scope isolation per the HIGH H2 verdict:
//   * User A feedback does NOT flip User B out of cold-start.
//   * Role A feedback does NOT flip Role B out of cold-start.
//   * Only feedback matching BOTH axes counts.

async fn seed_candidate_and_role(
    pool: &sqlx::PgPool,
    cand_name: &str,
    role_title: &str,
) -> (uuid::Uuid, uuid::Uuid) {
    let (cid,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO candidates (full_name, email_mask, years_experience, skills, completeness_score) \
         VALUES ($1, 'x***@x.com', 4, '{rust}', 80) RETURNING id",
    )
    .bind(cand_name)
    .fetch_one(pool)
    .await
    .unwrap();
    let (rid,): (uuid::Uuid,) = sqlx::query_as(
        "INSERT INTO roles_open (title, required_skills, min_years) \
         VALUES ($1, '{rust}', 3) RETURNING id",
    )
    .bind(role_title)
    .fetch_one(pool)
    .await
    .unwrap();
    (cid, rid)
}

#[actix_web::test]
async fn t_t6_cold_start_is_isolated_per_owner() {
    // Alice has authored 10 feedback rows against role R — Alice is
    // blended. Bob has authored none — Bob must still see cold-start
    // for the same role.
    let ctx = TestCtx::new().await;
    let (alice_id, _alice_tok) =
        authed(&ctx.pool, &ctx.keys, "h2-alice@example.com", &[Role::Recruiter]).await;
    let (_bob_id, bob_tok) =
        authed(&ctx.pool, &ctx.keys, "h2-bob@example.com", &[Role::Recruiter]).await;

    let (cid, rid) =
        seed_candidate_and_role(&ctx.pool, "H2-Alice-Candidate", "H2 Role Alice").await;

    for _ in 0..10 {
        sqlx::query(
            "INSERT INTO talent_feedback (candidate_id, role_id, owner_id, thumb) \
             VALUES ($1, $2, $3, 'up')",
        )
        .bind(cid)
        .bind(rid)
        .bind(alice_id)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    // Bob calls /recommendations for the SAME role. Under the scoped
    // cold-start contract, Bob has zero scoped feedback → cold_start
    // must still be true even though Alice already has 10.
    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/talent/recommendations?role_id={rid}"))
        .insert_header(("Authorization", format!("Bearer {bob_tok}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    assert_eq!(res.status(), StatusCode::OK);
    let body: Value = test::read_body_json(res).await;
    assert_eq!(
        body["cold_start"], true,
        "Bob has no scoped feedback; Alice's 10 must not move Bob out of cold-start"
    );
    assert_eq!(
        body["total_feedback"].as_i64().unwrap(),
        0,
        "scoped count for (Bob, role) is 0 despite Alice's 10 rows"
    );
}

#[actix_web::test]
async fn t_t6_cold_start_is_isolated_per_role() {
    // Alice has authored 10 feedback rows against role R_a — Alice is
    // blended for R_a. Alice calls /recommendations for a DIFFERENT
    // role R_b (same caller, different role) — must still be
    // cold-start for R_b.
    let ctx = TestCtx::new().await;
    let (alice_id, alice_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "h2-alice-roles@example.com",
        &[Role::Recruiter],
    )
    .await;

    let (cid, r_a) =
        seed_candidate_and_role(&ctx.pool, "H2-Roles-Candidate", "H2 Role A").await;
    let (_c2, r_b) =
        seed_candidate_and_role(&ctx.pool, "H2-Roles-Candidate-B", "H2 Role B").await;

    for _ in 0..10 {
        sqlx::query(
            "INSERT INTO talent_feedback (candidate_id, role_id, owner_id, thumb) \
             VALUES ($1, $2, $3, 'up')",
        )
        .bind(cid)
        .bind(r_a)
        .bind(alice_id)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    let app = test::init_service(build_test_app(ctx.state.clone())).await;

    // Role A: blended.
    let req_a = test::TestRequest::get()
        .uri(&format!("/api/v1/talent/recommendations?role_id={r_a}"))
        .insert_header(("Authorization", format!("Bearer {alice_tok}")))
        .to_request();
    let res_a = test::call_service(&app, req_a).await;
    let body_a: Value = test::read_body_json(res_a).await;
    assert_eq!(body_a["cold_start"], false, "role A has 10 scoped rows → blended");

    // Role B: still cold-start — same caller, different role.
    let req_b = test::TestRequest::get()
        .uri(&format!("/api/v1/talent/recommendations?role_id={r_b}"))
        .insert_header(("Authorization", format!("Bearer {alice_tok}")))
        .to_request();
    let res_b = test::call_service(&app, req_b).await;
    let body_b: Value = test::read_body_json(res_b).await;
    assert_eq!(
        body_b["cold_start"], true,
        "role B has 0 scoped rows for Alice; role A's 10 must not spill over"
    );
    assert_eq!(body_b["total_feedback"].as_i64().unwrap(), 0);
}

#[actix_web::test]
async fn t_t6_cold_start_only_scoped_feedback_triggers_transition() {
    // Exactly-at-threshold: 9 scoped rows → still cold-start. 10 → blended.
    // Additional noise rows (other users, other roles) must not shift
    // the transition.
    let ctx = TestCtx::new().await;
    let (me_id, my_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "h2-threshold@example.com",
        &[Role::Recruiter],
    )
    .await;
    let (other_id, _other_tok) = authed(
        &ctx.pool,
        &ctx.keys,
        "h2-threshold-other@example.com",
        &[Role::Recruiter],
    )
    .await;

    let (cid, my_role) =
        seed_candidate_and_role(&ctx.pool, "H2-Thresh-Candidate", "H2 Thresh Role").await;
    let (_c2, other_role) =
        seed_candidate_and_role(&ctx.pool, "H2-Thresh-Candidate-2", "H2 Thresh Role Other")
            .await;

    // Noise: 50 rows authored by "other" and/or bound to "other_role".
    // None of these are scoped to (me_id, my_role).
    for i in 0..50 {
        let (own, rle) = match i % 3 {
            0 => (other_id, my_role),
            1 => (me_id, other_role),
            _ => (other_id, other_role),
        };
        sqlx::query(
            "INSERT INTO talent_feedback (candidate_id, role_id, owner_id, thumb) \
             VALUES ($1, $2, $3, 'up')",
        )
        .bind(cid)
        .bind(rle)
        .bind(own)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    // Add exactly 9 scoped rows — must stay cold-start.
    for _ in 0..9 {
        sqlx::query(
            "INSERT INTO talent_feedback (candidate_id, role_id, owner_id, thumb) \
             VALUES ($1, $2, $3, 'up')",
        )
        .bind(cid)
        .bind(my_role)
        .bind(me_id)
        .execute(&ctx.pool)
        .await
        .unwrap();
    }

    let app = test::init_service(build_test_app(ctx.state.clone())).await;
    let req = test::TestRequest::get()
        .uri(&format!("/api/v1/talent/recommendations?role_id={my_role}"))
        .insert_header(("Authorization", format!("Bearer {my_tok}")))
        .to_request();
    let res = test::call_service(&app, req).await;
    let body: Value = test::read_body_json(res).await;
    assert_eq!(
        body["cold_start"], true,
        "9 scoped rows + 50 unrelated must still be cold-start"
    );
    assert_eq!(body["total_feedback"].as_i64().unwrap(), 9);

    // Add the 10th scoped row — transition occurs only now.
    sqlx::query(
        "INSERT INTO talent_feedback (candidate_id, role_id, owner_id, thumb) \
         VALUES ($1, $2, $3, 'up')",
    )
    .bind(cid)
    .bind(my_role)
    .bind(me_id)
    .execute(&ctx.pool)
    .await
    .unwrap();

    let req2 = test::TestRequest::get()
        .uri(&format!("/api/v1/talent/recommendations?role_id={my_role}"))
        .insert_header(("Authorization", format!("Bearer {my_tok}")))
        .to_request();
    let res2 = test::call_service(&app, req2).await;
    let body2: Value = test::read_body_json(res2).await;
    assert_eq!(
        body2["cold_start"], false,
        "10th scoped row flips transition; unrelated rows never did"
    );
    assert_eq!(body2["total_feedback"].as_i64().unwrap(), 10);
}
