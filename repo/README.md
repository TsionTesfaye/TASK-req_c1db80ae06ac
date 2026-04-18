# TerraOps — Offline Environmental & Catalog Intelligence Portal

## Project Type

**fullstack** (Rust backend + Rust/Yew frontend, PostgreSQL, delivered as a single-node offline Docker deployment).

## Overview

TerraOps is an offline, single-node operations portal for environmental
observations, catalog governance, and talent intelligence. It runs entirely
inside one Docker network, exposes one TLS origin on `https://localhost:8443`,
and has no outbound network dependencies at runtime.

This repository currently contains the **P0 scaffold**. Feature implementation
lands in phases P1 through P5 per [`plan.md`](./plan.md).

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

The full auth contract (Argon2id + HS256 JWT + opaque refresh rotation +
mTLS device pinning) lands in P1. The scaffold exposes only a disabled
`/login` placeholder.

Demo accounts (all passwords: `TerraOps!2026`; activated once
`scripts/seed_demo.sh` gains DB-write behaviour in P1):

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
Dockerfile.app              # scaffold: backend `cargo build --release` + prebuilt SPA shell copied
                            #           from crates/frontend/dist-scaffold/ (Trunk build stage
                            #           lands in P1 with the first real Yew code)
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

Scaffold-scope disclosures (update as features land):

- `scripts/seed_demo.sh` prints the demo credential matrix but performs
  **no DB writes** at scaffold level; real seeding is wired in P1.
- The Yew `/login` page is a **placeholder** with disabled inputs.
- All 77 REST endpoints except `/api/v1/health` and `/api/v1/ready` are
  unimplemented at P0; they are listed authoritatively in
  `../docs/api-spec.md` and ship in P1+.
- No outbound integrations exist. TerraOps is an offline system by contract.
- No `.env` / `.env.*` files are created or consumed by this repo. The
  Postgres bootstrap value in `docker-compose.yml` is a documented
  dev-only local-development credential, not a production secret;
  production secret management is out of scope for this offline
  single-node deployment.
- `scripts/dev_bootstrap.sh` is the sole first-boot bootstrap path and is
  explicitly labelled dev-only inside the script.
