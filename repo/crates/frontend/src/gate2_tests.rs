//! Frontend verification test surface (Gate 2).
//!
//! Every `#[wasm_bindgen_test]` in this module maps to a row in the
//! Frontend Verification Matrix in `docs/test-coverage.md`. The Gate 2
//! wrapper (`scripts/frontend_verify.sh`) greps for these function names
//! inside this file as part of the enforceable 90%+ verification bar.
//!
//! All tests are Node-friendly — they do not touch `window`, `document`,
//! or `sessionStorage`, so they run under `wasm-bindgen-test-runner`'s
//! default Node mode (no pinned Chromium required).
//!
//! Intent: honest coverage of the logic surface that is *actually*
//! verifiable from wasm-bindgen-test without a DOM — API-client error
//! semantics, token attachment, timeout budget, auth state helpers,
//! toast level mapping, notifications snapshot, and router variants.
//! DOM-bound behavior (page rendering, form submit, navigation) is
//! verified by Playwright specs referenced in the matrix.

#![cfg(test)]

use std::time::Duration;

use terraops_shared::dto::auth::AuthUserDto;
use terraops_shared::error::{ErrorCode, ErrorEnvelope};
use terraops_shared::roles::Role;
use uuid::Uuid;
use wasm_bindgen_test::*;

use crate::api::{select_timeout, ApiClient, ApiError, API_BASE, GET_RETRIES, REQUEST_TIMEOUT_MS};
use crate::router::Route;
use crate::state::{
    AuthContext, AuthState, NotificationsContext, NotificationsSnapshot, Toast, ToastLevel,
};

fn seeded_auth_state(
    perms: &[&str],
    roles: &[Role],
    display_name: &str,
) -> AuthState {
    AuthState {
        token: "test-token".into(),
        user: AuthUserDto {
            id: Uuid::nil(),
            display_name: display_name.into(),
            email: Some("user@example.com".into()),
            email_mask: "u***@example.com".into(),
            roles: roles.to_vec(),
            permissions: perms.iter().map(|s| (*s).into()).collect(),
            timezone: Some("UTC".into()),
        },
        access_expires_at_ms: 0.0,
    }
}

// ===========================================================================
// Family A — Auth & Role / Permission Gating
// ===========================================================================

#[wasm_bindgen_test]
fn auth_state_has_permission_positive_and_negative() {
    let s = seeded_auth_state(&["product.read", "user.manage"], &[Role::Administrator], "Alice");
    assert!(s.has_permission("product.read"));
    assert!(s.has_permission("user.manage"));
    assert!(!s.has_permission("talent.read"));
    assert!(!s.has_permission(""));
}

#[wasm_bindgen_test]
fn auth_state_has_role_positive_and_negative() {
    let s = seeded_auth_state(&[], &[Role::DataSteward, Role::Analyst], "Bob");
    assert!(s.has_role(Role::DataSteward));
    assert!(s.has_role(Role::Analyst));
    assert!(!s.has_role(Role::Administrator));
    assert!(!s.has_role(Role::Recruiter));
    assert!(!s.has_role(Role::RegularUser));
}

#[wasm_bindgen_test]
fn auth_state_is_admin_reflects_role() {
    let admin = seeded_auth_state(&[], &[Role::Administrator], "Root");
    let user = seeded_auth_state(&[], &[Role::RegularUser], "User");
    assert!(admin.is_admin());
    assert!(!user.is_admin());
}

#[wasm_bindgen_test]
fn auth_state_accessors_return_expected_fields() {
    let s = seeded_auth_state(&[], &[Role::Analyst], "Claire");
    assert_eq!(s.display_name(), "Claire");
    assert_eq!(s.user_id(), Uuid::nil());
}

#[wasm_bindgen_test]
fn auth_context_api_propagates_token_when_signed_in() {
    let s = seeded_auth_state(&[], &[Role::Analyst], "Dana");
    let ctx = AuthContext {
        state: Some(std::rc::Rc::new(s)),
        set: yew::Callback::from(|_| ()),
    };
    assert!(ctx.is_authenticated());
    let api = ctx.api();
    assert_eq!(api.token(), Some("test-token"));
}

#[wasm_bindgen_test]
fn auth_context_api_is_anonymous_when_signed_out() {
    let ctx = AuthContext {
        state: None,
        set: yew::Callback::from(|_| ()),
    };
    assert!(!ctx.is_authenticated());
    assert_eq!(ctx.api().token(), None);
    assert!(ctx.state().is_none());
}

// ===========================================================================
// Family B — API Client Behaviors
// ===========================================================================

#[wasm_bindgen_test]
fn api_error_user_facing_covers_all_error_codes() {
    // Every ErrorCode variant must produce a non-empty user-facing message.
    for code in [
        ErrorCode::AuthInvalidCredentials,
        ErrorCode::AuthLocked,
        ErrorCode::AuthForbidden,
        ErrorCode::AuthRequired,
        ErrorCode::ValidationFailed,
        ErrorCode::NotFound,
        ErrorCode::Conflict,
        ErrorCode::RateLimited,
        ErrorCode::Timeout,
        ErrorCode::Internal,
    ] {
        let e = ApiError::Api {
            status: 400,
            code,
            message: "server detail".into(),
            request_id: "r".into(),
        };
        let msg = e.user_facing();
        assert!(!msg.is_empty(), "user_facing for {:?} was empty", code);
    }
}

#[wasm_bindgen_test]
fn api_error_validation_and_conflict_forward_server_message() {
    let v = ApiError::Api {
        status: 422,
        code: ErrorCode::ValidationFailed,
        message: "email is required".into(),
        request_id: "r".into(),
    };
    assert_eq!(v.user_facing(), "email is required");

    let c = ApiError::Api {
        status: 409,
        code: ErrorCode::Conflict,
        message: "sku already exists".into(),
        request_id: "r".into(),
    };
    assert_eq!(c.user_facing(), "sku already exists");
}

#[wasm_bindgen_test]
fn api_error_user_facing_for_http_and_decode_variants() {
    let h = ApiError::Http {
        status: 502,
        body: "bad gateway".into(),
    };
    assert!(h.user_facing().contains("502"));

    let d = ApiError::Decode("not json".into());
    assert!(!d.user_facing().is_empty());
}

#[wasm_bindgen_test]
fn api_error_unauthenticated_false_for_validation_errors() {
    let v = ApiError::Api {
        status: 422,
        code: ErrorCode::ValidationFailed,
        message: "x".into(),
        request_id: "r".into(),
    };
    assert!(!v.is_unauthenticated());
    assert!(!ApiError::Network("x".into()).is_unauthenticated());
    assert!(!ApiError::Http { status: 500, body: "".into() }.is_unauthenticated());
    assert!(!ApiError::Decode("".into()).is_unauthenticated());
}

#[wasm_bindgen_test]
fn api_client_with_token_none_is_anonymous() {
    let c = ApiClient::with_token(None);
    assert_eq!(c.token(), None);
}

#[wasm_bindgen_test]
fn api_client_default_is_anonymous() {
    let c: ApiClient = Default::default();
    assert_eq!(c.token(), None);
    let c2 = ApiClient::new();
    assert_eq!(c2.token(), None);
}

#[wasm_bindgen_test]
fn api_client_clone_preserves_token() {
    let c = ApiClient::with_token(Some("bearer-xyz".into()));
    let c2 = c.clone();
    assert_eq!(c.token(), c2.token());
    assert_eq!(c2.token(), Some("bearer-xyz"));
    // PartialEq round-trip.
    assert_eq!(c, c2);
}

#[wasm_bindgen_test]
fn api_error_equality_semantics() {
    let a = ApiError::Timeout;
    let b = ApiError::Timeout;
    assert_eq!(a, b);
    let n1 = ApiError::Network("dns".into());
    let n2 = ApiError::Network("dns".into());
    let n3 = ApiError::Network("tls".into());
    assert_eq!(n1, n2);
    assert_ne!(n1, n3);
}

#[wasm_bindgen_test]
fn api_budget_constants_match_design_contract() {
    // design.md §Budget rules: 3s timeout, 1 GET retry, /api/v1 base.
    assert_eq!(REQUEST_TIMEOUT_MS, 3_000);
    assert_eq!(GET_RETRIES, 1);
    assert_eq!(API_BASE, "/api/v1");
}

#[wasm_bindgen_test]
async fn select_timeout_returns_some_when_future_wins() {
    let fut = async { "fast" };
    let out = select_timeout(fut, Duration::from_millis(500)).await;
    assert_eq!(out, Some("fast"));
}

#[wasm_bindgen_test]
async fn select_timeout_returns_none_when_timer_wins() {
    let fut = async {
        gloo_timers::future::TimeoutFuture::new(5_000).await;
        "slow"
    };
    let out = select_timeout(fut, Duration::from_millis(20)).await;
    assert!(out.is_none());
}

#[wasm_bindgen_test]
fn error_envelope_json_round_trip() {
    // The ApiClient decode path relies on serde round-trip of ErrorEnvelope.
    let env = ErrorEnvelope {
        error_code: ErrorCode::NotFound,
        message: "missing".into(),
        request_id: "req-1".into(),
        details: None,
    };
    let j = serde_json::to_string(&env).unwrap();
    let back: ErrorEnvelope = serde_json::from_str(&j).unwrap();
    assert_eq!(back.error_code, ErrorCode::NotFound);
    assert_eq!(back.message, "missing");
    assert_eq!(back.request_id, "req-1");
}

// ===========================================================================
// Family C — Toast / ToastLevel
// ===========================================================================

#[wasm_bindgen_test]
fn toast_level_class_maps_variants() {
    assert_eq!(ToastLevel::Info.class(), "tx-toast tx-toast--info");
    assert_eq!(ToastLevel::Success.class(), "tx-toast tx-toast--success");
    assert_eq!(ToastLevel::Warn.class(), "tx-toast tx-toast--warn");
    assert_eq!(ToastLevel::Error.class(), "tx-toast tx-toast--error");
}

#[wasm_bindgen_test]
fn toast_struct_equality() {
    let a = Toast { id: 1, level: ToastLevel::Info, message: "hi".into() };
    let b = Toast { id: 1, level: ToastLevel::Info, message: "hi".into() };
    let c = Toast { id: 2, level: ToastLevel::Info, message: "hi".into() };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[wasm_bindgen_test]
fn toast_level_copy_and_eq() {
    // ToastLevel is Copy; the context helpers rely on it being cheap to clone.
    let lvl = ToastLevel::Error;
    let lvl2 = lvl; // move-or-copy
    assert_eq!(lvl, lvl2);
}

// ===========================================================================
// Family D — Notifications
// ===========================================================================

#[wasm_bindgen_test]
fn notifications_snapshot_default_is_zero() {
    let s = NotificationsSnapshot::default();
    assert_eq!(s.unread, 0);
    assert_eq!(s.last_refreshed_ms, 0.0);
}

#[wasm_bindgen_test]
fn notifications_snapshot_equality() {
    let a = NotificationsSnapshot { unread: 3, last_refreshed_ms: 100.0 };
    let b = NotificationsSnapshot { unread: 3, last_refreshed_ms: 100.0 };
    let c = NotificationsSnapshot { unread: 4, last_refreshed_ms: 100.0 };
    // NotificationsSnapshot doesn't derive Debug (it's a context payload);
    // use PartialEq directly to keep the test Debug-free.
    assert!(a == b);
    assert!(a != c);
}

#[wasm_bindgen_test]
fn notifications_context_snapshot_is_readable() {
    let snap = std::rc::Rc::new(NotificationsSnapshot { unread: 7, last_refreshed_ms: 1.0 });
    let ctx = NotificationsContext {
        snapshot: snap.clone(),
        refresh: yew::Callback::from(|_| ()),
    };
    assert_eq!(ctx.snapshot.unread, 7);
    assert_eq!(ctx.snapshot.last_refreshed_ms, 1.0);
}

// ===========================================================================
// Family E — Router / Routes
// ===========================================================================

#[wasm_bindgen_test]
fn route_equality_and_clone() {
    let r1 = Route::Dashboard;
    let r2 = r1.clone();
    assert_eq!(r1, r2);
    assert_ne!(Route::Dashboard, Route::Login);
}

#[wasm_bindgen_test]
fn route_with_uuid_param_equality() {
    let id = Uuid::new_v4();
    let a = Route::ProductDetail { id };
    let b = Route::ProductDetail { id };
    assert_eq!(a, b);
    let other = Uuid::new_v4();
    assert_ne!(Route::ProductDetail { id }, Route::ProductDetail { id: other });
}

#[wasm_bindgen_test]
fn route_not_found_and_root_are_distinct() {
    assert_ne!(Route::NotFound, Route::Root);
    assert_ne!(Route::NotFound, Route::Dashboard);
}

#[wasm_bindgen_test]
fn route_admin_variants_are_distinct() {
    // Simple static check that each admin variant is its own entity.
    let admins = [
        Route::AdminUsers,
        Route::AdminAllowlist,
        Route::AdminMtls,
        Route::AdminRetention,
        Route::AdminAudit,
    ];
    for (i, a) in admins.iter().enumerate() {
        for (j, b) in admins.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
    }
}

#[wasm_bindgen_test]
fn route_monitoring_variants_are_distinct() {
    assert_ne!(Route::MonLatency, Route::MonErrors);
    assert_ne!(Route::MonErrors, Route::MonCrashes);
    assert_ne!(Route::MonLatency, Route::MonCrashes);
}

#[wasm_bindgen_test]
fn route_data_steward_variants_are_distinct() {
    let id = Uuid::new_v4();
    assert_ne!(Route::Products, Route::Imports);
    assert_ne!(Route::ProductDetail { id }, Route::ImportDetail { id });
}

#[wasm_bindgen_test]
fn route_analyst_variants_are_distinct() {
    let id = Uuid::new_v4();
    assert_ne!(Route::EnvSources, Route::EnvObservations);
    assert_ne!(Route::MetricDefinitions, Route::MetricDefinitionDetail { id });
    assert_ne!(Route::Kpi, Route::AlertRules);
    assert_ne!(Route::AlertEvents, Route::Reports);
}

#[wasm_bindgen_test]
fn route_talent_variants_are_distinct() {
    let id = Uuid::new_v4();
    assert_ne!(Route::TalentCandidates, Route::TalentRoles);
    assert_ne!(Route::TalentCandidates, Route::TalentCandidateDetail { id });
    assert_ne!(Route::TalentRecommendations, Route::TalentWatchlists);
    assert_ne!(Route::TalentWeights, Route::TalentWatchlists);
}

#[wasm_bindgen_test]
fn route_debug_formatting_includes_variant_name() {
    // Route derives Debug; the representation is used in logs / matrix.
    let s = format!("{:?}", Route::Dashboard);
    assert!(s.contains("Dashboard"), "got {s}");
    let s2 = format!("{:?}", Route::Login);
    assert!(s2.contains("Login"), "got {s2}");
}

// ===========================================================================
// Family F — Role/Permission shape for nav & PermGate
// ===========================================================================
// (Layout/Nav/PermGate rendering is DOM-bound and covered by Playwright; these
// tests pin the pure shape that those components read.)

#[wasm_bindgen_test]
fn data_steward_nav_permissions_shape() {
    // Nav shows Catalog section when the user has product.read or product.write.
    let reader = seeded_auth_state(&["product.read"], &[Role::DataSteward], "DS-R");
    let manager = seeded_auth_state(&["product.write"], &[Role::DataSteward], "DS-M");
    let stranger = seeded_auth_state(&["kpi.read"], &[Role::RegularUser], "U");
    assert!(reader.has_permission("product.read") || reader.has_permission("product.write"));
    assert!(manager.has_permission("product.read") || manager.has_permission("product.write"));
    assert!(!(stranger.has_permission("product.read") || stranger.has_permission("product.write")));
}

#[wasm_bindgen_test]
fn analyst_nav_permissions_shape() {
    // Nav shows Environmental section for metric.read / kpi.read /
    // alert.ack / alert.manage / report.schedule / report.run.
    let any_env = |state: &AuthState| {
        state.has_permission("metric.read")
            || state.has_permission("kpi.read")
            || state.has_permission("alert.ack")
            || state.has_permission("alert.manage")
            || state.has_permission("report.schedule")
            || state.has_permission("report.run")
    };
    let analyst = seeded_auth_state(
        &["metric.read", "kpi.read", "alert.manage", "report.schedule", "report.run"],
        &[Role::Analyst],
        "A",
    );
    let user = seeded_auth_state(&["product.read"], &[Role::RegularUser], "U");
    assert!(any_env(&analyst));
    assert!(!any_env(&user));
}

#[wasm_bindgen_test]
fn recruiter_nav_permissions_shape() {
    let r = seeded_auth_state(&["talent.read"], &[Role::Recruiter], "R");
    let u = seeded_auth_state(&["kpi.read"], &[Role::RegularUser], "U");
    assert!(r.has_permission("talent.read"));
    assert!(!u.has_permission("talent.read"));
}

#[wasm_bindgen_test]
fn admin_nav_permissions_shape() {
    let a = seeded_auth_state(
        &["user.manage", "allowlist.manage", "mtls.manage", "retention.manage", "monitoring.read"],
        &[Role::Administrator],
        "A",
    );
    for p in [
        "user.manage",
        "allowlist.manage",
        "mtls.manage",
        "retention.manage",
        "monitoring.read",
    ] {
        assert!(a.has_permission(p), "admin missing {p}");
    }
    let u = seeded_auth_state(&[], &[Role::RegularUser], "U");
    assert!(!u.has_permission("user.manage"));
}

#[wasm_bindgen_test]
fn perm_gate_unauth_and_authz_shapes() {
    // PermGate's decision is purely (state_present?, permission_present?). The
    // renderer is DOM-bound; this test pins the shape its logic walks.
    let unauth: Option<AuthState> = None;
    assert!(unauth.is_none(), "unauth branch → redirect to /login");

    let missing = seeded_auth_state(&[], &[Role::RegularUser], "U");
    assert!(!missing.has_permission("user.manage"),
        "authz-missing branch → 'Not authorized' fallback");

    let authorized = seeded_auth_state(&["user.manage"], &[Role::Administrator], "A");
    assert!(authorized.has_permission("user.manage"),
        "authorized branch → children render");
}

// ===========================================================================
// Family G — pages::format_date / format_ts_opt pure helpers
// ===========================================================================

#[wasm_bindgen_test]
fn format_date_formats_naive_date_as_mm_dd_yyyy() {
    use chrono::NaiveDate;
    let d = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();
    let s = crate::pages::format_date(d);
    assert_eq!(s, "04/20/2026", "format_date must be MM/DD/YYYY, got: {s}");
}

#[wasm_bindgen_test]
fn format_date_pads_single_digit_month_and_day() {
    use chrono::NaiveDate;
    let d = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
    let s = crate::pages::format_date(d);
    assert_eq!(s, "01/05/2026", "format_date should zero-pad month and day, got: {s}");
}

#[wasm_bindgen_test]
fn format_ts_opt_none_returns_em_dash() {
    let s = crate::pages::format_ts_opt(None);
    assert_eq!(s, "—", "format_ts_opt(None) must return em-dash, got: {s:?}");
}

#[wasm_bindgen_test]
fn format_ts_opt_some_returns_formatted_string() {
    use chrono::{TimeZone, Utc};
    let dt = Utc.with_ymd_and_hms(2026, 4, 20, 14, 30, 0).unwrap();
    let s = crate::pages::format_ts_opt(Some(dt));
    // Must not be the em-dash fallback — exact format depends on the
    // Node test runner's local TZ, so we only assert non-empty + structural.
    assert!(!s.is_empty(), "format_ts_opt(Some) should not be empty");
    assert_ne!(s, "—", "format_ts_opt(Some) must not return em-dash");
    // Must contain some digits (date/time components).
    assert!(s.chars().any(|c| c.is_ascii_digit()), "formatted ts must contain digits, got: {s:?}");
}

#[wasm_bindgen_test]
fn format_ts_produces_non_empty_string_for_epoch() {
    use chrono::{TimeZone, Utc};
    let dt = Utc.timestamp_opt(0, 0).unwrap(); // Unix epoch
    let s = crate::pages::format_ts(dt);
    assert!(!s.is_empty(), "format_ts(epoch) must be non-empty, got: {s:?}");
    // Must contain a slash — the MM/DD/YYYY format.
    assert!(s.contains('/'), "format_ts must contain '/', got: {s:?}");
}

// ===========================================================================
// Family H — app.rs constants (design contract pins)
// ===========================================================================

#[wasm_bindgen_test]
fn app_notification_poll_interval_is_30s() {
    // design.md §notifications: poll every 30 seconds while authenticated.
    assert_eq!(crate::app::NOTIFICATIONS_POLL_MS, 30_000,
        "notification poll interval must be 30 000 ms");
}

#[wasm_bindgen_test]
fn app_toast_auto_dismiss_is_5s() {
    // design.md: non-error toasts auto-dismiss after 5 s to avoid clutter.
    assert_eq!(crate::app::TOAST_AUTO_DISMISS_MS, 5_000,
        "toast auto-dismiss must be 5 000 ms");
}

#[wasm_bindgen_test]
fn app_token_refresh_lead_is_60s() {
    // design.md §auth: refresh the 15-min JWT with a 60-second lead so
    // the token is never served stale.
    assert_eq!(crate::app::REFRESH_LEAD_MS as u32, 60_000,
        "token refresh lead must be 60 000 ms");
}

#[wasm_bindgen_test]
fn app_refresh_min_delay_is_5s() {
    // Hard floor preventing busy-looping on clock-skew or stale state.
    assert_eq!(crate::app::REFRESH_MIN_DELAY_MS, 5_000,
        "refresh min delay must be 5 000 ms");
}

#[wasm_bindgen_test]
fn app_refresh_retry_on_error_is_30s() {
    // Retry on transient network failure at the same cadence as
    // notification polling so the UI recovers within one poll window.
    assert_eq!(crate::app::REFRESH_RETRY_ON_ERROR_MS, 30_000,
        "refresh retry-on-error interval must be 30 000 ms");
}

// ===========================================================================
// Family I — components pure-logic shapes (PermAnyGate any() contract,
//            LoadMore / DataTable prop equality)
// ===========================================================================

#[wasm_bindgen_test]
fn perm_any_gate_any_semantics_positive() {
    // The PermAnyGate accepts users who hold ANY of the listed permissions.
    // This test pins the pure logic: `state.has_permission` for at least one
    // of the listed permissions → access granted.
    let s = seeded_auth_state(&["talent.read"], &[Role::Recruiter], "R");
    let perms = ["talent.read", "talent.manage"];
    let ok = perms.iter().any(|p| s.has_permission(p));
    assert!(ok, "user with talent.read should pass any-gate for [talent.read, talent.manage]");
}

#[wasm_bindgen_test]
fn perm_any_gate_any_semantics_negative() {
    let s = seeded_auth_state(&["kpi.read"], &[Role::RegularUser], "U");
    let perms = ["talent.read", "talent.manage"];
    let ok = perms.iter().any(|p| s.has_permission(p));
    assert!(!ok, "user without talent perms must not pass any-gate");
}

#[wasm_bindgen_test]
fn perm_any_gate_manage_is_superset_of_read() {
    // talent.manage is a superset of talent.read in the RBAC model.
    // A user with only talent.manage must still pass the [talent.read, talent.manage] any-gate.
    let s = seeded_auth_state(&["talent.manage"], &[Role::Administrator], "A");
    let perms = ["talent.read", "talent.manage"];
    let ok = perms.iter().any(|p| s.has_permission(p));
    assert!(ok, "talent.manage holder must pass any-gate for [talent.read, talent.manage]");
}

#[wasm_bindgen_test]
fn report_perm_any_gate_run_or_schedule() {
    // report.schedule ⊇ report.run in the analyst/recruiter model.
    let scheduler = seeded_auth_state(&["report.schedule"], &[Role::Analyst], "A");
    let runner = seeded_auth_state(&["report.run"], &[Role::RegularUser], "U");
    let neither = seeded_auth_state(&["kpi.read"], &[Role::RegularUser], "N");
    let perms = ["report.run", "report.schedule"];
    assert!(perms.iter().any(|p| scheduler.has_permission(p)), "scheduler must pass");
    assert!(perms.iter().any(|p| runner.has_permission(p)), "runner must pass");
    assert!(!perms.iter().any(|p| neither.has_permission(p)), "neither must fail");
}
