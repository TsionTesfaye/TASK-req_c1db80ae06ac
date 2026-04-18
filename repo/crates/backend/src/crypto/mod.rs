//! Crypto primitives: argon2id password hashing, HS256 JWTs, AES-256-GCM
//! email encryption, HMAC-SHA256 email/image lookup, and image signed-URL
//! signing.
//!
//! Key material is loaded at boot from the runtime Docker volume (see
//! `scripts/dev_bootstrap.sh`). Nothing in this module touches the file
//! system at request time — the keys live in `AppState`.

pub mod argon;
pub mod email;
pub mod jwt;
pub mod keys;
pub mod signed_url;
