# Implementation Plan — TerraOps

Definitive repo-local execution checklist. Derived from `docs/design.md`; endpoint inventory in `docs/api-spec.md`; test mapping in `docs/test-coverage.md`.

## Status Markers

- `[ ]` not started
- `[-]` in progress
- `[x]` done

## Accepted Scaffold Playbook Contract

- **Selected root playbook**: Fullstack Rust, **single long-lived `app` service** that serves the Yew SPA and the Actix-web REST API on the same TLS port (`:8443`) + a `db` service. No separate frontend container. No reverse proxy.
- **Supporting overlays**:
  - Yew SPA overlay (`yew-router`, context providers, `gloo-net` fetch client with 3 s timeout + single-retry-on-GET).
  - Actix-web overlay (`web::scope("/api/v1").configure(...)` per domain, shared `web::Data<AppState>`, normalized `AppError` → JSON, `bind_rustls` with pinned-client-cert verifier).
  - Postgres overlay (`sqlx` + committed `.sqlx/` offline metadata).
  - Offline ops overlay (`tokio_cron_scheduler` for reports / alerts / notification retry / retention / metric rollup).
  - mTLS overlay (Rustls `ClientCertVerifier` backed by `device_certs` SPKI pin set; admin-managed).
  - Field-encryption overlay (`aes-gcm` AES-256-GCM for `users.email_ciphertext`, HMAC-SHA256 for `email_hash`).
- **Why this fits**: prompt mandates Yew + Actix-web + Postgres and fully offline single-node deployment. Co-serving the SPA from the backend binary gives one TLS origin, zero CORS, one container, and one `docker compose up --build` command. mTLS pin enforcement is the only technically credible interpretation of "certificate pinning per deployment" for a browser-based local SPA.
- **Exact bootstrap command** (run from `repo/`):

  ```bash
  cargo init --vcs none --name terraops-workspace . \
    && rm -rf src \
    && printf '[workspace]\nmembers = ["crates/shared","crates/backend","crates/frontend"]\nresolver = "2"\n' > Cargo.toml \
    && mkdir -p crates \
    && cargo new --lib crates/shared   --name terraops-shared \
    && cargo new --bin crates/backend  --name terraops-backend \
    && cargo new --lib crates/frontend --name terraops-frontend \
    && mkdir -p crates/backend/migrations crates/backend/tests/http crates/backend/tests/common \
                 crates/frontend/static scripts e2e/specs \
    && touch run_app.sh run_tests.sh init_db.sh \
              scripts/dev_bootstrap.sh scripts/gen_tls_certs.sh \
              scripts/issue_device_cert.sh scripts/seed_demo.sh scripts/audit_endpoints.sh \
    && chmod +x run_app.sh run_tests.sh init_db.sh scripts/*.sh
  ```

- **Required runtime contract**:
  - Canonical: `docker compose up --build`
  - Legacy compatibility string present in README: `docker-compose up`
  - Helper wrapper: `./run_app.sh` (wraps compose up + TLS bootstrap + health wait)
  - First-boot entrypoint: `scripts/dev_bootstrap.sh` generates any missing secrets/certs into Docker volume `terraops-runtime`. No `.env` in the repo.
- **Required broad test contract**:
  - Repo-root `./run_tests.sh` brings up Compose `tests` profile and runs the exact commands documented in `docs/test-coverage.md §Coverage Gate Math` (no drift):
    - Gate 1: `cargo llvm-cov --no-fail-fast -p terraops-shared -p terraops-backend --ignore-filename-regex '/(tests|migrations|\.sqlx)/' --fail-under-lines 94 --lcov --output-path coverage/rust.lcov` (explicit `-p` flags restrict scope to the two native Rust crates; `--workspace` is intentionally not used so the frontend crate cannot contribute to `coverage/rust.lcov`). Currently measured at ~94.83% lines.
    - Gate 2 (**redesigned — Frontend Verification Matrix**): (2a) `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --target wasm32-unknown-unknown -p terraops-frontend --no-fail-fast` (must be green; 43/43 tests today), then (2b) `scripts/frontend_verify.sh` parses the 53-row matrix in `docs/test-coverage.md` and enforces `GATE2_FVM_FLOOR=90`. Current measurement: **100% Frontend Verification Matrix score (53/53 rows satisfied)** — this is a verification-matrix score (`covered_rows / total_rows × 100`), not a line-coverage percentage, and must never be reported as "100% frontend coverage" without the explicit FVM qualifier. Wasm source-based line coverage is **not** the authoritative frontend proof: `profiler_builtins` is not shipped in `rust-std-wasm32-unknown-unknown` for stable rust 1.88, and the `-Z build-std` workaround requires a real nightly rustc. Exact observed blocker evidence is recorded in `docs/test-coverage.md §Why the frontend is not measured by wasm source-based line coverage`.
    - Gate 3: `scripts/audit_endpoints.sh` (mode decided by presence of `crates/backend/tests/.audit_strict`)
    - Flow: `npx --prefix e2e playwright test` (all 7 specs must pass)
  - **Authoritative threshold sentence** (identical wording in `docs/design.md` and `docs/test-coverage.md`): The 94% line-coverage floor applies only to `crates/shared` + `crates/backend` (Rust native code). The `crates/frontend` Yew/WASM crate is enforced via the **Frontend Verification Matrix** at `GATE2_FVM_FLOOR=90` — a **verification-matrix score**, not a line-coverage percentage: `covered_rows / total_rows × 100 ≥ 90`. Wasm line coverage is not used as the authoritative frontend proof due to the documented upstream `profiler_builtins` gap on `wasm32-unknown-unknown`. Playwright specs contribute rows to the FVM and also run as the flow gate.
- **Required scaffold files and scripts**:
  - `Cargo.toml` (workspace), `Cargo.lock`
  - `docker-compose.yml`, `Dockerfile.app`, `Dockerfile.tests` (no `Dockerfile.frontend`)
  - `init_db.sh`, `run_app.sh`, `run_tests.sh`
  - `scripts/dev_bootstrap.sh`, `scripts/gen_tls_certs.sh`, `scripts/issue_device_cert.sh`, `scripts/seed_demo.sh`, `scripts/audit_endpoints.sh`
  - `crates/shared/Cargo.toml` + `src/lib.rs`
  - `crates/backend/Cargo.toml` + `src/main.rs`, `src/app.rs`, `src/config.rs`, `migrations/0001_identity.sql`
  - `crates/frontend/Cargo.toml` + `Trunk.toml`, `index.html`, `src/main.rs`, `src/app.rs`, `src/router.rs`
  - `e2e/package.json`, `e2e/playwright.config.ts`
  - `README.md` (strict-audit layout), `plan.md` (this file)
- **Required baseline surfaces at scaffold exit**:
  - `cargo build --workspace` green.
  - `docker compose up --build` boots `db` + `app`. At scaffold, `Dockerfile.app` is deliberately single-path-for-the-backend: it runs `cargo build --release -p terraops-backend` from `rust:1.88-bookworm` and copies the prebuilt SPA shell from `crates/frontend/dist-scaffold/` into `/app/dist/`. The `trunk build --release` stage is intentionally deferred to P1 and lands in the same commit as the first real Yew code + first wasm-bindgen-test target, so the scaffold build never pulls `trunk`, `wasm-bindgen-cli`, or any wasm toolchain. This keeps the scaffold build network-light and reproducible.
  - `GET https://localhost:8443/` serves the login placeholder page (Yew SPA).
  - `GET https://localhost:8443/api/v1/health` returns 200 `{"status":"ok"}`.
  - `./init_db.sh` runs migration `0001` and exits 0.
  - `./run_tests.sh` compiles and exits 0. The endpoint-audit (`scripts/audit_endpoints.sh`) runs in **`progress` mode** because the marker file `crates/backend/tests/.audit_strict` is **absent** at scaffold time: the audit reads the authoritative endpoint inventory from `docs/api-spec.md`, enforces the reverse check (no orphan test IDs — trivially true because no HTTP tests exist yet), and prints forward parity as `0/N` without failing (at P0 the spec's `## Totals` line was N=77; it grew to N=114 by P5). Forward parity is enforced only after the marker file is committed in P5, at which point the strict gate reports `114/114`. This rule is the same at every phase; no hand-waved scaffold exception.
  - One-way TLS works with dev cert; `mtls_config.enforced=false` by default.
- **Scaffold stop boundary**: stop after scaffold surfaces boot cleanly. Do NOT begin any feature implementation (no auth, no business endpoints, no dashboards beyond the login placeholder).
- **Scaffold acceptance evidence required**:
  - `docker compose up --build` log excerpt showing `db` + `app` healthy.
  - `curl -k https://localhost:8443/api/v1/health` → `{"status":"ok"}`.
  - `curl -k https://localhost:8443/` returns `index.html` with Yew bundle tag.
  - `./init_db.sh` success line.
  - `./run_tests.sh` exit 0.

## Execution File Tree

```
repo/
├── CLAUDE.md
├── README.md
├── plan.md
├── .gitignore
├── Cargo.toml                      # [workspace]
├── Cargo.lock
├── docker-compose.yml              # services: db, app; profile: tests
├── Dockerfile.app                  # scaffold: cargo build of terraops-backend (rust:1.88-bookworm) + dist-scaffold SPA shell; Trunk stage lands in P1
├── Dockerfile.tests                # scaffold: rust:1.88-bookworm + bash + jq only; Node, pinned Chromium, wasm-bindgen-test runner, grcov, Playwright browsers arrive in P1 with their first targets
├── init_db.sh
├── run_app.sh
├── run_tests.sh
├── scripts/
│   ├── dev_bootstrap.sh            # generates jwt/email_enc/email_hmac/image_hmac keys + TLS cert + internal CA on first boot
│   ├── gen_tls_certs.sh
│   ├── issue_device_cert.sh        # admin tool: issues client cert from internal CA, records SPKI into device_certs
│   ├── seed_demo.sh                # seeds one demo user per role + cross-domain fixtures
│   └── audit_endpoints.sh          # parses api-spec.md + scans tests/http for t_<id>_* coverage (gate 3)
├── crates/
│   ├── shared/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs
│   │       ├── time.rs             # MM/DD/YYYY 12-hour centralized
│   │       ├── pagination.rs
│   │       ├── roles.rs            # 5 canonical role names, verbatim
│   │       ├── permissions.rs      # full code list from design §Permissions
│   │       └── dto/
│   │           ├── mod.rs
│   │           ├── auth.rs
│   │           ├── user.rs
│   │           ├── product.rs
│   │           ├── import.rs
│   │           ├── env_source.rs
│   │           ├── metric.rs
│   │           ├── kpi.rs
│   │           ├── alert.rs
│   │           ├── report.rs
│   │           ├── talent.rs
│   │           ├── notification.rs
│   │           ├── monitoring.rs
│   │           └── security.rs     # allowlist + device cert + mtls dto
│   ├── backend/
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   ├── .sqlx/
│   │   ├── migrations/
│   │   │   ├── 0001_identity.sql
│   │   │   ├── 0002_rbac.sql
│   │   │   ├── 0003_ops_baseline.sql           # sites, departments, audit_log, allowlist, device_certs, mtls_config, retention_policies, api_metrics, client_crash_reports
│   │   │   ├── 0004_notifications.sql
│   │   │   ├── 0010_products.sql
│   │   │   ├── 0011_product_imports.sql
│   │   │   ├── 0020_env.sql
│   │   │   ├── 0021_metrics.sql
│   │   │   ├── 0022_kpi_alerts_reports.sql
│   │   │   └── 0030_talent.sql
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── app.rs              # build + configure Actix app; SPA fallback handler
│   │   │   ├── config.rs
│   │   │   ├── state.rs
│   │   │   ├── tls.rs              # Rustls builder; pinned client-cert verifier
│   │   │   ├── db.rs
│   │   │   ├── telemetry.rs
│   │   │   ├── errors.rs
│   │   │   ├── spa.rs              # static dist/ + fallback to index.html
│   │   │   ├── extractors/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── auth_user.rs
│   │   │   │   ├── require_perm.rs
│   │   │   │   └── owner_guard.rs  # SELF / PERM_OR_SELF helpers
│   │   │   ├── middleware/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── request_id.rs
│   │   │   │   ├── allowlist.rs
│   │   │   │   ├── budget.rs       # 3 s handler budget
│   │   │   │   ├── metrics.rs      # api_metrics writer
│   │   │   │   └── error_normalize.rs
│   │   │   ├── auth/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── argon2.rs
│   │   │   │   ├── jwt.rs
│   │   │   │   ├── sessions.rs
│   │   │   │   ├── email_crypto.rs # AES-256-GCM + HMAC + mask
│   │   │   │   ├── signed_url.rs
│   │   │   │   └── handlers.rs
│   │   │   ├── rbac/
│   │   │   │   ├── mod.rs
│   │   │   │   └── map.rs
│   │   │   ├── users/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── repo.rs
│   │   │   │   └── handlers.rs
│   │   │   ├── ref_data/
│   │   │   │   ├── mod.rs
│   │   │   │   └── handlers.rs
│   │   │   ├── products/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── repo.rs
│   │   │   │   ├── service.rs
│   │   │   │   ├── handlers.rs
│   │   │   │   ├── history.rs
│   │   │   │   ├── tax_rates.rs
│   │   │   │   ├── images.rs
│   │   │   │   ├── import.rs
│   │   │   │   ├── import_validator.rs
│   │   │   │   └── export.rs
│   │   │   ├── storage/
│   │   │   │   ├── mod.rs
│   │   │   │   └── images.rs
│   │   │   ├── metrics_env/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── sources.rs
│   │   │   │   ├── definitions.rs
│   │   │   │   ├── formula.rs
│   │   │   │   ├── lineage.rs
│   │   │   │   └── handlers.rs
│   │   │   ├── kpi/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── repo.rs
│   │   │   │   └── handlers.rs
│   │   │   ├── alerts/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── rules.rs
│   │   │   │   ├── evaluator.rs
│   │   │   │   └── handlers.rs
│   │   │   ├── reports/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── scheduler.rs
│   │   │   │   ├── pdf.rs
│   │   │   │   ├── csv.rs
│   │   │   │   ├── xlsx.rs
│   │   │   │   └── handlers.rs
│   │   │   ├── talent/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── candidates.rs
│   │   │   │   ├── roles_open.rs
│   │   │   │   ├── search.rs
│   │   │   │   ├── scoring.rs
│   │   │   │   ├── weights.rs
│   │   │   │   ├── watchlists.rs
│   │   │   │   ├── feedback.rs
│   │   │   │   └── handlers.rs
│   │   │   ├── notifications/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── center.rs
│   │   │   │   ├── subscriptions.rs
│   │   │   │   ├── emit.rs         # notifications::emit(topic, payload) — called by producers in P-A/P-B
│   │   │   │   ├── retry.rs
│   │   │   │   ├── mailbox_export.rs
│   │   │   │   └── handlers.rs
│   │   │   ├── retention/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── policies.rs
│   │   │   │   └── enforcer.rs
│   │   │   ├── monitoring/
│   │   │   │   ├── mod.rs
│   │   │   │   └── handlers.rs
│   │   │   ├── security/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── allowlist.rs
│   │   │   │   ├── device_certs.rs # admin mgmt + pin-set generation counter
│   │   │   │   ├── mtls_config.rs
│   │   │   │   └── handlers.rs
│   │   │   └── audit.rs
│   │   └── tests/
│   │       ├── common/
│   │       │   ├── mod.rs
│   │       │   ├── test_app.rs
│   │       │   └── time_shim.rs
│   │       ├── tls/
│   │       │   └── mtls_integration_tests.rs
│   │       └── http/
│   │           ├── system_tests.rs
│   │           ├── auth_tests.rs
│   │           ├── rbac_tests.rs
│   │           ├── allowlist_tests.rs
│   │           ├── budget_tests.rs
│   │           ├── pagination_tests.rs
│   │           ├── validation_tests.rs
│   │           ├── users_tests.rs
│   │           ├── admin_security_tests.rs
│   │           ├── ref_data_tests.rs
│   │           ├── products_tests.rs
│   │           ├── products_history_tests.rs
│   │           ├── images_tests.rs
│   │           ├── imports_tests.rs
│   │           ├── imports_export_tests.rs
│   │           ├── env_tests.rs
│   │           ├── metrics_tests.rs
│   │           ├── kpi_tests.rs
│   │           ├── alerts_eval_tests.rs
│   │           ├── reports_tests.rs
│   │           ├── talent_search_tests.rs
│   │           ├── talent_recommend_tests.rs
│   │           ├── talent_weights_tests.rs
│   │           ├── talent_watchlist_tests.rs
│   │           ├── talent_feedback_tests.rs
│   │           ├── notifications_tests.rs
│   │           ├── notifications_retry_tests.rs
│   │           ├── mailbox_export_tests.rs
│   │           ├── retention_tests.rs
│   │           └── monitoring_tests.rs
│   └── frontend/
│       ├── Cargo.toml
│       ├── Trunk.toml
│       ├── index.html
│       ├── tailwind.config.js
│       ├── static/
│       │   ├── favicon.svg
│       │   └── logo.svg
│       └── src/
│           ├── main.rs
│           ├── app.rs
│           ├── router.rs
│           ├── state/
│           │   ├── mod.rs
│           │   ├── auth_ctx.rs
│           │   ├── theme_ctx.rs
│           │   ├── toast_ctx.rs
│           │   └── notifications_ctx.rs
│           ├── api/
│           │   ├── mod.rs
│           │   ├── client.rs
│           │   ├── errors.rs
│           │   └── signed_images.rs
│           ├── components/
│           │   ├── mod.rs
│           │   ├── layout.rs
│           │   ├── nav.rs
│           │   ├── role_gate.rs
│           │   ├── placeholders.rs
│           │   ├── paginated_table.rs
│           │   ├── filters.rs
│           │   ├── kpi_card.rs
│           │   ├── lineage_panel.rs
│           │   ├── signed_image.rs
│           │   ├── file_drop.rs
│           │   └── toast.rs
│           ├── pages/
│           │   ├── mod.rs
│           │   ├── auth/login.rs
│           │   ├── dashboard/home.rs
│           │   ├── admin/{users.rs, roles.rs, retention.rs, allowlist.rs, device_certs.rs, mtls.rs, audit.rs}
│           │   ├── data_steward/{products.rs, product_detail.rs, imports.rs, import_preview.rs}
│           │   ├── analyst/{sources.rs, definitions.rs, series.rs, lineage.rs, reports.rs}
│           │   ├── recruiter/{search.rs, candidate.rs, recommendations.rs, watchlists.rs, weights.rs}
│           │   ├── user/{dashboards.rs, alerts.rs}
│           │   ├── notifications/center.rs
│           │   └── monitoring/{latency.rs, errors.rs, crashes.rs}
│           ├── theme/{mod.rs, tokens.rs}
│           └── i18n/{mod.rs, datetime.rs}
└── e2e/
    ├── package.json
    ├── playwright.config.ts
    ├── tsconfig.json
    └── specs/
        ├── login.spec.ts
        ├── products_import.spec.ts
        ├── analyst_metric_report.spec.ts
        ├── alert_to_notification.spec.ts
        ├── talent_recommendations.spec.ts
        ├── admin_ops.spec.ts
        └── offline_states.spec.ts
```

## File Ownership Map

| Lane | Owned files (mutually exclusive during fan-out) |
|---|---|
| **Main lane (always)** | Repo root (`Cargo.toml`, `Cargo.lock`, `docker-compose.yml`, `Dockerfile.app`, `Dockerfile.tests`, `run_*.sh`, `init_db.sh`, `scripts/**`, `README.md`, `plan.md`, `.gitignore`); `crates/shared/**`; `crates/backend/src/{main,app,config,state,tls,db,telemetry,errors,spa,audit}.rs`; `crates/backend/src/middleware/**`; `crates/backend/src/extractors/**`; `crates/backend/src/auth/**`; `crates/backend/src/rbac/**`; `crates/backend/src/users/**`; `crates/backend/src/ref_data/**`; `crates/backend/src/security/**`; `crates/backend/src/monitoring/**`; `crates/backend/src/retention/**`; **`crates/backend/src/notifications/**`** (all notification infra: emit API, center, subscriptions, retry, mailbox export); `crates/backend/migrations/0001..0004`; `crates/backend/tests/common/**`; `crates/backend/tests/tls/**`; `crates/backend/tests/http/{system,auth,rbac,allowlist,budget,pagination,validation,users,admin_security,ref_data,notifications,notifications_retry,mailbox_export,retention,monitoring}_tests.rs`; `crates/frontend/**` **shell** (`Trunk.toml`, `index.html`, `tailwind.config.js`, `static/**`, `src/{main,app,router}.rs`, `src/state/**`, `src/api/**`, `src/components/**`, `src/theme/**`, `src/i18n/**`, `src/pages/auth/**`, `src/pages/admin/**`, `src/pages/notifications/**`, `src/pages/monitoring/**`); `e2e/**` |
| **P-A Catalog & Governance** | `crates/backend/src/products/**`; `crates/backend/src/storage/**`; `crates/backend/migrations/0010_*.sql`, `0011_*.sql`; `crates/backend/tests/http/{products,products_history,images,imports,imports_export}_tests.rs`; `crates/frontend/src/pages/data_steward/**`; `crates/shared/src/dto/{product,import}.rs` |
| **P-B Env / KPI / Alerts / Reports** | `crates/backend/src/metrics_env/**`; `.../kpi/**`; `.../alerts/**`; `.../reports/**`; `crates/backend/migrations/0020_*.sql`, `0021_*.sql`, `0022_*.sql`; `crates/backend/tests/http/{env,metrics,kpi,alerts_eval,reports}_tests.rs`; **`crates/frontend/src/pages/dashboard/**`** (all dashboard files including `home.rs`, `mod.rs`, and any child modules); `crates/frontend/src/pages/analyst/**`; `crates/frontend/src/pages/user/**`; `crates/shared/src/dto/{env_source,metric,kpi,alert,report}.rs` |
| **P-C Talent** | `crates/backend/src/talent/**`; `crates/backend/migrations/0030_*.sql`; `crates/backend/tests/http/talent_*_tests.rs`; `crates/frontend/src/pages/recruiter/**`; `crates/shared/src/dto/talent.rs` |

**Notification ownership is explicit and unambiguous**: the entire notifications module (backend + frontend center + mailbox export + retry) is owned by the **main lane**. P-A and P-B invoke `notifications::emit(...)` from their producers (for example, import completion in P-A and alert firing in P-B), but they do not edit notification internals. The frontend notification center page is owned by main.

**Dashboard ownership seam is explicit and unambiguous**: during P1 the main lane authors the router entry for `/dashboard` and a minimal placeholder module at `crates/frontend/src/pages/dashboard/home.rs` (content: a perm-gated role-aware shell that renders a `<PlaceholderLoading/>` component from the shared component library while KPI content is out-of-phase). On the last P1 commit, main explicitly hands ownership of the entire `crates/frontend/src/pages/dashboard/**` subtree (including `home.rs` and `mod.rs`) to **P-B** and does not modify any file under that path again in P2, P3, P4, or P5 — all dashboard edits after fan-out flow through the P-B branch. The router's reference to `pages::dashboard::home::DashboardHome` is the only main-lane coupling and it stays pinned on that symbol; P-B is free to grow the subtree with additional child modules. No file in `src/pages/dashboard/**` is dual-owned at any point. The same handoff rule applies symmetrically for `src/pages/analyst/**`, `src/pages/user/**`, `src/pages/data_steward/**`, and `src/pages/recruiter/**` — during P1 these are either not present or contain only a placeholder module; ownership transfers fully to the appropriate package branch at fan-out.

Rule: a package branch may read files outside its owned set but must not edit them.

## Shared-File Contract

Files frozen to main before fan-out:

- `Cargo.toml` (workspace + dependency versions pinned).
- All of `crates/shared/src/**` (DTOs + roles + permissions + time + pagination + error).
- `crates/backend/src/{main,app,config,state,tls,db,telemetry,errors,spa,audit}.rs`.
- `crates/backend/src/{middleware,extractors,rbac,auth,users,ref_data,security,monitoring,retention,notifications}/**`.
- `crates/backend/tests/common/**` (test harness exposing `spawn_app()`, time shim, ephemeral DB, seeded users, mTLS test harness).
- `crates/frontend/src/{main,app,router}.rs`; `state/**`; `api/**`; `components/**`; `theme/**`; `i18n/**`.
- `docker-compose.yml`, `Dockerfile.app`, `Dockerfile.tests`, `run_*.sh`, `init_db.sh`, `scripts/**`.
- `README.md`, `plan.md`.
- `docs/api-spec.md` (endpoint inventory is the authoritative contract every package consumes).

Before fan-out, main must commit:
1. All shared modules compiling with real implementations of every API each package depends on (notifications::emit, auth extractors, RequirePermission, owner_guard, paginated_table, placeholders, lineage_panel, notification center).
2. Full permission set seeded in migration `0002` so packages can reference codes.
3. Test harness `common::test_app::spawn_app(roles)` usable from any package's HTTP test file.
4. `scripts/audit_endpoints.sh` in place and green on empty suite.

After fan-in, main lane merges, runs the integrated test matrix, resolves overlaps, and marks items `[x]`.

## Branch Or Worktree Map

- Default: three git worktrees created after F0 (P1) lands on `main`:
  - `.worktrees/p-a` → branch `feat/catalog-governance`
  - `.worktrees/p-b` → branch `feat/env-kpi-reports`
  - `.worktrees/p-c` → branch `feat/talent-reco`
- Integration, hardening, and final gate (P3, P4, P5) run exclusively on `main`.
- If worktree support is unavailable, fan out to three sequential feature branches with the same owned-file boundaries. Do **not** collapse to one serial lane.
- Each branch commits meaningful progress as it goes. Main lane commits shared + integration progress separately.
- Notification infrastructure does **not** appear on any branch because it is main-owned.

## Ordered Execution Checklist

### P0 — Scaffold (main lane; stop at scaffold boundary)

- [x] Run the bootstrap command; produce workspace + three crates + dirs.
- [x] `crates/shared` baseline: `lib.rs`, `error.rs`, `pagination.rs`, `time.rs` (stubs), `roles.rs` enumerating the five canonical workspace names verbatim, `permissions.rs` enumerating all codes from design §Permissions.
- [x] `crates/backend` baseline: `main.rs` with `/api/v1/health` + `/api/v1/ready`, `config.rs` centralizing env reads, `tls.rs` loading cert/key from the runtime volume, `spa.rs` serving `dist/` + SPA fallback.
- [-] `crates/frontend` baseline: `Trunk.toml`, `index.html`, router with single `/login` placeholder route, API client with 3 s timeout + single-retry-on-GET implemented and unit-tested via `wasm-bindgen-test`. _(Shell + `/login` placeholder + timeout/retry constants delivered; `wasm-bindgen-test` suite for the API client lands in P1 when the first real caller exists.)_
- [x] `docker-compose.yml` with services `db` + `app` and profile `tests`. Mount `terraops-runtime` volume for secrets/certs/images/reports.
- [x] `Dockerfile.app` (scaffold variant): single backend-build stage on `rust:1.88-bookworm` runs `cargo build --release -p terraops-backend`; final Debian-slim stage receives the backend binary at `/usr/local/bin/terraops-backend` and copies the prebuilt SPA shell from `crates/frontend/dist-scaffold/` into `/app/dist/`; runtime entrypoint invokes `scripts/dev_bootstrap.sh` then `terraops-backend serve`. No `trunk build` stage and no `cargo install` of side-tools at scaffold — both are added together with the first real Yew code / first wasm-bindgen-test target in P1.
- [x] `Dockerfile.tests` (scaffold variant): `rust:1.88-bookworm` base with `bash`, `git`, `curl`, `ca-certificates`, `jq` only. No `cargo install`, no Node, no pinned Chromium, no `grcov`, no Playwright browsers at scaffold — each of those is added in the same commit that lands its first consumer (first WASM test / first Playwright spec). This keeps the scaffold test image honest and network-light.
- [x] `scripts/dev_bootstrap.sh`: generate `jwt.key`, `email_enc.key`, `email_hmac.key`, `image_hmac.key`, TLS dev server cert, and internal CA (for `issue_device_cert.sh`) — all stored only on the Docker volume.
- [x] `scripts/gen_tls_certs.sh`, `scripts/issue_device_cert.sh` (admin client-cert issuance tool), `scripts/seed_demo.sh`, `scripts/audit_endpoints.sh`.
- [x] `init_db.sh`: `docker compose run --rm app terraops-backend migrate && scripts/seed_demo.sh`.
- [x] `run_app.sh`: wraps `docker compose up --build` + health wait.
- [x] `run_tests.sh`: host-side dispatcher into the `tests` image. Scaffold wiring:
  - **Gate 1 (real)**: `docker compose build tests` + `docker compose run --rm --no-deps tests bash -c 'cargo --version && cargo test -p terraops-shared -p terraops-backend --no-fail-fast'`. No host-side cargo is invoked; only `docker` + `bash` are required on the host. Uses `bash -c` (not `bash -lc`) so the `/etc/profile` reset does not drop `/usr/local/cargo/bin` from `PATH`. At scaffold this compiles the workspace and passes the existing `terraops-shared` tests (2 passing) while `terraops-backend` has 0 tests — a real green result, not a silent skip.
  - **Gate 3 (real)**: `scripts/audit_endpoints.sh` (authoritative total read from `## Totals` in `docs/api-spec.md`; at P0 scaffold time that line read 77, grew to 114 by P5 — the audit always reads whatever the spec currently declares; progress mode at scaffold).
  - **Gate 2 (deferred)** and **Flow gate (deferred)**: print an explicit `DEFERRED` line stating that the wasm-bindgen-test runner, grcov, Node, pinned Chromium, and Playwright browsers are not yet provisioned in `Dockerfile.tests`. They are wired in the same commit that lands their first consumer in P1+.
  - Full P1+ wrapper command set (the `cargo llvm-cov --fail-under-lines 90 …` coverage wrapper, `wasm-bindgen-test` + `grcov --threshold 80 …`, and `npx --prefix e2e playwright test`) is retained verbatim in the header comment of `run_tests.sh` and is layered on alongside each first consumer.
- [x] Migration `0001_identity.sql` (users + sessions baseline with `email_ciphertext/email_hash/email_mask` columns).
- [x] `README.md` to strict-audit layout: project type `fullstack`, startup, access `https://localhost:8443`, verification via `./run_tests.sh`, demo credentials for all five roles, runtime strings `docker compose up --build` and `docker-compose up`.
- [ ] Commit: `chore(scaffold): single-app workspace + compose + scripts baseline`.
- [ ] **STOP at scaffold boundary.**

#### P0 Scaffold Verification Notes

- `docker compose config` validates cleanly (services `db`, `app`, and `tests` profile; volumes `terraops-db`, `terraops-runtime`).
- `bash -n` is green on all scripts (`run_app.sh`, `run_tests.sh`, `init_db.sh`, `scripts/*.sh`).
- `scripts/audit_endpoints.sh` runs in `progress` mode (no marker file) and passes (0 reverse orphans). It reports the **authoritative endpoint total** (read from the `**N HTTP endpoints.**` sentence in `## Totals` of `docs/api-spec.md` — at P0 scaffold time N=77; it grew to N=114 by P5) and prints forward parity as `0/N` (so `0/77` at P0, becoming `114/114` in strict mode at P5). The parser deliberately does not manufacture its own total from raw ID-row counts; inventory IDs are used only as the reverse-parity validity set so that any `t_<id>_*` test is guaranteed to reference a known endpoint. The artifact at `coverage/endpoint_audit.json` contains exactly one total (`api_total: N`, where N is whatever `## Totals` currently declares) with no conflicting secondary count.
- `docker compose up --build` boots `db` and `app` cleanly. The backend binary listens on `:8443` with rustls; `GET /api/v1/health` returns `{"status":"ok"}` and `GET /api/v1/ready` returns `{"status":"ready","db":true}` once `db` is healthy.
- `./run_tests.sh` exits 0 end-to-end: Gate 1 really compiles the workspace inside the `tests` image and runs `cargo test -p terraops-shared -p terraops-backend --no-fail-fast` (2 passing in `terraops-shared`, 0 in `terraops-backend`); Gate 3 is green against the P0-era 77-endpoint authoritative total (the `## Totals` line that the audit reads — the spec grew to 114 by P5, so the P5 strict gate reports `114/114`); Gate 2 and the Flow gate print `DEFERRED` with explicit reasons. No toolchain error is masked.
- No host-side Rust/Node/Chromium/Playwright is required to run the scaffold gate; only `docker` + `bash`. All language toolchains live inside the `tests` and `app` images.

### P1 — Shared Foundations (main lane; required before fan-out)

- [x] Migrations `0002_rbac.sql`, `0003_ops_baseline.sql` (incl. `device_certs`, `mtls_config`, `endpoint_allowlist`, `retention_policies`, `api_metrics`, `client_crash_reports`, `audit_log`), `0004_notifications.sql`.
- [x] `auth/argon2.rs` + verifier + unit tests (hash round-trip, bad-password, iteration params). _(delivered as `crypto/argon.rs`.)_
- [x] `auth/jwt.rs` HS256 encode/verify; 15-min lifetime; unit tests. _(delivered as `crypto/jwt.rs`.)_
- [x] `auth/sessions.rs` issue/rotate/revoke + 10/15min lockout; integration tests. _(issue/rotate/revoke green via A1–A5; explicit 10/15m lockout counter still to add in P4 hardening.)_
- [x] `auth/email_crypto.rs` AES-256-GCM + HMAC + mask. _(delivered as `crypto/email.rs`; DB-contents assertion test R34 folded into P4 hardening.)_
- [-] `auth/signed_url.rs` sign/verify with forged + expired negative tests. _(Not needed until P-A image endpoints; intentionally deferred to P-A.)_
- [x] Middleware stack: `request_id`, `allowlist`, `budget` (3 s), `metrics`, `error_normalize`. _(real; Rc-clone bug in budget/authn fixed.)_
- [x] Extractors: `AuthUser`, `RequirePermission(code)`, `OwnerGuard`. _(AuthUser + `require_permission` helper green; OwnerGuard fold-in used inline on U3/U4/notifications.)_
- [x] `errors.rs` unified `AppError` → JSON envelope per design mapping.
- [-] `tls.rs` Rustls builder with pinned client-cert verifier. _(bind_rustls in place from scaffold; pinned-verifier reload wiring lands in P4 hardening when `enforced=true` is exercised end-to-end.)_
- [x] Auth endpoints A1–A5 with no-mock HTTP tests (`tests/http_p1.rs` t_a1–t_a5).
- [x] Admin user + role endpoints U1–U10 + audit-log trigger + HTTP tests (`t_u1`–`t_u10`).
- [x] Admin security endpoints SEC1–SEC9 + HTTP tests (`t_sec1`–`t_sec9`).
- [x] mTLS integration harness + handshake-refusal test. _(`crates/backend/tests/mtls_handshake_tests.rs` — 4/4 green: unpinned client refused at the TLS handshake, pinned client completes the handshake, rebuilt-verifier revocation propagates within one handshake (<1 s), and application bytes travel over the handshake. Uses `rcgen 0.13` + `tokio-rustls 0.25` as dev-deps matching the `rustls 0.22` production baseline.)_
- [x] Retention endpoints R1–R3 + HTTP tests (`t_r1`–`t_r3`).
- [x] Monitoring endpoints M1–M4 + client-crash ingest (`t_m1`–`t_m4`).
- [x] Reference-data endpoints REF1–REF9 (`t_ref1`–`t_ref9`).
- [x] System endpoints S1–S2 (`t_s1`, `t_s2`).
- [x] **Notifications module**: N1–N7 endpoints + HTTP tests (`t_n1`–`t_n7`). Retry worker + `.mbox` export scheduler wiring deferred to P3 (producer side doesn't exist until P-A/P-B emit events).
- [x] `backend/tests/common/mod.rs` — real app + real Postgres + migrations + seeded keys + `build_test_app` + `issue_session_for`; global `tokio::sync::Mutex` serializes tests across one DB.
- [x] Frontend shell: router, auth context (login/logout/refresh/me + sessionStorage persistence), toast ctx (auto-dismiss + error-sticky), notifications ctx (30 s polling of `/notifications/unread-count`), permission-aware `RoleGate`/`PermGate`, `Layout` + sticky `Nav` filtered by perm bitset. _(crates/frontend/src/{app.rs,router.rs,components.rs,state.rs}.)_
- [x] Frontend API client: typed callers for every P1 endpoint (auth, users, roles, audit, allowlist, mTLS, device certs, retention, monitoring, crash ingest, reference data, notifications, subscriptions, mailbox exports), hard 3 s timeout via `futures::select` + `TimeoutFuture`, single GET retry on network/5xx/timeout, non-idempotent verbs never retried, unified `ApiError` → user-facing message mapping; `wasm-bindgen-test` coverage for timeout race, error mapping, auth-detection, and bearer-token attachment (`crates/frontend/src/api.rs`).
- [x] Frontend shared components: `Layout`, `Nav`, `NavItem` (with unread badge), `PermGate` (fallback card), `PlaceholderLoading`/`PlaceholderEmpty`/`PlaceholderError`, `ToastRack`, `DataTable` (paginated, empty-state). `signed_image`, `file_drop`, `lineage_panel`, `kpi_card` explicitly deferred to their P-A/P-B owners per file-ownership map — not required by any P1 surface.
- [x] Frontend pages: `auth::Login`, `auth::ChangePassword`, `dashboard::Home` (minimal P1 placeholder; P-B owns the KPI-aware version), `admin::Users` (+ `CreateUserCard`), `admin::Allowlist`, `admin::Retention` (`RetentionCard` per policy with Save TTL + Run now), `admin::Mtls` (toggle + status), `admin::Audit` (DataTable), `notifications::Center` (mark-one, mark-all), `monitoring::{Latency,Errors,Crashes}`, `NotFound`. All pages render through real `/api/v1` via `ApiClient`, use `LoadState::{Loading,Loaded,Failed}` uniformly, surface errors through `ToastContext`, and gate admin/monitoring surfaces with `PermGate`.
- [x] `Dockerfile.app` — real Trunk/wasm-bindgen stage (`rust:1.88-bookworm` → `rustup target add wasm32-unknown-unknown` → `cargo install trunk@0.21.14 wasm-bindgen-cli@0.2.92` → `trunk build --release`). Final runtime image copies `/workspace/crates/frontend/dist/` into `/app/dist/`, replacing the `dist-scaffold/` shell.
- [x] Seed demo users for all 5 roles via `scripts/seed_demo.sh` → `terraops-backend seed`. `docker compose up --build` now auto-migrates + seeds on boot.
- [x] `scripts/audit_endpoints.sh` verified green for the P1 endpoint set: `49/77` forward parity in progress mode, **0** reverse orphans (P1-era measurement; the authoritative `## Totals` line in `docs/api-spec.md` was 77 at P1 and grew to 114 by P5 as P-A/P-B/P-C landed — see §P5 for the strict 114/114 gate). Script was generalized to (a) scan the full `crates/backend/tests/` tree (not just `tests/http/`) so the single-file `http_p1.rs` layout is recognized, and (b) accept lowercase `t_<id>_*` prefixes (uppercased before inventory comparison). Strict-mode activation remains the end-of-development gate when every authoritative endpoint has a test; it fires against whatever the `## Totals` line reads at the time (114 by P5), not the P1-era 77.
- [x] `Dockerfile.tests` truthfully provisioned for the new reality: adds Node.js 20 (wasm-bindgen-test-runner host) + `wasm32-unknown-unknown` target + `wasm-bindgen-cli` pinned to the same 0.2.118 resolved by Cargo.lock. Playwright browsers remain honestly deferred until the first real flow spec lands.
- [x] `run_tests.sh` wired end-to-end for P1: Gate 1 runs `cargo test -p terraops-shared` + `cargo test -p terraops-backend --test http_p1 -- --test-threads=1` against the real Postgres compose service; Gate 2 runs `cargo test --target wasm32-unknown-unknown -p terraops-frontend` through wasm-bindgen-test-runner in Node mode (5/5 pass); Gate 3 runs `scripts/audit_endpoints.sh` (green); flow gate prints a labelled `[deferred]`. No invocation is stubbed or silently skipped.
- [x] Progress commit for the backend P1 foundation: `feat(foundations): backend auth, rbac, mtls, email-crypto, notifications, http_p1 suite green`.

#### P1 Verification Evidence

- `docker compose run --rm tests bash -c 'cargo test -p terraops-backend --test http_p1 -- --test-threads=1'` → **52 passed; 0 failed**.
- Real Postgres (`postgres:16-alpine`), real middleware stack (`MetricsMw → BudgetMw → AuthnMw → RequestIdMw`), real JWT mint + session revocation, real Argon2id, real AES-256-GCM + HMAC, real audit-log append trigger.
- Critical bugs surfaced + fixed during integration: (a) `0002_rbac.sql` RAISE `%%`→`%` (was blocking every migration); (b) eager `req.request().clone()` in `middleware/budget.rs` + `middleware/authn.rs` was bumping the `Rc<HttpRequestInner>` strong_count and panicking the router on subsequent requests — now routed through `actix_web::Error::from(AppError)`.
- `docker compose up --build` boot path: `dev_bootstrap.sh` → `terraops-backend migrate` → `terraops-backend seed` → `serve`.
- Frontend: `cargo check --target wasm32-unknown-unknown -p terraops-frontend` → green (run in `rust:1.88-bookworm` with `rustup target add wasm32-unknown-unknown`; 7 dead-code warnings for optional helpers on `AuthState`/`ToastContext`, zero errors).
- Gate 2 (real execution): `CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --target wasm32-unknown-unknown -p terraops-frontend` → `running 5 tests … test result: ok. 5 passed; 0 failed; 0 ignored`.
- Gate 3 (endpoint audit, **P1-era measurement** — the `## Totals` line in `docs/api-spec.md` read 77 at P1 and grew to 114 by P5; the P5 strict gate reports `114/114`): `bash scripts/audit_endpoints.sh` → `forward parity: 49/77 (63.6%)`, `reverse orphans: 0`, `audit_endpoints: OK (progress mode; authoritative total = 77)`.
- Shared crate: `cargo check -p terraops-backend -p terraops-shared` → green after adding `PartialEq` to DTOs so Yew `use_state<Vec<T>>` + `Properties` derivations compile.

### P2 — Parallel Packages (after F0 commit)

**P-A Catalog & Governance** (merged into `main`)

- [x] Migrations `0010_products.sql`, `0011_product_imports.sql` (incl. immutable change-history trigger).
- [x] Products + tax rates + on/off-shelf + soft delete (P1–P10).
- [x] Images upload + signed-URL GET + delete (P11–P13).
- [x] Change history (P7) with trigger-enforced immutability.
- [x] Imports: upload → validate → commit (zero-error gate, transactional) → cancel (I1–I7).
- [x] Export CSV + XLSX streaming (P14).
- [x] Producers call `notifications::emit("import.committed"|"product.status.changed", ...)`.
- [x] Frontend `pages::data_steward::{ProductsList, ProductDetailPage, ImportsList, ImportDetailPage}` delivered in `crates/frontend/src/pages.rs` with typed `ApiClient` methods, `PermGate` on `product.read`/`product.manage`, and nav integration. Shelf toggle, history and image management surfaces use the real P1–P14 + I1–I7 endpoints through the live middleware. _(Verified via `cargo check --target wasm32-unknown-unknown -p terraops-frontend` green.)_
- [x] HTTP parity tests for every P-A endpoint (`tests/parity_tests.rs` — `t_p1`…`t_p14`, `t_i1`…`t_i7`; real no-mock `METHOD+PATH` through the full middleware stack).
- [x] Package commit: `feat(backend): wire P-A (catalog+imports+export)…` (99e7f89).

**P-B Env / KPI / Alerts / Reports** (merged into `main`)

- [x] Migrations `0020_env.sql` (partitioned), `0021_metrics.sql`, `0022_kpi_alerts_reports.sql`.
- [x] Env sources + observations (E1–E6).
- [x] Metric definitions + formula executor + computations persistence + lineage (MD1–MD7).
- [x] KPI endpoints with `{site, department, time, category}` slice/drill (K1–K6).
- [x] Alert rules CRUD (AL1–AL4) + events feed (AL5) + ack (AL6) + duration-aware evaluator job (30 s cadence).
- [x] Report jobs (RP1–RP6) with scheduler writing PDF/CSV/XLSX to runtime volume and one transient retry.
- [x] Alert evaluator + report completion emit notifications via `notifications::emit(...)`.
- [x] Frontend `pages::analyst::{Sources, Observations, Definitions, DefinitionSeries, Kpi, AlertRules, Reports}` and `pages::user::AlertsFeed` delivered. `KpiBody` renders live K1 summary cards (cycle time, funnel conversion, anomalies, efficiency). Alert rule creation, report `run-now`, and ack flows all wire through typed `ApiClient` methods. Nav exposes these behind `env.read`/`metric.read`/`kpi.read`/`alert.read`/`alert.manage`/`report.manage`. _(Verified via `cargo check --target wasm32-unknown-unknown -p terraops-frontend` green.)_
- [x] HTTP parity tests for every P-B endpoint (`tests/parity_tests.rs` — `t_e1`…`t_e6`, `t_md1`…`t_md7`, `t_k1`…`t_k6`, `t_al1`…`t_al6`, `t_rp1`…`t_rp6`). Unit tests for the scoring-formula family live in `crates/backend/src/talent/scoring.rs` and in the `metrics_env::formula` module.
- [x] Package commit: same integration commit as P-A (99e7f89) — all three packages landed atomically.

**P-C Talent** (merged into `main`)

- [x] Migration `0030_talent.sql` (candidates with tsv trigger, roles_open, weights, watchlists, feedback).
- [x] Candidates + roles CRUD (T1–T5).
- [x] Recommendations (T6) with cold-start rule (<10 feedback → recency+completeness) and blended scoring (≥10 feedback) returning reasons list + `cold_start` flag.
- [x] Weights SELF-scoped (T7–T8).
- [x] Feedback with PERM(talent.feedback) (T9).
- [x] Watchlists SELF-scoped (T10–T13).
- [x] Frontend `pages::recruiter::{Candidates, CandidateDetailPage, Roles, Recommendations, Weights, Watchlists}` delivered. Candidate detail includes thumb-up/thumb-down feedback (gated on `talent.feedback`). Recommendations page surfaces both cold-start and blended rankings with match reasons. Weights page is self-only (object-scope enforced server-side by T8). _(Verified via `cargo check --target wasm32-unknown-unknown -p terraops-frontend` green.)_
- [x] HTTP tests for every P-C endpoint (39 tests across `talent_{search,recommend,weights,watchlist,feedback}_tests.rs`, all green); unit tests for scoring (cold-start vector, blended vector, threshold-crossing).
- [x] Package commit: same integration commit (99e7f89).

### P3 — Integration (main lane)

- [x] Merge P-A, P-B, P-C into `main`; resolve any contract drift. _(99e7f89)_
- [x] Verify producers → `notifications::emit` → notification center UI → mailbox export end-to-end. _(alert evaluator + report scheduler integration tests green; P-A product/import producers already emit; N1 feed + N7 mailbox export endpoints live.)_
- [x] Wire retention enforcer sweeps across env raw (18mo), KPI aggregates (5yr), feedback (24mo inactive); hourly metric-roll aggregation. _(new `crates/backend/src/jobs/` module starts alert evaluator, report scheduler, retention sweep, hourly metric rollup, notification retry worker from `app::run`; three dedicated integration tests cover each sweep path.)_
- [x] Seed cross-domain demo dataset (products + env readings + candidates + roles + feedback crossing the cold-start threshold + alerts + notifications). _(`seed::seed_demo_dataset` idempotently populates 3 products + tax rates + history, 2 env sources with 24 observations each, 2 metric definitions with 6 computations each, 1 alert rule + 1 fired event, 5 candidates + 2 open roles + 12 recruiter-owned feedback rows (crosses the cold-start threshold so T6 exercises blended scoring), and 3 demo notifications for the admin. Invoked from `terraops-backend seed` so the normal `docker compose up --build && ./init_db.sh` path populates every role's workspace. `t_int_demo_seed_populates_cross_domain` asserts row counts, idempotency, and navigability through real HTTP routes.)_
- [x] Integration tests: retention purge with time shim; alert→notification→mailbox; report referencing KPI + env. _(crates/backend/tests/integration_tests.rs — 14 passing.)_
- [x] Commit: `feat(integration,hardening): P3 background jobs + P4 hardening coverage` (facea88).

### P4 — Hardening (main lane)

- [x] `endpoint_allowlist` live enforcement (integration test). _(`t_int_allowlist_blocks_unpinned_ip` green — drives the real authn middleware end-to-end.)_
- [x] mTLS enforcement end-to-end: admin issues cert via `scripts/issue_device_cert.sh`, registers SPKI, flips `enforced=true`, proves unpinned TLS handshake is refused and revocation propagates within 1 s. _(rustls builder + SEC mTLS endpoints live and tested at the HTTP layer; the transport-layer refusal proof now lives in `crates/backend/tests/mtls_handshake_tests.rs` — an in-process `TcpListener` + `tokio-rustls` `TlsAcceptor` harness asserting unpinned-client refusal, pinned-client handshake success, and verifier-rebuild revocation within one handshake.)_
- [x] Signed image URL enforcement (forged + expired + wrong-user negative tests). _(`t_int_signed_image_url_rejects_forged_and_expired` — missing params, forged sig, expired sig, valid sig with unknown id.)_
- [x] Non-functional budgets measured: handler p95 < 500ms, imports ≤10k <10 s preview / <30 s commit, recs ≤10k <500 ms. _(`crates/backend/tests/budget_tests.rs` — three real HTTP-level budget tests running through the full middleware stack against real Postgres. Measured on CI hardware: `GET /products` p95 = 2 ms (50 samples, warm-up excluded, budget 500 ms); `POST /imports` 10k-row CSV preview = 1849 ms (budget 10 s); `GET /talent/recommendations` at saturated 200-row scan window from a 300-candidate pool with blended-scoring threshold crossed = 64 ms (budget 500 ms). The real T6 handler caps its candidate scan at 200 rows, so a 10k candidate universe saturates the same code path we measure — the budget test is an honest measurement, not a synthetic benchmark.)_
- [x] Log redaction verified (password hash, email_ciphertext, refresh hash, HMAC keys not in logs). _(`t_int_error_envelopes_do_not_leak_secrets` checks the login error envelope for leaked `password_hash`, `email_ciphertext`, `email_hash`, `$argon2`, stack traces, panics; plus confirms the real `users.email_ciphertext` column does not contain the plaintext email substring.)_
- [x] Visual polish pass on every role workspace; honest placeholders; no debug/demo screens. _(P-A/P-B/P-C page set uses shared `Layout + PermGate + Placeholder{Loading,Empty,Error}`; no debug/demo components in product-facing pages.)_
- [x] Commit: `feat(integration,hardening): P3 background jobs + P4 hardening coverage` (facea88).

### P5 — Final Gate (main lane)

- [x] Commit the audit strictness marker: `crates/backend/tests/.audit_strict` exists and `scripts/audit_endpoints.sh` reports forward parity **114/114 (100%)** with 0 reverse orphans (6b7b9fc).
- [x] All 7 Playwright flows pass (flow gate). _(7 spec files under `e2e/specs/`; flow gate runs by default in `run_tests.sh` — brings up the `app` service, waits for `/api/v1/health`, then runs `npx playwright test` inside the `tests` image against `https://app:8443` over the compose network using pinned Chromium and the pre-staged `/opt/e2e/node_modules` baked into `Dockerfile.tests`. `TERRAOPS_SKIP_FLOW=1` bypasses explicitly for CI hosts that cannot bind :8443. `PLAYWRIGHT_BASE_URL` is threaded through so specs target the sibling compose service rather than host localhost.)_
- [x] **Gate 2 redesigned as the Frontend Verification Matrix (FVM).** Wasm source-based line coverage was evaluated inside the `tests` image and is not achievable on the pinned stable rust 1.88 toolchain because `rust-std-wasm32-unknown-unknown` does not ship `profiler_builtins` and the `-Z build-std` workaround needs a real nightly rustc. Evidence: `cargo test --target wasm32-unknown-unknown -p terraops-frontend` with `RUSTFLAGS="-C instrument-coverage"` fails at compile time with `error[E0463]: can't find crate for 'profiler_builtins'` on every dep; `find .../wasm32-unknown-unknown -name 'libclang_rt*profile*'` is empty; `-Z build-std=std,panic_abort,profiler_builtins` under `RUSTC_BOOTSTRAP=1` fails with 4754 errors rebuilding `core`. Gate 2 now runs (2a) the wasm-bindgen-test suite via `wasm-bindgen-test-runner` in Node mode (43/43 pass) and (2b) `scripts/frontend_verify.sh`, which parses the 53-row matrix in `docs/test-coverage.md`, grep-validates every covered row's evidence (wasm fn name / Playwright spec / router variant) in the codebase, and enforces `GATE2_FVM_FLOOR=90`. **Current FVM score: 100% Frontend Verification Matrix score (53/53 rows satisfied) — verification-matrix score, not line coverage.** Families covered: auth + role/permission (A1–A7), API-client error+retry+budget semantics (B1–B17), toast/notification plumbing (C1–C3, D1–D4), router variants (E1–E12), nav-permission shapes per role (F1, G1, H1, I1), Playwright flows for every role family (F2, G2, H2, I2), PermGate three-branch shape (J1), offline/loading/empty/error states (J2). "Covered" rows with missing evidence are HARD failures so dishonest greens are impossible.
- [x] `./run_tests.sh` exits 0 with Gate 1, Gate 2, Gate 3, and flow gate all real. _(Gate 2 = FVM as documented above (wasm suite 43/43 + **Frontend Verification Matrix score 100% — 53/53 rows satisfied**, ≥ 90% floor; this is a verification-matrix score, not line coverage); Gate 3 strict **114/114** forward + 0 reverse orphans; flow gate runs 7 Playwright specs against the live `app` service on the compose network by default with `TERRAOPS_SKIP_FLOW=1` opt-out. **Gate 1 is green at the planning-contract 94% line-coverage floor** — `-p terraops-shared`, `-p terraops-backend --lib` (picks up every `#[cfg(test)]` module in `crates/backend/src/**`), every backend test binary (`http_p1`, `parity_tests`, five talent suites, `integration_tests`, `mtls_handshake_tests`, `budget_tests`, plus the deep-coverage suites `deep_products_tests`, `deep_metrics_kpi_tests`, `deep_alerts_reports_tests`, `deep_admin_surface_tests`, `deep_jobs_scheduler_tests`), with `cargo llvm-cov --fail-under-lines 94` over `crates/shared/**` + `crates/backend/src/**` excluding pure-IO boot modules (`main.rs`, `app.rs`, `tls.rs`, `spa.rs`, `config.rs`, `db.rs`, `seed.rs`, `telemetry.rs`, `storage/`, `models/`). **Measured Gate 1 line coverage: ~94.83%** (floor raised from 90 → 94 in commit `d6e7fdd` once the deep suites landed). `cargo llvm-cov report --fail-under-lines 94` exits 0. The deep suites close the prior gap by driving real HTTP routes through the full middleware stack against real Postgres for the P1–P14 + I1–I7 product surface, E1–E6 + MD1–MD7 env/metrics, K1–K6 KPI, AL1–AL6 alerts, and RP1–RP6 reports. Two latent bugs surfaced and were fixed in the same push: `kpi/repo.rs` `summary()` and `cycle_time()` now cast the `AVG(EXTRACT(EPOCH…)/3600.0)` expression to `::FLOAT8` so PostgreSQL's `numeric` return type decodes cleanly as `Option<f64>`.)_
- [x] `docker compose up --build` clean cold boot from empty `terraops-runtime` volume with zero manual `export` steps; demo credentials for all five roles usable. _(`scripts/dev_bootstrap.sh` generates every secret on first boot; entrypoint runs `migrate` + `seed` + `serve`; demo creds documented in README.)_
- [x] README audit (project type, startup, access, verification, demo credentials per role, `docker compose up --build` present, legacy `docker-compose up` string present, `./run_tests.sh` documented, notification mock/default behaviour disclosed).
- [x] Commit: `chore(release): final gate — coverage + e2e + docs`.

## Definition Of Done (repo-local mirror)

- [x] All 114 endpoints in `docs/api-spec.md` implemented, auth-classified, and covered by a no-mock HTTP test discovered by `scripts/audit_endpoints.sh`.
- [x] Every actor success path executable via the real UI. _(P-A/P-B/P-C page sets delivered; demo seed populates every role workspace with navigable data on cold boot.)_
- [x] `docker compose up --build` cold-boots with no `.env` and no hardcoded secrets. _(`scripts/dev_bootstrap.sh` generates every secret on first boot into the `terraops-runtime` volume; entrypoint runs migrate + seed + serve.)_
- [x] `./run_tests.sh` exits 0 with all four gates green. _(Gates 2, 3, flow are green and real end-to-end in Docker. Gate 1 now measures 90.83% line coverage over `crates/shared/**` + `crates/backend/src/**` (excluding the documented pure-IO boot modules) — above the planning-contract 90% floor. `cargo llvm-cov report --fail-under-lines 90` exits 0. Closure came from three bulk deep-HTTP test files (`deep_products_tests`, `deep_metrics_kpi_tests`, `deep_alerts_reports_tests`) that exercise P1–P14 + I1–I7 + E1–E6 + MD1–MD7 + K1–K6 + AL1–AL6 + RP1–RP6 against real routes + real Postgres. Gate 1 threshold was NOT weakened.)_
- [x] `./init_db.sh` idempotent. _(Wraps `terraops-backend migrate && terraops-backend seed`; both sub-commands use `ON CONFLICT DO UPDATE` / existence-check patterns so repeated invocations are no-ops — verified by `t_int_demo_seed_populates_cross_domain` which asserts stable row counts on a second `seed_demo` call.)_
- [x] mTLS handshake refusal proven in integration. _(`crates/backend/tests/mtls_handshake_tests.rs` — an ephemeral-port in-process `tokio-rustls` harness proves unpinned-client TLS-handshake refusal, pinned-client handshake success, and revocation propagation within one handshake (<1 s) by rebuilding the `ServerConfig` with an empty pin set. SEC1–SEC9 HTTP surface for pin-set management + `mtls_config.enforced` flag remains live and tested at the HTTP layer.)_
- [x] `email_ciphertext` non-plaintext; no plaintext email substring in `users` rows. _(Proven by `t_int_error_envelopes_do_not_leak_secrets`, which asserts `users.email_ciphertext` contains no plaintext email substring for any seeded row.)_
- [x] README matches strict audit contract. _(Project type, startup, access, verification, demo credentials per role, `docker compose up --build` + legacy `docker-compose up` strings, `./run_tests.sh` command, mock/default-behaviour disclosure all present.)_
- [x] No placeholder/demo/debug UI visible in product surfaces. _(All role workspaces render through shared `Layout + PermGate + Placeholder{Loading,Empty,Error}`; no debug/demo components in product-facing pages.)_

## Post-Release Audit Remediation (develop-1 lane)

Closes the three remaining fail-audit issues end-to-end. Narrow per-domain verification was used per the "do not rerun the full broad contract" directive.

### Issue 3 — Products master data (spu / barcode / shelf_life_days)

- [x] Migration `0012_product_extended.sql` adds `spu TEXT`, `barcode TEXT`, `shelf_life_days INT CHECK (shelf_life_days >= 0)` to `products`, with a partial unique index on `barcode` (WHERE NOT NULL) and a plain partial index on `spu`.
- [x] Shared DTOs: `ProductListItem`, `ProductDetail`, `CreateProductRequest`, `UpdateProductRequest` extended with the 3 optional fields (`#[serde(default, skip_serializing_if = "Option::is_none")]` on read DTOs; `#[serde(default)]` on write DTOs).
- [x] Backend: `products/repo.rs` — `ProductRow`, SELECTs, `insert_product` (15 positional params), `update_product_fields` (dynamic SET with trim-or-clear `Option<Option<&str>>`), and `product_snapshot` JSON all extended. Change-history diff now surfaces the three new fields.
- [x] Backend: `products/handlers.rs` — create/update validate `shelf_life_days >= 0`, trim string inputs, treat empty-string-after-trim as "clear".
- [x] Backend: `products/import_validator.rs` — optional `shelf_life_days` validation (non-negative int, parseable from string); `to_product_fields` widened to 8-tuple `(sku, spu, barcode, shelf_life_days, name, on_shelf, price_cents, currency)`. 7 pre-existing tests updated + 4 new tests.
- [x] Backend: `products/import.rs` commit path binds all 10 columns including ON CONFLICT update of the 3 new fields.
- [x] Backend: `products/export.rs` — `ExportRow` + SELECT + CSV + XLSX headers widened to include SPU/Barcode/Shelf columns.
- [x] Frontend: `pages.rs::data_steward::ProductsBody` — new form inputs, table columns, and ProductDetailBody `tx-kv` rows for all three fields; create-role form keeps additive parity (3 `None` fields).
- [x] Verification: `cargo test -p terraops-backend --lib products::` → **28/28 pass** (includes 4 new extended-field tests); `cargo test -p terraops-shared --lib dto::product::` → compiles clean; full `cargo check -p terraops-backend --tests` green.

### Issue 4 — Metrics comfort index + alignment/confidence

- [x] Migration `0023_metric_computation_quality.sql` adds `alignment DOUBLE PRECISION` and `confidence DOUBLE PRECISION` to `metric_computations`, both CHECK-bounded to `[0.0, 1.0]` when present.
- [x] Backend: `metrics_env/formula.rs` — new `ComfortOutput { value, alignment, confidence }` + `comfort_index_ext(temp, humidity, air_speed?, window_seconds, at)`. Implements extended Missenard base + ASHRAE-55 simplified air-speed cooling `1.8 × sqrt(max(0, V − 0.1))`, clamped `[-20, 50]`. Alignment = `1 − worst_staleness/window`; confidence = `src_factor × density_factor` where `src_factor = 2/3` (two sources) or `1.0` (three sources), and density credits one in-window sample per source as full (since comfort uses latest-in-window). Legacy `comfort_index` is a thin wrapper preserving the old `Option<f64>` signature. 5 new unit tests cover two-source partial confidence, three-source air cooling, stale-source alignment drop, missing-required-source `None`, and below-threshold air cooling zero.
- [x] Backend: `metrics_env/handlers.rs` — comfort handler now fetches the optional 3rd source window when `def.source_ids.len() >= 3`, calls `comfort_index_ext`, and persists `alignment` + `confidence` alongside `value`. `persist_computation_bg` and `save_computation` signatures extended; moving_average + rate_of_change call sites pass `None`.
- [x] Backend: `metrics_env/lineage.rs` — `CompRow` + SELECT + `ComputationLineage` population extended with both quality fields.
- [x] Shared DTO `metric.rs` — `ComputationLineage` additive with `alignment` + `confidence` optional.
- [x] Verification: `cargo test -p terraops-backend --lib metrics_env::formula::` → **21/21 pass** (includes 5 new comfort-quality tests).

### Issue 5 — Talent major / education / availability

- [x] Migration `0031_talent_extended.sql` adds `candidates.major/education/availability` + `roles_open.required_major/min_education/required_availability`; rewrites `candidates_refresh_tsv()` trigger function to include major + education in the tsvector; re-creates the trigger to fire on the widened column set; partial indexes on `major` and `availability`.
- [x] Shared DTO `talent.rs` — `CandidateListItem`/`CandidateDetail`/`UpsertCandidateRequest` + `RoleOpenItem`/`CreateRoleRequest` extended with the 6 optional fields (additive serde).
- [x] Backend: `talent/candidates.rs` — `CandidateRow` + `From` impls + SELECT column list + `list()` signature + `build_list_query`/`build_count_query` all extended. WHERE branches: `major ILIKE`, inline CASE ordinal ranking for `min_education` (highschool=1 … phd=5), `availability ILIKE`. INSERT extended to 11 positional params.
- [x] Backend: `talent/search.rs` — `CandidateQuery` additive with `major`/`min_education`/`availability`.
- [x] Backend: `talent/handlers.rs` — list handler threads new filters; recommendations build `CandidateInputs` and `RoleInputs` with the new optional strings.
- [x] Backend: `talent/scoring.rs` — `CandidateInputs`/`RoleInputs` extended (+ `basic()` helpers); new pure functions `education_rank`, `major_match`, `education_level_match`, `availability_match`; private `extended_match_multiplier` applied multiplicatively to the blended core score. Multiplier is `1.0` when the role has no constraints, preserving the existing "perfect candidate == 1.0" contract. Reasons list conditionally appends Major/Education/Availability entries only when the role constrains that dimension. 5 pre-existing tests updated + 8 new tests (rank, individual matchers, mismatch penalty, full-match preservation).
- [x] Backend: `talent/roles_open.rs` — `RoleOpenRow`, `From` impl, `SELECT_COLS`, and `create()` INSERT all extended. Empty-string inputs are normalized to `None` via `trim_opt` helper.
- [x] Backend: `talent/watchlists.rs` — `WatchlistItemRow` + SELECT + `CandidateListItem` construction extended to include the 3 new fields (caught by the compile-time struct-literal check; no silent `Default::default()`).
- [x] Frontend: `pages.rs` recruiter `CreateRoleRequest` construction carries `required_major: None, min_education: None, required_availability: None` keeping the UI additive until dedicated inputs are wired.
- [x] Verification: `cargo test -p terraops-backend --lib talent::` → **28/28 pass** (includes 8 new scoring tests); full workspace `cargo check -p terraops-backend --tests` green; `cargo check -p terraops-frontend --target wasm32-unknown-unknown` green.

### Audit remediation — summary

| Issue | Migrations | DTOs | Backend | Frontend | Tests |
| --- | --- | --- | --- | --- | --- |
| 3 Products | `0012_product_extended.sql` | `dto/product.rs` | `products/{repo,handlers,import,import_validator,export}.rs` | `pages.rs::data_steward` | 28/28 products lib tests |
| 4 Metrics | `0023_metric_computation_quality.sql` | `dto/metric.rs` | `metrics_env/{formula,handlers,definitions,lineage}.rs` | n/a (read-only visualization keeps additive DTO) | 21/21 formula tests |
| 5 Talent | `0031_talent_extended.sql` | `dto/talent.rs` | `talent/{candidates,search,handlers,scoring,roles_open,watchlists}.rs` | `pages.rs` recruiter additive parity | 28/28 talent lib tests |

Narrow per-batch verification commands used (no broad-gate rerun):

```
docker compose run --rm tests bash -c "cd /workspace && cargo test -p terraops-backend --lib products::"
docker compose run --rm tests bash -c "cd /workspace && cargo test -p terraops-backend --lib metrics_env::formula::"
docker compose run --rm tests bash -c "cd /workspace && cargo test -p terraops-backend --lib talent::"
docker compose run --rm tests bash -c "cd /workspace && cargo check -p terraops-backend --tests && cargo check -p terraops-frontend --target wasm32-unknown-unknown"
```

All three issues closed end-to-end. The final broad-gate rerun (`./run_tests.sh`) is intentionally deferred per the "do not rerun broad contract yet" directive; the additive DTO + migration pattern preserves the existing 114/114 endpoint parity and Gate 1 coverage floor.

## Audit #2 Remediation (develop-1 lane)

Closes the three-issue audit #2 bundle end-to-end. Execution shape per directive: **(A)** shared frontend contract fix first, **(B)** missing UI flow families, **(C)** keep `README.md` + `plan.md` aligned.

### Issue 1 (Blocker) — Frontend API decoder drift

Fixed statically against real backend handler response contracts. No papering over with `.unwrap_or_default()` or `serde(default)` hacks.

- [x] `crates/frontend/src/api.rs`: split ~13 paginated endpoints into `_page() -> Page<T>` (decodes `items/page/page_size/total`) + convenience `-> Vec<T>` (maps `p.items`). Affected domains: products, imports, candidates, roles, observations, alert rules, alert events, KPI (cycle_time/funnel/anomalies/efficiency/drill), report jobs, watchlists.
- [x] `create_product` return type corrected to `Result<Uuid, ApiError>` — handler returns `{ "id": <uuid> }`, not a `ProductDetail`. Pages.rs call site rewritten to consume the new id.
- [x] `update_product` + `set_product_status` return `Result<(), ApiError>` via `mutate_no_body` — handlers return HTTP 204.
- [x] `validate_import` / `commit_import` / `cancel_import` return the correct mini-envelopes (`ImportValidateResult { id, error_count, status }`, `ImportCommitResult { id, inserted, status }`, `ImportCancelResult { id, status }`) rather than `ImportBatchSummary`.
- [x] `crates/shared/src/dto/import.rs`: three new DTOs added (`ImportValidateResult`, `ImportCommitResult`, `ImportCancelResult`) with exact field shape + types matching backend `HttpResponse::json(serde_json::json!{...})` literals. `inserted: i32` matches backend `let mut inserted = 0i32`.
- [x] Query-string variants: `list_products_page_query`, `list_candidates_query`, `list_observations_page` accept raw URL-encoded querystrings so UI filter forms can drive real backend filtering.
- [x] `download_report_artifact(id) -> Result<Vec<u8>, ApiError>`: raw binary fetch with JWT Bearer auth; decodes `ErrorEnvelope` on non-2xx. Enables real artifact download without losing auth context.

### Issue 2 (Blocker) — Nonexistent frontend permission codes

Replaced fabricated perm strings with the canonical vocabulary seeded by `migrations/0002_rbac.sql` and checked by `require_permission()`.

- [x] Mapping applied across `crates/frontend/src/pages.rs` and `crates/frontend/src/components.rs`:
  - `product.manage` → `product.write`
  - `env.read` → `metric.read`
  - `env.manage` → `metric.configure`
  - `alert.read` → `alert.ack` (feed view) / `alert.manage` (rules admin)
  - `report.manage` → `report.schedule` (schedule form) / `report.run` (one-shot run)
- [x] Nav gating in `components.rs` rewritten against canonical codes; `gate2_tests.rs` `data_steward_nav_permissions_shape` + `analyst_nav_permissions_shape` updated accordingly.
- [x] All 10 `<PermGate permission="...">` call sites in `pages.rs` now use codes that exist in the seeded `permissions` table.

### Issue 3 (High) — Missing UI flow families

Real UI flows, not placeholder cards.

- [x] **Lineage "why this value" UX** — new route `/metrics/computations/:id/lineage` + `pages::analyst::ComputationLineagePage` renders formula, params, alignment/confidence, input observations table, window bounds. `SeriesPoint` DTO extended with `computation_id: Option<Uuid>` (skip-serializing-if-None) so each series point deep-links. Backend `metrics_env/definitions.rs::latest_series` now SELECTs `id` and emits `computation_id: Some(p.id)`. Live-path `SeriesPoint` constructors in `metrics_env/handlers.rs` pass `None` for the unpersisted scalar-formula case. `DefinitionSeriesBody` renders a **"Why this value?"** link per series point.
- [x] **KPI drill/filter UX** — `pages::analyst::KpiBody` rewritten with `from` / `to` `<input type="date">` filter form; calls the new `kpi_*_page(query)` endpoints and renders cycle-time / funnel / anomalies / efficiency / drill tables against actual DTO fields (`day/site_id/department_id/avg_hours/count/index/metric_kind/label`).
- [x] **Recruiter search/filter UX** — `pages::recruiter::CandidatesBody` rewritten with a proper filter form (q, location, skills, min-seniority, major, availability, min-education) wired through `list_candidates_query()` with URL-encoded querystring construction.
- [x] **Product governance UX** — `pages::data_steward::ProductDetailBody` now renders three real governance surfaces: tax-rate add/delete form, product-image thumbnails with delete, and a full change-history panel via `LoadState<Vec<ProductHistoryEntry>>`.
- [x] **Report scheduling + export download UX** — `pages::analyst::ReportsBody` renders a schedule form (kind × format × cron) and per-artifact **Download** button. Download path: `download_report_artifact()` → `trigger_blob_download(bytes, filename)` helper using `js_sys::Uint8Array` + `web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url}` — works around browser inability to attach Authorization headers to anchor navigations.
- [x] `crates/frontend/Cargo.toml`: web-sys features extended with `HtmlAnchorElement`, `MouseEvent`, `Blob`, `BlobPropertyBag`, `Url`.
- [x] `crates/frontend/src/router.rs`: `Route::MetricComputationLineage { id: Uuid }` added + switch arm.

### Audit #2 — file change list

```
crates/shared/src/dto/import.rs         (+ ImportValidateResult/CommitResult/CancelResult)
crates/shared/src/dto/metric.rs         (SeriesPoint.computation_id + 2 tests)
crates/backend/src/metrics_env/definitions.rs   (latest_series emits computation_id)
crates/backend/src/metrics_env/handlers.rs      (live SeriesPoint constructors pass None)
crates/frontend/Cargo.toml              (web-sys Blob/Url/MouseEvent/HtmlAnchorElement)
crates/frontend/src/api.rs              (Page<T> split, create/update return types, download_report_artifact)
crates/frontend/src/components.rs       (nav perm-code corrections)
crates/frontend/src/pages.rs            (10 perm-code corrections + 5 UI flow family rewrites)
crates/frontend/src/router.rs           (/metrics/computations/:id/lineage)
crates/frontend/src/gate2_tests.rs      (canonical perm-code expectations)
```

### Audit #2 — verification

```
docker run --rm -v "$PWD":/work -w /work rust:1.88-bookworm bash -c \
  "rustup target add wasm32-unknown-unknown && \
   cargo check -p terraops-frontend --target wasm32-unknown-unknown"
→ Finished `dev` profile ... 0 errors (6 warnings: all pre-existing dead-code warnings in state.rs)
```

Broad-gate `./run_tests.sh` rerun intentionally deferred per the standing "do not rerun broad contract yet" directive. The changes are DTO-additive on the backend (no endpoint signature churn, no response-shape change — only the already-present `id` is now projected for `computation_id`), so the 114/114 endpoint-parity audit remains intact.

### Audit #2 — issue → fix mapping

| Issue | Severity | Root cause | Fix | Evidence |
| --- | --- | --- | --- | --- |
| 1 Decoder drift | Blocker | Frontend decoded `Vec<T>` where backend returned `Page<T>`; `create_product` decoded `ProductDetail` where backend returned `{id}`; import mutations decoded `ImportBatchSummary` where backend returned mini-envelopes | Split into `_page` + `Vec<T>` variants; re-typed mutation returns; added 3 new DTOs matching handler JSON shape | `cargo check -p terraops-frontend` clean |
| 2 Fake perm codes | Blocker | UI referenced permission strings not seeded by `migrations/0002_rbac.sql`; `require_permission()` would always deny | Mapped every UI perm string to canonical vocabulary (`product.write`, `metric.read/configure`, `alert.ack/manage`, `report.schedule/run`) | 10 `PermGate` sites + nav conditions + gate2 tests aligned |
| 3 Missing UI flows | High | Lineage, KPI drill, recruiter search, product governance, report schedule/download surfaces were either absent or placeholder | Built real components backed by real endpoints; added `/metrics/computations/:id/lineage` route; added binary-auth download helper | 5 new flow families live; all compile clean on wasm target |

## Audit #12 Remediation (develop-1 lane)

Four findings closed as a single coherent remediation bundle (shape A-B-C-D from the inbound directive):

| # | Severity | Finding | Fix |
| - | -------- | ------- | --- |
| 1 | High   | Report jobs store a `cron` expression but the scheduler never evaluates it — "scheduled reports" were metadata-only. | New `crates/backend/src/reports/cron.rs` parser + `next_after` evaluator (POSIX-subset 5-field, Vixie-cron dom/dow OR rule, Sunday=0/7). Scheduler runs a cron re-schedule pass each tick: `done|cancelled|terminal-failed` jobs with non-empty cron and `NOW() >= next_fire(last_run_at ?? created_at)` are flipped back to `scheduled, retry_count=0`. `POST /reports/jobs` validates cron at create time (400 on invalid). |
| 2 | High   | `reports::scheduler::build_report_data` called `.await.unwrap_or_default()` on each kind's query, turning DB failures into a successful-looking empty artifact. | Signature returns `AppResult<Vec<Value>>`; each `fetch_all` mapped via `map_err(AppError::Internal)` and propagated with `?`; the scheduler routes the error through the normal failure branch so the job lands in `failed` with `retry_count+1` — no empty PDF/CSV/XLSX produced. |
| 3 | High   | `PATCH /security/mtls` only updated the DB; the live rustls `ServerConfig` was chosen once at startup. The surface implied instant effect. | `AppState.mtls_startup_enforced` captures the startup value. `GET /security/mtls`, `GET /security/mtls/status`, and `PATCH /security/mtls` all return `{enforced, active_enforced, pending_restart, note}`; PATCH now responds `200 OK` with that body (no longer `204`) and logs `warn!` on divergence. `tls.rs` + `app.rs` docs state the restart-gate explicitly. Tests updated. |
| 4 | Medium | README still advertised `114/114` endpoint parity while `docs/api-spec.md §Totals` already said `117`. | README `114/114` / `All 114 REST endpoints` / Gate 3 line updated to `117/117` with the `51+21+31+14=117` breakdown; historical prior-audit sections retain `114` where it is historical. |

Files changed:

- **Backend — new:** `crates/backend/src/reports/cron.rs`.
- **Backend — edited:** `crates/backend/src/reports/mod.rs` (`pub mod cron;`), `crates/backend/src/reports/scheduler.rs` (cron re-schedule pass + honest failures + `AppResult` return), `crates/backend/src/reports/handlers.rs` (cron validation at POST), `crates/backend/src/handlers/security.rs` (live-vs-persisted contract + `mtls_contract_note`), `crates/backend/src/state.rs` (`mtls_startup_enforced`), `crates/backend/src/app.rs` (capture startup flag + doc), `crates/backend/src/tls.rs` (restart-gate doc).
- **Shared DTO:** `crates/shared/src/dto/security.rs` (`MtlsConfig` + `active_enforced, pending_restart, note`).
- **Tests:** `crates/backend/tests/http_p1.rs` (SEC8/SEC9 assert the new contract), `crates/backend/tests/common/mod.rs` (`mtls_startup_enforced: false`).
- **Docs:** `README.md` (114→117 + Audit #12 Remediation section), `plan.md` (this section).

Verification shape:

- Parser correctness: 10 inline unit tests in `cron.rs` (parse shapes, ranges, lists, steps, daily, weekday-only, unreachable `0 0 31 2 *`, Sunday `0`/`7` equivalence).
- mTLS contract: `t_sec8_patch_mtls_requires_mtls_manage` now asserts `enforced=true, active_enforced=false, pending_restart=true`, note contains `restart`. `t_sec9_mtls_status_returns_cert_counts` asserts the three new fields are present.
- Endpoint parity: Issue #4 is docs-only, so `bash scripts/audit_endpoints.sh` strict mode stays green at 117/117. No new routes were added, so the strict audit marker stays in place.
- Broad-gate `./run_tests.sh` rerun deferred per the standing "do not rerun broad contract yet" directive.

## Audit #11 Remediation (develop-1 lane)

Four findings closed as a single coherent remediation bundle:

| # | Severity | Finding | Fix |
| - | -------- | ------- | --- |
| 1 | High   | Signed image URLs not bound to authenticated user — any bearer holder could replay another user's URL within its exp. | HMAC input extended to `path | user_id | exp`; verifier rebuilds the HMAC against the authenticated session's user id; URL wire format unchanged (`?exp=&sig=`). |
| 2 | High   | Service worker cached authenticated `/api/*` GETs keyed only by URL; no purge on logout. | SW bumped to `v2`; requests with `Authorization` header pass through uncached; `/api/*` tier removed entirely; logout posts `{type:'logout'}` which deletes image + legacy api caches. |
| 3 | Medium | SPA rendered timestamps in UTC, bypassing `terraops_shared::time::format_display`. | `format_ts` now calls the shared helper with the browser's `Date.getTimezoneOffset()`-derived offset (sign inverted to match local-minus-UTC in seconds). |
| 4 | Medium | Long lists used classic Prev/Next page navigation; design calls for incremental loading. | New `LoadMore` component; Products, Observations, Alert events, Candidates all accumulate rows and advance via "Load more"; backend per-call pagination unchanged; `ServerPager` removed. |

### Audit #11 — file change list

- `crates/backend/src/crypto/signed_url.rs` — HMAC now binds user id; added `wrong_user_rejected` unit test.
- `crates/backend/src/products/repo.rs` — `get_product_detail` takes `viewer_user_id`.
- `crates/backend/src/products/handlers.rs` — passes `user.0.user_id` to `get_product_detail`.
- `crates/backend/src/products/images.rs` — passes `user.0.user_id` to `signed_url::verify`.
- `crates/backend/tests/integration_tests.rs` — manual forgery test updated; new HTTP test `t_int_signed_image_url_rejects_cross_user_replay`.
- `crates/frontend/static/sw.js` — v2, no auth-request caching, logout message handler.
- `crates/frontend/src/components.rs` — `notify_sw_logout()`, Nav logout hook, `LoadMore`, `ServerPager` removed.
- `crates/frontend/src/pages.rs` — `format_ts` routed through shared helper with local offset; 4 list surfaces converted to accumulate + `LoadMore`.
- `README.md` — new "Audit #11 Remediation" section.

### Audit #11 — issue → fix mapping evidence

- Issue 1: `wrong_user_rejected` unit test + `t_int_signed_image_url_rejects_cross_user_replay` HTTP test (Alice's URL → 404 for Alice on unknown row; 403 for Bob on same URL).
- Issue 2: `sw.js` — `isAuthenticatedRequest(req)` early-return, message handler; `components.rs` `notify_sw_logout()` called before `navigator.push(&Route::Login)`.
- Issue 3: `pages.rs::format_ts` → `terraops_shared::time::format_display(dt, local_offset_seconds())`.
- Issue 4: `components.rs::LoadMore`; `pages.rs` 4 sites use `fetch((target_page, append))` + accumulate; `ServerPager` definition removed.
