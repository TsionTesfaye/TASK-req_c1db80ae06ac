//! TerraOps backend library.
//!
//! The `terraops-backend` binary composes these modules into an Actix-web
//! server. Exposing them through a library crate is what allows the repo's
//! `tests/http/` integration suite to spin up a real HTTP app against a
//! real Postgres database for no-mock endpoint testing.

pub mod alerts;
pub mod app;
pub mod auth;
pub mod config;
pub mod crypto;
pub mod db;
pub mod errors;
pub mod handlers;
pub mod kpi;
pub mod metrics_env;
pub mod middleware;
pub mod models;
pub mod products;
pub mod reports;
pub mod seed;
pub mod services;
pub mod spa;
pub mod state;
pub mod storage;
pub mod talent;
pub mod tls;
