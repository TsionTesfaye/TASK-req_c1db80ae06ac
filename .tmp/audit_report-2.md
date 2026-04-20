# TerraOps Static Delivery Acceptance & Architecture Audit

## 1. Verdict
- Overall conclusion: **Partial Pass**

## 2. Scope and Static Verification Boundary
- Reviewed (static only): architecture/docs/contracts, backend route/middleware/auth/authz/data modules, key migrations, frontend routing/state/API client/SW/caching surfaces, and test suites/contracts (`README.md`, `docs/api-spec.md`, `docs/design.md`, `docs/test-coverage.md`, `crates/backend/src/**`, `crates/frontend/src/**`, `crates/backend/tests/**`).
- Not reviewed: runtime behavior on a live server, browser rendering outcomes, Docker/network/container behavior, actual DB execution paths under load.
- Intentionally not executed: project startup, Docker, tests, external services (per audit boundary).
- Claims requiring manual verification:
  - End-to-end UX correctness/performance under real workloads.
  - TLS/mTLS live handshake behavior in deployment environment.
  - Visual fidelity/responsiveness in real browsers.

## 3. Repository / Requirement Mapping Summary
- Prompt core goal mapped: offline single-station portal combining environmental KPI intelligence, product master-data governance, and talent matching with role-based workspaces and local-first security/ops controls.
- Core flows mapped to code: auth/session/RBAC (`crates/backend/src/handlers/auth.rs:44`, `crates/backend/src/middleware/authn.rs:72`), products/imports/history/images (`crates/backend/src/products/handlers.rs:24`), env+metrics+lineage (`crates/backend/src/metrics_env/handlers.rs:27`), KPI/alerts/reports (`crates/backend/src/kpi/handlers.rs:24`, `crates/backend/src/alerts/handlers.rs:29`, `crates/backend/src/reports/handlers.rs:26`), talent/recommendations/watchlists/feedback (`crates/backend/src/talent/handlers.rs:313`, `crates/backend/src/talent/watchlists.rs:31`), notifications/retention/monitoring/security (`crates/backend/src/handlers/notifications.rs:33`, `crates/backend/src/handlers/retention.rs:28`, `crates/backend/src/handlers/monitoring.rs:239`, `crates/backend/src/handlers/security.rs:31`).
- Major constraints mapped: page defaults 50 (`crates/shared/src/pagination.rs:5`), 3s backend budget (`crates/backend/src/middleware/budget.rs:23`), GET retry policy in frontend (`crates/frontend/src/api.rs:52`), 15-minute access tokens (`crates/backend/src/crypto/jwt.rs:12`), TZ-aware persistence in migrations (`crates/backend/migrations/0001_identity.sql:20`), MM/DD/YYYY 12-hour display helper (`crates/shared/src/time.rs:15`).

## 4. Section-by-section Review

### 4.1 Hard Gates

#### 4.1.1 Documentation and static verifiability
- Conclusion: **Partial Pass**
- Rationale: documentation is extensive and maps to code structure, but there are material contract inconsistencies that reduce static trustworthiness.
- Evidence:
  - Startup/test/config guidance exists: `README.md:89`, `README.md:140`, `README.md:146`.
  - Route inventory + auth classes documented: `docs/api-spec.md:31`.
  - P13 contract contradictory inside authoritative docs/tests/handler:
    - `docs/api-spec.md:23` (says signed endpoints classified AUTH),
    - `docs/api-spec.md:120` (P13 marked SIGNED no bearer),
    - `docs/api-spec.md:243` (P13 says AUTH + signature),
    - `crates/backend/src/products/images.rs:141` (public signed handler),
    - `crates/backend/tests/parity_tests.rs:211` (expects 401 unauth).
- Manual verification note: validate single canonical P13 contract and align docs/tests/code before runtime acceptance.

#### 4.1.2 Material deviation from Prompt
- Conclusion: **Partial Pass**
- Rationale: implementation is largely aligned to prompt scope, but at least one core semantic deviates (cold-start scope) and one cross-cutting behavior bypasses designed notification semantics.
- Evidence:
  - Scope alignment across domains/routes: `crates/backend/src/handlers/mod.rs:24`, `crates/frontend/src/router.rs:47`.
  - Cold-start rule in design: `docs/design.md:192`; implementation uses global count: `crates/backend/src/talent/handlers.rs:325`, `crates/backend/src/talent/feedback.rs:60`.
  - Notification architecture says shared emitter path: `crates/backend/src/services/notifications.rs:3`; bypassed by direct inserts in core producers (`crates/backend/src/alerts/evaluator.rs:203`, `crates/backend/src/reports/scheduler.rs:367`).

### 4.2 Delivery Completeness

#### 4.2.1 Coverage of explicitly stated core functional requirements
- Conclusion: **Partial Pass**
- Rationale: most explicit prompt requirements are implemented statically (role workspaces, KPI/env/lineage, product governance, talent matching, local notifications, retention/security), but key defects affect requirement semantics.
- Evidence:
  - Five role names preserved in shared contract: `crates/shared/src/roles.rs:1`.
  - Product master + history/import/images routes: `crates/backend/src/products/handlers.rs:27`.
  - Env metrics + lineage route: `crates/backend/src/metrics_env/handlers.rs:45`.
  - KPI slice/drill surfaces: `crates/backend/src/kpi/handlers.rs:46`, `crates/backend/src/kpi/handlers.rs:228`.
  - Talent recommendations/watchlists/feedback: `crates/backend/src/talent/handlers.rs:313`, `crates/backend/src/talent/watchlists.rs:68`.
  - Retention/security/monitoring/notification endpoints: `crates/backend/src/handlers/security.rs:31`, `crates/backend/src/handlers/retention.rs:28`, `crates/backend/src/handlers/monitoring.rs:340`, `crates/backend/src/handlers/notifications.rs:33`.
  - Defects impacting requirement fidelity: cold-start scope mismatch (`docs/design.md:192` vs `crates/backend/src/talent/handlers.rs:325`) and notification subscription bypass (`crates/backend/src/services/notifications.rs:21` vs `crates/backend/src/alerts/evaluator.rs:203`).

#### 4.2.2 Basic end-to-end deliverable vs partial/demo fragment
- Conclusion: **Pass**
- Rationale: repository contains full multi-crate structure, migrations, frontend/backend integration surfaces, and non-trivial test inventory; not a single-file demo.
- Evidence: `README.md:318`, `crates/backend/migrations/0001_identity.sql:13`, `crates/backend/src/app.rs:100`, `crates/frontend/src/router.rs:14`, `run_tests.sh:84`.

### 4.3 Engineering and Architecture Quality

#### 4.3.1 Engineering structure and module decomposition
- Conclusion: **Pass**
- Rationale: separation by domain modules and middleware/extractor layers is clear and maintainable.
- Evidence: module mount map `crates/backend/src/handlers/mod.rs:15`; middleware chain `crates/backend/src/app.rs:105`; frontend routing/page split `crates/frontend/src/router.rs:98`.

#### 4.3.2 Maintainability and extensibility
- Conclusion: **Partial Pass**
- Rationale: codebase is generally extensible, but architectural drift (notification emitter bypass) introduces coupling and inconsistent behavior.
- Evidence:
  - Intended single emitter abstraction: `crates/backend/src/services/notifications.rs:3`.
  - Bypass sites: `crates/backend/src/alerts/evaluator.rs:203`, `crates/backend/src/reports/scheduler.rs:367`.

### 4.4 Engineering Details and Professionalism

#### 4.4.1 Error handling, logging, validation, API design
- Conclusion: **Partial Pass**
- Rationale: strong baseline exists (unified error envelope, request-id middleware, validation, pagination headers), but there are material contract drifts and one security-control drift.
- Evidence:
  - Unified error envelope + internal logging: `crates/backend/src/errors.rs:68`.
  - Request-id propagation/rewriting: `crates/backend/src/middleware/request_id.rs:32`, `crates/backend/src/middleware/request_id.rs:125`.
  - Crash redaction/limits: `crates/backend/src/handlers/monitoring.rs:114`, `crates/backend/src/handlers/monitoring.rs:141`.
  - Pagination defaults and clamping: `crates/shared/src/pagination.rs:5`, headers in handlers e.g. `crates/backend/src/kpi/handlers.rs:85`.
  - Drift: mutation header contract documented but not enforced/attached (`docs/api-spec.md:11`, `crates/frontend/src/api.rs:164`).

#### 4.4.2 Product-like service vs demo-level implementation
- Conclusion: **Pass**
- Rationale: includes real persistence, background jobs, RBAC/security modules, and broad route/test contracts.
- Evidence: jobs startup `crates/backend/src/jobs/mod.rs:29`; security endpoints `crates/backend/src/handlers/security.rs:31`; test suites listed in `run_tests.sh:84`.

### 4.5 Prompt Understanding and Requirement Fit

#### 4.5.1 Business goal and constraint fit
- Conclusion: **Partial Pass**
- Rationale: most business objectives and constraints are directly represented, but two important semantic mismatches remain (cold-start scoping and notification subscription/retry pathway consistency).
- Evidence:
  - Offline/local/TLS/session constraints reflected in docs+config: `README.md:9`, `crates/backend/src/config.rs:45`, `crates/backend/src/crypto/jwt.rs:12`.
  - Semantic mismatch evidence: `docs/design.md:192` vs `crates/backend/src/talent/handlers.rs:325`; `crates/backend/src/services/notifications.rs:21` vs `crates/backend/src/alerts/evaluator.rs:203`.

### 4.6 Aesthetics (frontend)

#### 4.6.1 Visual and interaction quality fit
- Conclusion: **Cannot Confirm Statistically**
- Rationale: static code shows structured nav/layout classes and interaction states, but visual quality/responsiveness/render fidelity cannot be proven without running in browser.
- Evidence:
  - Layout/nav + badge/toast interactions: `crates/frontend/src/components.rs:53`, `crates/frontend/src/components.rs:109`, `crates/frontend/src/components.rs:223`.
  - Route-level page partitioning: `crates/frontend/src/router.rs:27`.
  - Tailwind + SW registration present: `crates/frontend/index.html:10`, `crates/frontend/index.html:18`.
- Manual verification note: browser-based review required for hierarchy, spacing consistency, responsive behavior, and interaction feedback quality.

## 5. Issues / Suggestions (Severity-Rated)

### High

1) Severity: **High**
- Title: Notification producer paths bypass shared `emit` contract
- Conclusion: **Fail**
- Evidence: `crates/backend/src/services/notifications.rs:3`, `crates/backend/src/alerts/evaluator.rs:203`, `crates/backend/src/reports/scheduler.rs:367`
- Impact: subscription opt-out and delivery-attempt semantics can be skipped by major producers, causing inconsistent user-notification behavior and weakening retry/state observability guarantees.
- Minimum actionable fix: refactor alert/report producers to call `services::notifications::emit(...)` (or enforce equivalent subscription + attempt creation in one shared API).

2) Severity: **High**
- Title: Cold-start threshold uses global feedback count instead of scoped count
- Conclusion: **Fail**
- Evidence: `docs/design.md:192`, `crates/backend/src/talent/handlers.rs:325`, `crates/backend/src/talent/feedback.rs:60`, `crates/backend/tests/talent_recommend_tests.rs:187`
- Impact: recommendation mode can transition out of cold-start due to unrelated users/roles, violating expected personalization semantics.
- Minimum actionable fix: replace global `count_total` with scoped count by user+role scope, and add tests proving isolation of cold-start transitions.

3) Severity: **High**
- Title: P13 signed-image auth contract is internally contradictory
- Conclusion: **Fail**
- Evidence: `docs/api-spec.md:23`, `docs/api-spec.md:120`, `docs/api-spec.md:243`, `crates/backend/src/products/images.rs:141`, `crates/backend/tests/parity_tests.rs:211`
- Impact: authoritative contract cannot be trusted; parity/audit gate can assert incompatible expectations for the same endpoint.
- Minimum actionable fix: choose one canonical P13 contract (capability URL vs bearer-required), then align `docs/api-spec.md`, handler comments/logic, and parity tests.

### Medium

4) Severity: **Medium**
- Title: Mutation-request header contract (`X-Requested-With`) is documented but not implemented
- Conclusion: **Fail**
- Evidence: `docs/api-spec.md:11`, `docs/design.md:266`, `crates/frontend/src/api.rs:164`
- Impact: security/CSRF-style control claimed by contract is absent in client/server behavior.
- Minimum actionable fix: either enforce + emit `X-Requested-With: terraops` on mutation routes and SPA client, or remove/replace the documented requirement.

### Low

5) Severity: **Low**
- Title: Request-budget comment contradicts actual middleware scope
- Conclusion: **Partial Fail**
- Evidence: `crates/backend/src/middleware/budget.rs:5`, `crates/backend/src/app.rs:106`, `crates/backend/src/middleware/budget.rs:60`
- Impact: operational confusion for maintainers; docs/comments imply health/ready exemption that is not visible in middleware implementation.
- Minimum actionable fix: either implement explicit path exemption for `/api/v1/health` and `/api/v1/ready` or update comments to match behavior.

## 6. Security Review Summary

- Authentication entry points: **Pass**
  - Evidence: login/refresh/logout/me/change-password mounted and implemented with refresh-cookie rotation and JWT/session checks (`crates/backend/src/handlers/auth.rs:44`, `crates/backend/src/handlers/auth.rs:147`, `crates/backend/src/auth/sessions.rs:83`, `crates/backend/src/middleware/authn.rs:76`).

- Route-level authorization: **Pass**
  - Evidence: explicit `require_permission` / `require_any_permission` checks in handlers (`crates/backend/src/auth/extractors.rs:99`, `crates/backend/src/reports/handlers.rs:93`, `crates/backend/src/handlers/security.rs:47`, `crates/backend/src/talent/handlers.rs:321`).

- Object-level authorization: **Pass**
  - Evidence: owner guards/self-scoped filtering for reports/watchlists/notifications (`crates/backend/src/reports/handlers.rs:177`, `crates/backend/src/talent/watchlists.rs:68`, `crates/backend/src/handlers/notifications.rs:66`).

- Function-level authorization: **Partial Pass**
  - Evidence: permission checks present broadly (`crates/backend/src/handlers/security.rs:220`, `crates/backend/src/products/handlers.rs:123`), but contract drift exists for signed image access semantics (`crates/backend/src/products/images.rs:141`, `crates/backend/tests/parity_tests.rs:211`).

- Tenant / user data isolation: **Partial Pass**
  - Evidence: many self-scoped queries and owner filters (`crates/backend/src/handlers/notifications.rs:66`, `crates/backend/src/talent/watchlists.rs:38`, `crates/backend/src/reports/handlers.rs:98`), but recommendation cold-start uses global feedback count (`crates/backend/src/talent/handlers.rs:325`) reducing per-user isolation semantics in scoring mode selection.

- Admin / internal / debug endpoint protection: **Pass**
  - Evidence: security/retention/monitoring/admin routes gated by admin permissions (`crates/backend/src/handlers/security.rs:47`, `crates/backend/src/handlers/retention.rs:57`, `crates/backend/src/handlers/monitoring.rs:345`).

## 7. Tests and Logging Review

- Unit tests: **Pass (static evidence)**
  - Evidence: unit tests present in core modules (e.g., JWT/time/redaction): `crates/backend/src/crypto/jwt.rs:53`, `crates/shared/src/time.rs:18`, `crates/backend/src/handlers/monitoring.rs:292`.

- API / integration tests: **Partial Pass (static evidence)**
  - Evidence: broad HTTP/integration/deep suites listed and present (`run_tests.sh:84`, `crates/backend/tests/http_p1.rs:1`, `crates/backend/tests/integration_tests.rs:1`, `crates/backend/tests/deep_jobs_scheduler_tests.rs:1`).
  - Gap: at least one contract inconsistency exists between tests and handler/docs for P13 (`crates/backend/tests/parity_tests.rs:211`, `crates/backend/src/products/images.rs:141`, `docs/api-spec.md:120`).

- Logging categories / observability: **Pass**
  - Evidence: tracing across app/jobs/security and request-id propagation (`crates/backend/src/main.rs:31`, `crates/backend/src/jobs/mod.rs:54`, `crates/backend/src/middleware/request_id.rs:32`).

- Sensitive-data leakage risk in logs / responses: **Partial Pass**
  - Evidence: crash ingest redaction + limits and `/auth/me` email masking (`crates/backend/src/handlers/monitoring.rs:141`, `crates/backend/src/handlers/monitoring.rs:246`, `crates/backend/src/handlers/auth.rs:224`).
  - Residual static risk: no centralized global log-scrubber layer found for arbitrary `tracing` fields outside guarded pathways; manual verification recommended for runtime log content policy.

## 8. Test Coverage Assessment (Static Audit)

### 8.1 Test Overview
- Unit tests exist: yes (`crates/backend/src/**` and `crates/shared/src/**` modules include `#[cfg(test)]`).
- API/integration tests exist: yes (`crates/backend/tests/http_p1.rs`, `parity_tests.rs`, talent suites, deep suites, integration suites).
- Frameworks: Rust `cargo test`, `actix_web::test`, `wasm-bindgen-test`, Playwright references in docs/scripts.
- Test entry points documented: yes (`README.md:140`, `README.md:146`, `run_tests.sh:84`).
- Test commands documented: yes (`README.md:143`, `README.md:192`, `README.md:205`).
- Static boundary: execution was not performed in this audit.

### 8.2 Coverage Mapping Table

| Requirement / Risk Point | Mapped Test Case(s) | Key Assertion / Fixture / Mock | Coverage Assessment | Gap | Minimum Test Addition |
|---|---|---|---|---|---|
| Auth login/refresh/logout/me contracts | `crates/backend/tests/http_p1.rs:63`, `:143`, `:186`, `:229` | cookie presence, 401 on bad creds, refresh rotation, `/me` masking | sufficient | none major statically | add lockout-boundary regression with exact 10/15-min semantics |
| 401/403 boundaries on protected endpoints | `crates/backend/tests/parity_tests.rs:40`, `crates/backend/tests/http_p1.rs:322` | unauthorized/forbidden role probes per endpoint ID | basically covered | P13 expectation drift | align P13 tests with canonical contract + add explicit signed-capability tests in parity suite |
| Object-level authorization (SELF) watchlists/notifications/reports | `crates/backend/tests/talent_watchlist_tests.rs:28`, `:132`, `crates/backend/tests/http_p1.rs:1305`, `crates/backend/tests/deep_alerts_reports_tests.rs:408` | cross-user access denied (403/404), owner-only filters | sufficient | none major statically | add cross-role owner tests after permission revocation for all SELF routes |
| Pagination defaults and response totals | `crates/shared/src/pagination.rs:54`, endpoint tests using `page/page_size` (e.g. `crates/backend/tests/deep_admin_surface_tests.rs:346`) | default/clamp assertions and paged endpoint use | basically covered | inconsistent direct assertion of default=50 across all list handlers | add integration tests asserting omitted pagination returns `page_size=50` + `X-Total-Count` per key list endpoints |
| 3s budget + idempotent GET retry contract | Frontend FVM rows `docs/test-coverage.md:177`, `:190`; backend budget suite listed `run_tests.sh:86` | timeout/select_timeout and constants evidence | basically covered | static matrix evidence only for frontend; no execution in this audit | add backend HTTP test proving 504 envelope with request_id on forced slow handler |
| Signed image URL security path | `crates/backend/tests/integration_tests.rs:854`, `:933` | forged/expired/tampered signature checks | insufficient | contract inconsistency vs parity/docs remains | add one canonical test set covering chosen auth mode + docs parity check |
| Notification subscriptions + retry queue | `crates/backend/tests/integration_tests.rs:766`, `:719` | `emit` respects opt-out; retry advancement | insufficient | producer bypass paths are untested (`alerts/reports` direct inserts) | add tests asserting alert/report emission respects subscription opt-out + delivery-attempt semantics |
| Cold-start recommendation threshold semantics | `crates/backend/tests/talent_recommend_tests.rs:187` | transition after 10 feedback records | insufficient | test encodes global-count behavior, not scoped requirement | add tests for per-user/per-role scoped feedback counts and transition boundaries |
| Retention policy behavior | `crates/backend/tests/integration_tests.rs:49`, `:128`, `:170`, `:245`, `:300` | domain-specific deletes + inactive-user semantics | sufficient | none major statically | add DST/timezone edge-case retention cutoff tests |
| Allowlist + mTLS admin/security | `crates/backend/tests/integration_tests.rs:809`, `crates/backend/tests/http_p1.rs:831`, `crates/backend/tests/mtls_handshake_tests.rs:185` | 403 on unmatched IP, mtls status/patch, handshake refusal/success | basically covered | restart-gated behavior should be explicitly checked in one integration assertion set | add explicit test combining `/security/mtls` patch + no-live-flip until restart contract |

### 8.3 Security Coverage Audit
- Authentication: **Basically covered** (broad tests for A1-A5 exist, static evidence strong).
- Route authorization: **Basically covered** (parity + deep suites include many 403 checks).
- Object-level authorization: **Basically covered** (watchlist/notification/report owner-scope tests present).
- Tenant / data isolation: **Insufficient** (cold-start scope semantics untested for per-user/role isolation and currently appear mismatched).
- Admin / internal protection: **Basically covered** (security/monitoring/retention tests exist), but runtime-only aspects still need manual verification.

### 8.4 Final Coverage Judgment
**Partial Pass**

Major authn/authz/object-scope, retention, allowlist, and many endpoint behaviors are statically well-covered by tests. However, uncovered/misaligned high-risk areas remain: notification producer bypass paths are not coverage-enforced, cold-start scope tests encode likely-wrong semantics, and P13 contract inconsistency means tests could pass while significant access-control contract defects still remain.

## 9. Final Notes
- This audit is static-only; no runtime success is asserted.
- Repository is substantial and close to prompt intent, but high-severity contract/architecture mismatches should be resolved before full acceptance.
- Recommended acceptance state: **Partial Pass with mandatory remediation of High issues above**.
