# TerraOps Static Audit

## 1. Verdict

- Overall conclusion: Partial Pass

## 2. Scope and Static Verification Boundary

- Reviewed: `README.md`, `docker-compose.yml`, `run_tests.sh`, `Dockerfile.tests`, backend route registration and core modules under `crates/backend/src/**`, frontend router/pages/API/state under `crates/frontend/src/**`, shared DTOs/permissions/roles under `crates/shared/src/**`, SQL migrations, and representative backend/frontend tests.
- Not reviewed exhaustively: every frontend component branch and every individual SQL statement in large files.
- Intentionally not executed: project startup, Docker, tests, Playwright, browser UI, HTTP requests, database migrations, TLS handshakes.
- Claims requiring manual verification: runtime startup, real TLS behavior in deployment, actual Yew rendering/layout fidelity, service worker behavior in browser, report artifact generation, scheduler timing, and any flow that depends on background jobs or browser interaction.

## 3. Repository / Requirement Mapping Summary

- Prompt core goal: deliver an offline local portal combining environmental intelligence, product master-data governance, and talent matching, with a Yew UI and Actix/Postgres backend, RBAC, local auth, TLS, signed image URLs, retention policies, notifications, offline caching, KPI/reporting, and recruiter recommendation logic.
- Main implementation areas mapped: auth/RBAC/session/TLS (`auth`, `middleware`, `tls`, `security`), product/import/export/history/images (`products/**`), environmental metrics/KPI/alerts/reports (`metrics_env/**`, `kpi/**`, `alerts/**`, `reports/**`, `jobs/**`), talent search/recommendations/watchlists/feedback (`talent/**`), frontend route/page coverage (`router.rs`, `pages.rs`), and static verification/test contracts (`README.md`, `docs/test-coverage.md`, backend/frontend tests).

## 4. Section-by-section Review

### 1. Hard Gates

#### 1.1 Documentation and static verifiability
- Conclusion: Partial Pass
- Rationale: The repo has strong static artifacts for startup/testing and a traceable route/module structure, but core docs contradict the current codebase in material places, which weakens reviewer trust.
- Evidence: `README.md:85`, `README.md:134`, `crates/backend/src/handlers/mod.rs:15`, `crates/frontend/src/router.rs:47`, `README.md:73`, `Dockerfile.tests:24`, `run_tests.sh:37`
- Manual verification note: Runtime commands are documented, but correctness of those commands was not executed.

#### 1.2 Material deviation from the Prompt
- Conclusion: Partial Pass
- Rationale: The repository is clearly centered on the TerraOps prompt and implements the major domains, but some update/edit contracts are incomplete in ways that directly affect prompt-required master-data and environmental workflows.
- Evidence: `crates/backend/src/handlers/mod.rs:24`, `crates/frontend/src/router.rs:116`, `crates/backend/src/metrics_env/handlers.rs:104`, `crates/shared/src/dto/product.rs:92`

### 2. Delivery Completeness

#### 2.1 Core prompt coverage
- Conclusion: Partial Pass
- Rationale: Major backend and frontend surfaces exist for products, metrics/KPI/alerts/reports, talent, admin/security, notifications, and monitoring, but at least two edit paths are materially incomplete: environmental source reassignment and product optional-field clearing.
- Evidence: `crates/frontend/src/router.rs:116`, `crates/frontend/src/pages.rs:1490`, `crates/frontend/src/pages.rs:2276`, `crates/frontend/src/pages.rs:4363`, `crates/backend/src/metrics_env/handlers.rs:109`, `crates/backend/src/products/handlers.rs:215`, `crates/shared/src/dto/product.rs:92`

#### 2.2 0-to-1 deliverable vs partial/demo
- Conclusion: Pass
- Rationale: This is a full workspace with backend, frontend, migrations, Docker/test scripts, shared DTOs, and substantial integration tests. Demo seed data exists, but the product is not merely a demo scaffold.
- Evidence: `Cargo.toml:1`, `README.md:9`, `docker-compose.yml:1`, `crates/backend/src/main.rs:1`, `crates/frontend/src/main.rs:1`, `crates/backend/tests/http_p1.rs:1`

### 3. Engineering and Architecture Quality

#### 3.1 Structure and module decomposition
- Conclusion: Pass
- Rationale: The codebase is decomposed by domain with central route registration, config, shared DTOs, middleware, services, and migrations. Responsibilities are traceable.
- Evidence: `crates/backend/src/handlers/mod.rs:15`, `crates/backend/src/app.rs:24`, `crates/backend/src/config.rs:1`, `crates/frontend/src/router.rs:14`, `crates/shared/src/dto/mod.rs:1`

#### 3.2 Maintainability and extensibility
- Conclusion: Partial Pass
- Rationale: Overall structure is maintainable, but several update paths have brittle request modeling or incomplete handler wiring, which will make future product maintenance error-prone.
- Evidence: `crates/shared/src/dto/product.rs:92`, `crates/backend/src/products/handlers.rs:219`, `crates/backend/src/metrics_env/handlers.rs:109`, `crates/backend/src/reports/handlers.rs:157`

### 4. Engineering Details and Professionalism

#### 4.1 Error handling, logging, validation, API design
- Conclusion: Partial Pass
- Rationale: The project has a normalized error model, request IDs, middleware budgets, and meaningful logging, but some important input validation is missing, causing DB constraint failures to surface as generic 500s.
- Evidence: `crates/backend/src/errors.rs:97`, `crates/backend/src/middleware/request_id.rs:8`, `crates/backend/src/middleware/budget.rs:1`, `crates/backend/src/products/handlers.rs:133`, `crates/backend/migrations/0010_products.sql:15`, `crates/backend/src/errors.rs:77`

#### 4.2 Real product/service shape vs demo sample
- Conclusion: Pass
- Rationale: The repo looks like a real product: typed DTOs, RBAC, persistence, background jobs, admin/security surfaces, offline caching, notifications, and no-mock HTTP tests.
- Evidence: `crates/backend/src/jobs/mod.rs:1`, `crates/backend/src/handlers/security.rs:1`, `crates/backend/src/handlers/notifications.rs:1`, `crates/frontend/static/sw.js:1`, `crates/backend/tests/integration_tests.rs:1`

### 5. Prompt Understanding and Requirement Fit

#### 5.1 Business goal and implicit constraints fit
- Conclusion: Partial Pass
- Rationale: The implementation understands the prompt well and maps the required actors and domains, but some business-critical edit behaviors do not fully support the governance workflows described in the prompt.
- Evidence: `crates/shared/src/roles.rs:1`, `crates/shared/src/permissions.rs:1`, `crates/frontend/src/pages.rs:156`, `crates/backend/src/talent/scoring.rs:8`, `crates/backend/src/metrics_env/handlers.rs:229`, `crates/backend/src/metrics_env/handlers.rs:109`

### 6. Aesthetics (frontend)

#### 6.1 Visual and interaction quality
- Conclusion: Cannot Confirm Statistically
- Rationale: Static code shows distinct role-specific pages, layout wrappers, tables, pagers, and offline states, but rendered visual quality, responsiveness, spacing, and interaction feedback require browser execution.
- Evidence: `crates/frontend/src/router.rs:98`, `crates/frontend/src/pages.rs:393`, `crates/frontend/src/pages.rs:1490`, `crates/frontend/src/pages.rs:2276`, `crates/frontend/src/pages.rs:4363`
- Manual verification note: Open the real UI and verify desktop/mobile rendering, spacing, hover/focus states, and actual service-worker/offline behavior.

## 5. Issues / Suggestions (Severity-Rated)

### High

#### 1. Environmental source PATCH silently ignores site/department/unit reassignment
- Severity: High
- Conclusion: Fail
- Evidence: `crates/shared/src/dto/env_source.rs:33`, `crates/backend/src/metrics_env/handlers.rs:104`, `crates/backend/src/metrics_env/handlers.rs:109`, `crates/backend/src/metrics_env/sources.rs:92`
- Impact: Analysts cannot actually reassign a source's site, department, or unit after creation even though the API contract exposes those fields. That breaks prompt-required slicing/alignment maintenance and creates a misleading API surface.
- Minimum actionable fix: Thread `site_id`, `department_id`, and `unit_id` from `UpdateEnvSourceRequest` into `sources::update`, with explicit semantics for unchanged vs cleared values, and add HTTP tests covering reassignment and clearing.

#### 2. Product PATCH cannot clear optional master-data fields
- Severity: High
- Conclusion: Fail
- Evidence: `crates/shared/src/dto/product.rs:92`, `crates/backend/src/products/handlers.rs:219`, `crates/backend/src/products/handlers.rs:230`, `crates/backend/src/products/repo.rs:380`
- Impact: Optional governance fields such as `spu`, `barcode`, `category_id`, `site_id`, and `department_id` cannot be explicitly cleared through the published PATCH contract because the DTO uses plain `Option<T>` while the repository expects tri-state `Option<Option<T>>`. This leaves stale master data stuck once set.
- Minimum actionable fix: Replace `UpdateProductRequest` optional fields that support clearing with a tri-state representation and add end-to-end tests proving both partial updates and explicit null-clears.

### Medium

#### 3. Invalid negative product price is not validated and will surface as a generic 500
- Severity: Medium
- Conclusion: Fail
- Evidence: `crates/backend/src/products/handlers.rs:133`, `crates/backend/src/products/handlers.rs:209`, `crates/backend/migrations/0010_products.sql:15`, `crates/backend/src/errors.rs:77`
- Impact: `price_cents < 0` is blocked only by the DB constraint, and non-unique SQL errors are normalized to `Internal("database error")`. Users therefore get a server error instead of a user-safe validation error for a basic boundary condition.
- Minimum actionable fix: Validate `price_cents >= 0` in create/update handlers before DB write and add HTTP tests for negative-price rejection.

#### 4. Documentation contradicts the current frontend and test-delivery state
- Severity: Medium
- Conclusion: Fail
- Evidence: `README.md:73`, `crates/frontend/src/router.rs:116`, `crates/frontend/src/pages.rs:1490`, `Dockerfile.tests:24`, `run_tests.sh:37`
- Impact: Static reviewers are told the P-A/P-B/P-C frontend is still remaining work and that Playwright is deferred, while the codebase actually contains those frontend surfaces and an active Playwright flow gate. This undermines documentation trust during acceptance.
- Minimum actionable fix: Update `README.md` and `Dockerfile.tests` to match the current delivered frontend surface and active flow-gate contract.

#### 5. Report self-service endpoints rely on ownership without re-checking report permissions
- Severity: Medium
- Conclusion: Partial Fail
- Evidence: `crates/backend/src/reports/handlers.rs:157`, `crates/backend/src/reports/handlers.rs:170`, `crates/backend/src/reports/handlers.rs:200`, `crates/backend/src/reports/handlers.rs:224`, `crates/backend/tests/deep_alerts_reports_tests.rs:407`
- Impact: A user who created a report job while permitted can still get, run, cancel, and download that job later based only on ownership if their role/permissions are reduced. Route-level authorization is therefore weaker than the rest of the report surface.
- Minimum actionable fix: Require `report.run` on RP3-RP6 in addition to the owner guard, and add a test where the owner loses report permissions before retrying those routes.

## 6. Security Review Summary

- Authentication entry points: Pass
  Evidence: `crates/backend/src/handlers/auth.rs:39`, `crates/backend/src/auth/password.rs:33`, `crates/backend/src/auth/sessions.rs:52`, `crates/backend/src/crypto/jwt.rs:12`
  Reasoning: Local username/password auth, Argon2id hashing, opaque refresh tokens, session revocation, and 15-minute JWTs are implemented.

- Route-level authorization: Partial Pass
  Evidence: `crates/backend/src/handlers/users.rs:60`, `crates/backend/src/handlers/security.rs:47`, `crates/backend/src/metrics_env/handlers.rs:57`, `crates/backend/src/talent/handlers.rs:91`, `crates/backend/src/reports/handlers.rs:157`
  Reasoning: Most route families explicitly require permissions, but report RP3-RP6 rely only on ownership after authentication.

- Object-level authorization: Pass
  Evidence: `crates/backend/src/auth/extractors.rs:69`, `crates/backend/src/talent/watchlists.rs:66`, `crates/backend/src/handlers/notifications.rs:108`, `crates/backend/src/reports/handlers.rs:163`
  Reasoning: Notifications, watchlists, and reports all enforce self/owner scoping at the object level.

- Function-level authorization: Partial Pass
  Evidence: `crates/backend/src/auth/extractors.rs:98`, `crates/backend/src/talent/handlers.rs:91`, `crates/backend/src/reports/handlers.rs:157`
  Reasoning: Authorization helpers are used consistently, but some handlers still omit permission rechecks once ownership is established.

- Tenant / user isolation: Pass
  Evidence: `crates/backend/src/handlers/notifications.rs:64`, `crates/backend/src/talent/watchlists.rs:31`, `crates/backend/tests/talent_watchlist_tests.rs:28`, `crates/backend/tests/http_p1.rs:1219`
  Reasoning: The portal is single-node, but user-scoped data is separated by `user_id`/`owner_id` and backed by self-scope tests.

- Admin / internal / debug protection: Pass
  Evidence: `crates/backend/src/handlers/security.rs:46`, `crates/backend/src/handlers/monitoring.rs:33`, `crates/backend/src/handlers/retention.rs:37`, `crates/backend/tests/http_p1.rs:260`, `crates/backend/tests/deep_admin_surface_tests.rs:1`
  Reasoning: Admin/security/monitoring surfaces are permission-gated; no unauthenticated debug surface was found.

## 7. Tests and Logging Review

- Unit tests: Pass
  Evidence: `crates/backend/src/crypto/argon.rs:35`, `crates/backend/src/metrics_env/lineage.rs:95`, `crates/backend/src/handlers/monitoring.rs:292`, `crates/frontend/src/gate2_tests.rs:59`
  Reasoning: There are meaningful unit tests for crypto, parsing, redaction, DTO behavior, and frontend client/state logic.

- API / integration tests: Pass
  Evidence: `crates/backend/tests/http_p1.rs:1`, `crates/backend/tests/parity_tests.rs:1`, `crates/backend/tests/deep_products_tests.rs:1`, `crates/backend/tests/deep_metrics_kpi_tests.rs:1`, `crates/backend/tests/deep_alerts_reports_tests.rs:1`, `crates/backend/tests/integration_tests.rs:1`
  Reasoning: The repo has broad no-mock HTTP coverage plus deeper domain suites and transport-layer TLS tests.

- Logging categories / observability: Pass
  Evidence: `crates/backend/src/main.rs:31`, `crates/backend/src/middleware/metrics.rs:1`, `crates/backend/src/middleware/request_id.rs:8`, `crates/backend/src/jobs/mod.rs:209`, `crates/backend/src/reports/scheduler.rs:36`
  Reasoning: Structured JSON tracing, request IDs, API metrics, and background-job failure logging are present.

- Sensitive-data leakage risk in logs / responses: Partial Pass
  Evidence: `crates/backend/src/handlers/auth.rs:65`, `crates/backend/src/handlers/monitoring.rs:119`, `crates/backend/src/errors.rs:86`
  Reasoning: The code avoids returning plaintext emails broadly and redacts crash-report payloads, but generic `sqlx` error logging may still log raw DB error text and should be monitored carefully. Static review did not find obvious password/token response leaks.

## 8. Test Coverage Assessment (Static Audit)

### 8.1 Test Overview

- Unit tests exist in backend and frontend modules.
- API / integration tests exist under `crates/backend/tests/*.rs` and use Actix test services with a real Postgres harness.
- Frontend tests use `wasm-bindgen-test`; backend tests use Rust test/Actix test; Playwright specs exist under `e2e/specs/`.
- Documented broad test entry point: `./run_tests.sh`.
- Evidence: `run_tests.sh:1`, `README.md:136`, `crates/backend/tests/http_p1.rs:1`, `crates/frontend/src/gate2_tests.rs:1`, `docs/test-coverage.md:20`

### 8.2 Coverage Mapping Table

| Requirement / Risk Point | Mapped Test Case(s) | Key Assertion / Fixture / Mock | Coverage Assessment | Gap | Minimum Test Addition |
|---|---|---|---|---|---|
| Auth login/refresh/logout/change-password | `crates/backend/tests/http_p1.rs:62`, `crates/backend/tests/http_p1.rs:117`, `crates/backend/tests/http_p1.rs:159`, `crates/backend/tests/http_p1.rs:229` | Refresh cookie presence, unauthorized after logout, old token revoked after password change at `crates/backend/tests/http_p1.rs:80`, `crates/backend/tests/http_p1.rs:191`, `crates/backend/tests/http_p1.rs:250` | Sufficient | None material from static review | Add explicit failed-login lockout test if desired |
| Admin/user authorization and self-scope | `crates/backend/tests/http_p1.rs:322`, `crates/backend/tests/deep_admin_surface_tests.rs:244` | Self vs admin user read, self-delete blocked | Basically covered | Did not find explicit tests for every admin surface negative case in this pass | Add a compact allowlist/retention/monitoring negative-role matrix if higher assurance needed |
| Product CRUD/import/history | `crates/backend/tests/deep_products_tests.rs:24`, `crates/backend/tests/parity_tests.rs:63` | Full CRUD, list filters, tax/image/import flows | Basically covered | No test proves optional-field clearing on PATCH or negative-price validation | Add product PATCH null-clear test and negative `price_cents` validation test |
| Env source CRUD and observations | `crates/backend/tests/deep_metrics_kpi_tests.rs:22` | Source create/list/update/delete and bulk observations | Insufficient | Existing test updates only `name`/`kind`; no coverage for `site_id`/`department_id`/`unit_id` reassignment, which is exactly where the code is broken | Add PATCH tests for reassignment and explicit clear semantics |
| Metric definitions, lineage, KPI slicing | `crates/backend/tests/deep_metrics_kpi_tests.rs:145`, `crates/backend/tests/deep_metrics_kpi_tests.rs:332`, `crates/backend/tests/deep_metrics_kpi_tests.rs:352` | Create all formula kinds, fetch series/lineage, KPI summary/cycle/funnel/anomalies/efficiency/drill | Basically covered | Did not verify every slice combination or malformed filter input path statically | Add targeted malformed slice/filter tests |
| Alerts and report jobs | `crates/backend/tests/deep_alerts_reports_tests.rs:240`, `crates/backend/tests/deep_alerts_reports_tests.rs:343`, `crates/backend/tests/integration_tests.rs:404` | Report create/list/get/run/cancel/artifact, scheduler artifact + notification | Basically covered | No test covers owner who later loses `report.run` permission | Add deprivileged-owner authorization test for RP3-RP6 |
| Talent search/recommendations/watchlists/feedback | `crates/backend/tests/talent_search_tests.rs:16`, `crates/backend/tests/talent_recommend_tests.rs:1`, `crates/backend/tests/talent_watchlist_tests.rs:16`, `crates/backend/tests/talent_feedback_tests.rs:129`, `crates/backend/tests/talent_weights_tests.rs:133` | Auth/permission/self-scope checks, cold-start/blended ranking, owner_id from caller | Sufficient | No major gap found in static scan | Add role-degradation tests only if business policy requires it |
| TLS / allowlist / signed URLs | `crates/backend/tests/mtls_handshake_tests.rs:181`, `crates/backend/tests/integration_tests.rs:700` | Real TLS refusal/acceptance; signed URL negative paths and allowlist behavior in integration suite | Basically covered | Deployment-specific certificate handling still needs manual runtime verification | Add restart/reload pin-refresh test only if runtime confidence is needed |
| Frontend route/state/API logic | `crates/frontend/src/gate2_tests.rs:59`, `docs/test-coverage.md:168`, `e2e/specs/*.spec.ts` | Wasm tests for auth/API/router state; Playwright files listed in FVM | Basically covered | Gate 2 is explicitly not source-line coverage and does not prove full DOM behavior | Keep Playwright flow coverage broad; add DOM-level tests only if toolchain changes |

### 8.3 Security Coverage Audit

- Authentication: Covered meaningfully.
  Evidence: `crates/backend/tests/http_p1.rs:62`, `crates/backend/tests/http_p1.rs:117`, `crates/backend/tests/http_p1.rs:159`, `crates/backend/tests/http_p1.rs:229`
  Residual risk: Lockout and rate-limit paths were not fully reviewed in tests here.

- Route authorization: Basically covered.
  Evidence: `crates/backend/tests/parity_tests.rs:63`, `crates/backend/tests/talent_search_tests.rs:27`
  Residual risk: The report self-endpoint permission-gap could remain undetected because owner-isolation tests use still-authorized users.

- Object-level authorization: Covered meaningfully.
  Evidence: `crates/backend/tests/talent_watchlist_tests.rs:132`, `crates/backend/tests/deep_alerts_reports_tests.rs:407`, `crates/backend/tests/http_p1.rs:1219`
  Residual risk: None major found statically for scoped routes reviewed.

- Tenant / data isolation: Basically covered.
  Evidence: `crates/backend/tests/talent_watchlist_tests.rs:28`, `crates/backend/tests/http_p1.rs:1292`, `crates/backend/tests/http_p1.rs:1352`
  Residual risk: Single-node app has no true tenant model, so this is user-isolation coverage only.

- Admin / internal protection: Basically covered.
  Evidence: `crates/backend/tests/parity_tests.rs:85`, `crates/backend/tests/http_p1.rs:260`, `crates/backend/tests/deep_admin_surface_tests.rs:1`
  Residual risk: I did not build a full matrix for every admin route in this static pass.

### 8.4 Final Coverage Judgment

- Partial Pass

- Major risks covered: auth/session basics, broad RBAC gating, self-scope for notifications/watchlists/reports, product/env/talent happy-path flows, scheduler/notification integration, and transport-layer TLS tests.
- Major uncovered risks: environmental source reassignment, product PATCH null-clear semantics, negative-price validation, and report-permission behavior after role changes. Existing tests could still pass while those defects remain.

## 9. Final Notes

- The repository is substantially closer to an acceptance-ready product than a demo scaffold.
- The main blockers to a stronger acceptance verdict are not missing entire domains; they are specific contract breaches in update semantics and a small number of documentation/security consistency gaps.
- No runtime claim above should be read as execution proof; all conclusions are static and evidence-based.
