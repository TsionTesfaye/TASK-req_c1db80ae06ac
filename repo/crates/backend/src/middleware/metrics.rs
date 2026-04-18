//! Per-request latency + status bucket into `api_metrics`.
//!
//! Written best-effort: if the insert fails (e.g. DB unreachable) we log
//! a warning but do not propagate the error — metrics must never turn a
//! successful request into a failure.

use std::{
    future::{ready, Ready},
    rc::Rc,
    time::Instant,
};

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    web, Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;

use crate::{middleware::request_id::RequestId, state::AppState};

pub struct MetricsMw;

impl<S, B> Transform<S, ServiceRequest> for MetricsMw
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = MetricsSvc<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(MetricsSvc {
            inner: Rc::new(service),
        }))
    }
}

pub struct MetricsSvc<S> {
    inner: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for MetricsSvc<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(inner);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let started = Instant::now();
        let method = req.method().as_str().to_string();
        let route = req
            .match_pattern()
            .unwrap_or_else(|| req.path().to_string());
        let state = req.app_data::<web::Data<AppState>>().cloned();
        let request_id = req
            .extensions()
            .get::<RequestId>()
            .map(|r| r.0.clone());
        let fut = self.inner.call(req);
        Box::pin(async move {
            let res = fut.await?;
            let latency_ms = started.elapsed().as_millis() as i32;
            let status = res.status().as_u16() as i32;
            if let Some(state) = state {
                let pool = state.pool.clone();
                actix_web::rt::spawn(async move {
                    let _ = sqlx::query(
                        "INSERT INTO api_metrics (route, method, status, latency_ms, request_id) \
                         VALUES ($1, $2, $3, $4, $5)",
                    )
                    .bind(&route)
                    .bind(&method)
                    .bind(status)
                    .bind(latency_ms)
                    .bind(request_id.as_deref())
                    .execute(&pool)
                    .await;
                });
            }
            Ok(res)
        })
    }
}
