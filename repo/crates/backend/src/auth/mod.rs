//! Authentication & authorization layer.
//!
//! Real implementations (no stubs):
//!   * `sessions`   — opaque refresh tokens (SHA-256 at rest, 30d idle /
//!                    90d absolute, rotated on every refresh).
//!   * `extractors` — Actix extractors: `AuthUser`, `RequirePermission`,
//!                    `OwnerGuard` (for `SELF` / `PERM_OR_SELF`).
//!   * `password`   — login verify + change-password (argon2id + lockout).

pub mod extractors;
pub mod password;
pub mod sessions;

pub use extractors::{AuthUser, OwnerGuard, RequirePermission};
