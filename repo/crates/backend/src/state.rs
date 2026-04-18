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
}
