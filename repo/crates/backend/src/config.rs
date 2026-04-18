//! Single source of truth for runtime configuration.
//!
//! Reads process environment only. No `.env` file is consulted; see
//! `scripts/dev_bootstrap.sh` for how values are populated at container
//! start.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    pub static_dir: PathBuf,
    pub tls_cert_path: PathBuf,
    pub tls_key_path: PathBuf,
    pub runtime_dir: PathBuf,
    pub default_timezone: String,
    pub enforce_tls: bool,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        fn req(key: &str) -> anyhow::Result<String> {
            std::env::var(key).map_err(|_| anyhow::anyhow!("missing required env var {key}"))
        }
        let bind_addr =
            std::env::var("TERRAOPS_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8443".into());
        let database_url = req("DATABASE_URL")?;
        let static_dir = PathBuf::from(
            std::env::var("TERRAOPS_STATIC_DIR").unwrap_or_else(|_| "/app/dist".into()),
        );
        let tls_cert_path = PathBuf::from(
            std::env::var("TERRAOPS_TLS_CERT")
                .unwrap_or_else(|_| "/runtime/certs/server.crt".into()),
        );
        let tls_key_path = PathBuf::from(
            std::env::var("TERRAOPS_TLS_KEY")
                .unwrap_or_else(|_| "/runtime/certs/server.key".into()),
        );
        let runtime_dir = PathBuf::from(
            std::env::var("TERRAOPS_RUNTIME_DIR").unwrap_or_else(|_| "/runtime".into()),
        );
        let default_timezone = std::env::var("TERRAOPS_DEFAULT_TZ")
            .unwrap_or_else(|_| "America/New_York".into());
        let enforce_tls = std::env::var("TERRAOPS_ENFORCE_TLS")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(true);
        Ok(Self {
            bind_addr,
            database_url,
            static_dir,
            tls_cert_path,
            tls_key_path,
            runtime_dir,
            default_timezone,
            enforce_tls,
        })
    }

    /// Test-only constructor used by `tests/common::test_app::spawn_app`.
    pub fn for_testing(database_url: String, runtime_dir: PathBuf) -> Self {
        Self {
            bind_addr: "127.0.0.1:0".into(),
            database_url,
            static_dir: PathBuf::from("/tmp/terraops-test-dist"),
            tls_cert_path: PathBuf::new(),
            tls_key_path: PathBuf::new(),
            runtime_dir,
            default_timezone: "America/New_York".into(),
            enforce_tls: false,
        }
    }
}
