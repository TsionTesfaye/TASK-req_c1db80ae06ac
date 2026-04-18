//! HTTP middleware stack.
//!
//! Contents land in P1:
//!   * `request_id`     — X-Request-Id in/out + structured log context.
//!   * `allowlist`      — CIDR allow-list enforcement for authenticated routes.
//!   * `budget`         — 3s hard request budget → 504 TIMEOUT.
//!   * `metrics`        — per-route latency + error bucketing into `api_metrics`.
//!   * `error_normalize`— unified JSON error envelope.
//!   * `authn`          — bearer-token parse + session validity check.
