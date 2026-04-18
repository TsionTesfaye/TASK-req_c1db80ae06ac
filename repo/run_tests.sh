#!/usr/bin/env bash
# run_tests.sh — broad test gate (repo-root, project-standard).
#
# Truthful scaffold (P0) wiring:
#
#   Gate 1 (Rust cargo tests for terraops-backend + terraops-shared) runs
#   for real inside the `tests` Docker image via `docker compose run --rm
#   tests …`. No host-side cargo is ever invoked; the host only needs
#   `docker` + `bash`. At scaffold, no #[test] functions exist yet, so the
#   gate cleanly compiles the workspace and reports "0 passed" — a real
#   green result, not a silent skip. When the first Rust test lands in P1,
#   this same invocation starts exercising it immediately, and the coverage
#   wrapper (`cargo llvm-cov --fail-under-lines 90 …`) is layered on in
#   the same commit per docs/test-coverage.md §Coverage Gate Math.
#
#   Gate 3 (endpoint parity audit) runs for real on the host; it is
#   deterministic and has no toolchain dependencies beyond `bash` + `awk`.
#
#   Gate 2 (`wasm-bindgen-test + grcov ≥ 80%`) and the Playwright flow gate
#   stay deferred: the WASM runner, Node, pinned Chromium, and Playwright
#   browsers are not yet provisioned in `Dockerfile.tests`. They will be
#   added in the same commit that lands the first wasm-bindgen-test target
#   and the first Playwright spec, respectively. Until then this script
#   prints a clearly labelled "DEFERRED" line for each of them — never
#   swallows a broken toolchain invocation.
#
# Full gate contract (activated in P1+), per docs/test-coverage.md
# §Coverage Gate Math:
#   Gate 1 : docker compose run --rm tests cargo llvm-cov --no-fail-fast \
#              -p terraops-shared -p terraops-backend \
#              --ignore-filename-regex '/(tests|migrations|\.sqlx)/' \
#              --fail-under-lines 90 --lcov --output-path coverage/rust.lcov
#   Gate 2 : docker compose run --rm tests  (wasm-bindgen-test + grcov ≥ 80)
#   Gate 3 : scripts/audit_endpoints.sh  (mode = strict iff marker present)
#   Flow   : docker compose run --rm tests  (Playwright, 7 specs)
#
# Host requirements: bash + docker (Compose v2). No host-side Rust, Node,
# Chromium, or Playwright.

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

# Compose CLI: v2 (`docker compose`) is the canonical path. We do not fall
# back to the legacy `docker-compose` binary because the project contract
# is Compose v2.
compose() { docker compose "$@"; }

failed=0

# ---- Gate 1 : Rust cargo tests via the tests image ------------------------
section "Gate 1 — cargo test (terraops-backend + terraops-shared, via docker compose tests image)"
# Build the tests image first so a failure surfaces as a real build error
# (not hidden behind `run`). This is also the honest signal that the
# toolchain is actually present: `cargo --version` is exercised before any
# test invocation.
if ! compose build tests; then
    echo "[gate1] FAILED — could not build the 'tests' image." >&2
    failed=1
else
    # NOTE: use `bash -c` (not `-lc`). The rust:* base image exposes cargo
    # via PATH entries set as Dockerfile ENVs; a login shell would source
    # /etc/profile and reset PATH, dropping /usr/local/cargo/bin and
    # producing a misleading "cargo: command not found".
    if ! compose run --rm --no-deps tests \
            bash -c 'cargo --version && cargo test -p terraops-shared -p terraops-backend --no-fail-fast'; then
        echo "[gate1] FAILED — cargo test reported failures." >&2
        failed=1
    fi
fi

# ---- Gate 3 : endpoint parity audit ---------------------------------------
section "Gate 3 — endpoint parity audit  (mode: $([[ -f "${MARKER}" ]] && echo strict || echo progress))"
if ! bash "${REPO_ROOT}/scripts/audit_endpoints.sh"; then
    failed=1
fi

# ---- Gate 2 / Flow gate : deferred at scaffold ----------------------------
section "Gate 2 — frontend WASM coverage ≥ 80%"
echo "[scaffold] DEFERRED. wasm-bindgen-test runner + grcov are not yet provisioned"
echo "[scaffold]   in Dockerfile.tests. Wired in P1 alongside the first WASM test."

section "Flow gate — Playwright specs"
echo "[scaffold] DEFERRED. Node + pinned Chromium + Playwright browsers are not yet"
echo "[scaffold]   provisioned in Dockerfile.tests. Wired in P1+ alongside the first spec."

if [[ ${failed} -ne 0 ]]; then
    echo ""
    echo "run_tests: FAILED"
    exit 1
fi

echo ""
echo "run_tests: OK (scaffold-level gates green)"
