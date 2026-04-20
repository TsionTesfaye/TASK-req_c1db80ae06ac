# TerraOps — Test Coverage Contract

This file is the repo-enforceable coverage contract. Two gates in
`run_tests.sh` read from it and two human-readable sections explain
what is proven and what is explicitly not.

> **Gate 1 and Gate 2 measure different things.** They are not the
> same kind of number and must never be conflated.
>
> | Gate | What it measures | Unit | Floor | Current |
> |---|---|---|---|---|
> | **Gate 1** (backend) | native-Rust source lines executed by tests | `lines_covered / lines_total × 100` (real **line coverage**) | 94 | ~94.83 % |
> | **Gate 2** (frontend) | requirement matrix rows with grep-verifiable evidence | `covered_rows / total_rows × 100` (**Frontend Verification Matrix score**, NOT line coverage) | 90 | 100 % (67/67 rows satisfied) |
>
> The frontend 100 is a **verification-matrix score**, not a
> line-coverage percentage. Anywhere in this repo that it is reported
> without the explicit qualifier "Frontend Verification Matrix score"
> is a bug and should be fixed.

- **Gate 1 (backend / shared — real line coverage)**
  Native Rust line coverage over `crates/shared/**` + `crates/backend/src/**`
  measured end-to-end with `cargo llvm-cov`, floor `GATE1_LINE_FLOOR=94`.
  Currently measured at ~94.83 %. This IS line coverage, in the
  conventional sense: `lines_covered / lines_total × 100`.

- **Gate 2 (frontend — Frontend Verification Matrix, FVM)**

  A requirement-row **verification-matrix score** enforced by
  `scripts/frontend_verify.sh`. The score is
  `covered_rows / total_rows × 100`.

  > **This is NOT line coverage.** It is not the percentage of
  > `crates/frontend/src/**` source lines executed by tests. Wasm
  > source-based line coverage is not the authoritative frontend
  > proof on this toolchain — see §Why the frontend is not measured
  > by wasm source-based line coverage below for the exact
  > toolchain blockers observed. The FVM is what replaced that
  > would-be line-coverage gate as the authoritative frontend proof.

  - **Floor:** `GATE2_FVM_FLOOR=90`.
  - **Current measured value:** **100% Frontend Verification Matrix
    score (67/67 rows satisfied)**. This exact phrasing is the
    canonical way to report it. Never shorten it to "100% frontend
    coverage".

  **What the 100 % score actually means.** 67 of 67 requirement
  rows in the matrix below (authoritative, in this file) have a
  grep-verifiable piece of evidence that exists in the codebase
  exactly as declared. Each row declares one of three evidence
  kinds:

  - `wasm_bindgen_test` — a `#[wasm_bindgen_test]` function with the
    declared name exists in `crates/frontend/src/**/*.rs`.
  - `playwright` — a non-empty spec file exists at
    `e2e/specs/<name>.spec.ts`.
  - `route` — a `Route::<Variant>` enum variant with the declared
    name exists in `crates/frontend/src/router.rs`.

  `scripts/frontend_verify.sh` mechanically verifies every row, and
  a `covered` row whose evidence is missing or misspelled is a HARD
  failure, not a silent zero-score row — so dishonest greens are
  impossible and the score cannot be inflated by stale rows.

  **Gate 2 is layered alongside — not a replacement for — real test
  execution.** Gate 2 itself runs in two halves:

  - **Gate 2a** runs the actual `wasm-bindgen-test` suite for
    `terraops-frontend` via `wasm-bindgen-test-runner` in Node mode
    and fails on any test failure (real regression gate on frontend
    logic: auth, api client, toast, notifications, router).
  - **Gate 2b** runs `scripts/frontend_verify.sh` against this file
    and enforces `GATE2_FVM_FLOOR=90`.

  The 7 Playwright specs that appear as matrix rows also run under
  the separate **Flow gate** (Playwright specs against the live
  `app` service on the compose network). A matrix row pointing at a
  Playwright spec is therefore backed by the spec's real end-to-end
  execution in the flow gate, not just its file existence.

  **What the FVM score explicitly does NOT prove.**

  1. It does not prove that every source line in
     `crates/frontend/src/**` is executed by a test. No
     statement-level wasm line count is enforced by this gate.
  2. A row's score is only as strong as the evidence declared. A
     trivial `#[wasm_bindgen_test]` that only touches a helper still
     scores 1. Reviewers should read the `Evidence` column together
     with the `Requirement` column.
  3. Playwright specs prove end-to-end behavior for the concrete
     flow they script; they do not prove that every component they
     transit is individually unit-tested.
  4. `route` evidence only proves that a `Route::<Variant>` enum
     variant is declared in `router.rs`. It does not prove the
     route is reachable from the nav, linked from any page, or
     rendered correctly. Route-kind rows are minimum-floor evidence
     for the router's type-level shape and nothing more; the
     per-role Playwright specs (FVM-F2/G2/H2/I2) and the nav-shape
     wasm tests (FVM-F1/G1/H1/I1) carry the real routing proof.

## Why the frontend is not measured by wasm source-based line coverage

The earlier plan attempted `-C instrument-coverage` on
`wasm32-unknown-unknown` and aggregated `.profraw` files with `grcov`.
That path is not achievable on the pinned stable toolchain used by
`Dockerfile.tests` (`rust:1.88-bookworm`). The exact observed blocker:

- `cargo test --target wasm32-unknown-unknown -p terraops-frontend`
  with `RUSTFLAGS="-C instrument-coverage"` fails at compile time for
  every crate with `error[E0463]: can't find crate for 'profiler_builtins'`
  — rustc's `wasm32-unknown-unknown` sysroot (`rust-std-wasm32-unknown-unknown`)
  does not ship the `libclang_rt.profile` runtime.
- The standard `-Z build-std=std,panic_abort,profiler_builtins
  -Z build-std-features=profiler` workaround requires a real nightly
  rustc; under stable 1.88 with `RUSTC_BOOTSTRAP=1` the rust-src copy
  of `core` fails to build (4754 errors against portable-simd /
  const-stdlib features).
- `terraops-frontend` is declared bin-only (`[[bin]]`, no `[lib]`)
  and every source file takes a hard dependency on `yew`, `web-sys`,
  `gloo-net`, `gloo-storage`, `gloo-timers` at the type level. The
  crate cannot cross-compile to a native target for coverage without
  a material re-architecture that would split wasm-only surfaces
  behind `cfg(target_arch = "wasm32")`.

All three workarounds are toolchain-level changes that fall outside
Gate 2's bounded scope. Rather than ship a dishonest "line coverage"
number that does not reflect the wasm runtime, Gate 2 enforces a
matrix-based 90%+ **verification-matrix score** whose evidence is
grep-checked against real test code and Playwright specs. The score
therefore replaces source-line coverage as the authoritative frontend
proof, but it is a different kind of measurement and must be read as
such — not as line coverage.

## Evidence kinds (what can appear in the matrix)

Each matrix row has an **Evidence** token whose existence is
mechanically verified by `scripts/frontend_verify.sh`:

| Kind | Verified by |
|---|---|
| `wasm_bindgen_test` | grep for `fn <token>` inside `crates/frontend/src/**/*.rs` — the function must exist and carry a `#[wasm_bindgen_test]` attribute. |
| `playwright` | file `e2e/specs/<token>` exists and is non-empty. |
| `route` | grep for `Route::<token>` inside `crates/frontend/src/router.rs`. |

Rows marked **covered** whose evidence fails the existence check are
HARD fails in `scripts/frontend_verify.sh` — that blocks the gate
from passing on stale or misspelled rows (no dishonest "green" from
a typo).

## Scoring rule

```
score = rows where Status == "covered" AND evidence verified
total = rows with Status in {"covered", "deferred"}
Gate 2 passes iff (score / total) * 100 >= GATE2_FVM_FLOOR (default 90).
```

`deferred` rows count toward the denominator but not the numerator;
they are the honest escape hatch for known gaps. Every `deferred`
row must carry a prose reason.

---

## Frontend Verification Matrix

The table below is authoritative. `scripts/frontend_verify.sh` parses
every row whose first column is `FVM-*`.

| ID | Family | Requirement | Evidence Kind | Evidence | Status | Notes |
|---|---|---|---|---|---|---|
| FVM-A1 | auth | AuthState.has_permission positive + negative | wasm_bindgen_test | auth_state_has_permission_positive_and_negative | covered | pure logic |
| FVM-A2 | auth | AuthState.has_role for every Role variant | wasm_bindgen_test | auth_state_has_role_positive_and_negative | covered | pure logic |
| FVM-A3 | auth | AuthState.is_admin reflects Administrator role | wasm_bindgen_test | auth_state_is_admin_reflects_role | covered | pure logic |
| FVM-A4 | auth | AuthState.display_name / user_id accessors | wasm_bindgen_test | auth_state_accessors_return_expected_fields | covered | pure logic |
| FVM-A5 | auth | AuthContext.api() forwards token when signed in | wasm_bindgen_test | auth_context_api_propagates_token_when_signed_in | covered | pure logic |
| FVM-A6 | auth | AuthContext anonymous path when signed out | wasm_bindgen_test | auth_context_api_is_anonymous_when_signed_out | covered | pure logic |
| FVM-A7 | auth | Login flow end-to-end (real HTTPS) | playwright | login.spec.ts | covered | existing spec |
| FVM-B1 | api | 3s timeout fires for slow future | wasm_bindgen_test | timeout_fires_for_slow_future | covered | existing |
| FVM-B2 | api | timeout does not fire for fast future | wasm_bindgen_test | timeout_does_not_fire_for_fast_future | covered | existing |
| FVM-B3 | api | user_facing mapping for core codes | wasm_bindgen_test | api_error_user_facing_maps_codes | covered | existing |
| FVM-B4 | api | is_unauthenticated for both auth codes | wasm_bindgen_test | api_error_unauthenticated_detects_both_codes | covered | existing |
| FVM-B5 | api | bearer token attachment | wasm_bindgen_test | client_attaches_token | covered | existing |
| FVM-B6 | api | user_facing covers every ErrorCode | wasm_bindgen_test | api_error_user_facing_covers_all_error_codes | covered | new |
| FVM-B7 | api | validation + conflict forward server message | wasm_bindgen_test | api_error_validation_and_conflict_forward_server_message | covered | new |
| FVM-B8 | api | user_facing for Http + Decode variants | wasm_bindgen_test | api_error_user_facing_for_http_and_decode_variants | covered | new |
| FVM-B9 | api | is_unauthenticated false for non-auth errors | wasm_bindgen_test | api_error_unauthenticated_false_for_validation_errors | covered | new |
| FVM-B10 | api | ApiClient::with_token(None) anonymous | wasm_bindgen_test | api_client_with_token_none_is_anonymous | covered | new |
| FVM-B11 | api | ApiClient::default / ::new are anonymous | wasm_bindgen_test | api_client_default_is_anonymous | covered | new |
| FVM-B12 | api | ApiClient clone preserves token | wasm_bindgen_test | api_client_clone_preserves_token | covered | new |
| FVM-B13 | api | ApiError equality + discriminants | wasm_bindgen_test | api_error_equality_semantics | covered | new |
| FVM-B14 | api | Budget constants match design.md (3000ms / 1 retry / /api/v1) | wasm_bindgen_test | api_budget_constants_match_design_contract | covered | new |
| FVM-B15 | api | select_timeout returns Some on fast future | wasm_bindgen_test | select_timeout_returns_some_when_future_wins | covered | new |
| FVM-B16 | api | select_timeout returns None on slow future | wasm_bindgen_test | select_timeout_returns_none_when_timer_wins | covered | new |
| FVM-B17 | api | ErrorEnvelope serde round-trip | wasm_bindgen_test | error_envelope_json_round_trip | covered | new |
| FVM-C1 | toast | ToastLevel.class for every variant | wasm_bindgen_test | toast_level_class_maps_variants | covered | new |
| FVM-C2 | toast | Toast struct equality | wasm_bindgen_test | toast_struct_equality | covered | new |
| FVM-C3 | toast | ToastLevel Copy + Eq | wasm_bindgen_test | toast_level_copy_and_eq | covered | new |
| FVM-D1 | notif | NotificationsSnapshot default zero | wasm_bindgen_test | notifications_snapshot_default_is_zero | covered | new |
| FVM-D2 | notif | NotificationsSnapshot equality | wasm_bindgen_test | notifications_snapshot_equality | covered | new |
| FVM-D3 | notif | NotificationsContext.snapshot readable | wasm_bindgen_test | notifications_context_snapshot_is_readable | covered | new |
| FVM-D4 | notif | alert → notification flow end-to-end | playwright | alert_to_notification.spec.ts | covered | existing |
| FVM-E1 | router | Route equality + clone | wasm_bindgen_test | route_equality_and_clone | covered | new |
| FVM-E2 | router | Route with Uuid param equality | wasm_bindgen_test | route_with_uuid_param_equality | covered | new |
| FVM-E3 | router | NotFound + Root are distinct | wasm_bindgen_test | route_not_found_and_root_are_distinct | covered | new |
| FVM-E4 | router | Admin route variants are distinct | wasm_bindgen_test | route_admin_variants_are_distinct | covered | new |
| FVM-E5 | router | Monitoring route variants are distinct | wasm_bindgen_test | route_monitoring_variants_are_distinct | covered | new |
| FVM-E6 | router | Data steward route variants are distinct | wasm_bindgen_test | route_data_steward_variants_are_distinct | covered | new |
| FVM-E7 | router | Analyst route variants are distinct | wasm_bindgen_test | route_analyst_variants_are_distinct | covered | new |
| FVM-E8 | router | Talent route variants are distinct | wasm_bindgen_test | route_talent_variants_are_distinct | covered | new |
| FVM-E9 | router | Route Debug formatting | wasm_bindgen_test | route_debug_formatting_includes_variant_name | covered | new |
| FVM-E10 | router | ProductDetail route variant exists | route | ProductDetail | covered | router.rs |
| FVM-E11 | router | TalentCandidateDetail route variant exists | route | TalentCandidateDetail | covered | router.rs |
| FVM-E12 | router | MetricDefinitionDetail route variant exists | route | MetricDefinitionDetail | covered | router.rs |
| FVM-F1 | data_steward | Nav permissions shape (product.read / manage) | wasm_bindgen_test | data_steward_nav_permissions_shape | covered | new |
| FVM-F2 | data_steward | Products + imports flow end-to-end | playwright | products_import.spec.ts | covered | existing |
| FVM-G1 | analyst | Nav permissions shape (env/metric/kpi/alert/report) | wasm_bindgen_test | analyst_nav_permissions_shape | covered | new |
| FVM-G2 | analyst | Metric + report flow end-to-end | playwright | analyst_metric_report.spec.ts | covered | existing |
| FVM-H1 | recruiter | Nav permissions shape (talent.read) | wasm_bindgen_test | recruiter_nav_permissions_shape | covered | new |
| FVM-H2 | recruiter | Talent recommendations flow end-to-end | playwright | talent_recommendations.spec.ts | covered | existing |
| FVM-I1 | admin | Nav permissions shape (user / allowlist / mtls / retention / monitoring) | wasm_bindgen_test | admin_nav_permissions_shape | covered | new |
| FVM-I2 | admin | Admin ops flow end-to-end | playwright | admin_ops.spec.ts | covered | existing |
| FVM-J1 | state_model | PermGate three-branch shape (unauth / denied / authorized) | wasm_bindgen_test | perm_gate_unauth_and_authz_shapes | covered | new |
| FVM-J2 | state_model | Offline / loading / empty / error states | playwright | offline_states.spec.ts | covered | existing |
| FVM-K1 | pages | format_date formats NaiveDate as MM/DD/YYYY | wasm_bindgen_test | format_date_formats_naive_date_as_mm_dd_yyyy | covered | pages pure helper |
| FVM-K2 | pages | format_date zero-pads single-digit month and day | wasm_bindgen_test | format_date_pads_single_digit_month_and_day | covered | pages pure helper |
| FVM-K3 | pages | format_ts_opt None returns em-dash | wasm_bindgen_test | format_ts_opt_none_returns_em_dash | covered | pages pure helper |
| FVM-K4 | pages | format_ts_opt Some returns formatted string | wasm_bindgen_test | format_ts_opt_some_returns_formatted_string | covered | pages pure helper |
| FVM-K5 | pages | format_ts produces non-empty string with slashes | wasm_bindgen_test | format_ts_produces_non_empty_string_for_epoch | covered | pages pure helper |
| FVM-L1 | app | notifications poll interval is 30 s | wasm_bindgen_test | app_notification_poll_interval_is_30s | covered | app.rs constant |
| FVM-L2 | app | toast auto-dismiss is 5 s | wasm_bindgen_test | app_toast_auto_dismiss_is_5s | covered | app.rs constant |
| FVM-L3 | app | token refresh lead is 60 s | wasm_bindgen_test | app_token_refresh_lead_is_60s | covered | app.rs constant |
| FVM-L4 | app | refresh min delay is 5 s | wasm_bindgen_test | app_refresh_min_delay_is_5s | covered | app.rs constant |
| FVM-L5 | app | refresh retry-on-error is 30 s | wasm_bindgen_test | app_refresh_retry_on_error_is_30s | covered | app.rs constant |
| FVM-M1 | components | PermAnyGate any-semantics positive | wasm_bindgen_test | perm_any_gate_any_semantics_positive | covered | components logic |
| FVM-M2 | components | PermAnyGate any-semantics negative | wasm_bindgen_test | perm_any_gate_any_semantics_negative | covered | components logic |
| FVM-M3 | components | PermAnyGate manage is superset of read | wasm_bindgen_test | perm_any_gate_manage_is_superset_of_read | covered | components RBAC |
| FVM-M4 | components | PermAnyGate report run-or-schedule any-gate | wasm_bindgen_test | report_perm_any_gate_run_or_schedule | covered | components RBAC |

### Summary (prose)

- **67 rows** above, **0 `deferred`**, **67 `covered`** → current
  Frontend Verification Matrix score **100% (67/67 rows satisfied)**.
  This is a requirement-row score, not wasm source-line coverage.
- The 67 rows span 13 families: auth gating, API-client error/retry/budget
  semantics, toast/notification plumbing, router variants, nav-permission
  shapes (one wasm + one Playwright per role), state-model gates,
  pages.rs pure helpers (format_date / format_ts / format_ts_opt),
  app.rs design-contract constants, and components.rs PermAnyGate logic.
- Playwright specs contribute 7 rows (`login.spec.ts`,
  `alert_to_notification.spec.ts`, `products_import.spec.ts`,
  `analyst_metric_report.spec.ts`, `talent_recommendations.spec.ts`,
  `admin_ops.spec.ts`, `offline_states.spec.ts`). Every recruiter/
  data-steward/analyst/admin page family has at least one wasm-bindgen
  row **plus** one Playwright row.
- **Static verification (wasm-bindgen unit + logic tests): 57 rows.**
- **Playwright flow verification: 7 rows.**
- **Router static enumeration: 3 rows.**

## Endpoint Audit Rules (Gate 3)

`scripts/audit_endpoints.sh` is the repo-enforced parity gate between
the real mounted Actix-web router and the inventory in
`docs/api-spec.md`. It runs three checks on every `run_tests.sh`.

1. **Reverse ID parity (always enforced).** Every `t_<id>_*` integration
   test fn in `crates/backend/tests/**/*.rs` must reference an ID that
   appears in the `## Endpoint Inventory` tables of `docs/api-spec.md`.
2. **Forward ID parity (strict mode only — marker file
   `crates/backend/tests/.audit_strict` exists).** Every authoritative ID
   in the inventory must have at least one matching `t_<id>_*` test fn.
3. **Method+path parity (always enforced, audit #10 issue #1).** The
   `(METHOD, PATH)` set derived from `api-spec.md` inventory rows must
   equal the `(METHOD, PATH)` set mounted by the backend router. Drift
   in either direction — a route documented but not mounted, or mounted
   but not documented — is a HARD failure. This is the check that
   catches "spec lies about what is mounted" the way ID-only parity
   cannot.

Parser contracts:

| Input | Regex / shape |
|---|---|
| Inventory ID | `^[A-Z]{1,5}[0-9]+$` |
| Inventory row | `\| <ID> \| <METHOD> \| ` + backticked `<path>` + ` \| <auth> \| <notes> \|` |
| Test fn | `\bfn[[:space:]]+t_([A-Za-z]{1,5}[0-9]+)_[A-Za-z0-9_]*[[:space:]]*\(` (case-insensitive; captured ID is upper-cased before comparison) |
| Router single-line | `.route("<p>", web::<m>().to(...))` inside a `web::scope("/<scope>")` block or bare `cfg.route("<p>", web::<m>().to(...))` |
| Router multi-line | `.route(` on one line, then the path literal, then `web::<m>().to(...),`, then `),` — the script assembles the tuple across lines |
| Mounted path | `/api/v1` + scope + relative — the app-level `web::scope("/api/v1")` is not a feature family and is skipped |

The authoritative endpoint total is a single declarative sentence
`**N HTTP endpoints.**` inside the `## Totals` section of
`docs/api-spec.md`. The script reads that sentence directly; it does
not derive the total from heuristics over table rows.

## Endpoint-to-Test Map

Every row in `docs/api-spec.md §Endpoint Inventory` has at least one
`t_<id>_*` test function. The table below is human-navigable; the
mechanical proof lives in `scripts/audit_endpoints.sh`.

| ID | Test fn (first match) | Location |
|---|---|---|
| S1 | `t_s1_health_is_public_and_ok` | `crates/backend/tests/http_p1.rs` |
| S2 | `t_s2_ready_is_public_and_ok` | `crates/backend/tests/http_p1.rs` |
| A1 | `t_a1_login_success_sets_refresh_cookie` + `t_a1_login_rejects_email_as_username` | `crates/backend/tests/http_p1.rs` |
| A2 | `t_a2_refresh_rotates_and_revokes_old_session` | `crates/backend/tests/http_p1.rs` |
| A3 | `t_a3_logout_revokes_current_session` | `crates/backend/tests/http_p1.rs` |
| A4 | `t_a4_me_returns_caller_profile` | `crates/backend/tests/http_p1.rs` |
| A5 | `t_a5_change_password_requires_current_and_rotates_hash` | `crates/backend/tests/http_p1.rs` |
| U1–U10 | `t_u1_..` through `t_u10_..` | `crates/backend/tests/http_p1.rs` |
| SEC1–SEC9 | `t_sec1_..` through `t_sec9_..` | `crates/backend/tests/http_p1.rs` |
| R1–R3 | `t_r1_..` through `t_r3_..` | `crates/backend/tests/http_p1.rs` |
| M1–M4 | `t_m1_..` through `t_m4_..` | `crates/backend/tests/http_p1.rs` |
| REF1–REF9 | `t_ref1_..` through `t_ref9_..` | `crates/backend/tests/http_p1.rs` |
| N1–N9 | `t_n1_..` through `t_n9_..` (N8/N9 added audit #10 issue #1) | `crates/backend/tests/http_p1.rs` |
| P1–P14 | `t_p1_..` through `t_p14_..` | `crates/backend/tests/http_products_*.rs` |
| I1–I7 | `t_i1_..` through `t_i7_..` | `crates/backend/tests/http_imports_tests.rs` |
| E1–E6 | `t_e1_..` through `t_e6_..` | `crates/backend/tests/metrics_env_tests.rs` |
| MD1–MD7 | `t_md1_..` through `t_md7_..` | `crates/backend/tests/metrics_def_tests.rs` |
| K1–K6 | `t_k1_..` through `t_k6_..` | `crates/backend/tests/kpi_tests.rs` |
| AL1–AL6 | `t_al1_..` through `t_al6_..` | `crates/backend/tests/alerts_tests.rs` |
| RP1–RP6 | `t_rp1_..` through `t_rp6_..` | `crates/backend/tests/reports_tests.rs` |
| T1–T13 | `t_t1_..` through `t_t13_..` | `crates/backend/tests/talent_*.rs` |
| T14 | `t_t14_list_items_owner_scoped` + `t_t14_list_items_forbidden_for_other_user` (added audit #10 issue #1) | `crates/backend/tests/talent_watchlist_tests.rs` |

Exact fn names may drift; the audit script never compares names beyond
the `t_<id>_` prefix regex above. The columns are maintained for human
reviewers who want to open the right file quickly.

## Limitations (explicit, honest)

1. Gate 2 does not measure statement-level line coverage of wasm
   code, and the reported FVM score must never be read as line
   coverage. The upstream blocker on `profiler_builtins` for wasm32
   means no honest wasm line-coverage number is available today. The
   matrix is the substitute proof: every requirement family has at
   least one grep-verifiable test or Playwright spec touching its
   real code path, but satisfaction is row-level, not line-level.
2. DOM-rendered behavior (form submit, in-page navigation, loading
   spinner visibility, toast rendering) is intentionally verified by
   Playwright, not by wasm-bindgen-test — wasm-bindgen-test in Node
   mode has no DOM, and a DOM-mode runner would require pinned
   Chromium which currently belongs to the flow gate.
3. `PermGate`'s redirect path is proved via the state shape it walks
   (wasm test) plus the `login.spec.ts` flow (Playwright). Rendering
   the `Redirect<Route>` component itself requires yew + DOM and is
   considered covered by the flow gate.
4. Ownership: this file is the single source of truth for the
   frontend verification contract. Editing the matrix without
   adding the corresponding test will fail `scripts/frontend_verify.sh`.
