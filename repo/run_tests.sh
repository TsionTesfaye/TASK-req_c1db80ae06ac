#!/usr/bin/env bash
# run_tests.sh — broad test gate (repo-root, project-standard).
#
# P1 truthful wiring:
#
#   Gate 1 (Rust cargo tests for terraops-backend + terraops-shared) runs
#   for real inside the `tests` Docker image against a real Postgres,
#   using the same startup-value model as `docker compose up --build`.
#   The 52-test `http_p1.rs` no-mock integration suite is executed
#   serialized with `--test-threads=1` because it reuses a single DB.
#
#   Gate 2 (frontend `wasm-bindgen-test` suite for the API client) runs
#   for real in the tests image via `cargo test --target
#   wasm32-unknown-unknown -p terraops-frontend`. The tests are
#   configured with `run_in_node`, so no browser is required — the
#   wasm-bindgen-test-runner uses the Node.js interpreter installed in
#   `Dockerfile.tests`. grcov and the 80% line-coverage threshold ride
#   in alongside the first coverage-gated frontend commit; the current
#   gate proves the wasm tests execute end-to-end.
#
#   Gate 3 (endpoint parity audit) runs on the host; deterministic and
#   dependency-free beyond `bash` + `awk`.
#
#   Flow gate (Playwright specs) stays honestly DEFERRED: no spec exists
#   yet, and `Dockerfile.tests` does not bundle pinned Chromium /
#   Playwright browsers. This is wired in the same commit that lands
#   the first real flow spec (P5 at the latest).
#
# Full gate contract (coverage thresholds layer on per
# docs/test-coverage.md §Coverage Gate Math):
#   Gate 1 : cargo llvm-cov --fail-under-lines 90 …
#            (threshold wrapper lands with the first feature-complete
#             coverage-gated commit; today we run the cargo test surface
#             without the threshold to keep failures honest and fast)
#   Gate 2 : cargo test --target wasm32-unknown-unknown -p terraops-frontend
#            (+ grcov --threshold 80 once the frontend coverage floor is
#             wired)
#   Gate 3 : scripts/audit_endpoints.sh
#   Flow   : npx --prefix e2e playwright test  (DEFERRED)
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

# ---- Gate 1 : backend + shared cargo tests --------------------------------
section "Gate 1 — cargo test (terraops-backend + terraops-shared, via docker compose tests image)"
if ! compose build tests; then
    echo "[gate1] FAILED — could not build the 'tests' image." >&2
    failed=1
else
    # The backend http_p1 suite hits a real Postgres; run it against the
    # compose `db` service and serialize with --test-threads=1 because
    # tests share one database. `--no-deps` is intentionally NOT used
    # here — we want compose to bring up `db` before running tests.
    if ! compose run --rm tests \
            bash -c 'cargo --version \
              && cargo test -p terraops-shared --no-fail-fast \
              && cargo test -p terraops-backend --test http_p1 -- --test-threads=1'; then
        echo "[gate1] FAILED — cargo test reported failures." >&2
        failed=1
    fi
fi

# ---- Gate 2 : frontend wasm-bindgen-test suite ----------------------------
section "Gate 2 — frontend wasm-bindgen-test (API client: timeout, retry policy, error mapping, token attach)"
if ! compose run --rm --no-deps tests \
        bash -c 'cargo --version \
          && node --version \
          && wasm-bindgen --version \
          && CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
             cargo test --target wasm32-unknown-unknown -p terraops-frontend --no-fail-fast'; then
    echo "[gate2] FAILED — frontend wasm tests failed." >&2
    failed=1
fi

# ---- Gate 3 : endpoint parity audit ---------------------------------------
section "Gate 3 — endpoint parity audit  (mode: $([[ -f "${MARKER}" ]] && echo strict || echo progress))"
if ! bash "${REPO_ROOT}/scripts/audit_endpoints.sh"; then
    failed=1
fi

# ---- Flow gate : deferred (no spec yet) -----------------------------------
section "Flow gate — Playwright specs"
echo "[deferred] No Playwright specs exist in e2e/ yet; pinned Chromium and Playwright"
echo "[deferred]   browsers are intentionally not provisioned in Dockerfile.tests."
echo "[deferred]   Wired alongside the first real flow spec in a later phase."

if [[ ${failed} -ne 0 ]]; then
    echo ""
    echo "run_tests: FAILED"
    exit 1
fi

echo ""
echo "run_tests: OK (Gate 1 + Gate 2 + Gate 3 green; flow gate declared deferred)"
