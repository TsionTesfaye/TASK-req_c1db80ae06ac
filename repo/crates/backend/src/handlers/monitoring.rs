//! Admin monitoring endpoints M1–M4.
//!
//!   M1 GET  /api/v1/monitoring/latency
//!   M2 GET  /api/v1/monitoring/errors
//!   M3 POST /api/v1/monitoring/crash-report     (auth'd; any user)
//!   M4 GET  /api/v1/monitoring/crash-reports    (admin)

use actix_web::{web, HttpResponse, Responder};
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use terraops_shared::{
    dto::monitoring::{CrashReport, ErrorBucket, IngestCrashReport, LatencyBucket},
    pagination::{Page, PageQuery},
};
use uuid::Uuid;

use crate::{
    auth::extractors::{require_permission, AuthUser},
    errors::AppResult,
    state::AppState,
};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/monitoring")
            .route("/latency", web::get().to(latency))
            .route("/errors", web::get().to(errors))
            .route("/crash-report", web::post().to(ingest_crash))
            .route("/crash-reports", web::get().to(list_crashes)),
    );
}

async fn latency(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    require_permission(&user.0, "monitoring.read")?;
    #[derive(FromRow)]
    struct Row {
        route: String,
        method: String,
        count: i64,
        p50_ms: Option<f64>,
        p95_ms: Option<f64>,
        p99_ms: Option<f64>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT route, method, COUNT(*)::BIGINT AS count, \
                percentile_cont(0.50) WITHIN GROUP (ORDER BY latency_ms) AS p50_ms, \
                percentile_cont(0.95) WITHIN GROUP (ORDER BY latency_ms) AS p95_ms, \
                percentile_cont(0.99) WITHIN GROUP (ORDER BY latency_ms) AS p99_ms \
         FROM api_metrics WHERE at > NOW() - INTERVAL '1 hour' \
         GROUP BY route, method ORDER BY count DESC LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;
    let items: Vec<LatencyBucket> = rows
        .into_iter()
        .map(|r| LatencyBucket {
            route: r.route,
            method: r.method,
            count: r.count,
            p50_ms: r.p50_ms.unwrap_or(0.0) as i64,
            p95_ms: r.p95_ms.unwrap_or(0.0) as i64,
            p99_ms: r.p99_ms.unwrap_or(0.0) as i64,
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

async fn errors(user: AuthUser, state: web::Data<AppState>) -> AppResult<impl Responder> {
    require_permission(&user.0, "monitoring.read")?;
    #[derive(FromRow)]
    struct Row {
        route: String,
        method: String,
        total: i64,
        errors: i64,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT route, method, COUNT(*)::BIGINT AS total, \
                SUM(CASE WHEN status >= 500 THEN 1 ELSE 0 END)::BIGINT AS errors \
         FROM api_metrics WHERE at > NOW() - INTERVAL '1 hour' \
         GROUP BY route, method ORDER BY errors DESC LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await?;
    let items: Vec<ErrorBucket> = rows
        .into_iter()
        .map(|r| {
            let rate = if r.total > 0 {
                r.errors as f64 / r.total as f64
            } else {
                0.0
            };
            ErrorBucket {
                route: r.route,
                method: r.method,
                total: r.total,
                errors: r.errors,
                error_rate: rate,
            }
        })
        .collect();
    Ok(HttpResponse::Ok().json(items))
}

// ---------------------------------------------------------------------------
// Audit #8 Issue #5 — Crash ingest guards.
//
// Before this change `ingest_crash` stored arbitrary user-supplied `page`,
// `agent`, `stack`, and `payload_json` verbatim. A misbehaving client
// could flood the column with multi-megabyte text, embed secrets lifted
// from a browser session (bearer tokens, cookies, email addresses), or
// park arbitrary payload trees in the store. The contract is now:
//
//   * Field-level hard size limits (enforced server-side):
//       - page     ≤ `MAX_PAGE_LEN`     bytes   (2 KiB)
//       - agent    ≤ `MAX_AGENT_LEN`    bytes   (1 KiB)
//       - stack    ≤ `MAX_STACK_LEN`    bytes   (64 KiB, truncated, not rejected)
//       - payload  ≤ `MAX_PAYLOAD_BYTES` after serialize (128 KiB, rejected)
//   * Redaction sweep on `stack` and on every string leaf inside
//     `payload` for well-known sensitive token shapes:
//       - `Authorization: Bearer <jwt>` headers
//       - bare JWT-looking triples (three `.`-separated base64url chunks)
//       - `password=...` / `api_key=...` / `secret=...` query fragments
//       - email addresses (reduced to `<redacted-email>`)
//     The token literal is replaced with `<redacted>`. The purpose is to
//     stop this surface from silently becoming a secrets sink — real
//     server-side leak-prevention still depends on the client not
//     constructing full secrets into the stack string in the first place.
//   * Over-size inputs return a user-safe 400 instead of storing a
//     truncated record silently.
// ---------------------------------------------------------------------------
const MAX_PAGE_LEN: usize = 2 * 1024;
const MAX_AGENT_LEN: usize = 1024;
const MAX_STACK_LEN: usize = 64 * 1024;
const MAX_PAYLOAD_BYTES: usize = 128 * 1024;

fn too_long_err(field: &str, limit: usize) -> crate::errors::AppError {
    crate::errors::AppError::Validation(format!("{field} exceeds {limit} bytes"))
}

/// Replace well-known secret-shaped substrings with `<redacted>`. This is
/// a best-effort belt-and-suspenders sweep layered on top of client-side
/// discipline; it is not a substitute for not capturing secrets.
fn redact(input: &str) -> String {
    let mut out = input.to_string();
    // Authorization: Bearer <token> — case-insensitive header form.
    // Keep it simple: walk and replace.
    let lc = out.to_ascii_lowercase();
    if let Some(pos) = lc.find("authorization:") {
        // Replace from `authorization:` to end of line.
        let end = out[pos..].find('\n').map(|e| pos + e).unwrap_or(out.len());
        out.replace_range(pos..end, "Authorization: <redacted>");
    }
    // Bearer <token> anywhere.
    while let Some(pos) = out.to_ascii_lowercase().find("bearer ") {
        let after = &out[pos + 7..];
        let end_off = after
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .unwrap_or(after.len());
        out.replace_range(pos..pos + 7 + end_off, "Bearer <redacted>");
    }
    // Key-eq-value style.
    for key in ["password", "api_key", "apikey", "secret", "token"] {
        let lc_key = format!("{key}=");
        while let Some(pos) = out.to_ascii_lowercase().find(&lc_key) {
            let after_start = pos + lc_key.len();
            let end_off = out[after_start..]
                .find(|c: char| c == '&' || c == ' ' || c == '"' || c == '\'' || c == '\n')
                .unwrap_or(out.len() - after_start);
            out.replace_range(pos..after_start + end_off, &format!("{key}=<redacted>"));
        }
    }
    // JWT-looking triples — three base64url segments joined by `.`.
    let mut cleaned = String::with_capacity(out.len());
    for token in out.split_inclusive(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
        let trim = token.trim_end_matches(|c: char| c.is_whitespace() || c == '"' || c == '\'');
        let tail_len = token.len() - trim.len();
        let parts: Vec<&str> = trim.split('.').collect();
        let looks_like_jwt = parts.len() == 3
            && parts.iter().all(|p| {
                p.len() >= 10
                    && p.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            });
        if looks_like_jwt {
            cleaned.push_str("<redacted-jwt>");
            cleaned.push_str(&token[token.len() - tail_len..]);
        } else {
            cleaned.push_str(token);
        }
    }
    out = cleaned;
    // Email addresses — collapse user@host tokens.
    let mut email_clean = String::with_capacity(out.len());
    let bytes: Vec<char> = out.chars().collect();
    let is_local = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '%' | '+' | '-');
    let is_dom = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '-');
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '@' && i > 0 && is_local(bytes[i - 1]) {
            // Walk back.
            let mut start = i;
            while start > 0 && is_local(bytes[start - 1]) {
                start -= 1;
            }
            // Walk forward (domain).
            let mut end = i + 1;
            while end < bytes.len() && is_dom(bytes[end]) {
                end += 1;
            }
            if end > i + 1 && bytes[i + 1..end].contains(&'.') {
                // Drop anything already copied for this local part.
                let copied_local = i - start;
                for _ in 0..copied_local {
                    email_clean.pop();
                }
                email_clean.push_str("<redacted-email>");
                i = end;
                continue;
            }
        }
        email_clean.push(bytes[i]);
        i += 1;
    }
    email_clean
}

/// Apply `redact` to every string leaf inside a JSON value.
fn redact_json(v: &mut serde_json::Value) {
    use serde_json::Value;
    match v {
        Value::String(s) => *s = redact(s),
        Value::Array(a) => a.iter_mut().for_each(redact_json),
        Value::Object(m) => m.values_mut().for_each(redact_json),
        _ => {}
    }
}

async fn ingest_crash(
    user: AuthUser,
    state: web::Data<AppState>,
    body: web::Json<IngestCrashReport>,
) -> AppResult<impl Responder> {
    let req = body.into_inner();

    if let Some(ref p) = req.page {
        if p.len() > MAX_PAGE_LEN {
            return Err(too_long_err("page", MAX_PAGE_LEN));
        }
    }
    if let Some(ref a) = req.agent {
        if a.len() > MAX_AGENT_LEN {
            return Err(too_long_err("agent", MAX_AGENT_LEN));
        }
    }
    // Stack is truncated (not rejected) so partial captures still land —
    // but we document the cap and redact the retained portion.
    let stack_trimmed: Option<String> = req.stack.as_deref().map(|s| {
        let truncated = if s.len() > MAX_STACK_LEN {
            &s[..MAX_STACK_LEN]
        } else {
            s
        };
        redact(truncated)
    });
    let page_clean = req.page.as_deref().map(redact);
    let agent_clean = req.agent.as_deref().map(redact);

    let mut payload = req.payload.unwrap_or_else(|| serde_json::json!({}));
    let payload_bytes = serde_json::to_vec(&payload)
        .map(|v| v.len())
        .unwrap_or(0);
    if payload_bytes > MAX_PAYLOAD_BYTES {
        return Err(too_long_err("payload", MAX_PAYLOAD_BYTES));
    }
    redact_json(&mut payload);

    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO client_crash_reports (user_id, page, agent, stack, payload_json) \
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(user.0.user_id)
    .bind(page_clean.as_deref())
    .bind(agent_clean.as_deref())
    .bind(stack_trimmed.as_deref())
    .bind(payload)
    .fetch_one(&state.pool)
    .await?;
    Ok(HttpResponse::Created().json(serde_json::json!({"id": row.0})))
}

#[cfg(test)]
mod redact_tests {
    use super::*;

    #[test]
    fn redacts_bearer_and_auth_header() {
        let r = redact("Authorization: Bearer abc.def.ghi\nnext line");
        assert!(r.contains("Authorization: <redacted>"));
    }

    #[test]
    fn redacts_standalone_bearer_token() {
        let r = redact("prefix Bearer abc123xyz suffix");
        assert!(r.contains("Bearer <redacted>"));
        assert!(!r.contains("abc123xyz"));
    }

    #[test]
    fn redacts_key_equals_value() {
        let r = redact("foo password=hunter2&bar=baz");
        assert!(r.contains("password=<redacted>"));
    }

    #[test]
    fn redacts_jwt_triple() {
        let tok = "aaaaaaaaaa.bbbbbbbbbb.cccccccccc";
        let r = redact(&format!("stack with token {tok} end"));
        assert!(r.contains("<redacted-jwt>"));
        assert!(!r.contains(tok));
    }

    #[test]
    fn redacts_email() {
        let r = redact("contact ops@example.com now");
        assert!(r.contains("<redacted-email>"));
        assert!(!r.contains("ops@example.com"));
    }

    #[test]
    fn redacts_json_leaves() {
        let mut v = serde_json::json!({"err": "Bearer abc123xyz", "nested": {"user": "a@b.co"}});
        redact_json(&mut v);
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("Bearer <redacted>"));
        assert!(s.contains("<redacted-email>"));
    }
}

async fn list_crashes(
    user: AuthUser,
    state: web::Data<AppState>,
    q: web::Query<PageQuery>,
) -> AppResult<impl Responder> {
    require_permission(&user.0, "monitoring.read")?;
    let r = q.into_inner().resolved();
    #[derive(FromRow)]
    struct Row {
        id: Uuid,
        user_id: Option<Uuid>,
        page: Option<String>,
        agent: Option<String>,
        stack: Option<String>,
        payload_json: serde_json::Value,
        reported_at: DateTime<Utc>,
    }
    let rows: Vec<Row> = sqlx::query_as::<_, Row>(
        "SELECT id, user_id, page, agent, stack, payload_json, reported_at \
         FROM client_crash_reports ORDER BY reported_at DESC LIMIT $1 OFFSET $2",
    )
    .bind(r.limit() as i64)
    .bind(r.offset() as i64)
    .fetch_all(&state.pool)
    .await?;
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*)::BIGINT FROM client_crash_reports")
        .fetch_one(&state.pool)
        .await?;
    let items: Vec<CrashReport> = rows
        .into_iter()
        .map(|r| CrashReport {
            id: r.id,
            user_id: r.user_id,
            page: r.page,
            agent: r.agent,
            stack: r.stack,
            payload: r.payload_json,
            reported_at: r.reported_at,
        })
        .collect();
    let page = Page {
        items,
        page: r.page,
        page_size: r.page_size,
        total: total.0 as u64,
    };
    Ok(HttpResponse::Ok()
        .insert_header(("X-Total-Count", total.0.to_string()))
        .json(page))
}
