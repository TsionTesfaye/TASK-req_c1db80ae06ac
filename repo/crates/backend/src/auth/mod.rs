//! Authentication & authorization layer.
//!
//! Contents land in P1:
//!   * `sessions`   — opaque refresh tokens (SHA-256 at rest, 30d idle /
//!                    90d absolute, rotated on every refresh).
//!   * `extractors` — Actix extractors: `AuthUser`, `RequirePermission`,
//!                    `OwnerGuard` (for `SELF` / `PERM_OR_SELF`).
//!   * `password`   — login + change-password flow (argon2id verify +
//!                    lockout counter).
