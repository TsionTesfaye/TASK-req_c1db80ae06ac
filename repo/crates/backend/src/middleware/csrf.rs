//! CSRF guard for state-changing requests (design.md §CSRF, api-spec.md §auth).
//!
//! Contract:
//!
//! * Every mutation request (`POST`, `PUT`, `PATCH`, `DELETE`) against
//!   `/api/v1/*` MUST carry the header `X-Requested-With: terraops`.
//! * Requests without that header (or with a different value) are
//!   rejected with `403 FORBIDDEN` and the normalized
//!   `ErrorCode::AuthForbidden` envelope.
//! * `GET` / `HEAD` / `OPTIONS` are not gated — they are considered
//!   side-effect-free and do not carry the CSRF burden.
//!
//! Rationale: the SPA's bearer access token is held in memory, but a
//! browser attacker attempting a cross-site form POST cannot inject a
//! custom header without triggering a CORS preflight, and the SPA's
//! own origin is the only one allowed to serve the header. This is
//! the "custom request header" CSRF control referenced in
//! `docs/design.md` §Security.
//!
//! This middleware sits AFTER `AuthnMw` in the wrap() stack — we want
//! unauthenticated mutations to still return `401 UNAUTHORIZED` first
//! (the clearer, more specific signal), and only enforce CSRF on
//! requests that already have a session.

use std::{
    future::{ready, Ready},
    rc::Rc,
};

use actix_web::{
    body::{BoxBody, EitherBody},
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::Method,
    Error,
};
use futures_util::future::LocalBoxFuture;

use crate::errors::AppError;

/// Required header name + value on every mutation request.
pub const CSRF_HEADER: &str = "x-requested-with";
pub const CSRF_EXPECTED_VALUE: &str = "terraops";

pub struct CsrfMw;

impl<S, B> Transform<S, ServiceRequest> for CsrfMw
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type InitError = ();
    type Transform = CsrfSvc<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CsrfSvc {
            inner: Rc::new(service),
        }))
    }
}

pub struct CsrfSvc<S> {
    inner: Rc<S>,
}

fn is_mutation(method: &Method) -> bool {
    matches!(
        method,
        &Method::POST | &Method::PUT | &Method::PATCH | &Method::DELETE
    )
}

impl<S, B> Service<ServiceRequest> for CsrfSvc<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(inner);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.inner.clone();
        let method = req.method().clone();
        Box::pin(async move {
            if is_mutation(&method) {
                let ok = req
                    .headers()
                    .get(CSRF_HEADER)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s == CSRF_EXPECTED_VALUE)
                    .unwrap_or(false);
                if !ok {
                    return Err(actix_web::Error::from(AppError::Forbidden(
                        "csrf: missing or invalid X-Requested-With header on mutation",
                    )));
                }
            }
            let res = svc.call(req).await?;
            Ok(res.map_into_left_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_methods_classified() {
        assert!(is_mutation(&Method::POST));
        assert!(is_mutation(&Method::PUT));
        assert!(is_mutation(&Method::PATCH));
        assert!(is_mutation(&Method::DELETE));
        assert!(!is_mutation(&Method::GET));
        assert!(!is_mutation(&Method::HEAD));
        assert!(!is_mutation(&Method::OPTIONS));
    }

    #[test]
    fn constants_exact() {
        assert_eq!(CSRF_HEADER, "x-requested-with");
        assert_eq!(CSRF_EXPECTED_VALUE, "terraops");
    }
}
