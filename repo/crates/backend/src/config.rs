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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env reads in `from_env` race across tests; serialize them.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce() -> R, R>(pairs: &[(&str, Option<&str>)], f: F) -> R {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // Snapshot+set
        let saved: Vec<(String, Option<String>)> = pairs
            .iter()
            .map(|(k, _)| (k.to_string(), std::env::var(k).ok()))
            .collect();
        for (k, v) in pairs {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
        let out = f();
        // Restore
        for (k, v) in saved {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
        out
    }

    #[test]
    fn from_env_errors_when_database_url_missing() {
        with_env(&[("DATABASE_URL", None)], || {
            let err = Config::from_env().unwrap_err().to_string();
            assert!(
                err.contains("DATABASE_URL"),
                "error should name missing var, got: {err}"
            );
        });
    }

    #[test]
    fn from_env_applies_defaults_when_optional_vars_absent() {
        with_env(
            &[
                ("DATABASE_URL", Some("postgres://u:p@h/db")),
                ("TERRAOPS_BIND_ADDR", None),
                ("TERRAOPS_STATIC_DIR", None),
                ("TERRAOPS_TLS_CERT", None),
                ("TERRAOPS_TLS_KEY", None),
                ("TERRAOPS_RUNTIME_DIR", None),
                ("TERRAOPS_DEFAULT_TZ", None),
                ("TERRAOPS_ENFORCE_TLS", None),
            ],
            || {
                let c = Config::from_env().expect("defaults should yield valid config");
                assert_eq!(c.bind_addr, "0.0.0.0:8443");
                assert_eq!(c.database_url, "postgres://u:p@h/db");
                assert_eq!(c.static_dir, PathBuf::from("/app/dist"));
                assert_eq!(c.tls_cert_path, PathBuf::from("/runtime/certs/server.crt"));
                assert_eq!(c.tls_key_path, PathBuf::from("/runtime/certs/server.key"));
                assert_eq!(c.runtime_dir, PathBuf::from("/runtime"));
                assert_eq!(c.default_timezone, "America/New_York");
                assert!(c.enforce_tls, "enforce_tls defaults to true");
            },
        );
    }

    #[test]
    fn from_env_enforce_tls_parses_truthy_and_falsy() {
        for (raw, expected) in [
            ("1", true),
            ("true", true),
            ("TRUE", true),
            ("yes", true),
            ("0", false),
            ("false", false),
            ("no", false),
            ("nonsense", false),
            ("", false),
        ] {
            with_env(
                &[
                    ("DATABASE_URL", Some("postgres://u:p@h/db")),
                    ("TERRAOPS_ENFORCE_TLS", Some(raw)),
                ],
                || {
                    let c = Config::from_env().unwrap();
                    assert_eq!(
                        c.enforce_tls, expected,
                        "TERRAOPS_ENFORCE_TLS={raw:?} → expected {expected}"
                    );
                },
            );
        }
    }

    #[test]
    fn from_env_honors_all_overrides() {
        with_env(
            &[
                ("DATABASE_URL", Some("postgres://x")),
                ("TERRAOPS_BIND_ADDR", Some("127.0.0.1:4443")),
                ("TERRAOPS_STATIC_DIR", Some("/srv/www")),
                ("TERRAOPS_TLS_CERT", Some("/k/s.crt")),
                ("TERRAOPS_TLS_KEY", Some("/k/s.key")),
                ("TERRAOPS_RUNTIME_DIR", Some("/var/run/t")),
                ("TERRAOPS_DEFAULT_TZ", Some("UTC")),
                ("TERRAOPS_ENFORCE_TLS", Some("false")),
            ],
            || {
                let c = Config::from_env().unwrap();
                assert_eq!(c.bind_addr, "127.0.0.1:4443");
                assert_eq!(c.static_dir, PathBuf::from("/srv/www"));
                assert_eq!(c.tls_cert_path, PathBuf::from("/k/s.crt"));
                assert_eq!(c.tls_key_path, PathBuf::from("/k/s.key"));
                assert_eq!(c.runtime_dir, PathBuf::from("/var/run/t"));
                assert_eq!(c.default_timezone, "UTC");
                assert!(!c.enforce_tls);
            },
        );
    }

    #[test]
    fn for_testing_sets_expected_shape() {
        let c = Config::for_testing("postgres://test".into(), PathBuf::from("/tmp/rt"));
        assert_eq!(c.database_url, "postgres://test");
        assert_eq!(c.runtime_dir, PathBuf::from("/tmp/rt"));
        assert_eq!(c.bind_addr, "127.0.0.1:0");
        assert!(!c.enforce_tls, "test harness never enforces TLS");
        assert_eq!(c.default_timezone, "America/New_York");
    }

    #[test]
    fn config_is_clone_and_debug() {
        // Catches accidental removal of the derived traits that the rest
        // of the app relies on (AppState clones Config into middleware
        // init, tracing logs Debug).
        let c = Config::for_testing("postgres://t".into(), PathBuf::from("/rt"));
        let _clone = c.clone();
        let dbg = format!("{c:?}");
        assert!(dbg.contains("Config"));
    }
}
