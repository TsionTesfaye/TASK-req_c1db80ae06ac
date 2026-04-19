//! HTTP route handlers, grouped by feature domain.

pub mod auth;
pub mod monitoring;
pub mod notifications;
pub mod ref_data;
pub mod retention;
pub mod security;
pub mod system;
pub mod users;

use actix_web::web;

/// Mount every route family under `/api/v1`.
pub fn configure(cfg: &mut web::ServiceConfig) {
    system::configure(cfg);
    auth::configure(cfg);
    users::configure(cfg);
    security::configure(cfg);
    retention::configure(cfg);
    monitoring::configure(cfg);
    ref_data::configure(cfg);
    notifications::configure(cfg);
    // P-A: Catalog & Governance
    crate::products::handlers::configure(cfg);
    // P-B: Environmental Intelligence / KPI / Alerts / Reports
    crate::metrics_env::handlers::configure(cfg);
    crate::kpi::handlers::configure(cfg);
    crate::alerts::handlers::configure(cfg);
    crate::reports::handlers::configure(cfg);
    // P-C: Talent Intelligence
    crate::talent::handlers::configure(cfg);
}
