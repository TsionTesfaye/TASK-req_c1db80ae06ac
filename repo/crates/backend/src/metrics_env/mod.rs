//! Env sources + observations + metric definitions + formula executor + lineage.
//!
//! Sub-modules:
//!   * `sources`     — CRUD for env_sources
//!   * `definitions` — CRUD for metric_definitions
//!   * `formula`     — pure formula implementations (moving_average, rate_of_change, comfort_index)
//!   * `lineage`     — computation lineage retrieval
//!   * `handlers`    — Actix-web route handlers (E1–E6, MD1–MD7)

pub mod definitions;
pub mod formula;
pub mod handlers;
pub mod lineage;
pub mod sources;
