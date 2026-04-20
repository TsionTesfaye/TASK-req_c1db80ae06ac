#!/usr/bin/env bash
# run_tests.sh — broad test gate (repo-root, project-standard).
#
# Final-gate wiring (all four gates real):
#
#   Gate 1 — cargo tests for `terraops-shared` + every `terraops-backend`
#     test binary (incl. `http_p1`, `parity_tests`, the five `talent_*`
#     suites, `integration_tests`, `mtls_handshake_tests`) plus the
#     backend `--lib` step (picks up every `#[cfg(test)]` module inside
#     `crates/backend/src`) run inside the `tests` Docker image against a
#     real Postgres, serialised with `--test-threads=1` because they
#     share one DB. Line coverage is measured end-to-end with
#     `cargo llvm-cov` and enforced against the planning-contract floor
#     `GATE1_LINE_FLOOR=94` over `crates/shared/**` + `crates/backend/src/**`
#     excluding pure-IO boot modules (`main.rs`, `app.rs`, `tls.rs`,
#     `spa.rs`, `config.rs`, `db.rs`, `storage/`, `models/`, `seed.rs`)
#     which are exercised by `docker compose up --build` rather than
#     cargo tests.
#
#   Gate 2 — Frontend Verification Matrix (FVM). Wasm source-based line
#     coverage is not the authoritative frontend proof on this toolchain
#     (rust-std wasm32-unknown-unknown does not ship `profiler_builtins`;
#     see `docs/test-coverage.md §Why the frontend is not measured by
#     wasm source-based line coverage`). Gate 2 instead runs, in order:
#       (a) the `wasm-bindgen-test` suite for `terraops-frontend` under
#           `wasm-bindgen-test-runner` (Node mode; no pinned Chromium);
#       (b) `scripts/frontend_verify.sh`, which parses
#           `docs/test-coverage.md`'s 53-row matrix, grep-validates every
#           row's evidence in the codebase, and enforces the floor
#           `GATE2_FVM_FLOOR=90`. "covered" rows with missing evidence
#           are HARD failures (no dishonest greens).
#
#   Gate 3 — endpoint parity audit via `scripts/audit_endpoints.sh`
#     (deterministic; bash + awk only). Strict mode active when
#     `crates/backend/tests/.audit_strict` exists.
#
#   Flow gate — Playwright specs under `e2e/specs/` (7 flows) run
#     against the live `app` service on the compose network
#     (`https://app:8443`) using the pinned Chromium and pre-staged
#     `/opt/e2e/node_modules` baked into `Dockerfile.tests`. Set
#     `TERRAOPS_SKIP_FLOW=1` to bypass explicitly (loud, not silent).
#
# Host requirements: bash + docker (Compose v2). No host-side Rust,
# Node, Chromium, or Playwright.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
cd "${REPO_ROOT}"

MARKER="${REPO_ROOT}/crates/backend/tests/.audit_strict"
mkdir -p "${REPO_ROOT}/coverage"

section() { printf '\n===== %s =====\n' "$*"; }

if ! command -v docker >/dev/null 2>&1; then
    echo "run_tests: 'docker' is required on PATH; see README.md §Running." >&2
    exit 2
fi

compose() { docker compose "$@"; }

failed=0

# ---- Gate 1 : backend + shared cargo tests + line-coverage threshold ------
#
# Runs every native-Rust test binary once under `cargo llvm-cov --no-report`
# so every invocation contributes to a single merged profraw set. The final
# `cargo llvm-cov report --fail-under-lines ${GATE1_LINE_FLOOR}` applies the
# line-coverage floor across the accumulated data.
#
# Tests share one Postgres database; `--test-threads=1` serializes them.
# Backend test binaries are listed explicitly (not `--workspace`) so the
# frontend WASM crate cannot dilute `coverage/rust.lcov` — this mirrors the
# `-p terraops-shared -p terraops-backend` scope rule in docs/design.md.
#
# GATE1_LINE_FLOOR is the planning-contract floor for native-Rust code
# (shared + backend). The ignore regex below excludes pure-IO boot/plumbing
# modules that are exercised end-to-end by `docker compose up --build`
# rather than by cargo tests.
GATE1_LINE_FLOOR="${GATE1_LINE_FLOOR:-94}"
GATE1_IGNORE_REGEX='/(tests|migrations|\.sqlx)/|backend/src/(main|app|tls|spa|config|db|seed)\.rs|backend/src/(storage|models)/|backend/src/telemetry\.rs'

GATE1_TESTS=(http_p1 parity_tests talent_search_tests talent_recommend_tests \
             talent_weights_tests talent_watchlist_tests talent_feedback_tests \
             integration_tests mtls_handshake_tests budget_tests \
             deep_products_tests deep_metrics_kpi_tests deep_alerts_reports_tests \
             deep_admin_surface_tests deep_jobs_scheduler_tests)

section "Gate 1 — cargo test + cargo llvm-cov (terraops-backend + terraops-shared, --fail-under-lines ${GATE1_LINE_FLOOR})"
if ! compose build tests; then
    echo "[gate1] FAILED — could not build the 'tests' image." >&2
    failed=1
else
    # Gate 1a: real pass/fail for every backend + shared test binary. This
    # is the test-regression gate — any failing test here fails Gate 1.
    gate1a_cmd='set -e
      cargo --version
      cargo test -p terraops-shared --no-fail-fast
      cargo test -p terraops-backend --lib --no-fail-fast'
    for t in "${GATE1_TESTS[@]}"; do
        gate1a_cmd="${gate1a_cmd}
      cargo test -p terraops-backend --test ${t} -- --test-threads=1"
    done

    # Gate 1b: line-coverage measurement + floor enforcement. Uses
    # `cargo llvm-cov` with `|| true` tolerance on individual test
    # invocations so accumulated profraw data covers every binary even
    # if a given test flakes under instrumentation — the real
    # pass/fail gate is 1a above; this step exists only to enforce the
    # coverage floor on the accumulated data.
    gate1b_cmd='set +e
      cargo llvm-cov --version
      cargo llvm-cov clean --workspace
      cargo llvm-cov --no-report -p terraops-shared --no-fail-fast --quiet
      cargo llvm-cov --no-report -p terraops-backend --lib --no-fail-fast --quiet || true'
    for t in "${GATE1_TESTS[@]}"; do
        gate1b_cmd="${gate1b_cmd}
      cargo llvm-cov --no-report -p terraops-backend --test ${t} --no-fail-fast --quiet -- --test-threads=1 || true"
    done
    gate1b_cmd="${gate1b_cmd}
      set -e
      mkdir -p coverage
      cargo llvm-cov report \\
          --ignore-filename-regex '${GATE1_IGNORE_REGEX}' \\
          --fail-under-lines ${GATE1_LINE_FLOOR} \\
          --lcov --output-path coverage/rust.lcov
      cargo llvm-cov report \\
          --ignore-filename-regex '${GATE1_IGNORE_REGEX}' \\
          --summary-only | tail -5"

    if ! compose run --rm tests bash -c "${gate1a_cmd}"; then
        echo "[gate1a] FAILED — cargo test reported failures (test-regression gate)." >&2
        failed=1
    elif ! compose run --rm tests bash -c "${gate1b_cmd}"; then
        echo "[gate1b] FAILED — backend line coverage below ${GATE1_LINE_FLOOR}%% floor." >&2
        failed=1
    fi
fi

# ---- Gate 2 : Frontend Verification Matrix (FVM) --------------------------
#
# Two-part gate:
#   (a) wasm-bindgen-test suite green under Node's wasm-bindgen-test-runner.
#       This is the hard regression gate on frontend logic (auth, api
#       client, toast, notifications, router).
#   (b) scripts/frontend_verify.sh validates docs/test-coverage.md's
#       Frontend Verification Matrix: every "covered" row's evidence
#       (wasm test fn name / Playwright spec file / router variant)
#       must exist in the codebase, and the score covered/total must be
#       at or above GATE2_FVM_FLOOR.
#
# Wasm source-based line coverage was evaluated and found not achievable
# on the pinned stable toolchain (profiler_builtins is not shipped in
# rust-std-wasm32-unknown-unknown). docs/test-coverage.md §Why the
# frontend is not measured by wasm source-based line coverage records
# the exact observed blocker evidence.
GATE2_FVM_FLOOR="${GATE2_FVM_FLOOR:-90}"

section "Gate 2a — frontend wasm-bindgen-test suite (Node mode, no Chromium)"
if ! compose run --rm --no-deps tests bash -c '
    set -e
    cargo --version
    node --version
    wasm-bindgen --version
    CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
        cargo test --target wasm32-unknown-unknown -p terraops-frontend --no-fail-fast
'; then
    echo "[gate2a] FAILED — frontend wasm-bindgen-test suite reported failures." >&2
    failed=1
fi

section "Gate 2b — Frontend Verification Matrix (floor ${GATE2_FVM_FLOOR}%)"
if ! GATE2_FVM_FLOOR="${GATE2_FVM_FLOOR}" bash "${REPO_ROOT}/scripts/frontend_verify.sh"; then
    echo "[gate2b] FAILED — Frontend Verification Matrix below floor or has dishonest rows." >&2
    failed=1
fi

# ---- Gate 3 : endpoint parity audit ---------------------------------------
section "Gate 3 — endpoint parity audit  (mode: $([[ -f "${MARKER}" ]] && echo strict || echo progress))"
if ! bash "${REPO_ROOT}/scripts/audit_endpoints.sh"; then
    failed=1
fi

# ---- Flow gate : Playwright specs -----------------------------------------
#
# Chromium + Playwright npm deps are baked into `Dockerfile.tests` under
# `/opt/e2e` and `/opt/playwright-browsers` so the gate does not depend
# on any first-use download at test time.
#
# Set TERRAOPS_SKIP_FLOW=1 to bypass this gate explicitly (CI environments
# without the ability to bind :8443 on the host). Skipping is loud and
# exit-1 is still possible for other gates.
section "Flow gate — Playwright specs"
if [[ "${TERRAOPS_SKIP_FLOW:-0}" == "1" ]]; then
    echo "[flow] SKIPPED — TERRAOPS_SKIP_FLOW=1 set; Playwright gate not executed."
else
    # Bring the `app` service up (TLS on 8443) so specs have a real target.
    compose up -d app
    # Wait for /api/v1/health to answer 200 before running specs.
    ready=0
    for i in $(seq 1 60); do
        if curl -ks https://localhost:8443/api/v1/health 2>/dev/null | grep -q '"status":"ok"'; then
            ready=1
            break
        fi
        sleep 1
    done
    flow_ok=1
    if [[ "${ready}" != "1" ]]; then
        echo "[flow] FAILED — app service did not become healthy on :8443 within 60s." >&2
        failed=1
        flow_ok=0
    else
        # Re-seed demo users before Playwright runs. Gate 1's backend test
        # suite shares one Postgres database with `app` (see docker-compose.yml)
        # and TRUNCATEs the users table (crates/backend/tests/common/mod.rs)
        # for test isolation, which wipes the demo users that Dockerfile.app's
        # boot CMD seeded originally. Playwright's auth flows log in as
        # `admin@terraops.local` / `TerraOps!2026` etc., so the demo accounts
        # must exist at flow-gate time. `terraops-backend seed` is idempotent:
        # it re-creates any missing demo user, leaves existing passwords alone,
        # and rebuilds the role grants to the README matrix. Running it here
        # is the single authoritative point that guarantees the demo accounts
        # are present when Playwright runs, regardless of what Gate 1 did to
        # the shared DB.
        if ! compose exec -T app terraops-backend seed; then
            echo "[flow] FAILED — could not reseed demo users before Playwright." >&2
            failed=1
            flow_ok=0
        fi
    fi
    if [[ "${flow_ok}" == "1" ]] && ! compose run --rm --no-deps \
            -e PLAYWRIGHT_BROWSERS_PATH=/opt/playwright-browsers \
            -e PLAYWRIGHT_BASE_URL="https://app:8443" \
            -v "${REPO_ROOT}/e2e:/workspace/e2e" \
            tests bash -c '
                set -e
                cd /workspace/e2e
                # Use the pre-installed node_modules from the image so no
                # network access is required at test time.
                if [[ ! -d node_modules ]]; then
                    ln -s /opt/e2e/node_modules node_modules || cp -r /opt/e2e/node_modules .
                fi
                npx playwright test --reporter=list'; then
        echo "[flow] FAILED — Playwright specs reported failures." >&2
        failed=1
    fi
fi

if [[ ${failed} -ne 0 ]]; then
    echo ""
    echo "run_tests: FAILED"
    exit 1
fi

echo ""
if [[ "${TERRAOPS_SKIP_FLOW:-0}" == "1" ]]; then
    echo "run_tests: OK (Gate 1 line coverage + Gate 2 Frontend Verification Matrix + Gate 3 endpoint parity green; flow gate skipped via TERRAOPS_SKIP_FLOW=1)"
else
    echo "run_tests: OK (Gate 1 line coverage + Gate 2 Frontend Verification Matrix + Gate 3 endpoint parity + Flow gate Playwright specs all green)"
fi
