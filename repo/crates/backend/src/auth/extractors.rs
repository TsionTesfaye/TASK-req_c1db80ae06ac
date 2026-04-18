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
