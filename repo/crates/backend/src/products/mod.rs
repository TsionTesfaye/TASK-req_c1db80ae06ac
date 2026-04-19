//! Products + Imports domain — Catalog & Governance (P-A).
//!
//! Sub-modules:
//!   repo             — database access layer (products CRUD)
//!   service          — business logic, notification emit
//!   handlers         — Actix HTTP handlers (P1–P14 + I1–I7)
//!   history          — product_history write helpers
//!   tax_rates        — product_tax_rates handlers
//!   images           — product_images upload/delete + signed URL
//!   import           — import batch upload/parse/commit/cancel
//!   import_validator — row-level validation rules
//!   export           — CSV + XLSX streaming export

pub mod export;
pub mod handlers;
pub mod history;
pub mod images;
pub mod import;
pub mod import_validator;
pub mod repo;
pub mod service;
pub mod tax_rates;
