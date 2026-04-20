//! 3-second hard request budget (design §Budget rules).
//!
//! Any handler that takes longer than 3s is cancelled and a normalized
//! `504 TIMEOUT` envelope is returned. The middleware is registered via
//! `App::new().wrap(BudgetMw)` and therefore applies to **every** route
//! mounted on the app — including the unauthenticated system probes
//! `/api/v1/health` and `/api/v1/ready`. Those probes always return well
//! under 3s in practice, so the budget is effectively a ceiling rather
//! than an exemption; there is no explicit health/ready allow-list here.
//! Audit L1: this comment previously claimed the probes were "outside
//! the budget on purpose" — they are not; the code has no exemption
//! logic, and the probes simply complete fast enough that the ceiling
//! is never tripped.

use std::{
    future::{ready, Ready},
    rc::Rc,
    time::Duration,
};

use actix_web::{
    body::{BoxBody, EitherBody},
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use futures_util::future::LocalBoxFuture;

use crate::errors::AppError;

pub const BUDGET_MS: u64 = 3_000;

pub struct BudgetMw;

impl<S, B> Transform<S, ServiceRequest> for BudgetMw
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
    type Error = Error;
    type InitError = ();
    type Transform = BudgetSvc<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(BudgetSvc {
            inner: Rc::new(service),
        }))
    }
}

pub struct BudgetSvc<S> {
    inner: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for BudgetSvc<S>
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
        Box::pin(async move {
            // NOTE: do not eagerly clone `req.request()` before forwarding —
            // that bumps the inner `Rc<HttpRequestInner>` strong_count and
            // later panics when the router calls `Rc::get_mut` to populate
            // match_info. Instead, let the actix framework materialize the
            // response from the returned `AppError` on the timeout path.
            let fut = svc.call(req);
            match tokio::time::timeout(Duration::from_millis(BUDGET_MS), fut).await {
                Ok(Ok(res)) => Ok(res.map_into_left_body()),
                Ok(Err(e)) => Err(e),
                Err(_) => Err(actix_web::Error::from(AppError::Timeout)),
            }
        })
    }
}
