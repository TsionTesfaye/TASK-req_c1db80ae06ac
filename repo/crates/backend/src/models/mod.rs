//! DB row types shared across services and handlers.

pub mod user;

pub use user::{UserRow, UserWithRoles};
