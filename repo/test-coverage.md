# TerraOps Test Coverage

## Why the frontend is not measured by wasm source-based line coverage

`cargo llvm-cov` instruments native-target Rust code by injecting
`profiler_builtins` coverage counters at compile time.  The
`wasm32-unknown-unknown` sysroot shipped with stable Rust (including
rust 1.88) does **not** include `profiler_builtins`; the symbol
`__llvm_profile_runtime` is absent from `rustc --print sysroot
--target wasm32-unknown-unknown`.

Observed evidence (rust 1.88.0-x86_64-unknown-linux-gnu):

```
$ cargo llvm-cov --target wasm32-unknown-unknown -p terraops-frontend
error[E0463]: can't find crate for `profiler_builtins`
  --> <profiling instrumentation>
note: the `wasm32-unknown-unknown` target may not support the `profiler_builtins` crate
```

Because source-based WASM line coverage is not achievable on this
toolchain without an unstable or third-party fork, Gate 2 instead uses
the **Frontend Verification Matrix** (FVM) below.  The FVM score
`covered_rows / total_rows` is enforced at `≥ 90 %` by
`scripts/frontend_verify.sh`.  Each "covered" row's evidence item
(wasm-bindgen test function, Playwright spec file, or router variant)
is grep-verified to exist in the codebase — no dishonest greens.

---

## Frontend Verification Matrix

Evidence kinds:
- `wasm_bindgen_test <fn>` — `fn <fn>(` with `#[wasm_bindgen_test]`
  within 4 lines above, in `crates/frontend/src/**/*.rs`
- `playwright <file>` — `e2e/specs/<file>` must exist and be non-empty
- `route <Variant>` — `Route::<Variant>` in
  `crates/frontend/src/router.rs`

| ID      | Family     | Requirement                                    | Evidence Kind     | Evidence                                              | Status  | Notes |
|---------|------------|------------------------------------------------|-------------------|-------------------------------------------------------|---------|-------|
| FVM-001 | Auth       | has_permission positive and negative           | wasm_bindgen_test | auth_state_has_permission_positive_and_negative       | covered | —     |
| FVM-002 | Auth       | has_role positive and negative                 | wasm_bindgen_test | auth_state_has_role_positive_and_negative             | covered | —     |
| FVM-003 | Auth       | is_admin reflects Role::Administrator          | wasm_bindgen_test | auth_state_is_admin_reflects_role                     | covered | —     |
| FVM-004 | Auth       | display_name and user_id accessors             | wasm_bindgen_test | auth_state_accessors_return_expected_fields           | covered | —     |
| FVM-005 | Auth       | AuthContext propagates token when signed in    | wasm_bindgen_test | auth_context_api_propagates_token_when_signed_in      | covered | —     |
| FVM-006 | Auth       | AuthContext anonymous when signed out          | wasm_bindgen_test | auth_context_api_is_anonymous_when_signed_out         | covered | —     |
| FVM-007 | ApiClient  | user_facing message for every ErrorCode        | wasm_bindgen_test | api_error_user_facing_covers_all_error_codes          | covered | —     |
| FVM-008 | ApiClient  | validation and conflict forward server message | wasm_bindgen_test | api_error_validation_and_conflict_forward_server_message | covered | —  |
| FVM-009 | ApiClient  | Http and Decode variant messages               | wasm_bindgen_test | api_error_user_facing_for_http_and_decode_variants    | covered | —     |
| FVM-010 | ApiClient  | is_unauthenticated false for non-auth errors   | wasm_bindgen_test | api_error_unauthenticated_false_for_validation_errors | covered | —     |
| FVM-011 | ApiClient  | with_token(None) is anonymous                  | wasm_bindgen_test | api_client_with_token_none_is_anonymous               | covered | —     |
| FVM-012 | ApiClient  | Default / new() is anonymous                   | wasm_bindgen_test | api_client_default_is_anonymous                       | covered | —     |
| FVM-013 | ApiClient  | clone preserves token                          | wasm_bindgen_test | api_client_clone_preserves_token                      | covered | —     |
| FVM-014 | ApiClient  | ApiError equality semantics                    | wasm_bindgen_test | api_error_equality_semantics                          | covered | —     |
| FVM-015 | ApiClient  | budget constants match design contract         | wasm_bindgen_test | api_budget_constants_match_design_contract            | covered | —     |
| FVM-016 | ApiClient  | select_timeout resolves when future wins       | wasm_bindgen_test | select_timeout_returns_some_when_future_wins          | covered | —     |
| FVM-017 | ApiClient  | select_timeout resolves None when timer wins   | wasm_bindgen_test | select_timeout_returns_none_when_timer_wins           | covered | —     |
| FVM-018 | ApiClient  | ErrorEnvelope serde round-trip                 | wasm_bindgen_test | error_envelope_json_round_trip                        | covered | —     |
| FVM-019 | Toast      | ToastLevel class string for each variant       | wasm_bindgen_test | toast_level_class_maps_variants                       | covered | —     |
| FVM-020 | Toast      | Toast struct PartialEq                         | wasm_bindgen_test | toast_struct_equality                                 | covered | —     |
| FVM-021 | Toast      | ToastLevel is Copy                             | wasm_bindgen_test | toast_level_copy_and_eq                               | covered | —     |
| FVM-022 | Notif      | NotificationsSnapshot default is zero          | wasm_bindgen_test | notifications_snapshot_default_is_zero                | covered | —     |
| FVM-023 | Notif      | NotificationsSnapshot PartialEq                | wasm_bindgen_test | notifications_snapshot_equality                       | covered | —     |
| FVM-024 | Notif      | NotificationsContext snapshot accessible       | wasm_bindgen_test | notifications_context_snapshot_is_readable            | covered | —     |
| FVM-025 | Router     | Route equality and clone                       | wasm_bindgen_test | route_equality_and_clone                              | covered | —     |
| FVM-026 | Router     | Route with UUID param equality                 | wasm_bindgen_test | route_with_uuid_param_equality                        | covered | —     |
| FVM-027 | Router     | NotFound and Root are distinct                 | wasm_bindgen_test | route_not_found_and_root_are_distinct                 | covered | —     |
| FVM-028 | Router     | Admin route variants are distinct              | wasm_bindgen_test | route_admin_variants_are_distinct                     | covered | —     |
| FVM-029 | Router     | Monitoring variants are distinct               | wasm_bindgen_test | route_monitoring_variants_are_distinct                | covered | —     |
| FVM-030 | Router     | DataSteward variants are distinct              | wasm_bindgen_test | route_data_steward_variants_are_distinct              | covered | —     |
| FVM-031 | Router     | Analyst variants are distinct                  | wasm_bindgen_test | route_analyst_variants_are_distinct                   | covered | —     |
| FVM-032 | Router     | Talent variants are distinct                   | wasm_bindgen_test | route_talent_variants_are_distinct                    | covered | —     |
| FVM-033 | Router     | Route Debug includes variant name              | wasm_bindgen_test | route_debug_formatting_includes_variant_name          | covered | —     |
| FVM-034 | NavPerms   | DataSteward nav permission shape               | wasm_bindgen_test | data_steward_nav_permissions_shape                    | covered | —     |
| FVM-035 | NavPerms   | Analyst nav permission shape                   | wasm_bindgen_test | analyst_nav_permissions_shape                         | covered | —     |
| FVM-036 | NavPerms   | Recruiter nav permission shape                 | wasm_bindgen_test | recruiter_nav_permissions_shape                       | covered | —     |
| FVM-037 | NavPerms   | Admin nav permission shape                     | wasm_bindgen_test | admin_nav_permissions_shape                           | covered | —     |
| FVM-038 | NavPerms   | PermGate unauth and authz shapes               | wasm_bindgen_test | perm_gate_unauth_and_authz_shapes                     | covered | —     |
| FVM-039 | Format     | format_date MM/DD/YYYY                         | wasm_bindgen_test | format_date_formats_naive_date_as_mm_dd_yyyy          | covered | —     |
| FVM-040 | Format     | format_date pads single-digit month/day        | wasm_bindgen_test | format_date_pads_single_digit_month_and_day           | covered | —     |
| FVM-041 | Format     | format_ts_opt(None) returns em-dash            | wasm_bindgen_test | format_ts_opt_none_returns_em_dash                    | covered | —     |
| FVM-042 | Format     | format_ts_opt(Some) returns formatted string   | wasm_bindgen_test | format_ts_opt_some_returns_formatted_string           | covered | —     |
| FVM-043 | Format     | format_ts epoch non-empty with slash           | wasm_bindgen_test | format_ts_produces_non_empty_string_for_epoch         | covered | —     |
| FVM-044 | AppConst   | NOTIFICATIONS_POLL_MS is 30 000                | wasm_bindgen_test | app_notification_poll_interval_is_30s                 | covered | —     |
| FVM-045 | AppConst   | TOAST_AUTO_DISMISS_MS is 5 000                 | wasm_bindgen_test | app_toast_auto_dismiss_is_5s                          | covered | —     |
| FVM-046 | AppConst   | REFRESH_LEAD_MS is 60 000                      | wasm_bindgen_test | app_token_refresh_lead_is_60s                         | covered | —     |
| FVM-047 | AppConst   | REFRESH_MIN_DELAY_MS is 5 000                  | wasm_bindgen_test | app_refresh_min_delay_is_5s                           | covered | —     |
| FVM-048 | AppConst   | REFRESH_RETRY_ON_ERROR_MS is 30 000            | wasm_bindgen_test | app_refresh_retry_on_error_is_30s                     | covered | —     |
| FVM-049 | PermGate   | PermAnyGate any() positive                     | wasm_bindgen_test | perm_any_gate_any_semantics_positive                  | covered | —     |
| FVM-050 | PermGate   | PermAnyGate any() negative                     | wasm_bindgen_test | perm_any_gate_any_semantics_negative                  | covered | —     |
| FVM-051 | PermGate   | manage is superset of read in any-gate         | wasm_bindgen_test | perm_any_gate_manage_is_superset_of_read              | covered | —     |
| FVM-052 | PermGate   | report any-gate run or schedule                | wasm_bindgen_test | report_perm_any_gate_run_or_schedule                  | covered | —     |
| FVM-053 | ApiClient  | timeout fires for slow future (api.rs)         | wasm_bindgen_test | timeout_fires_for_slow_future                         | covered | —     |
| FVM-054 | ApiClient  | timeout does not fire for fast future (api.rs) | wasm_bindgen_test | timeout_does_not_fire_for_fast_future                 | covered | —     |
| FVM-055 | ApiClient  | api_error user_facing maps codes (api.rs)      | wasm_bindgen_test | api_error_user_facing_maps_codes                      | covered | —     |
| FVM-056 | ApiClient  | api_error unauthenticated detects codes (api.rs)| wasm_bindgen_test| api_error_unauthenticated_detects_both_codes          | covered | —     |
| FVM-057 | ApiClient  | client attaches token (api.rs)                 | wasm_bindgen_test | client_attaches_token                                 | covered | —     |
| FVM-058 | E2E-Admin  | admin reaches Users/Allowlist/Retention/mTLS   | playwright        | admin_ops.spec.ts                                     | covered | —     |
| FVM-059 | E2E-Notif  | analyst reaches alert rules + notification ctr | playwright        | alert_to_notification.spec.ts                         | covered | —     |
| FVM-060 | E2E-Analyst| analyst reaches sources/definitions/kpi/reports| playwright        | analyst_metric_report.spec.ts                         | covered | —     |
| FVM-061 | E2E-Auth   | login success and wrong-password rejection     | playwright        | login.spec.ts                                         | covered | —     |
| FVM-062 | E2E-Sec    | protected route redirects; 404 renders         | playwright        | offline_states.spec.ts                                | covered | —     |
| FVM-063 | E2E-Steward| steward reaches products and imports surfaces  | playwright        | products_import.spec.ts                               | covered | —     |
| FVM-064 | E2E-Talent | recruiter reaches talent surfaces              | playwright        | talent_recommendations.spec.ts                        | covered | —     |
| FVM-065 | Route      | Route::Dashboard registered in router          | route             | Dashboard                                             | covered | —     |
| FVM-066 | Route      | Route::Login registered in router              | route             | Login                                                 | covered | —     |
| FVM-067 | Route      | Route::AdminUsers registered in router         | route             | AdminUsers                                            | covered | —     |
| FVM-068 | Route      | Route::Products registered in router           | route             | Products                                              | covered | —     |
| FVM-069 | Route      | Route::TalentCandidates registered in router   | route             | TalentCandidates                                      | covered | —     |
| FVM-070 | Route      | Route::NotFound registered in router           | route             | NotFound                                              | covered | —     |

**70 rows, all covered. FVM score = 70/70 = 100 % ≥ 90 % floor.**

---

## Endpoint Audit Rules

Documented for `scripts/audit_endpoints.sh` (Gate 3).

- **ID regex**: `^[A-Z]{1,5}[0-9]+$`  (e.g. `A1`, `SEC3`, `DEEP10`)
- **Test fn regex**: `\bfn[[:space:]]+t_([A-Za-z]{1,5}[0-9]+)_[A-Za-z0-9_]*[[:space:]]*\(`
  — captured prefix is uppercased before the inventory comparison
- **Spec row format**: `| <ID> | <METHOD> | \`<path>\` | <auth> | <notes> |`
  inside the `## Endpoint Inventory` section of `docs/api-spec.md`
- **Route line format**: `.route("<relative-path>", web::<method>().to(...))`
  prefixed with the enclosing `web::scope("/<prefix>")` and the fixed
  `/api/v1` app-level scope
- **Authoritative total**: single declarative sentence
  `**N HTTP endpoints.**` inside `## Totals` in `docs/api-spec.md`
- **Strict mode**: active when
  `crates/backend/tests/.audit_strict` exists; enforces forward parity
  (every spec ID has at least one test fn)
