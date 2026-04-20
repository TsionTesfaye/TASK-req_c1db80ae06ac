# TerraOps — Offline Environmental & Catalog Intelligence Portal

## Project Type

**fullstack** (Rust backend + Rust/Yew frontend, PostgreSQL, delivered as a single-node offline Docker deployment).

## Overview

TerraOps is an offline, single-node operations portal for environmental
observations, catalog governance, and talent intelligence. It runs entirely
inside one Docker network, exposes one TLS origin on `https://localhost:8443`,
and has no outbound network dependencies at runtime.

This repository contains the **P1 shared foundations** (identity, RBAC,
sessions, admin security, retention, monitoring, reference data,
notifications, middleware stack, 52 no-mock integration tests) **plus**
the complete P1 frontend surface (Yew SPA shell, router, auth / toast /
notifications context, typed API client with 3-second timeout +
single-GET-retry, `PermGate`-aware nav, real admin / monitoring /
notifications pages), **plus the complete P-A Catalog & Governance,
P-B Environmental Intelligence / KPI / Alerts / Reports, and P-C Talent
Intelligence packages — both backend endpoints and the corresponding
Yew frontend role workspaces (data-steward catalog + imports, analyst
KPI / metrics / alerts / reports, recruiter talent workbench) are
delivered and live in `dist/`**, **plus P3 cross-domain integration
(background jobs) and P4 hardening coverage**:

- **P-A** — products P1–P14, imports I1–I7, CSV/XLSX streaming export,
  trigger-enforced immutable change history, signed image URLs.
- **P-B** — env sources + observations E1–E6, metric definitions + formula
  executor + lineage MD1–MD7, KPI K1–K6 (cycle time, funnel, anomalies,
  efficiency, drill), alert rules + events AL1–AL6 with a 30 s duration-aware
  evaluator, report jobs RP1–RP6 with PDF/CSV/XLSX output + one transient
  retry.
- **P-C** — candidates T1–T3 (tsvector search), open roles T4–T5,
  recommendations T6 (cold-start below 10 feedback datapoints → recency +
  completeness; blended thereafter with self-scoped weights), self-scoped
  weights T7–T8, scoped feedback T9, self-scoped watchlists T10–T13.
  Weights (T7/T8) and watchlists (T10–T13) require `talent.read` in
  addition to SELF-ownership — ordinary RegularUsers cannot read or
  mutate the recruiter-scoped talent surface.
- **P3 — Integration** — `crates/backend/src/jobs/` starts five tokio
  background loops from `app::run` at server boot: the 30-second alert
  evaluator, the 10-second report scheduler, an hourly retention sweep
  (env_raw, kpi, feedback), an hourly metric rollup (materialises
  `kpi_rollup_daily` from the last 36 hours of `metric_computations`),
  and the 30-second notification retry worker. Each loop logs-and-
  continues on failure so one bad cycle never kills the process.
- **P4 — Hardening** — real HTTP integration coverage for endpoint
  allowlist enforcement, signed-image-URL negative paths (missing params
  / forged signature / expired exp / valid sig with unknown row), error
  envelope + `email_ciphertext` redaction, notification subscription
  opt-out, retention sweep fan-out, metric-rollup idempotency,
  notification retry advancement, plus the existing alert-evaluator →
  notification-center and report-scheduler → artifact-plus-notification
  pipelines. See `crates/backend/tests/integration_tests.rs` (14
  passing).

The endpoint-parity audit (`scripts/audit_endpoints.sh`) runs in strict
mode and reports **forward parity 117/117 (100 %)** with 0 reverse orphans
(one `t_<id>_*` no-mock HTTP test per authoritative endpoint in
`docs/api-spec.md`). This 100 % is an endpoint-parity count
(endpoints-with-tests ÷ authoritative-endpoints × 100) and is
**unrelated** to the frontend gate's 100 %.

The frontend gate (Gate 2) separately reports a **Frontend Verification
Matrix score of 100 % (53/53 rows satisfied)**. This is a
**requirement-row verification score**
(`covered_rows / total_rows × 100`), **not a source-line coverage
percentage** and **not the same quantity** as the 117/117 endpoint
parity above. Wasm source-based line coverage is not the authoritative
frontend proof on this toolchain — see "Verification Method" below and
`docs/test-coverage.md §Why the frontend is not measured by wasm
source-based line coverage`.

The complete P-A/P-B/P-C frontend surfaces, P3 cross-domain integration,
and the P4/P5 hardening gate are all delivered and gated by
`./run_tests.sh`. The canonical execution checklist and remediation
history live in [`plan.md`](./plan.md).

## Tech Stack

- Backend: **Rust** + **Actix-web 4** + **Rustls 0.22** + **sqlx 0.7** (Postgres)
- Frontend: **Rust** + **Yew 0.21** + **Trunk** + **Tailwind CSS**
- Database: **PostgreSQL 16**
- Delivery: **Docker Compose** (one `app` service serves both SPA and REST API)
- Tests: `cargo llvm-cov` (Gate 1 native line coverage), `wasm-bindgen-test` + Frontend Verification Matrix (Gate 2), Playwright (Chromium) flow gate

## Startup Instructions

Canonical:

```bash
docker compose up --build
```

Legacy compatibility: `docker-compose up` also works on systems where the
legacy `docker-compose` binary is installed. The canonical `docker compose up --build`
remains the one required runtime command.

Convenience wrapper:

```bash
./run_app.sh
```

`run_app.sh` only wraps `docker compose up --build`; it adds nothing to
the host-side toolchain contract (Docker is the only host dependency).

First-boot secrets and TLS material are generated automatically by
`scripts/dev_bootstrap.sh` into the `terraops-runtime` Docker volume. There
are **no `.env` files** in this repo and none are required.

### Database initialization

```bash
./init_db.sh
```

Applies SQL migrations via the backend binary's `migrate` subcommand and
then runs `scripts/seed_demo.sh`.

## Access Method

After startup, open:

```
https://localhost:8443
```

The server presents a dev-only self-signed TLS certificate; your browser
will prompt to trust it on first visit. Health probe:

```
GET https://localhost:8443/api/v1/health  →  {"status":"ok"}
```

## Verification Method

Run the broad test gate from the repo root:

```bash
./run_tests.sh
```

`run_tests.sh` enforces (per [`docs/test-coverage.md`](docs/test-coverage.md)):

- **Gate 1** — backend cargo tests (terraops-shared + terraops-backend)
  via the `tests` Docker image, executed against a real Postgres. The
  no-mock integration suites (`http_p1.rs` = 52 tests, `parity_tests.rs`
  = 52 P-A/P-B endpoint tests, `talent_{search,recommend,weights,
  watchlist,feedback}_tests.rs` = 39 tests, `integration_tests.rs` = 14
  P3/P4 cross-domain + hardening tests, plus `mtls_handshake_tests.rs` =
  4 real transport-layer rustls tests — ~161 total) run serialized
  (`--test-threads=1`) because they reuse one DB. Line coverage is
  measured by `cargo llvm-cov` across the backend `--lib` unit tests
  **and** every backend test binary (so `#[cfg(test)]` modules inside
  `crates/backend/src/**` contribute too) and enforced against the
  planning-contract floor `GATE1_LINE_FLOOR=94` (currently measured at
  ~94.83% lines). The coverage scope
  excludes pure-IO boot modules (`main.rs`, `app.rs`, `tls.rs`, `spa.rs`,
  `config.rs`, `db.rs`, `seed.rs`, `storage/`, `models/`) which are
  exercised end-to-end by `docker compose up --build` rather than by
  cargo tests. `docs/test-coverage.md` documents the exact scope rule.
- **Gate 2 — Frontend Verification Matrix (FVM).**
  **Canonical result: `100% Frontend Verification Matrix score
  (53/53 rows satisfied)`.** This is the exact phrasing to use — never
  shorten it to "100% frontend coverage" or "100% frontend line
  coverage", because it is neither.

  **What this 100 % actually measures.** It is a **verification-matrix
  score** (`covered_rows / total_rows × 100`) over the 53-row matrix in
  [`docs/test-coverage.md`](./docs/test-coverage.md). Each row declares
  one grep-verifiable piece of evidence — a `#[wasm_bindgen_test]`
  function name, a Playwright spec file in `e2e/specs/`, or a `Route::`
  enum variant in `crates/frontend/src/router.rs` — and
  `scripts/frontend_verify.sh` mechanically verifies every row exists
  exactly as declared. 53 of 53 `covered` rows pass that check today.

  **What it is NOT.** It is **not** source-line coverage of
  `crates/frontend/src/**`. No statement-level wasm line count is
  enforced by this gate. The FVM is an evidence-checked frontend
  verification gate layered alongside the wasm-bindgen-test suite
  (Gate 2a, real test execution) and the Playwright flow gate (real
  end-to-end execution against the live `app` service), not a
  replacement for either of them. Wasm source-based line coverage is
  explicitly rejected as the authoritative frontend proof on this
  toolchain for the reasons recorded below under "Why the frontend is
  not measured by wasm source-based line coverage".

  Gate 2 runs in two halves:
  - **(2a)** frontend `wasm-bindgen-test` suite executed via
    `cargo test --target wasm32-unknown-unknown -p terraops-frontend`
    inside the `tests` image. Tests run in Node.js through
    `wasm-bindgen-test-runner` (no pinned Chromium required). The full
    P1 suite (43/43 passing) covers the `ApiClient` primitives
    (3-second timeout race, retry/budget constants, unified error-code
    → user-message mapping for every `ErrorCode` variant, Http/Decode
    variants, token attachment, clone, default, equality,
    ErrorEnvelope serde round-trip), `AuthState` / `AuthContext` role
    and permission helpers, `ToastLevel` class mapping, Toast
    equality, `NotificationsSnapshot` default + equality, and every
    `Route` variant family (admin, monitoring, data steward, analyst,
    talent).
  - **(2b)** `scripts/frontend_verify.sh` parses the 53-row **Frontend
    Verification Matrix** in [`docs/test-coverage.md`](./docs/test-coverage.md)
    and enforces `GATE2_FVM_FLOOR=90`. Current result: **100% Frontend
    Verification Matrix score (53/53 rows satisfied)** — a
    verification-matrix score, not a line-coverage percentage. Every
    row declares an evidence token — a wasm-bindgen-test function
    name, a Playwright spec file in `e2e/specs/`, or a `Route::`
    variant — which the script grep-validates against the codebase.
    Rows marked "covered" whose evidence is missing cause a HARD
    failure so a stale or misspelled row cannot produce a dishonest
    "green".

  **Why the frontend is not measured by wasm source-based line
  coverage.** We evaluated `-C instrument-coverage` on the
  `wasm32-unknown-unknown` target with `grcov` aggregation and found
  it is **not achievable on the pinned stable toolchain** used by
  `Dockerfile.tests` (`rust:1.88-bookworm`). The compile fails with
  `error[E0463]: can't find crate for 'profiler_builtins'` on every
  dependency, because the rustc `wasm32-unknown-unknown` sysroot does
  not ship `libclang_rt.profile`. The standard `-Z
  build-std=std,panic_abort,profiler_builtins -Z
  build-std-features=profiler` workaround requires a real nightly
  rustc; under stable 1.88 with `RUSTC_BOOTSTRAP=1` the rust-src copy
  of `core` fails to build. `terraops-frontend` is also bin-only and
  every source file depends on `yew`/`web-sys`/`gloo-*` at the type
  level, so cross-compiling to a native target for coverage would
  require a material re-architecture. Rather than ship a dishonest
  line-coverage number that does not reflect the wasm runtime, Gate 2
  enforces the matrix-based 90%+ verification contract above, whose
  evidence is deterministically grep-checked against real test code
  and Playwright specs. `docs/test-coverage.md §Why the frontend is
  not measured by wasm source-based line coverage` records the
  exact observed blocker evidence.
- **Gate 3** — endpoint parity audit via `scripts/audit_endpoints.sh`.
  Mode is controlled by the presence of
  `crates/backend/tests/.audit_strict`: absent → `progress` mode
  (reverse check enforced, forward parity reported); present
  (committed at the end-of-development gate) → `strict` mode (both
  checks enforced). The marker is now present and the audit reports
  `117/117` (100 %) forward parity with 0 reverse orphans — green.
- **Flow gate** — seven Playwright specs live in `e2e/specs/`
  (`login`, `admin_ops`, `products_import`, `analyst_metric_report`,
  `alert_to_notification`, `talent_recommendations`, `offline_states`).
  The gate is **on by default**: `run_tests.sh` brings up the `app`
  service, waits for `/api/v1/health`, then runs `npx playwright test`
  inside the `tests` image against `https://app:8443` over the compose
  network, and fails the gate on any spec failure. `Dockerfile.tests`
  bundles pinned Chromium + the Playwright npm deps into `/opt/e2e` and
  `/opt/playwright-browsers`, so no first-use download is required at
  test time. Set `TERRAOPS_SKIP_FLOW=1` to bypass the flow gate (for CI
  hosts that cannot bind `:8443`); bypassing is loud, not silent.

`./run_tests.sh` invokes every gate against the same `tests` image
produced by `Dockerfile.tests`. No host-side cargo, Node, or Rust
toolchain is required; the host only needs `docker` (Compose v2) and
`bash`. Legacy compatibility string for discoverability: `docker-compose up`
remains an acceptable alias for `docker compose up --build` under
Compose v1 environments.

## Authentication

P1 delivers the full backend auth contract: Argon2id password hashing,
HS256 JWT access tokens (15-minute lifetime), opaque refresh tokens
rotated on every use with a revocable `sessions` table, bearer-token
`Authorization: Bearer <jwt>` on every privileged request, normalized
`AUTH_INVALID_CREDENTIALS` / `FORBIDDEN` / `RATE_LIMITED` error
envelopes, and CIDR allowlist enforcement. Device mTLS pinning is
schema-ready (admin-managed `device_certs` + `mtls_config`); the Rustls
pinned-client verifier flips on once an admin sets `enforced=true`.
Transport-layer handshake refusal is proven by
`crates/backend/tests/mtls_handshake_tests.rs` — four in-process
rustls/tokio-rustls tests bind an ephemeral port and assert
(a) an unpinned client is refused at the TLS handshake (HTTP handler
never runs), (b) a pinned client chained to the trusted CA completes
the handshake, (c) rebuilding the server config with a different pin
set propagates revocation within one handshake (well under the 1 s
design budget), and (d) application bytes travel over the handshake.

`docker compose up --build` now runs migrations and seeds the demo
accounts automatically on first boot (see `Dockerfile.app` CMD). You
can also force re-seed any time with `./init_db.sh` or
`docker compose exec app terraops-backend seed`.

Demo accounts (all passwords `TerraOps!2026`). **Sign-in is
username-first**: the login form, `/api/v1/auth/login`, and the audit
log all key off `username`, not email. The email column is still
populated (encrypted at rest) for notifications, but logging in with an
email address is not a supported path.

| Role           | Username    | Email (notifications only) |
| -------------- | ----------- | -------------------------- |
| Administrator  | `admin`     | admin@terraops.local       |
| Data Steward   | `steward`   | steward@terraops.local     |
| Analyst        | `analyst`   | analyst@terraops.local     |
| Recruiter      | `recruiter` | recruiter@terraops.local   |
| Regular User   | `user`      | user@terraops.local        |

## Roles And Workflows

Five canonical roles (names used verbatim in code, DB, and docs):

- **Administrator** — user & role management, security (mTLS, allowlist, retention), audit log.
- **Data Steward** — product catalog, imports, images, change history.
- **Analyst** — environmental KPIs, reports, dashboards.
- **Recruiter** — talent search, candidate recommendations, mailbox export.
- **Regular User** — personal notifications, profile, self-service.

The full actor-to-workflow map, permission codes, and workflow state
machines are defined in `docs/design.md`.

## Main Repo Contents

```
Cargo.toml                  # Rust workspace (3 crates)
crates/shared/              # shared DTOs, errors, roles, permissions, pagination
crates/backend/             # Actix-web server (serves SPA + REST API on :8443)
crates/frontend/            # Yew SPA (built by Trunk, bundled into the backend image)
docker-compose.yml          # services: db, app; profile: tests
Dockerfile.app              # two build stages (backend `cargo build --release` +
                            #           frontend `trunk build --release`) plus a slim
                            #           debian runtime that serves the real Yew SPA
                            #           from /app/dist and the REST API on :8443
Dockerfile.tests            # scaffold: Rust toolchain only (rust:1.88-bookworm + bash + jq);
                            #           Node, pinned Chromium, wasm-bindgen-test runner, grcov,
                            #           and Playwright browsers arrive in P1 alongside their
                            #           first targets
run_app.sh                  # convenience wrapper around `docker compose up --build`
run_tests.sh                # broad test gate (see "Verification Method")
init_db.sh                  # apply migrations + seed demo users
scripts/
  dev_bootstrap.sh          # generates runtime secrets + TLS cert on first boot
  gen_tls_certs.sh          # dev-only self-signed TLS server cert
  issue_device_cert.sh      # admin tool: issue client cert from internal CA + SPKI pin
  seed_demo.sh              # seed one demo user per role + cross-domain fixtures
  audit_endpoints.sh        # Gate 3: endpoint parity audit
e2e/                        # Playwright flow specs (populated in P1+)
plan.md                     # repo-local execution checklist
```

## Architecture

- **Single TLS origin.** One Actix-web process binds `:8443` with Rustls,
  serves the Yew SPA from `/app/dist/` with HTML5 fallback, and mounts the
  REST API under `/api/v1/*`. No reverse proxy; no separate frontend
  container; zero CORS.
- **Central config.** All runtime values are read once via
  `crates/backend/src/config.rs` from the process environment. No
  scattered `env::var` calls in business logic; no `.env` files consumed.
- **Centralised secrets.** Runtime secrets and TLS material live only on
  the `terraops-runtime` Docker volume, generated on first boot by
  `scripts/dev_bootstrap.sh`.
- **Data-at-rest.** Email addresses are stored as `email_ciphertext`
  (AES-256-GCM), `email_hash` (HMAC-SHA256), and `email_mask` (precomputed
  display). Passwords are Argon2id. Schema baseline is in migration
  `0001_identity.sql`; RBAC and feature tables arrive in `0002+`.
- **mTLS device pinning.** `scripts/issue_device_cert.sh` issues a client
  certificate from the internal CA and prints its SPKI-SHA256 pin; the
  enforcing Rustls `ClientCertVerifier` and admin endpoints land in P1.

## Important Notes

Current-scope disclosures (updated as features land):

- **Backend P1 + P-A + P-B + P-C + P3 + P4 hardening are complete and
  integrated.** 157 no-mock integration tests against real Postgres
  through the full middleware stack:
    - `http_p1.rs` — 52 P1 tests (system S1–S2, auth A1–A5, users U1–U10,
      security SEC1–SEC9, retention R1–R3, monitoring M1–M4, reference-data
      REF1–REF9, notifications N1–N7).
    - `parity_tests.rs` — 52 P-A/P-B tests (products P1–P14, imports
      I1–I7, env E1–E6, metrics MD1–MD7, KPI K1–K6, alerts AL1–AL6,
      reports RP1–RP6) covering the auth/RBAC surface for every endpoint.
    - `talent_{search,recommend,weights,watchlist,feedback}_tests.rs` —
      39 P-C talent tests covering T1–T13 including cold-start →
      blended scoring transition at the 10-feedback threshold.
    - `integration_tests.rs` — 14 P3/P4 cross-domain tests: retention
      env_raw/kpi/feedback purges, audit-indefinite policy, ttl=0
      retains, alert evaluator → notification, report scheduler →
      artifact + notification, retention sweep job, metric rollup
      idempotency, notification retry, notification opt-out,
      allowlist live enforcement, signed URL forged/expired/unknown
      negatives, error envelope + email ciphertext redaction.
  Run them with `docker compose run --rm tests bash -c 'cargo test
  -p terraops-backend -- --test-threads=1'`.
- `scripts/seed_demo.sh` now invokes `terraops-backend seed`, which is
  idempotent and creates/updates the five demo users with the role
  matrix above. Running it repeatedly preserves operator-set passwords.
- **Frontend SPA status:** P1 + P-A/B/C complete and live in `dist/`.
  The Yew SPA ships a real router, auth/toast/notifications context
  providers, permission-aware nav, a typed `ApiClient` with hard 3 s
  timeout + single-GET retry + unified error mapping
  (`wasm-bindgen-test` coverage for the timeout race and error
  mapping), and real pages for login, change-password, admin (users,
  allowlist, retention, mTLS, audit), monitoring (latency, errors,
  crashes), the notifications center, the data-steward catalog +
  imports surfaces, and the analyst KPI / metrics / alerts / reports
  / talent workbench. The `dashboard::Home` surface renders the live
  `kpi_summary` — cycle-time average, funnel conversion, 24h anomaly
  count, efficiency index — for any user holding `kpi.read`, with a
  permission-gated fallback card for users without it. The
  `Dockerfile.app` ships the real Trunk + wasm-bindgen stage that
  replaces the P0 `dist-scaffold` shell.
- All **117** REST endpoints are listed authoritatively in
  `docs/api-spec.md` (51 P1 + 21 P-A + 31 P-B + 14 P-C, with the full
  breakdown reproduced in the `## Totals` section). P1 shipped the
  shared-foundation endpoints; P-A, P-B, and P-C now ship the remainder.
  The endpoint-parity audit runs in strict mode and reports
  `117/117` forward parity with 0 reverse orphans. (Audit #12 Issue #4
  aligns this count with `docs/api-spec.md`; earlier `114` references
  in prior-audit sections below are historical.)
- No outbound integrations exist. TerraOps is an offline system by contract.
- No `.env` / `.env.*` files are created or consumed by this repo. The
  Postgres bootstrap value in `docker-compose.yml` is a documented
  dev-only local-development credential, not a production secret;
  production secret management is out of scope for this offline
  single-node deployment.
- `scripts/dev_bootstrap.sh` is the sole first-boot bootstrap path and is
  explicitly labelled dev-only inside the script.

## Audit #2 Remediation — frontend contract + UI flows

The `develop-1` remediation bundle closes three audit-#2 issues. Full
execution detail is in `plan.md` under **Audit #2 Remediation**; the
delivered shape from a reviewer's perspective is:

- **Frontend ↔ backend decoder alignment.** `crates/frontend/src/api.rs`
  decodes real handler shapes now: paginated list endpoints decode
  `Page<T>` (via new `_page()` methods that mirror the backend's
  `items/page/page_size/total` envelope), `create_product` decodes
  `{ id }` and returns `Uuid`, `update_product` and `set_product_status`
  return `()` (HTTP 204), and `validate_import` / `commit_import` /
  `cancel_import` decode mini-envelopes (`ImportValidateResult`,
  `ImportCommitResult`, `ImportCancelResult` in
  `crates/shared/src/dto/import.rs`) rather than `ImportBatchSummary`.
- **Canonical RBAC vocabulary in the SPA.** Every `PermGate` and nav
  condition now uses codes seeded by `migrations/0002_rbac.sql` and
  enforced by `require_permission()`: `product.write`, `metric.read`,
  `metric.configure`, `alert.ack`, `alert.manage`, `report.schedule`,
  `report.run`, `kpi.read`, `talent.read`, `talent.manage`.
- **Missing UI flow families now live:**
  - **Lineage "why this value" UX** — new route
    `/metrics/computations/:id/lineage` + `ComputationLineagePage`;
    `SeriesPoint` now carries an optional `computation_id` so each point
    on a series chart links straight to its full lineage (inputs,
    formula, params, alignment, confidence, window).
  - **KPI drill/filter UX** — date-range filter wired through the new
    paginated KPI endpoints; real tables for cycle-time, funnel,
    anomalies, efficiency, and drill.
  - **Recruiter search/filter UX** — real filter form (q, location,
    skills, min-seniority, major, availability, min-education) via
    `list_candidates_query()`.
  - **Product governance UX** — tax-rate add/delete, image thumbnails
    with delete, and full change-history panel on the product detail
    page.
  - **Report schedule + export download UX** — schedule form
    (kind × format × cron) plus per-artifact Download button. Downloads
    go through a `fetch → Blob → synthetic anchor` helper
    (`trigger_blob_download`) so the JWT Bearer header stays attached
    (a plain anchor navigation cannot set `Authorization`).

Verification: `cargo check -p terraops-frontend --target wasm32-unknown-unknown`
and `cargo check -p terraops-backend -p terraops-shared` are both clean
(warnings only, all pre-existing dead-code notices). The DTO-additive
backend change (`metric_computations.id` is now projected as
`computation_id` on series points) preserves the endpoint-parity
audit.

## Audit #7 Remediation — retention, history, offline cache, export slicing

The `develop-1` remediation bundle closes six audit-#7 issues:

- **Feedback retention by inactivity (High).** `talent_feedback`
  retention is now driven by **24 months of inactivity** rather than by
  each row's own `created_at`. Both the on-demand runner
  (`crates/backend/src/handlers/retention.rs`) and the hourly sweep
  (`crates/backend/src/jobs/mod.rs`) delete feedback only for candidates
  whose most recent feedback is older than the TTL window — touching a
  candidate inside the window preserves the full thread.
- **Truthful bulk-import history (High).** Product import commits now
  snapshot the pre-upsert row (`to_jsonb(p)`) when a SKU already
  exists and record a real `update` entry with both `before_json` and
  `after_json`. Fresh SKUs still record as `create` with `after_json`
  only. The commit response body also reports both `inserted` and
  `updated` counts.
  (`crates/backend/src/products/import.rs`.)
- **Tiered offline cache for static resources/images (High).** The SPA
  ships a service worker at `/sw.js` with three cache tiers:
  cache-first for the app shell (Yew wasm/js, CSS, favicon/logo),
  stale-while-revalidate for images, and network-first for `/api/v1/*`
  reads with a cached fallback on offline. The backend serves the
  worker with `Service-Worker-Allowed: /` so it can control the whole
  origin. Non-GET requests bypass the cache entirely.
  (`crates/frontend/static/sw.js`, `crates/frontend/index.html`,
  `crates/backend/src/spa.rs`.)
- **Idempotent-read retry contract (Medium).** The SPA honors a single
  retry on network error / timeout / 5xx for all safe GETs. This now
  applies uniformly to paged helpers (`get_with_total` previously
  skipped the retry loop, which drifted from the documented contract).
  The backend request path remains side-effect-free on GETs and emits
  the same error envelope on both the first call and a retry, so clients
  can retry safely.
  (`crates/frontend/src/api.rs`.)
- **Timestamp display MM/DD/YYYY 12-hour (Medium).** Every timestamp in
  the SPA now routes through `format_ts` / `format_ts_opt` / new
  `format_date` (for daily rollup `NaiveDate` columns). The remaining
  raw `%Y-%m-%d` and ISO `NaiveDate::to_string()` displays on the
  dashboard and KPI drill tables have been swept.
  (`crates/frontend/src/pages.rs`.)
- **Report slicing/export scope (Medium).** `env_series` reports now
  accept optional `site_id` and `department_id` filters in addition to
  `source_id`, and the SPA Reports workspace exposes real site +
  department selectors when the env-series kind is chosen. `alert_digest`
  intentionally does not expose spatial slicing (alert rules reference
  metric definitions which have no direct site/dept column, so a
  correlated filter would be ambiguous across multi-source aggregations);
  this boundary is documented in the scheduler module.
  (`crates/backend/src/reports/scheduler.rs`,
  `crates/frontend/src/pages.rs`.)

## Audit #8 Remediation — RBAC coherence, funnel slicing, repo-local docs

The `develop-1` audit-#8 remediation bundle closes three high-severity
findings:

- **Frontend RBAC drift (High).** Admins carry `talent.manage` (a
  superset of `talent.read`) and Regular Users carry `report.run`
  without `report.schedule`; both were previously blocked from the
  real SPA surfaces. A new `PermAnyGate` component applies **OR**
  semantics at the SPA gate, and a matching backend helper
  `require_any_permission()` does the same at every talent handler.
  All talent pages now admit `talent.read || talent.manage`, and the
  Reports page admits `report.run || report.schedule`. Nav entries and
  dashboard quick-links were updated to match, so admins see Talent
  and report-runners see Reports.
  (`crates/backend/src/auth/extractors.rs`,
  `crates/backend/src/talent/handlers.rs`,
  `crates/frontend/src/components.rs`,
  `crates/frontend/src/pages.rs`.)
- **KPI funnel slicing (High).** K3 is now a real slice-and-drill
  surface. The backend `/api/v1/kpi/funnel` accepts `from`, `to`,
  `site_id`, `department_id`, and `severity` (also accepted as
  `category` for axis symmetry) and correlates alert events through
  `alert_rules.metric_definition_id → metric_definitions.source_ids →
  env_sources` to honor site/department. The SPA KPI workspace adds a
  severity selector and renders a dedicated sliced-funnel card with
  per-stage counts, per-stage conversion, and overall conversion.
  (`crates/backend/src/kpi/handlers.rs`,
  `crates/backend/src/kpi/repo.rs`,
  `crates/frontend/src/api.rs`,
  `crates/frontend/src/pages.rs`.)
- **Repo stands alone (High).** `docs/design.md` and `docs/api-spec.md`
  have been brought into the repo alongside the existing
  `docs/test-coverage.md` so static review works without any
  parent-directory dependency. Every repo-local reference
  (`README.md`, `plan.md`, `scripts/audit_endpoints.sh`,
  `crates/frontend/src/api.rs`) now points at the in-repo `docs/...`
  paths instead of `../docs/...`.
- **Recruiter role workflow — extended attributes + sort (Medium).**
  `GET /api/v1/talent/roles` now filters on `required_major`,
  `min_education` (ordinal whitelist), and `required_availability`,
  and sorts by a whitelisted `sort_by` column (`created_at` |
  `opened_at` | `title` | `min_years` | `status`) in a whitelisted
  `sort_dir` (`asc` | `desc`). The SPA search card exposes matching
  inputs and sort selectors, and the recruiter create-role card
  accepts `required_major`, `min_education`, `required_availability`
  on submit. Unknown values are rejected server-side with a
  user-safe 400.
  (`crates/backend/src/talent/roles_open.rs`,
  `crates/backend/src/talent/handlers.rs`,
  `crates/frontend/src/api.rs`,
  `crates/frontend/src/pages.rs`.)
- **Crash-report ingest guards (Medium).** `POST
  /api/v1/monitoring/crash-report` now enforces field-level size
  limits (page ≤ 2 KiB, agent ≤ 1 KiB, stack ≤ 64 KiB truncated,
  payload ≤ 128 KiB rejected) and runs a redaction sweep for
  well-known secret-shaped substrings (`Authorization: Bearer`,
  bare bearer tokens, `password=`/`api_key=`/`token=` fragments,
  JWT-shaped triples, email addresses) on both the stack string and
  every string leaf inside the payload JSON. The contract is
  documented at the module header, and five unit tests pin the
  redaction rules.
  (`crates/backend/src/handlers/monitoring.rs`.)
- **Username-first sign-in contract (Low).** Docs and inline
  module comments now consistently describe sign-in as
  username-first. The README demo-accounts table adds an explicit
  `Username` column and states that login keys off `username`, not
  email. `crates/backend/src/handlers/auth.rs` A1 docstring
  corrected from "email + password" to "username + password".
  `crates/backend/src/crypto/email.rs` clarifies that the email
  hash is used for admin lookup/uniqueness, not for login.

## Audit #12 Remediation — real cron scheduling, honest report failures, restart-gated mTLS contract, parity docs

The `develop-1` audit-#12 remediation bundle closes four findings:

- **Report `cron` is now actually evaluated (High).** The new module
  `crates/backend/src/reports/cron.rs` parses a POSIX-subset 5-field
  cron expression (`* / N / A-B / A,B,C / */N / A-B/N`; Sunday is both
  `0` and `7`; Vixie-cron dom/dow OR rule). Every scheduler tick, the
  reports/scheduler loop now runs a "cron re-schedule pass" before the
  failed→scheduled retry promotion: any job with a non-empty `cron`
  sitting in `done`, `cancelled`, or terminal-`failed` state has its
  `next_fire` computed from `last_run_at` (or `created_at` if never
  run); if `NOW()` has reached it, the row is flipped back to
  `scheduled` with `retry_count=0` and picked up by the normal queue.
  Cron expressions are also validated at `POST /reports/jobs` time —
  invalid crons return `400 invalid cron: …` instead of silently
  sitting inert in the DB. Unit tests for the parser and `next_after`
  live inline in `cron.rs`.
- **Report data-assembly failures no longer masquerade as empty
  artifacts (High).** `reports::scheduler::build_report_data` used to
  call `.fetch_all(...).await.unwrap_or_default()` on each kind's query
  and then happily render an empty PDF/CSV/XLSX as a "successful" job.
  It now returns `AppResult<Vec<serde_json::Value>>` and propagates the
  SQL error through the normal failure path. The scheduler catches the
  error and marks the job `failed` with `retry_count+1` (so the
  existing one-shot retry still applies); no artifact is written. This
  closes the "silent failure produces an empty file and a green
  notification" trap.
- **mTLS PATCH is restart-gated, and the contract says so (High).**
  The rustls `ServerConfig` is built once at `app::run` startup from
  `mtls_config.enforced`. `AppState` now carries the captured startup
  value as `mtls_startup_enforced`. `GET /security/mtls`,
  `GET /security/mtls/status`, and `PATCH /security/mtls` all surface
  `enforced` (persisted-desired), `active_enforced` (startup-live),
  `pending_restart` (delta flag), and a human-readable `note`. The
  PATCH now returns `200 OK` with that contract body (instead of the
  old silent `204 No Content`) and emits a `tracing::warn!` when a
  restart is required. Device-cert SPKI pins still refresh live every
  30 s via the existing `tls::spawn_pin_refresher`. The contract is
  stated explicitly in `crates/backend/src/tls.rs` and `app.rs`
  module docs. Tests: `t_sec8_patch_mtls_requires_mtls_manage` and
  `t_sec9_mtls_status_returns_cert_counts` now assert
  `active_enforced`, `pending_restart`, and `note`.
- **Endpoint-parity count aligned at 117/117 (Medium).** `README.md`
  no longer advertises the stale `114/114` figure. The authoritative
  count in `docs/api-spec.md §Totals` is `51 + 21 + 31 + 14 = 117`,
  and the README headline + Gate 3 line + Architecture bullet all
  read `117/117` now. Historical prior-audit sections keep their
  `114` wording where the figure is historical to that audit.

Verification (code-level; broad `./run_tests.sh` rerun deferred per the
standing "do not rerun broad contract yet" directive):

- `crates/backend/src/reports/cron.rs` inline unit tests cover
  parse errors, `*`/range/list/step fields, daily, weekday-only,
  unreachable-expression (`0 0 31 2 *`), and Sunday `0`/`7`
  equivalence.
- `t_sec8_patch_mtls_requires_mtls_manage` asserts the PATCH
  response body carries `enforced=true`, `active_enforced=false`,
  `pending_restart=true`, and a `note` string containing the word
  `restart`.
- `bash scripts/audit_endpoints.sh` strict mode remains green at
  `117/117` (Issue #4 was docs-only; no endpoint signatures changed).

## Audit #11 Remediation — signed-URL user binding, SW cache isolation, local-TZ display, incremental loading

The `develop-1` audit-#11 remediation bundle closes four findings:

- **Signed image URLs bound to the authenticated user (High).**
  `GET /api/v1/images/{id}?exp=..&sig=..` previously authenticated the
  signature over `path | exp` only, so a URL minted for user A could be
  replayed by any other authenticated user B inside the 10-minute `exp`
  window. The HMAC input now includes the authenticated caller's user
  id (`HMAC(key, path | user_id | exp)`), computed server-side from the
  session — the user id is still never placed in the URL text, so
  nothing identifying leaks in logs or referers. The verifier recomputes
  the HMAC against the actual bearer-holder on serve; a URL signed for
  A returns 403 when requested by B. Unit coverage:
  `wrong_user_rejected` in `crates/backend/src/crypto/signed_url.rs`;
  HTTP coverage: `t_int_signed_image_url_rejects_cross_user_replay` in
  `crates/backend/tests/integration_tests.rs` (Alice mints, Alice sees
  404 for an unknown row, Bob replaying Alice's exact URL sees 403).
  (`crates/backend/src/crypto/signed_url.rs`,
  `crates/backend/src/products/repo.rs`,
  `crates/backend/src/products/handlers.rs`,
  `crates/backend/src/products/images.rs`.)
- **Service worker no longer caches authenticated traffic (High).** The
  Audit #7 SW shipped a network-first `/api/*` tier and an image tier
  keyed only by URL, which could expose a previous signed-in user's
  cached responses to the next user on a shared device. The SW is
  bumped to `terraops-v2` and now: (a) passes every request with an
  `Authorization` header straight through without caching, (b) never
  writes to an `/api/*` cache regardless of header presence, (c) leaves
  only the unauthenticated static app-shell and unauthenticated image
  tier cacheable, and (d) accepts a `{type:'logout'}` message that
  deletes the image + api caches. The SPA posts that message from the
  Nav logout callback immediately before redirecting to `/login`, so a
  subsequent sign-in on the same device does not see the prior
  account's fetched images.
  (`crates/frontend/static/sw.js`,
  `crates/frontend/src/components.rs`.)
- **Timestamps render in the browser's local timezone (Medium).**
  `format_ts` in `crates/frontend/src/pages.rs` previously formatted
  UTC directly, which drifted from the design contract that "display
  uses `MM/DD/YYYY hh:mm AM/PM` in the user's local timezone" and from
  the centralized helper `terraops_shared::time::format_display`. It
  now calls that shared helper, supplying the offset-from-UTC in
  seconds derived from `js_sys::Date::new_0().get_timezone_offset()`
  (ECMAScript returns minutes-west-of-UTC, so the sign is inverted).
  One code path, one display contract, browser-local values on every
  page.
  (`crates/frontend/src/pages.rs`,
  `crates/shared/src/time.rs` — unchanged helper reused.)
- **Incremental loading instead of Prev/Next pagination (Medium).**
  The Audit #6 server-paginated Prev/Next pager has been replaced by a
  "Load more" control across the four long-list surfaces (Products,
  Observations, Alert events, Candidates). Rows accumulate on the
  client across fetches; the backend still receives
  `page=N&page_size=50` per call so server paging is unchanged, only
  the UX of advancing through the list changes. A new `LoadMore`
  component renders `N loaded` or `N of M loaded` plus a "Load more"
  button that disables when the total is reached or while a fetch is
  in flight. `ServerPager` has been removed.
  (`crates/frontend/src/components.rs`,
  `crates/frontend/src/pages.rs`.)

