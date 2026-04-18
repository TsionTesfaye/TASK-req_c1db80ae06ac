# Test Coverage — TerraOps

Authoritative coverage plan. Aligned with `docs/design.md` and `docs/api-spec.md`.

## Coverage Gate Math (authoritative)

**Authoritative threshold sentence** (identical wording in `design.md §Verification Strategy` and `plan.md`):

> The 90% line-coverage floor applies **only to `crates/shared` + `crates/backend`** (Rust native code). The `crates/frontend` Yew/WASM crate has its own **separately enforced 80%** line-coverage floor. Playwright contributes flow verification only and **does not contribute to either line-coverage number**.

`./run_tests.sh` enforces three independent gates plus a flow gate. Failing any one fails the run.

### Gate 1 — Backend + Shared line coverage ≥ 90%

- Scope: `crates/shared/**` + `crates/backend/src/**`, **excluding** `crates/backend/tests/**`, `crates/backend/build.rs`, `crates/backend/.sqlx/**`, and `crates/backend/migrations/**`.
- Canonical command (single invocation generates LCOV **and** enforces the 90% floor via `--fail-under-lines`):

  ```bash
  # Scope is mechanically restricted to the two native Rust crates via explicit -p
  # flags. --workspace is intentionally NOT used, so the frontend crate is never
  # compiled or instrumented under this invocation and cannot contribute lines to
  # coverage/rust.lcov. The --ignore-filename-regex additionally excludes tests,
  # migrations, and generated sqlx metadata from the numerator/denominator.
  cargo llvm-cov --no-fail-fast \
    -p terraops-shared -p terraops-backend \
    --ignore-filename-regex '/(tests|migrations|\.sqlx)/' \
    --fail-under-lines 90 \
    --lcov --output-path coverage/rust.lcov
  ```

- Every `#[actix_web::test]` no-mock HTTP test in `crates/backend/tests/http/**` contributes real handler/service/repo coverage against an ephemeral Postgres.

### Gate 2 — Frontend (Yew/WASM) line coverage ≥ 80%

- Scope: `crates/frontend/src/**` excluding `main.rs` boot shim.
- Canonical commands:

  ```bash
  RUSTFLAGS='-C instrument-coverage' \
  LLVM_PROFILE_FILE='target/wasm32-coverage/cov-%p-%m.profraw' \
  cargo test -p terraops-frontend --target wasm32-unknown-unknown

  grcov ./target/wasm32-coverage \
    --binary-path ./target/wasm32-unknown-unknown/debug \
    -s crates/frontend/src -t lcov --ignore 'main.rs' \
    --threshold 80 -o coverage/frontend.lcov
  ```

- `grcov`'s `--threshold 80` is the enforcement mechanism; `./run_tests.sh` propagates its exit status. Rationale: WASM coverage tooling is noisier than native Rust, so 80% is the honest floor while still forcing real component coverage.

### Gate 3 — Endpoint parity audit

- `scripts/audit_endpoints.sh` (invoked unconditionally by `./run_tests.sh`) implements the parser described in §Endpoint Audit Rules below.
- Strictness is **mode-driven**, not phase-hand-waved:
  - If a marker file `crates/backend/tests/.audit_strict` exists, the script runs in `strict` mode and requires 100% forward parity (every endpoint ID in `docs/api-spec.md` has at least one matching test). This file is committed during **P5**.
  - Without the marker file (scaffold, P1, P2, P3, P4), the script runs in `progress` mode: it **always** enforces the reverse check (no orphan test IDs — every `t_<id>_*` function references an ID that exists in `docs/api-spec.md`) and it **reports** forward parity as a printable percentage without failing the build.
- This makes scaffold-time test runs deterministic and consistent with the already-authoritative 77-endpoint inventory: the inventory is never empty, the audit always runs, and the strict-forward check activates deterministically when the marker file is committed.

### Flow verification — Playwright (NOT a line-coverage contributor)

- Runs the seven flow specs in `e2e/specs/`. All seven must pass. Line coverage from Playwright is **not** counted toward Gate 1 or Gate 2.

### Coverage artifacts produced by `./run_tests.sh`

- `coverage/rust.lcov` (backend + shared)
- `coverage/frontend.lcov`
- `coverage/summary.json` (`{rust_lines_pct, frontend_lines_pct, endpoint_audit_mode, endpoint_forward_pct, endpoint_reverse_ok, e2e_specs_passed}`)
- `coverage/playwright-report/` (flow evidence only)

## Endpoint Audit Rules

`scripts/audit_endpoints.sh` is ID-driven, not path-driven. This sidesteps any ambiguity around templated segments (`{id}`), query strings, signed-URL parameters, and repeated paths with different methods.

1. **Endpoint ID is the sole key.** Every row in `docs/api-spec.md` under a `### …` subsection of `## Endpoint Inventory` has its first column as the endpoint ID (e.g. `A1`, `P13`, `SEC9`). IDs match the regex `^[A-Z]{1,5}\d+$` and are unique across the whole spec.
2. **Parser contract for api-spec.md**: within the `## Endpoint Inventory` section, the script reads every Markdown table row whose first non-empty cell matches the ID regex. It ignores rows outside that section and ignores columns other than the first. Path, method, auth class, and notes are documentation only — they are not parsed by the audit.
3. **Parser contract for tests**: the script scans `crates/backend/tests/http/**/*.rs` with the regex `\bfn\s+t_([A-Z]{1,5}\d+)_[A-Za-z0-9_]*\s*\(` to collect covered IDs. An ID is "covered" if at least one matching function exists.
4. **Forward check**: `api_ids \ covered_ids == ∅`.
5. **Reverse check**: `covered_ids \ api_ids == ∅` (no orphan test targets).
6. **Mode selection**: if `crates/backend/tests/.audit_strict` exists, both checks must pass; otherwise only the reverse check must pass and the forward gap is reported numerically. Scaffold phase commits the marker file absent; P5 commits the marker file.
7. **Query strings and signed-URL semantics (P13)**: have no effect on the audit — P13's coverage is satisfied by `t_P13_*` regardless of whether tests exercise valid, expired, forged, or missing-signature cases. Those semantic cases are verified by the signature-specific tests required under R12, not by the parity gate.
8. **Templated paths (`{id}`, `{imgid}`, etc.)**: have no effect — coverage is ID-based and paths are never compared textually.

This contract is stable enough that the script can be written mechanically, and missing coverage is obvious from the script's printable report without manual audit interpretation.

## Mock Classification Rules

- **Real no-mock HTTP** is required for every endpoint in the inventory. Mechanism: `actix-web::test::init_service` against the real `App::configure(...)` wired to an ephemeral Postgres (per-suite schema, real migrations, real seed). No handler bypass, no service-level shortcut.
- **Unit** is for pure functions (Argon2 round-trip, JWT encode/verify, AES-256-GCM round-trip, HMAC stability, email-mask formatting, signed URL sign/verify, cold-start scorer, comfort-index, 15-min MA, rate-of-change, import row validator, cron-next-fire, retention selector, mailbox-export writer).
- **Repo/integration** for DB-level invariants (change-history immutability trigger, audit-log append-only, partitioned `env_observations` insert path, import-commit transaction, users `email_ciphertext` non-plaintext).
- **Frontend component** (`wasm-bindgen-test`) for the paginated table, filter bar, lineage panel, notification center, signed image, placeholder states, login form, API client retry/timeout.
- **TLS-level integration** for mTLS pin enforcement (handshake-layer refusal, not HTTP response).
- **HTTP-with-mocking** is **not accepted** as a substitute for a real endpoint test.

## Requirement And Risk Matrix

| # | Requirement / risk | Test file | Key assertion | Gate |
|---|---|---|---|---|
| R1 | Argon2id hashing + lockout | `http/auth_tests.rs` + `auth::argon2` unit | 10 failed → 423; correct pw works; lockout expires | G1 |
| R2 | 15-min access + rotating refresh | `http/auth_tests.rs` | old refresh invalid after rotation | G1 |
| R3 | RBAC route + object level | `http/rbac_tests.rs` | forbidden without perm; owner-only object scope | G1 |
| R4 | Allowlist enforcement | `http/allowlist_tests.rs` | outside-CIDR → 403 pre-auth | G1 |
| R5 | 3 s handler budget + GET retry | `http/budget_tests.rs` + frontend `api_client_test` | slow handler → 504; retry once on GET | G1/G2 |
| R6 | Pagination defaults | `http/pagination_tests.rs` | default 50; reject >200; `X-Total-Count` present | G1 |
| R7 | Validation envelope | `http/validation_tests.rs` | 422 + field_errors + request_id | G1 |
| R8 | Product CRUD/tax/images/status | `http/products_tests.rs` | full lifecycle | G1 |
| R9 | Immutable change history | `http/products_history_tests.rs` + repo integration | UPDATE blocked by trigger | G1 |
| R10 | Import preview + row validation + commit | `http/imports_tests.rs` | errors block commit; zero-error commit atomic | G1 |
| R11 | CSV + XLSX round-trip | `http/imports_export_tests.rs` | both formats | G1 |
| R12 | Signed URL HMAC + 10-min expiry | `http/signed_url_tests.rs` + `auth::signed_url` unit | forged/expired rejected | G1 |
| R13 | Env source + observation ingest | `http/env_tests.rs` | bulk ingest + windowed filter | G1 |
| R14 | Metric def + lineage | `http/metrics_tests.rs` | lineage returns sources + alignment + confidence | G1 |
| R15 | Comfort index formula | `metrics_env::formula` unit | canonical vectors | G1 |
| R16 | 15-min MA + RoC | `metrics_env::formula` unit | vectors incl. low-sample confidence | G1 |
| R17 | KPI summary + drill | `http/kpi_tests.rs` | sliced results match seed | G1 |
| R18 | Alert duration-aware evaluator | `http/alerts_eval_tests.rs` | CI>82 for 15 min fires; <15 min does not; SKU<95%/day fires | G1 |
| R19 | Report job lifecycle | `http/reports_tests.rs` | schedule/run-now/cancel/artifact; one transient retry | G1 |
| R20 | PDF/CSV/XLSX artifact integrity | unit + `http/reports_tests.rs` | non-zero bytes; correct mime; sha recorded | G1 |
| R21 | Talent search composable filters | `http/talent_search_tests.rs` | keyword/skill/major/experience/availability compose | G1 |
| R22 | Cold-start threshold crossing | `http/talent_recommend_tests.rs` | <10 feedback → recency+completeness; ≥10 → blended | G1 |
| R23 | Preference weights tuning | `http/talent_weights_tests.rs` | weight change reflected in ranking | G1 |
| R24 | Private watchlists | `http/talent_watchlist_tests.rs` | other user 403/404 | G1 |
| R25 | Feedback records | `http/talent_feedback_tests.rs` | recorded + private | G1 |
| R26 | Notification center + subscriptions | `http/notifications_tests.rs` | list/read/filter; subscription gate | G1 |
| R27 | Notification retry (5 attempts, backoff) | `http/notifications_retry_tests.rs` + integration | fails scheduled 5× then terminal | G1 |
| R28 | Mailbox `.mbox` export | `http/mailbox_export_tests.rs` | file written locally; zero outbound sockets | G1 |
| R29 | Retention enforcement | `http/retention_tests.rs` + time shim | env raw + feedback purged past TTL; KPI retained | G1 |
| R30 | Monitoring queries | `http/monitoring_tests.rs` | latency/errors/crashes visible | G1 |
| R31 | Client crash ingest | `http/monitoring_tests.rs` | POST persists; admin-only read | G1 |
| R32 | Audit log append-only | repo integration | UPDATE/DELETE blocked | G1 |
| R33 | Timestamp storage + display | `shared::time` unit | TZ-aware round-trip; US 12-hour format | G1 |
| R34 | Email encryption + masking | `http/users_tests.rs` + DB-contents integration | admin/self see full email; others see mask; plaintext substring absent from `users` rows | G1 |
| R35 | TLS bootstrap | integration | server boots with generated cert | G1 |
| R36 | mTLS pin management + enforcement | `http/admin_security_tests.rs` + `tls/mtls_integration_tests.rs` | admin CRUD works; pinned client connects; unpinned handshake refused; revoke propagates ≤1 s | G1 |
| R37 | Offline tiered cache + stale placeholder | frontend component + Playwright | stale badge renders on forced 504 | G2 + flow |
| R38 | Incremental long-list loading | frontend component | sentinel triggers next page | G2 |
| R39 | Placeholder states (loading/empty/error/stale) | frontend component | all render | G2 |
| R40 | No outbound network | test harness | tests pass while container has no non-loopback egress | G1 |

## Endpoint-to-Test Map (concrete, reviewable)

Each row names the test file and the **test function name prefix** the endpoint-audit gate will look for. Every test function begins with `t_<id>_` and lives in the named file. Implementation adds more tests per endpoint as needed; the gate only enforces presence.

### System / Public

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| S1 | GET /api/v1/health | PUBLIC | `tests/http/system_tests.rs` | `t_S1_health` |
| S2 | GET /api/v1/ready | PUBLIC | `tests/http/system_tests.rs` | `t_S2_ready` |

### Auth

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| A1 | POST /auth/login | PUBLIC | `tests/http/auth_tests.rs` | `t_A1_login` |
| A2 | POST /auth/refresh | PUBLIC | `tests/http/auth_tests.rs` | `t_A2_refresh` |
| A3 | POST /auth/logout | AUTH | `tests/http/auth_tests.rs` | `t_A3_logout` |
| A4 | GET /auth/me | AUTH | `tests/http/auth_tests.rs` | `t_A4_me` |
| A5 | POST /auth/change-password | SELF | `tests/http/auth_tests.rs` | `t_A5_change_password` |

### Users / Admin

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| U1 | GET /admin/users | PERM(user.manage) | `tests/http/users_tests.rs` | `t_U1_list` |
| U2 | POST /admin/users | PERM(user.manage) | `tests/http/users_tests.rs` | `t_U2_create` |
| U3 | GET /admin/users/{id} | PERM(user.manage) | `tests/http/users_tests.rs` | `t_U3_detail` |
| U4 | PATCH /admin/users/{id} | PERM(user.manage) | `tests/http/users_tests.rs` | `t_U4_update` |
| U5 | DELETE /admin/users/{id} | PERM(user.manage) | `tests/http/users_tests.rs` | `t_U5_disable` |
| U6 | POST /admin/users/{id}/roles | PERM(role.assign) | `tests/http/users_tests.rs` | `t_U6_roles` |
| U7 | POST /admin/users/{id}/unlock | PERM(user.manage) | `tests/http/users_tests.rs` | `t_U7_unlock` |
| U8 | POST /admin/users/{id}/reset-password | PERM(user.manage) | `tests/http/users_tests.rs` | `t_U8_reset_pw` |
| U9 | GET /admin/roles | PERM(user.manage) | `tests/http/users_tests.rs` | `t_U9_roles_list` |
| U10 | GET /admin/audit-log | PERM(user.manage) | `tests/http/users_tests.rs` | `t_U10_audit` |

### Security & Ops

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| SEC1 | GET /admin/allowlist | PERM(allowlist.manage) | `tests/http/admin_security_tests.rs` | `t_SEC1_list` |
| SEC2 | POST /admin/allowlist | PERM(allowlist.manage) | `tests/http/admin_security_tests.rs` | `t_SEC2_add` |
| SEC3 | DELETE /admin/allowlist/{id} | PERM(allowlist.manage) | `tests/http/admin_security_tests.rs` | `t_SEC3_remove` |
| SEC4 | GET /admin/device-certs | PERM(mtls.manage) | `tests/http/admin_security_tests.rs` | `t_SEC4_list` |
| SEC5 | POST /admin/device-certs | PERM(mtls.manage) | `tests/http/admin_security_tests.rs` | `t_SEC5_register` |
| SEC6 | POST /admin/device-certs/{id}/revoke | PERM(mtls.manage) | `tests/http/admin_security_tests.rs` | `t_SEC6_revoke` |
| SEC7 | DELETE /admin/device-certs/{id} | PERM(mtls.manage) | `tests/http/admin_security_tests.rs` | `t_SEC7_delete` |
| SEC8 | GET /admin/mtls-config | PERM(mtls.manage) | `tests/http/admin_security_tests.rs` | `t_SEC8_get` |
| SEC9 | PUT /admin/mtls-config | PERM(mtls.manage) | `tests/http/admin_security_tests.rs` | `t_SEC9_set` |
| R1 | GET /admin/retention | PERM(retention.manage) | `tests/http/retention_tests.rs` | `t_R1_list` |
| R2 | PUT /admin/retention/{domain} | PERM(retention.manage) | `tests/http/retention_tests.rs` | `t_R2_update` |
| R3 | POST /admin/retention/{domain}/run | PERM(retention.manage) | `tests/http/retention_tests.rs` | `t_R3_run` |

### Monitoring

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| M1 | GET /monitoring/latency | PERM(monitoring.read) | `tests/http/monitoring_tests.rs` | `t_M1_latency` |
| M2 | GET /monitoring/errors | PERM(monitoring.read) | `tests/http/monitoring_tests.rs` | `t_M2_errors` |
| M3 | GET /monitoring/crashes | PERM(monitoring.read) | `tests/http/monitoring_tests.rs` | `t_M3_crashes` |
| M4 | POST /monitoring/crashes | AUTH | `tests/http/monitoring_tests.rs` | `t_M4_ingest` |

### Reference Data

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| REF1 | GET /ref/sites | AUTH | `tests/http/ref_data_tests.rs` | `t_REF1_sites` |
| REF2 | GET /ref/departments | AUTH | `tests/http/ref_data_tests.rs` | `t_REF2_departments` |
| REF3 | GET /ref/categories | AUTH | `tests/http/ref_data_tests.rs` | `t_REF3_categories` |
| REF4 | GET /ref/brands | AUTH | `tests/http/ref_data_tests.rs` | `t_REF4_brands` |
| REF5 | GET /ref/units | AUTH | `tests/http/ref_data_tests.rs` | `t_REF5_units` |
| REF6 | GET /ref/states | AUTH | `tests/http/ref_data_tests.rs` | `t_REF6_states` |
| REF7 | POST /ref/categories | PERM(ref.write) | `tests/http/ref_data_tests.rs` | `t_REF7_add_cat` |
| REF8 | POST /ref/brands | PERM(ref.write) | `tests/http/ref_data_tests.rs` | `t_REF8_add_brand` |
| REF9 | POST /ref/units | PERM(ref.write) | `tests/http/ref_data_tests.rs` | `t_REF9_add_unit` |

### Products

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| P1 | GET /products | PERM(product.read) | `tests/http/products_tests.rs` | `t_P1_list` |
| P2 | POST /products | PERM(product.write) | `tests/http/products_tests.rs` | `t_P2_create` |
| P3 | GET /products/{id} | PERM(product.read) | `tests/http/products_tests.rs` | `t_P3_detail` |
| P4 | PATCH /products/{id} | PERM(product.write) | `tests/http/products_tests.rs` | `t_P4_update` |
| P5 | DELETE /products/{id} | PERM(product.write) | `tests/http/products_tests.rs` | `t_P5_delete` |
| P6 | POST /products/{id}/status | PERM(product.write) | `tests/http/products_tests.rs` | `t_P6_status` |
| P7 | GET /products/{id}/history | PERM(product.history.read) | `tests/http/products_history_tests.rs` | `t_P7_history` |
| P8 | POST /products/{id}/tax-rates | PERM(product.write) | `tests/http/products_tests.rs` | `t_P8_tax_add` |
| P9 | PATCH /products/{id}/tax-rates/{rid} | PERM(product.write) | `tests/http/products_tests.rs` | `t_P9_tax_update` |
| P10 | DELETE /products/{id}/tax-rates/{rid} | PERM(product.write) | `tests/http/products_tests.rs` | `t_P10_tax_del` |
| P11 | POST /products/{id}/images | PERM(product.write) | `tests/http/images_tests.rs` | `t_P11_upload` |
| P12 | DELETE /products/{id}/images/{imgid} | PERM(product.write) | `tests/http/images_tests.rs` | `t_P12_delete` |
| P13 | GET /images/{imgid} | AUTH | `tests/http/images_tests.rs` | `t_P13_signed_get` |
| P14 | POST /products/export | PERM(product.read) | `tests/http/imports_export_tests.rs` | `t_P14_export` |

### Imports

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| I1 | POST /imports | PERM(product.import) | `tests/http/imports_tests.rs` | `t_I1_upload` |
| I2 | GET /imports | PERM(product.import) | `tests/http/imports_tests.rs` | `t_I2_list` |
| I3 | GET /imports/{id} | PERM(product.import) | `tests/http/imports_tests.rs` | `t_I3_detail` |
| I4 | GET /imports/{id}/rows | PERM(product.import) | `tests/http/imports_tests.rs` | `t_I4_rows` |
| I5 | POST /imports/{id}/validate | PERM(product.import) | `tests/http/imports_tests.rs` | `t_I5_validate` |
| I6 | POST /imports/{id}/commit | PERM(product.import) | `tests/http/imports_tests.rs` | `t_I6_commit` |
| I7 | POST /imports/{id}/cancel | PERM(product.import) | `tests/http/imports_tests.rs` | `t_I7_cancel` |

### Environmental

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| E1 | GET /env/sources | PERM(metric.read) | `tests/http/env_tests.rs` | `t_E1_list` |
| E2 | POST /env/sources | PERM(metric.configure) | `tests/http/env_tests.rs` | `t_E2_create` |
| E3 | PATCH /env/sources/{id} | PERM(metric.configure) | `tests/http/env_tests.rs` | `t_E3_update` |
| E4 | DELETE /env/sources/{id} | PERM(metric.configure) | `tests/http/env_tests.rs` | `t_E4_delete` |
| E5 | POST /env/sources/{id}/observations | PERM(metric.configure) | `tests/http/env_tests.rs` | `t_E5_ingest` |
| E6 | GET /env/observations | PERM(metric.read) | `tests/http/env_tests.rs` | `t_E6_query` |

### Metric Definitions

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| MD1 | GET /metrics/definitions | PERM(metric.read) | `tests/http/metrics_tests.rs` | `t_MD1_list` |
| MD2 | POST /metrics/definitions | PERM(metric.configure) | `tests/http/metrics_tests.rs` | `t_MD2_create` |
| MD3 | GET /metrics/definitions/{id} | PERM(metric.read) | `tests/http/metrics_tests.rs` | `t_MD3_detail` |
| MD4 | PATCH /metrics/definitions/{id} | PERM(metric.configure) | `tests/http/metrics_tests.rs` | `t_MD4_update` |
| MD5 | DELETE /metrics/definitions/{id} | PERM(metric.configure) | `tests/http/metrics_tests.rs` | `t_MD5_disable` |
| MD6 | GET /metrics/definitions/{id}/series | PERM(metric.read) | `tests/http/metrics_tests.rs` | `t_MD6_series` |
| MD7 | GET /metrics/computations/{id}/lineage | PERM(metric.read) | `tests/http/metrics_tests.rs` | `t_MD7_lineage` |

### KPIs

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| K1 | GET /kpi/summary | PERM(kpi.read) | `tests/http/kpi_tests.rs` | `t_K1_summary` |
| K2 | GET /kpi/cycle-time | PERM(kpi.read) | `tests/http/kpi_tests.rs` | `t_K2_cycle` |
| K3 | GET /kpi/funnel | PERM(kpi.read) | `tests/http/kpi_tests.rs` | `t_K3_funnel` |
| K4 | GET /kpi/anomalies | PERM(kpi.read) | `tests/http/kpi_tests.rs` | `t_K4_anomalies` |
| K5 | GET /kpi/efficiency | PERM(kpi.read) | `tests/http/kpi_tests.rs` | `t_K5_efficiency` |
| K6 | GET /kpi/drill | PERM(kpi.read) | `tests/http/kpi_tests.rs` | `t_K6_drill` |

### Alerts

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| AL1 | GET /alerts/rules | PERM(alert.manage) | `tests/http/alerts_eval_tests.rs` | `t_AL1_list_rules` |
| AL2 | POST /alerts/rules | PERM(alert.manage) | `tests/http/alerts_eval_tests.rs` | `t_AL2_create_rule` |
| AL3 | PATCH /alerts/rules/{id} | PERM(alert.manage) | `tests/http/alerts_eval_tests.rs` | `t_AL3_update_rule` |
| AL4 | DELETE /alerts/rules/{id} | PERM(alert.manage) | `tests/http/alerts_eval_tests.rs` | `t_AL4_delete_rule` |
| AL5 | GET /alerts/events | PERM(kpi.read) | `tests/http/alerts_eval_tests.rs` | `t_AL5_events` |
| AL6 | POST /alerts/events/{id}/ack | PERM(alert.ack) | `tests/http/alerts_eval_tests.rs` | `t_AL6_ack` |

### Reports

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| RP1 | GET /reports/jobs | PERM(report.run) | `tests/http/reports_tests.rs` | `t_RP1_list_own` |
| RP2 | POST /reports/jobs | PERM(report.schedule) | `tests/http/reports_tests.rs` | `t_RP2_create` |
| RP3 | GET /reports/jobs/{id} | SELF | `tests/http/reports_tests.rs` | `t_RP3_detail_owner` |
| RP4 | POST /reports/jobs/{id}/run-now | SELF | `tests/http/reports_tests.rs` | `t_RP4_run_now` |
| RP5 | POST /reports/jobs/{id}/cancel | SELF | `tests/http/reports_tests.rs` | `t_RP5_cancel` |
| RP6 | GET /reports/jobs/{id}/artifact | SELF | `tests/http/reports_tests.rs` | `t_RP6_artifact` |

### Talent

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| T1 | GET /talent/candidates | PERM(talent.read) | `tests/http/talent_search_tests.rs` | `t_T1_search` |
| T2 | POST /talent/candidates | PERM(talent.manage) | `tests/http/talent_search_tests.rs` | `t_T2_create` |
| T3 | GET /talent/candidates/{id} | PERM(talent.read) | `tests/http/talent_search_tests.rs` | `t_T3_detail` |
| T4 | GET /talent/roles | PERM(talent.read) | `tests/http/talent_search_tests.rs` | `t_T4_roles` |
| T5 | POST /talent/roles | PERM(talent.manage) | `tests/http/talent_search_tests.rs` | `t_T5_role_create` |
| T6 | GET /talent/recommendations | PERM(talent.read) | `tests/http/talent_recommend_tests.rs` | `t_T6_reco` |
| T7 | GET /talent/weights | SELF | `tests/http/talent_weights_tests.rs` | `t_T7_get` |
| T8 | PUT /talent/weights | SELF | `tests/http/talent_weights_tests.rs` | `t_T8_put` |
| T9 | POST /talent/feedback | PERM(talent.feedback) | `tests/http/talent_feedback_tests.rs` | `t_T9_feedback` |
| T10 | GET /talent/watchlists | SELF | `tests/http/talent_watchlist_tests.rs` | `t_T10_list` |
| T11 | POST /talent/watchlists | SELF | `tests/http/talent_watchlist_tests.rs` | `t_T11_create` |
| T12 | POST /talent/watchlists/{id}/items | SELF | `tests/http/talent_watchlist_tests.rs` | `t_T12_add_item` |
| T13 | DELETE /talent/watchlists/{id}/items/{cid} | SELF | `tests/http/talent_watchlist_tests.rs` | `t_T13_del_item` |

### Notifications

| ID | Endpoint | Auth | Test file | Test fn prefix |
|---|---|---|---|---|
| N1 | GET /notifications | SELF | `tests/http/notifications_tests.rs` | `t_N1_list` |
| N2 | POST /notifications/{id}/read | SELF | `tests/http/notifications_tests.rs` | `t_N2_read` |
| N3 | POST /notifications/read-all | SELF | `tests/http/notifications_tests.rs` | `t_N3_read_all` |
| N4 | GET /notifications/subscriptions | SELF | `tests/http/notifications_tests.rs` | `t_N4_sub_get` |
| N5 | PUT /notifications/subscriptions | SELF | `tests/http/notifications_tests.rs` | `t_N5_sub_put` |
| N6 | POST /notifications/mailbox-export | SELF | `tests/http/mailbox_export_tests.rs` | `t_N6_export` |
| N7 | GET /notifications/mailbox-exports/{id} | SELF | `tests/http/mailbox_export_tests.rs` | `t_N7_download` |

## Unit Test Summary

| Module family | Covered by | Critical modules |
|---|---|---|
| `shared::{time,pagination,roles,permissions,error}` | unit tests in `shared/tests/` | time formatter, pagination validator |
| `backend::auth` | unit + HTTP | argon2, jwt, signed_url, sessions, email AES-GCM round-trip |
| `backend::metrics_env::formula` | unit | comfort index, SMA, RoC |
| `backend::talent::scoring` | unit | cold-start + blended + threshold |
| `backend::products::import_validator` | unit | row validation matrix |
| `backend::reports::{pdf,csv,xlsx}` | unit + integration | artifact integrity |
| `backend::retention::selector` | unit with time shim | TTL selection |
| `backend::notifications::retry` | unit with time shim | backoff sequence |
| `frontend::api` | `wasm-bindgen-test` | 3 s timeout + retry-only-GET |
| `frontend::components` | `wasm-bindgen-test` | paginated table, filters, lineage, placeholders, notifications center |

## E2E Flow Matrix (Playwright — flow evidence only)

| Flow | Roles touched | Spec file |
|---|---|---|
| Login + role-aware landing for all 5 roles | all | `e2e/specs/login.spec.ts` |
| Products: upload → preview → commit → history → export | Data Steward | `e2e/specs/products_import.spec.ts` |
| Metric def → lineage → schedule + observe report | Analyst | `e2e/specs/analyst_metric_report.spec.ts` |
| Alert fires → notification appears → mailbox export | Analyst + Regular User | `e2e/specs/alert_to_notification.spec.ts` |
| Talent search + weight tune + cold-start→blended threshold crossing | Recruiter (hiring-manager identity) | `e2e/specs/talent_recommendations.spec.ts` |
| Admin retention + monitoring + mTLS pin lifecycle UI | Administrator | `e2e/specs/admin_ops.spec.ts` |
| Offline placeholders + stale-cache indicator | Regular User | `e2e/specs/offline_states.spec.ts` |

## Coverage Summary

- Total endpoints (planned): **77**
- Endpoints with no-mock HTTP tests (target): **77** (enforced by Gate 3)
- Gate 1 floor: backend+shared ≥ **90%** (measured; enforced)
- Gate 2 floor: frontend ≥ **80%** (measured; enforced)
- Playwright: 7 specs required to pass (flow evidence only)

## Major Gaps To Close During Implementation

1. `time_shim` utility (P1) for retention + alert-duration + notification-retry tests.
2. mTLS integration harness: ephemeral CA, two client certs, switchable `enforced` flag (P1, used in P4).
3. Ephemeral Postgres spin-up inside `./run_tests.sh` via Compose `tests` profile, with schema-per-suite isolation.
4. `scripts/audit_endpoints.sh` — parses `docs/api-spec.md` + scans `tests/http/**` for `t_<id>_*` functions.
5. Network egress fence in `Dockerfile.tests` (no non-loopback routes) to prove R40.
6. `grcov` WASM coverage wiring (P1 infra; used P2+).
