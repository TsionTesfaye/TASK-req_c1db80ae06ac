//! `AppState` — the Actix-web `web::Data` payload wiring the pool, the key
//! material loaded on boot, and the mTLS pin-set (shared by the admin
//! endpoints and — in a future P4 addition — the rustls client-cert
//! verifier).

use std::{path::PathBuf, sync::Arc};

use sqlx::postgres::PgPool;

use crate::crypto::keys::RuntimeKeys;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub keys: Arc<RuntimeKeys>,
    pub static_dir: PathBuf,
    pub default_timezone: String,
    /// Runtime directory for locally-materialized artifacts (mailbox .mbox
    /// exports, signed images, etc). Populated from
    /// `Config::runtime_dir`; defaults to `/runtime` in Docker and to the
    /// test runtime dir when under `Config::for_testing`.
    pub runtime_dir: PathBuf,
    /// Value of `mtls_config.enforced` captured at process startup. The
    /// rustls `ServerConfig` is built once from this flag, so it is the
    /// *live* TLS mode for this process; later DB PATCHes to
    /// `mtls_config` only take effect on the next restart. Exposed via
    /// `GET /security/mtls` and `/security/mtls/status` so the admin
    /// contract is honest about the restart gate (Audit #12 Issue #3).
    pub mtls_startup_enforced: bool,
}
