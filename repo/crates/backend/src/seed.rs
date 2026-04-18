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

use sqlx::PgPool;
use terraops_shared::roles::Role;

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

    Ok(())
}
