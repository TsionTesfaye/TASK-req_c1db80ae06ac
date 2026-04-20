//! X-Request-Id in/out middleware.
//!
//! Accepts an inbound `X-Request-Id` header (trimmed to 128 chars) or
//! generates a fresh UUID when absent. The id is stored in request
//! extensions so handlers can surface it, and is echoed as a response
//! header so operators can correlate across tiers.
//!
//! Audit #4 Issue #6: error envelopes must carry the actual request id,
//! not the `"unknown"` placeholder emitted by `AppError::error_response`.
//! Because `actix_web::ResponseError::error_response` is called without
//! access to the `HttpRequest`, the envelope is rendered with the
//! sentinel `"unknown"` (or `"__REQUEST_ID__"`) and this middleware
//! rewrites the body after the fact when:
//!   * the response is a JSON envelope (content-type starts with
//!     `application/json`), and
//!   * the parsed envelope carries a placeholder request id.

use std::{
    future::{ready, Ready},
    rc::Rc,
};

use actix_web::{
    body::{BoxBody, EitherBody, MessageBody},
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue, CONTENT_LENGTH, CONTENT_TYPE},
    Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use uuid::Uuid;

pub const REQUEST_ID_HEADER: &str = "x-request-id";
/// Sentinel written into `ErrorEnvelope.request_id` by `AppError::error_response`;
/// replaced by the real per-request id in the middleware on the way out.
pub const REQUEST_ID_PLACEHOLDER: &str = "__REQUEST_ID__";

#[derive(Debug, Clone)]
pub struct RequestId(pub String);

pub struct RequestIdMw;

impl<S, B> Transform<S, ServiceRequest> for RequestIdMw
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
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
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<B, BoxBody>>;
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
            let res = svc.call(req).await?;

            // Echo back as response header regardless.
            let (req, mut resp) = res.into_parts();
            if let Ok(hv) = HeaderValue::from_str(&id) {
                resp.headers_mut()
                    .insert(HeaderName::from_static(REQUEST_ID_HEADER), hv);
            }

            // Body rewrite: only for JSON responses, and only when the
            // envelope carries a placeholder request_id. Anything else is
            // passed through as the left body variant.
            let is_json = resp
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.starts_with("application/json"))
                .unwrap_or(false);

            if !is_json {
                return Ok(ServiceResponse::new(req, resp.map_into_left_body()));
            }

            let (head, body) = resp.into_parts();
            let collected = actix_web::body::to_bytes(body)
                .await
                .map_err(|_| actix_web::error::ErrorInternalServerError("body read"))?;

            // Cheap substring check before paying for JSON parse.
            if !memchr_contains(&collected, REQUEST_ID_PLACEHOLDER.as_bytes())
                && !memchr_contains(&collected, b"\"request_id\":\"unknown\"")
            {
                let resp = head.set_body(collected).map_into_boxed_body();
                return Ok(ServiceResponse::new(req, resp.map_into_right_body()));
            }

            // Replace both the new placeholder and the legacy "unknown"
            // form. Do it on raw bytes so we never touch non-envelope JSON
            // that happens to carry the substring.
            let replaced = replace_placeholder(&collected, &id);
            let mut new_head = head;
            // Reset Content-Length; actix will compute a fresh one.
            new_head.headers_mut().remove(CONTENT_LENGTH);
            let resp = new_head.set_body(replaced).map_into_boxed_body();
            Ok(ServiceResponse::new(req, resp.map_into_right_body()))
        })
    }
}

fn memchr_contains(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || hay.len() < needle.len() {
        return false;
    }
    hay.windows(needle.len()).any(|w| w == needle)
}

fn replace_placeholder(body: &[u8], request_id: &str) -> Vec<u8> {
    // We replace only inside `"request_id":"..."` pairs, to avoid rewriting
    // unrelated JSON content that happens to contain the sentinel string.
    let s = match std::str::from_utf8(body) {
        Ok(s) => s,
        Err(_) => return body.to_vec(),
    };
    let replaced = s
        .replace(
            &format!("\"request_id\":\"{REQUEST_ID_PLACEHOLDER}\""),
            &format!("\"request_id\":\"{request_id}\""),
        )
        .replace(
            "\"request_id\":\"unknown\"",
            &format!("\"request_id\":\"{request_id}\""),
        );
    replaced.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_targets_only_request_id_key() {
        let body =
            br#"{"error_code":"validation_failed","message":"x","request_id":"__REQUEST_ID__","details":null}"#;
        let out = replace_placeholder(body, "abc-123");
        let s = std::str::from_utf8(&out).unwrap();
        assert!(s.contains("\"request_id\":\"abc-123\""));
        assert!(!s.contains("__REQUEST_ID__"));
    }

    #[test]
    fn replace_legacy_unknown_sentinel() {
        let body = br#"{"request_id":"unknown","message":"x"}"#;
        let out = replace_placeholder(body, "r-42");
        let s = std::str::from_utf8(&out).unwrap();
        assert!(s.contains("\"request_id\":\"r-42\""));
    }

    #[test]
    fn replace_noop_when_absent() {
        let body = br#"{"request_id":"already-set","other":"unknown"}"#;
        let out = replace_placeholder(body, "new");
        let s = std::str::from_utf8(&out).unwrap();
        assert!(s.contains("already-set"));
        assert!(s.contains("\"other\":\"unknown\""));
    }

    #[test]
    fn memchr_contains_basic() {
        assert!(memchr_contains(b"hello world", b"world"));
        assert!(!memchr_contains(b"hello", b"world"));
        assert!(!memchr_contains(b"", b"x"));
    }
}
