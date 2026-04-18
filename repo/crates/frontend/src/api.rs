//! Minimal API client seed.
//!
//! Scaffold-level: exposes the base URL and the hard contract timings
//! (3 s timeout, single GET retry). Real typed endpoint callers and the
//! `wasm-bindgen-test` unit tests land in P1 per `plan.md`.

/// Request timeout, per design.md §Budget rules.
pub const REQUEST_TIMEOUT_MS: u32 = 3_000;

/// Single-retry-on-GET policy: the first GET failure (network or 5xx) is
/// retried exactly once. Non-GET verbs are never retried.
pub const GET_RETRIES: u32 = 1;

/// Base URL for the REST API. The SPA and API share a single TLS origin
/// (`:8443`) so a relative prefix is correct in every deployment.
pub const API_BASE: &str = "/api/v1";
