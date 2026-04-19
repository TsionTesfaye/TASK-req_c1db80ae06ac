# TerraOps — Offline Environmental & Catalog Intelligence Portal

## Project Type

**fullstack** (Rust backend + Rust/Yew frontend, PostgreSQL, delivered as a single-node offline Docker deployment).

## Overview

TerraOps is an offline, single-node operations portal for environmental
observations, catalog governance, and talent intelligence. It runs entirely
inside one Docker network, exposes one TLS origin on `https://localhost:8443`,
and has no outbound network dependencies at runtime.

This repository currently contains the **P1 shared foundations** —
complete backend (identity, RBAC, sessions, admin security, retention,
monitoring, reference data, notifications, middleware stack, 52
no-mock integration tests) **plus** the complete P1 frontend surface
(Yew SPA shell, router, auth / toast / notifications context, typed
API client with 3-second timeout + single-GET-retry, `PermGate`-aware
nav, and real admin / monitoring / notifications pages). The P2–P5
feature phases (P-A Catalog, P-B KPI/Environmental, P-C Talent, and
the final hardening gate) are tracked in [`plan.md`](./plan.md).

## Tech Stack

- Backend: **Rust** + **Actix-web 4** + **Rustls 0.22** + **sqlx 0.7** (Postgres)
- Frontend: **Rust** + **Yew 0.21** + **Trunk** + **Tailwind CSS**
- Database: **PostgreSQL 16**
- Delivery: **Docker Compose** (one `app` service serves both SPA and REST API)
- Tests: `cargo llvm-cov`, `wasm-bindgen-test` + `grcov`, Playwright (Chromium)

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

`run_tests.sh` enforces (per [`../docs/test-coverage.md`](../docs/test-coverage.md)):

- **Gate 1** — backend coverage ≥ 90% via `cargo llvm-cov -p terraops-shared -p terraops-backend`.
- **Gate 2** — frontend WASM coverage ≥ 80% via `wasm-bindgen-test` + `grcov`.
- **Gate 3** — endpoint parity audit via `scripts/audit_endpoints.sh`. Mode
  is controlled by the presence of `crates/backend/tests/.audit_strict`:
  absent → `progress` mode (reverse check enforced, forward parity
  reported); present (committed in P5) → `strict` mode (both checks enforced).
- **Flow gate** — Playwright specs under `e2e/` (activated in P1+).

Scaffold (P0) behaviour of `./run_tests.sh` — the gate is already real, not
stubbed:

- **Gate 1** really runs inside the `tests` Docker image. `run_tests.sh`
  executes `docker compose build tests` and then
  `docker compose run --rm --no-deps tests bash -c 'cargo --version &&
  cargo test -p terraops-shared -p terraops-backend --no-fail-fast'`.
  No host-side cargo is invoked; the host only needs `docker` + `bash`.
  At scaffold this compiles the workspace and runs whatever `#[test]`
  functions already exist (the `terraops-shared` crate has 2 passing
  tests; `terraops-backend` has 0). Any real failure fails the gate.
- **Gate 3** runs in progress mode. The authoritative total is read
  from `## Totals` in `../docs/api-spec.md` (77 endpoints). Forward
  parity is reported as `0/77` at scaffold; reverse parity is enforced
  (no orphan test IDs — trivially true because no HTTP tests exist
  yet).
- **Gate 2** and **Flow gate** print an explicit `DEFERRED` line with
  the reason: the wasm-bindgen-test runner, grcov, Node, pinned
  Chromium, and Playwright browsers are not yet provisioned in
  `Dockerfile.tests`. They are wired in the same commit that lands the
  first WASM test / first Playwright spec in P1+. This is a declared
  deferral, not a silent skip of a broken invocation.

## Authentication

P1 delivers the full backend auth contract: Argon2id password hashing,
HS256 JWT access tokens (15-minute lifetime), opaque refresh tokens
rotated on every use with a revocable `sessions` table, bearer-token
`Authorization: Bearer <jwt>` on every privileged request, normalized
`AUTH_INVALID_CREDENTIALS` / `FORBIDDEN` / `RATE_LIMITED` error
envelopes, and CIDR allowlist enforcement. Device mTLS pinning is
schema-ready (admin-managed `device_certs` + `mtls_config`); the Rustls
pinned-client verifier flips on once an admin sets `enforced=true`.

`docker compose up --build` now runs migrations and seeds the demo
accounts automatically on first boot (see `Dockerfile.app` CMD). You
can also force re-seed any time with `./init_db.sh` or
`docker compose exec app terraops-backend seed`.

Demo accounts (all passwords `TerraOps!2026`):

| Role           | Email                      |
| -------------- | -------------------------- |
| Administrator  | admin@terraops.local       |
| Data Steward   | steward@terraops.local     |
| Analyst        | analyst@terraops.local     |
| Recruiter      | recruiter@terraops.local   |
| Regular User   | user@terraops.local        |

## Roles And Workflows

Five canonical roles (names used verbatim in code, DB, and docs):

- **Administrator** — user & role management, security (mTLS, allowlist, retention), audit log.
- **Data Steward** — product catalog, imports, images, change history.
- **Analyst** — environmental KPIs, reports, dashboards.
- **Recruiter** — talent search, candidate recommendations, mailbox export.
- **Regular User** — personal notifications, profile, self-service.

The full actor-to-workflow map, permission codes, and workflow state
machines are defined in `../docs/design.md`.

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

- **Backend P1 is complete and integrated.** 52 no-mock integration tests
  in `crates/backend/tests/http_p1.rs` exercise every P1 endpoint
  (system S1–S2, auth A1–A5, users U1–U10, security SEC1–SEC9, retention
  R1–R3, monitoring M1–M4, reference-data REF1–REF9, notifications N1–N7)
  through the real middleware stack and real Postgres. Run them with
  `docker compose run --rm tests bash -c 'cargo test -p terraops-backend
  --test http_p1 -- --test-threads=1'` — current result: 52 passed.
- `scripts/seed_demo.sh` now invokes `terraops-backend seed`, which is
  idempotent and creates/updates the five demo users with the role
  matrix above. Running it repeatedly preserves operator-set passwords.
- **Frontend SPA status:** P1 complete. The Yew SPA ships a real
  router, auth/toast/notifications context providers, permission-aware
  nav, a typed `ApiClient` with hard 3 s timeout + single-GET retry +
  unified error mapping (`wasm-bindgen-test` coverage for the timeout
  race and error mapping), and real pages for login, change-password,
  admin (users, allowlist, retention, mTLS, audit), monitoring
  (latency, errors, crashes), and the notifications center. The
  minimal `dashboard::Home` placeholder shows identity + permissions
  and hands off to the P-B KPI experience later. The `Dockerfile.app`
  now has a real Trunk + wasm-bindgen stage that replaces the P0
  `dist-scaffold` shell.
- All 77 REST endpoints are listed authoritatively in
  `../docs/api-spec.md`. P1 delivers 40 of them (the shared-foundation
  subset); P-A/P-B/P-C packages ship the remaining 37.
- No outbound integrations exist. TerraOps is an offline system by contract.
- No `.env` / `.env.*` files are created or consumed by this repo. The
  Postgres bootstrap value in `docker-compose.yml` is a documented
  dev-only local-development credential, not a production secret;
  production secret management is out of scope for this offline
  single-node deployment.
- `scripts/dev_bootstrap.sh` is the sole first-boot bootstrap path and is
  explicitly labelled dev-only inside the script.
