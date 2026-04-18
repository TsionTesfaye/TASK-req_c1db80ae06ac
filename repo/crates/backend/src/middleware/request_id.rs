//! X-Request-Id in/out middleware.
//!
//! Accepts an inbound `X-Request-Id` header (trimmed to 128 chars) or
//! generates a fresh UUID when absent. The id is stored in request
//! extensions so handlers and the error envelope can surface it, and is
//! echoed as a response header so operators can correlate across tiers.

use std::{
    future::{ready, Ready},
    rc::Rc,
};

use actix_web::{
    body::EitherBody,
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use uuid::Uuid;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

pub struct RequestIdMw;

impl<S, B> Transform<S, ServiceRequest> for RequestIdMw
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestIdSvc<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestIdSvc {
            inner: Rc::new(service),
        }))
    }
}

pub struct RequestIdSvc<S> {
    inner: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for RequestIdSvc<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(inner);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let svc = self.inner.clone();
        Box::pin(async move {
            let id = req
                .headers()
                .get(REQUEST_ID_HEADER)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.chars().take(128).collect::<String>())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            req.extensions_mut().insert(RequestId(id.clone()));
            let mut res = svc.call(req).await?;
            if let Ok(hv) = HeaderValue::from_str(&id) {
                res.headers_mut()
                    .insert(HeaderName::from_static(REQUEST_ID_HEADER), hv);
            }
            Ok(res.map_into_left_body())
        })
    }
}
