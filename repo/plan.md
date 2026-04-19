# Implementation Plan — TerraOps

Definitive repo-local execution checklist. Derived from `../docs/design.md`; endpoint inventory in `../docs/api-spec.md`; test mapping in `../docs/test-coverage.md`.

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
    - Gate 1: `cargo llvm-cov --no-fail-fast -p terraops-shared -p terraops-backend --ignore-filename-regex '/(tests|migrations|\.sqlx)/' --fail-under-lines 90 --lcov --output-path coverage/rust.lcov` (explicit `-p` flags restrict scope to the two native Rust crates; `--workspace` is intentionally not used so the frontend crate cannot contribute to `coverage/rust.lcov`)
    - Gate 2: `cargo test -p terraops-frontend --target wasm32-unknown-unknown` with `RUSTFLAGS='-C instrument-coverage'`, then `grcov … --threshold 80 -o coverage/frontend.lcov`
    - Gate 3: `scripts/audit_endpoints.sh` (mode decided by presence of `crates/backend/tests/.audit_strict`)
    - Flow: `npx --prefix e2e playwright test` (all 7 specs must pass)
  - **Authoritative threshold sentence** (identical wording in `../docs/design.md` and `../docs/test-coverage.md`): The 90% line-coverage floor applies only to `crates/shared` + `crates/backend` (Rust native code). The `crates/frontend` Yew/WASM crate has its own separately enforced 80% line-coverage floor. Playwright contributes flow verification only and does not contribute to either line-coverage number.
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
  - `./run_tests.sh` compiles and exits 0. The endpoint-audit (`scripts/audit_endpoints.sh`) runs in **`progress` mode** because the marker file `crates/backend/tests/.audit_strict` is **absent** at scaffold time: the audit reads the authoritative 77-row inventory from `docs/api-spec.md`, enforces the reverse check (no orphan test IDs — trivially true because no HTTP tests exist yet), and prints forward parity as `0/77` without failing. Forward parity is enforced only after the marker file is committed in P5. This rule is the same at every phase; no hand-waved scaffold exception.
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
  - **Gate 3 (real)**: `scripts/audit_endpoints.sh` (authoritative total = 77 from `## Totals`; progress mode at scaffold).
  - **Gate 2 (deferred)** and **Flow gate (deferred)**: print an explicit `DEFERRED` line stating that the wasm-bindgen-test runner, grcov, Node, pinned Chromium, and Playwright browsers are not yet provisioned in `Dockerfile.tests`. They are wired in the same commit that lands their first consumer in P1+.
  - Full P1+ wrapper command set (the `cargo llvm-cov --fail-under-lines 90 …` coverage wrapper, `wasm-bindgen-test` + `grcov --threshold 80 …`, and `npx --prefix e2e playwright test`) is retained verbatim in the header comment of `run_tests.sh` and is layered on alongside each first consumer.
- [x] Migration `0001_identity.sql` (users + sessions baseline with `email_ciphertext/email_hash/email_mask` columns).
- [x] `README.md` to strict-audit layout: project type `fullstack`, startup, access `https://localhost:8443`, verification via `./run_tests.sh`, demo credentials for all five roles, runtime strings `docker compose up --build` and `docker-compose up`.
- [ ] Commit: `chore(scaffold): single-app workspace + compose + scripts baseline`.
- [ ] **STOP at scaffold boundary.**

#### P0 Scaffold Verification Notes

- `docker compose config` validates cleanly (services `db`, `app`, and `tests` profile; volumes `terraops-db`, `terraops-runtime`).
- `bash -n` is green on all scripts (`run_app.sh`, `run_tests.sh`, `init_db.sh`, `scripts/*.sh`).
- `scripts/audit_endpoints.sh` runs in `progress` mode (no marker file) and passes (0 reverse orphans). It reports the **authoritative 77-endpoint total** (read from the `**77 HTTP endpoints.**` sentence in `## Totals` of `docs/api-spec.md`) and prints forward parity as `0/77`. The parser deliberately does not manufacture its own total from raw ID-row counts; inventory IDs are used only as the reverse-parity validity set so that any `t_<id>_*` test is guaranteed to reference a known endpoint. The artifact at `coverage/endpoint_audit.json` therefore contains exactly one total (`api_total: 77`) with no conflicting secondary count.
- `docker compose up --build` boots `db` and `app` cleanly. The backend binary listens on `:8443` with rustls; `GET /api/v1/health` returns `{"status":"ok"}` and `GET /api/v1/ready` returns `{"status":"ready","db":true}` once `db` is healthy.
- `./run_tests.sh` exits 0 end-to-end: Gate 1 really compiles the workspace inside the `tests` image and runs `cargo test -p terraops-shared -p terraops-backend --no-fail-fast` (2 passing in `terraops-shared`, 0 in `terraops-backend`); Gate 3 is green against the 77-endpoint authoritative total; Gate 2 and the Flow gate print `DEFERRED` with explicit reasons. No toolchain error is masked.
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
- [-] mTLS integration harness + handshake-refusal test. _(deferred to P4 hardening where `mtls_config.enforced=true` is exercised end-to-end.)_
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
- [x] `scripts/audit_endpoints.sh` verified green for the P1 endpoint set: `49/77` forward parity in progress mode, **0** reverse orphans. Script was generalized to (a) scan the full `crates/backend/tests/` tree (not just `tests/http/`) so the single-file `http_p1.rs` layout is recognized, and (b) accept lowercase `t_<id>_*` prefixes (uppercased before inventory comparison). Strict-mode activation remains the end-of-development gate when all 77 endpoints have tests.
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
- Gate 3 (endpoint audit): `bash scripts/audit_endpoints.sh` → `forward parity: 49/77 (63.6%)`, `reverse orphans: 0`, `audit_endpoints: OK (progress mode; authoritative total = 77)`.
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
- [ ] Frontend `pages/data_steward/**`: products list with filters, detail drawer, image manager, import wizard (upload → preview with row-level errors → commit → history), exports. _(Deferred — backend + parity tests landed; frontend surfaces remain in the P-A frontend follow-on.)_
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
- [ ] Frontend `pages/dashboard/home.rs` full role-aware KPI experience; `pages/user/{dashboards,alerts}`; `pages/analyst/{sources,definitions,series,lineage,reports}`; lineage panel wired to backend. _(Deferred — backend + parity tests landed; frontend surfaces remain in the P-B frontend follow-on.)_
- [x] HTTP parity tests for every P-B endpoint (`tests/parity_tests.rs` — `t_e1`…`t_e6`, `t_md1`…`t_md7`, `t_k1`…`t_k6`, `t_al1`…`t_al6`, `t_rp1`…`t_rp6`). Unit tests for the scoring-formula family live in `crates/backend/src/talent/scoring.rs` and in the `metrics_env::formula` module.
- [x] Package commit: same integration commit as P-A (99e7f89) — all three packages landed atomically.

**P-C Talent** (merged into `main`)

- [x] Migration `0030_talent.sql` (candidates with tsv trigger, roles_open, weights, watchlists, feedback).
- [x] Candidates + roles CRUD (T1–T5).
- [x] Recommendations (T6) with cold-start rule (<10 feedback → recency+completeness) and blended scoring (≥10 feedback) returning reasons list + `cold_start` flag.
- [x] Weights SELF-scoped (T7–T8).
- [x] Feedback with PERM(talent.feedback) (T9).
- [x] Watchlists SELF-scoped (T10–T13).
- [ ] Frontend `pages/recruiter/**`: search, candidate detail, recommendations with match reasons, watchlist management, weight tuning. _(Deferred — backend + HTTP tests landed; frontend surfaces remain in the P-C frontend follow-on.)_
- [x] HTTP tests for every P-C endpoint (39 tests across `talent_{search,recommend,weights,watchlist,feedback}_tests.rs`, all green); unit tests for scoring (cold-start vector, blended vector, threshold-crossing).
- [x] Package commit: same integration commit (99e7f89).

### P3 — Integration (main lane)

- [ ] Merge P-A, P-B, P-C into `main`; resolve any contract drift.
- [ ] Verify producers → `notifications::emit` → notification center UI → mailbox export end-to-end.
- [ ] Wire retention enforcer sweeps across env raw (18mo), KPI aggregates (5yr), feedback (24mo inactive); hourly metric-roll aggregation.
- [ ] Seed cross-domain demo dataset (products + env readings + candidates + roles + feedback crossing the cold-start threshold + alerts + notifications).
- [ ] Integration tests: retention purge with time shim; alert→notification→mailbox; report referencing KPI + env.
- [ ] Commit: `feat(integration): cross-domain wiring + retention`.

### P4 — Hardening (main lane)

- [ ] `endpoint_allowlist` live enforcement (integration test).
- [ ] mTLS enforcement end-to-end: admin issues cert via `scripts/issue_device_cert.sh`, registers SPKI, flips `enforced=true`, proves unpinned TLS handshake is refused and revocation propagates within 1 s.
- [ ] Signed image URL enforcement (forged + expired + wrong-user negative tests).
- [ ] Non-functional budgets measured: handler p95 < 500ms, imports ≤10k <10 s preview / <30 s commit, recs ≤10k <500 ms.
- [ ] Log redaction verified (password hash, email_ciphertext, refresh hash, HMAC keys not in logs).
- [ ] Visual polish pass on every role workspace; honest placeholders; no debug/demo screens.
- [ ] Commit: `feat(hardening): security, perf, polish`.

### P5 — Final Gate (main lane)

- [x] Commit the audit strictness marker: `crates/backend/tests/.audit_strict` exists and `scripts/audit_endpoints.sh` reports forward parity **114/114 (100%)** with 0 reverse orphans (6b7b9fc).
- [ ] All 7 Playwright flows pass (flow gate).
- [ ] `./run_tests.sh` exits 0 with Gate 1 ≥ 90%, Gate 2 ≥ 80%, Gate 3 strict-forward = 100% (enabled by the marker) and reverse = 100%, flow gate pass.
- [ ] `docker compose up --build` clean cold boot from empty `terraops-runtime` volume with zero manual `export` steps; demo credentials for all five roles usable.
- [ ] README audit (project type, startup, access, verification, demo credentials per role, `docker compose up --build` present, legacy `docker-compose up` string present, `./run_tests.sh` documented, notification mock/default behaviour disclosed).
- [ ] Commit: `chore(release): final gate — coverage + e2e + docs`.

## Definition Of Done (repo-local mirror)

- [x] All 114 endpoints in `docs/api-spec.md` implemented, auth-classified, and covered by a no-mock HTTP test discovered by `scripts/audit_endpoints.sh`.
- [ ] Every actor success path executable via the real UI.
- [ ] `docker compose up --build` cold-boots with no `.env` and no hardcoded secrets.
- [ ] `./run_tests.sh` exits 0 with all four gates green.
- [ ] `./init_db.sh` idempotent.
- [ ] mTLS handshake refusal proven in integration.
- [ ] `email_ciphertext` non-plaintext; no plaintext email substring in `users` rows.
- [ ] README matches strict audit contract.
- [ ] No placeholder/demo/debug UI visible in product surfaces.
