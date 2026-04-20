//! Database connection + migration runner.

use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::config::Config;

pub async fn connect(cfg: &Config) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(16)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&cfg.database_url)
        .await?;
    Ok(pool)
}

/// Apply migrations bundled from `crates/backend/migrations/`.
pub async fn run_migrations(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Focused edge-case tests for the DB bootstrap path. These don't
    //! require a real Postgres because they exercise the connection
    //! URL parsing / connect-timeout surface, which is the part most
    //! likely to regress silently.

    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;

    #[tokio::test]
    async fn connect_fails_fast_on_malformed_url() {
        let cfg = Config::for_testing("not-a-postgres-url".into(), PathBuf::from("/tmp"));
        let res = connect(&cfg).await;
        assert!(res.is_err(), "expected connect() to reject malformed URL");
    }

    #[tokio::test]
    async fn connect_fails_on_unreachable_host_within_timeout() {
        // Reserved TEST-NET-1 address (RFC 5737) — never routable, so
        // the `.acquire_timeout(5s)` budget must kick in rather than
        // hanging the test runner indefinitely.
        let cfg = Config::for_testing(
            "postgres://user:pw@192.0.2.1:5432/nope".into(),
            PathBuf::from("/tmp"),
        );
        let start = std::time::Instant::now();
        let res = tokio::time::timeout(std::time::Duration::from_secs(10), connect(&cfg)).await;
        let elapsed = start.elapsed();
        assert!(res.is_ok(), "connect() must honor acquire_timeout, not hang");
        assert!(res.unwrap().is_err(), "unreachable host must produce Err");
        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "connect() should return within the 5s acquire_timeout budget (plus grace), got {elapsed:?}"
        );
    }
}
