//! Idempotent demo seeder.
//!
//! Creates one user per canonical role with the README-documented
//! credentials so `docker compose up --build && ./init_db.sh` leaves the
//! admin console usable right away.
//!
//!   admin@terraops.local       → Administrator
//!   steward@terraops.local     → Data Steward
//!   analyst@terraops.local     → Analyst
//!   recruiter@terraops.local   → Recruiter
//!   user@terraops.local        → Regular User
//!
//! Password for every account: `TerraOps!2026`.
//!
//! The seeder re-runs cleanly: existing users keep their id + password
//! hash, missing users are created, and the role grants are rebuilt so
//! the matrix always matches the committed expectation.

use chrono::{Duration, Utc};
use sqlx::PgPool;
use terraops_shared::roles::Role;
use uuid::Uuid;

use crate::{
    crypto::{
        argon,
        email::{email_hash, email_mask, encrypt_email, normalize_email},
        keys::RuntimeKeys,
    },
    errors::AppResult,
};

pub const DEMO_PASSWORD: &str = "TerraOps!2026";

struct Demo {
    email: &'static str,
    display: &'static str,
    role: Role,
}

pub async fn seed_demo(pool: &PgPool, keys: &RuntimeKeys) -> AppResult<()> {
    let demos = [
        Demo {
            email: "admin@terraops.local",
            display: "Demo Administrator",
            role: Role::Administrator,
        },
        Demo {
            email: "steward@terraops.local",
            display: "Demo Data Steward",
            role: Role::DataSteward,
        },
        Demo {
            email: "analyst@terraops.local",
            display: "Demo Analyst",
            role: Role::Analyst,
        },
        Demo {
            email: "recruiter@terraops.local",
            display: "Demo Recruiter",
            role: Role::Recruiter,
        },
        Demo {
            email: "user@terraops.local",
            display: "Demo Regular User",
            role: Role::RegularUser,
        },
    ];

    for d in demos {
        let normalized = normalize_email(d.email);
        let hash = email_hash(&normalized, &keys.email_hmac).to_vec();
        // Look up by email_hash; if present, leave password/ct untouched
        // (operator-set passwords win over re-seed).
        let existing: Option<(uuid::Uuid,)> =
            sqlx::query_as("SELECT id FROM users WHERE email_hash = $1")
                .bind(&hash)
                .fetch_optional(pool)
                .await?;
        let user_id = if let Some((id,)) = existing {
            id
        } else {
            let ct = encrypt_email(&normalized, &keys.email_enc)
                .map_err(|e| crate::errors::AppError::Internal(format!("email enc: {e}")))?;
            let mask = email_mask(&normalized);
            let phc = argon::hash_password(DEMO_PASSWORD)
                .map_err(|e| crate::errors::AppError::Internal(format!("argon: {e}")))?;
            let row: (uuid::Uuid,) = sqlx::query_as(
                "INSERT INTO users (display_name, email_ciphertext, email_hash, \
                                    email_mask, password_hash, timezone) \
                 VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
            )
            .bind(d.display)
            .bind(&ct)
            .bind(&hash)
            .bind(&mask)
            .bind(&phc)
            .bind("America/New_York")
            .fetch_one(pool)
            .await?;
            row.0
        };

        // Rebuild role grant to exactly one role (the demo role).
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO user_roles (user_id, role_id) \
             SELECT $1, r.id FROM roles r WHERE r.name = $2",
        )
        .bind(user_id)
        .bind(d.role.as_db())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
    }

    // Seed the cross-domain demo dataset so the normal init flow leaves the
    // admin console navigable out of the box (not just the 5 users).
    seed_demo_dataset(pool, keys).await?;

    Ok(())
}

/// Look up a demo user's id by email (returns None if missing).
async fn user_id_by_email(
    pool: &PgPool,
    keys: &RuntimeKeys,
    email: &str,
) -> AppResult<Option<Uuid>> {
    let normalized = normalize_email(email);
    let hash = email_hash(&normalized, &keys.email_hmac).to_vec();
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE email_hash = $1")
        .bind(&hash)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(id,)| id))
}

/// Idempotent cross-domain fixture seeder.
///
/// Leaves a demo repo that lets every role exercise their real UI the
/// moment `docker compose up --build && ./init_db.sh` finishes:
///
///   * 1 site + 2 departments + 1 category + 1 brand + 1 unit
///   * 3 products (on-shelf) with tax rates + change history
///   * 2 env sources with 24 recent observations each
///   * 2 metric definitions with 12 computations
///   * 1 alert rule + 1 fired alert event
///   * 5 candidates + 2 open roles + 12 talent_feedback rows owned by the
///     demo recruiter (crosses the 10-row cold-start threshold)
///   * 3 unread notifications for the demo admin
///
/// Uses sentinel names/SKUs + `ON CONFLICT DO NOTHING` / existence checks
/// so a second invocation is a no-op (matches the existing user seeder).
pub async fn seed_demo_dataset(pool: &PgPool, keys: &RuntimeKeys) -> AppResult<()> {
    let admin_id = user_id_by_email(pool, keys, "admin@terraops.local").await?;
    let analyst_id = user_id_by_email(pool, keys, "analyst@terraops.local").await?;
    let steward_id = user_id_by_email(pool, keys, "steward@terraops.local").await?;
    let recruiter_id = user_id_by_email(pool, keys, "recruiter@terraops.local").await?;

    // ── Reference data ──────────────────────────────────────────────────
    let (site_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO sites (code, name) VALUES ('DEMO-HQ', 'Demo Headquarters') \
         ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name RETURNING id",
    )
    .fetch_one(pool)
    .await?;

    let (dept_a,): (Uuid,) = sqlx::query_as(
        "INSERT INTO departments (site_id, code, name) VALUES ($1, 'OPS', 'Operations') \
         ON CONFLICT (site_id, code) DO UPDATE SET name = EXCLUDED.name RETURNING id",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?;

    let (_dept_b,): (Uuid,) = sqlx::query_as(
        "INSERT INTO departments (site_id, code, name) VALUES ($1, 'ENG', 'Engineering') \
         ON CONFLICT (site_id, code) DO UPDATE SET name = EXCLUDED.name RETURNING id",
    )
    .bind(site_id)
    .fetch_one(pool)
    .await?;

    let cat_row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM categories WHERE name = 'Demo Catalog' AND parent_id IS NULL")
            .fetch_optional(pool)
            .await?;
    let cat_id = if let Some((id,)) = cat_row {
        id
    } else {
        let (id,): (Uuid,) =
            sqlx::query_as("INSERT INTO categories (name) VALUES ('Demo Catalog') RETURNING id")
                .fetch_one(pool)
                .await?;
        id
    };

    let (brand_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO brands (name) VALUES ('DemoBrand') \
         ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name RETURNING id",
    )
    .fetch_one(pool)
    .await?;

    let (unit_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO units (code, description) VALUES ('EA', 'Each') \
         ON CONFLICT (code) DO UPDATE SET description = EXCLUDED.description RETURNING id",
    )
    .fetch_one(pool)
    .await?;

    // ── Products ────────────────────────────────────────────────────────
    let demo_skus = [
        ("DEMO-001", "Demo Widget Alpha", 1999),
        ("DEMO-002", "Demo Widget Beta", 2499),
        ("DEMO-003", "Demo Widget Gamma", 3499),
    ];
    for (sku, name, price_cents) in demo_skus {
        let existing: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM products WHERE sku = $1")
            .bind(sku)
            .fetch_optional(pool)
            .await?;
        if existing.is_some() {
            continue;
        }
        let (product_id,): (Uuid,) = sqlx::query_as(
            "INSERT INTO products (sku, name, description, category_id, brand_id, unit_id, \
                                   site_id, department_id, on_shelf, price_cents, currency, \
                                   created_by, updated_by) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, TRUE, $9, 'USD', $10, $10) \
             RETURNING id",
        )
        .bind(sku)
        .bind(name)
        .bind(format!("Seeded demo product for {sku}."))
        .bind(cat_id)
        .bind(brand_id)
        .bind(unit_id)
        .bind(site_id)
        .bind(dept_a)
        .bind(price_cents as i32)
        .bind(steward_id)
        .fetch_one(pool)
        .await?;

        sqlx::query(
            "INSERT INTO product_tax_rates (product_id, state_code, rate_bp) \
             VALUES ($1, 'CA', 725) ON CONFLICT DO NOTHING",
        )
        .bind(product_id)
        .execute(pool)
        .await?;

        sqlx::query(
            "INSERT INTO product_history (product_id, action, changed_by, after_json) \
             VALUES ($1, 'create', $2, $3::jsonb)",
        )
        .bind(product_id)
        .bind(steward_id)
        .bind(format!(
            "{{\"sku\":\"{sku}\",\"name\":\"{name}\",\"seeded\":true}}"
        ))
        .execute(pool)
        .await?;
    }

    // ── Env sources + observations ──────────────────────────────────────
    let env_sources = [
        ("Demo Temperature Sensor", "temperature", "celsius"),
        ("Demo Humidity Sensor", "humidity", "percent"),
    ];
    let mut src_ids: Vec<Uuid> = Vec::new();
    for (name, kind, unit) in env_sources {
        let existing: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM env_sources WHERE name = $1")
                .bind(name)
                .fetch_optional(pool)
                .await?;
        let src_id = if let Some((id,)) = existing {
            id
        } else {
            let (id,): (Uuid,) = sqlx::query_as(
                "INSERT INTO env_sources (name, kind, site_id, department_id, unit_id) \
                 VALUES ($1, $2, $3, $4, $5) RETURNING id",
            )
            .bind(name)
            .bind(kind)
            .bind(site_id)
            .bind(dept_a)
            .bind(unit_id)
            .fetch_one(pool)
            .await?;
            id
        };
        src_ids.push(src_id);

        // Seed observations only on first seed (guard by counting).
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM env_observations WHERE source_id = $1")
                .bind(src_id)
                .fetch_one(pool)
                .await?;
        if n == 0 {
            let base = Utc::now();
            for i in 0..24i64 {
                let t = base - Duration::hours(i);
                let val: f64 = if kind == "temperature" {
                    20.0 + (i as f64) * 0.25
                } else {
                    45.0 + (i as f64) * 0.4
                };
                sqlx::query(
                    "INSERT INTO env_observations (source_id, observed_at, value, unit) \
                     VALUES ($1, $2, $3, $4)",
                )
                .bind(src_id)
                .bind(t)
                .bind(val)
                .bind(unit)
                .execute(pool)
                .await?;
            }
        }
    }

    // ── Metric definitions + computations ───────────────────────────────
    let metric_specs = [
        ("demo.temp.moving_avg", "moving_average"),
        ("demo.humidity.rate_of_change", "rate_of_change"),
    ];
    for (idx, (mname, mkind)) in metric_specs.iter().enumerate() {
        let existing: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM metric_definitions WHERE name = $1")
                .bind(mname)
                .fetch_optional(pool)
                .await?;
        let def_id = if let Some((id,)) = existing {
            id
        } else {
            let src_slice: &[Uuid] = if idx < src_ids.len() {
                &src_ids[idx..=idx]
            } else {
                &[]
            };
            let (id,): (Uuid,) = sqlx::query_as(
                "INSERT INTO metric_definitions (name, formula_kind, params, source_ids, \
                                                 window_seconds, enabled, created_by) \
                 VALUES ($1, $2, '{}'::jsonb, $3, 3600, TRUE, $4) RETURNING id",
            )
            .bind(mname)
            .bind(mkind)
            .bind(src_slice)
            .bind(analyst_id)
            .fetch_one(pool)
            .await?;
            id
        };

        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM metric_computations WHERE definition_id = $1")
                .bind(def_id)
                .fetch_one(pool)
                .await?;
        if n == 0 {
            let base = Utc::now();
            for i in 0..6i64 {
                let t = base - Duration::hours(i);
                sqlx::query(
                    "INSERT INTO metric_computations (definition_id, computed_at, result, \
                                                      inputs, window_start, window_end) \
                     VALUES ($1, $2, $3, '[]'::jsonb, $4, $5)",
                )
                .bind(def_id)
                .bind(t)
                .bind(20.0 + i as f64)
                .bind(t - Duration::hours(1))
                .bind(t)
                .execute(pool)
                .await?;
            }
        }

        // Alert rule + a fired event on the first metric only.
        if idx == 0 {
            let existing_rule: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM alert_rules WHERE metric_definition_id = $1 LIMIT 1",
            )
            .bind(def_id)
            .fetch_optional(pool)
            .await?;
            let rule_id = if let Some((id,)) = existing_rule {
                id
            } else {
                let (id,): (Uuid,) = sqlx::query_as(
                    "INSERT INTO alert_rules (metric_definition_id, threshold, operator, \
                                              duration_seconds, severity, enabled, created_by) \
                     VALUES ($1, 25.0, '>', 0, 'warning', TRUE, $2) RETURNING id",
                )
                .bind(def_id)
                .bind(analyst_id)
                .fetch_one(pool)
                .await?;
                id
            };

            let (n_evt,): (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM alert_events WHERE rule_id = $1")
                    .bind(rule_id)
                    .fetch_one(pool)
                    .await?;
            if n_evt == 0 {
                sqlx::query(
                    "INSERT INTO alert_events (rule_id, fired_at, value) \
                     VALUES ($1, NOW() - INTERVAL '10 minutes', 26.5)",
                )
                .bind(rule_id)
                .execute(pool)
                .await?;
            }
        }
    }

    // ── Talent: candidates + roles + feedback (crosses cold-start) ──────
    let candidates = [
        ("Avery Baker", "avery@demo", "Remote", 5, vec!["rust", "sql"], 78),
        ("Jordan Cruz", "jordan@demo", "Austin", 7, vec!["rust", "postgres", "actix"], 88),
        ("Morgan Diaz", "morgan@demo", "NYC", 3, vec!["yew", "wasm"], 62),
        ("Riley Evans", "riley@demo", "Seattle", 10, vec!["rust", "distributed"], 92),
        ("Sam Fisher", "sam@demo", "Remote", 2, vec!["sql", "bi"], 55),
    ];
    let mut cand_ids: Vec<Uuid> = Vec::new();
    for (name, email_hint, loc, yrs, skills, completeness) in &candidates {
        let existing: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM candidates WHERE email_mask = $1")
                .bind(email_hint)
                .fetch_optional(pool)
                .await?;
        let cid = if let Some((id,)) = existing {
            id
        } else {
            let (id,): (Uuid,) = sqlx::query_as(
                "INSERT INTO candidates (full_name, email_mask, location, years_experience, \
                                         skills, bio, completeness_score, last_active_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, NOW()) RETURNING id",
            )
            .bind(name)
            .bind(email_hint)
            .bind(loc)
            .bind(*yrs as i32)
            .bind(skills)
            .bind(format!("Seeded demo candidate {name}."))
            .bind(*completeness as i32)
            .fetch_one(pool)
            .await?;
            id
        };
        cand_ids.push(cid);
    }

    let open_roles = [
        ("Senior Backend Engineer", vec!["rust", "postgres"], 5),
        ("Frontend WASM Engineer", vec!["yew", "wasm"], 2),
    ];
    let mut role_ids: Vec<Uuid> = Vec::new();
    for (title, skills, min_yrs) in &open_roles {
        let existing: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM roles_open WHERE title = $1")
                .bind(title)
                .fetch_optional(pool)
                .await?;
        let rid = if let Some((id,)) = existing {
            id
        } else {
            let (id,): (Uuid,) = sqlx::query_as(
                "INSERT INTO roles_open (title, department_id, required_skills, min_years, \
                                         site_id, status, created_by) \
                 VALUES ($1, $2, $3, $4, $5, 'open', $6) RETURNING id",
            )
            .bind(title)
            .bind(dept_a)
            .bind(skills)
            .bind(*min_yrs as i32)
            .bind(site_id)
            .bind(recruiter_id)
            .fetch_one(pool)
            .await?;
            id
        };
        role_ids.push(rid);
    }

    // Feedback: seed 12 rows owned by the recruiter (crosses the 10-row
    // cold-start threshold). Idempotent: only seed if count < 12.
    if let Some(rec_id) = recruiter_id {
        let (fb_n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM talent_feedback WHERE owner_id = $1")
                .bind(rec_id)
                .fetch_one(pool)
                .await?;
        if fb_n < 12 && !cand_ids.is_empty() {
            let needed = 12 - fb_n;
            for i in 0..needed {
                let cand = cand_ids[(i as usize) % cand_ids.len()];
                let role = role_ids.first().copied();
                let thumb = if i % 3 == 0 { "down" } else { "up" };
                sqlx::query(
                    "INSERT INTO talent_feedback (candidate_id, role_id, owner_id, thumb, note) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(cand)
                .bind(role)
                .bind(rec_id)
                .bind(thumb)
                .bind(format!("Seeded feedback #{}", i + 1))
                .execute(pool)
                .await?;
            }
        }
    }

    // ── Notifications for admin (3 unread) ──────────────────────────────
    if let Some(a_id) = admin_id {
        let (n_notif,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND topic LIKE 'demo.%'",
        )
        .bind(a_id)
        .fetch_one(pool)
        .await?;
        if n_notif == 0 {
            let seeds = [
                ("demo.welcome", "Welcome to TerraOps", "Demo dataset seeded; explore every role workspace."),
                ("demo.import.committed", "Demo import committed", "3 products seeded into DEMO-HQ / Operations."),
                ("demo.alert.fired", "Demo alert fired", "Temperature moving average exceeded 25.0."),
            ];
            for (topic, title, body) in seeds {
                sqlx::query(
                    "INSERT INTO notifications (user_id, topic, title, body) \
                     VALUES ($1, $2, $3, $4)",
                )
                .bind(a_id)
                .bind(topic)
                .bind(title)
                .bind(body)
                .execute(pool)
                .await?;
            }
        }
    }

    Ok(())
}
