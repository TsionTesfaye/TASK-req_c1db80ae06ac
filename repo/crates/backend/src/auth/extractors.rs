//! Actix extractors that turn the validated JWT into a typed `AuthUser`
//! and carry per-request authorization decisions (`RequirePermission`,
//! `OwnerGuard`).
//!
//! The JWT is validated once by the `authn` middleware, which stuffs an
//! `AuthContext` into request extensions. Extractors below just read that.

use std::future::{ready, Ready};

use actix_web::{dev::Payload, FromRequest, HttpMessage, HttpRequest};
use uuid::Uuid;

use crate::errors::AppError;

/// Populated by `middleware::authn` once the bearer token has been verified
/// against both the HS256 key AND the session row (so revoking a session
/// immediately invalidates any in-flight JWT).
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub roles: Vec<terraops_shared::roles::Role>,
    pub permissions: Vec<String>,
    pub display_name: String,
    pub email_mask: String,
    pub timezone: Option<String>,
}

impl AuthContext {
    pub fn has_permission(&self, code: &str) -> bool {
        self.permissions.iter().any(|p| p == code)
    }
}

/// Extractor: requires an authenticated user. Produces 401 when absent.
pub struct AuthUser(pub AuthContext);

impl FromRequest for AuthUser {
    type Error = AppError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _pl: &mut Payload) -> Self::Future {
        ready(
            req.extensions()
                .get::<AuthContext>()
                .cloned()
                .map(AuthUser)
                .ok_or(AppError::AuthRequired),
        )
    }
}

/// Extractor: requires a specific permission code. Use via
/// `actix_web::guard::fn_guard` style is replaced by explicit check in
/// the handler using `AuthUser` + `.require_permission(...)` below — this
/// type exists for documentation / future guard wiring.
pub struct RequirePermission(pub AuthContext);

impl RequirePermission {
    pub fn check(ctx: &AuthContext, code: &str) -> Result<(), AppError> {
        if ctx.has_permission(code) {
            Ok(())
        } else {
            Err(AppError::Forbidden("missing permission"))
        }
    }
}

/// Object-scope guard for `SELF` and `PERM_OR_SELF` routes.
///
/// Usage: `OwnerGuard::allow(&ctx, path_user_id, "user.manage")?;`
/// — allows the actor if they are the owner OR they hold the given
/// override permission.
pub struct OwnerGuard;

impl OwnerGuard {
    pub fn allow_self(ctx: &AuthContext, owner_id: Uuid) -> Result<(), AppError> {
        if ctx.user_id == owner_id {
            Ok(())
        } else {
            Err(AppError::Forbidden("not owner"))
        }
    }

    pub fn allow_self_or_permission(
        ctx: &AuthContext,
        owner_id: Uuid,
        override_permission: &str,
    ) -> Result<(), AppError> {
        if ctx.user_id == owner_id || ctx.has_permission(override_permission) {
            Ok(())
        } else {
            Err(AppError::Forbidden("not owner and missing override permission"))
        }
    }
}

/// Convenience: require a permission on the already-authenticated user.
pub fn require_permission(ctx: &AuthContext, code: &str) -> Result<(), AppError> {
    RequirePermission::check(ctx, code)
}

/// Require *any* of the listed permissions — the caller needs at least one.
/// Used where a role's capabilities imply a strict superset (e.g.
/// `talent.manage` → can also do everything `talent.read` can), so the
/// gate accepts either permission rather than duplicating grants at the
/// RBAC seed layer (Audit #8 Issue #1).
pub fn require_any_permission(ctx: &AuthContext, codes: &[&str]) -> Result<(), AppError> {
    for code in codes {
        if ctx.permissions.iter().any(|p| p == *code) {
            return Ok(());
        }
    }
    Err(AppError::Forbidden("missing permission"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn ctx(perms: &[&str]) -> AuthContext {
        AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            roles: vec![],
            permissions: perms.iter().map(|s| s.to_string()).collect(),
            display_name: "Test".into(),
            email_mask: "t***@example.com".into(),
            timezone: None,
        }
    }

    #[test]
    fn has_permission_positive() {
        let c = ctx(&["product.read", "product.manage"]);
        assert!(c.has_permission("product.read"));
    }

    #[test]
    fn has_permission_negative() {
        let c = ctx(&["product.read"]);
        assert!(!c.has_permission("product.manage"));
    }

    #[test]
    fn require_permission_ok() {
        let c = ctx(&["talent.read"]);
        assert!(RequirePermission::check(&c, "talent.read").is_ok());
    }

    #[test]
    fn require_permission_forbidden() {
        let c = ctx(&["talent.read"]);
        assert!(RequirePermission::check(&c, "talent.manage").is_err());
    }

    #[test]
    fn owner_guard_allow_self_ok() {
        let c = ctx(&[]);
        assert!(OwnerGuard::allow_self(&c, c.user_id).is_ok());
    }

    #[test]
    fn owner_guard_allow_self_rejects_other() {
        let c = ctx(&[]);
        assert!(OwnerGuard::allow_self(&c, Uuid::new_v4()).is_err());
    }

    #[test]
    fn owner_guard_allow_self_or_permission_owner_wins() {
        let c = ctx(&[]);
        assert!(OwnerGuard::allow_self_or_permission(&c, c.user_id, "user.manage").is_ok());
    }

    #[test]
    fn owner_guard_allow_self_or_permission_perm_wins() {
        let c = ctx(&["user.manage"]);
        assert!(OwnerGuard::allow_self_or_permission(&c, Uuid::new_v4(), "user.manage").is_ok());
    }

    #[test]
    fn owner_guard_allow_self_or_permission_neither_fails() {
        let c = ctx(&["product.read"]);
        assert!(OwnerGuard::allow_self_or_permission(&c, Uuid::new_v4(), "user.manage").is_err());
    }

    #[test]
    fn require_any_permission_first_match_ok() {
        let c = ctx(&["talent.read"]);
        assert!(require_any_permission(&c, &["talent.manage", "talent.read"]).is_ok());
    }

    #[test]
    fn require_any_permission_none_fails() {
        let c = ctx(&["product.read"]);
        assert!(require_any_permission(&c, &["talent.manage", "talent.read"]).is_err());
    }

    #[test]
    fn require_permission_fn_delegates_to_check() {
        let c = ctx(&["x.y"]);
        assert!(require_permission(&c, "x.y").is_ok());
        assert!(require_permission(&c, "x.z").is_err());
    }
}
