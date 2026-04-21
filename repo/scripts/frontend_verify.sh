#!/usr/bin/env bash
# frontend_verify.sh — Gate 2 enforcement for the Frontend Verification Matrix.
#
# Reads the markdown matrix at test-coverage.md (repo root), validates every row's
# evidence exists in the repo, and enforces the score floor
# GATE2_FVM_FLOOR (default 90).
#
# Evidence kinds:
#   * wasm_bindgen_test <fn_name>
#       -> greps for `fn <fn_name>` in crates/frontend/src/**/*.rs, AND
#          requires a `#[wasm_bindgen_test]` attribute within 3 lines above.
#   * playwright <file.spec.ts>
#       -> requires e2e/specs/<file> to exist and be non-empty.
#   * route <Variant>
#       -> greps for `Route::<Variant>` (as enum variant name) in
#          crates/frontend/src/router.rs.
#
# A "covered" row whose evidence fails existence is a HARD failure (no silent
# dishonest green). "deferred" rows count toward the denominator only.
#
# Host requirements: bash + awk + grep. No network, no docker.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." &>/dev/null && pwd)"
MATRIX="${REPO_ROOT}/test-coverage.md"
FLOOR="${GATE2_FVM_FLOOR:-90}"

if [[ ! -f "${MATRIX}" ]]; then
    echo "[fvm] FAILED — matrix not found at ${MATRIX}" >&2
    exit 2
fi

# Extract every matrix row (first column starts with "FVM-"). Columns:
#   | ID | Family | Requirement | Evidence Kind | Evidence | Status | Notes |
# We use awk with field separator '|'. Leading/trailing '|' produce empty
# fields at $1 and $NF; actual content starts at $2.
rows_file="$(mktemp)"
trap 'rm -f "${rows_file}"' EXIT

awk -F'|' '
    /^\| *FVM-/ {
        id=$2; family=$3; req=$4; kind=$5; ev=$6; status=$7;
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", id);
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", family);
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", kind);
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", ev);
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", status);
        print id "\t" family "\t" kind "\t" ev "\t" status;
    }
' "${MATRIX}" > "${rows_file}"

total=0
covered=0
deferred=0
hard_failures=0

check_wasm_bindgen_test() {
    local fn_name="$1"
    # awk pass: find the line `fn <fn_name>` and confirm the closest
    # preceding non-blank line carries `#[wasm_bindgen_test]` (allowing
    # attribute macros like `#[wasm_bindgen_test(unsupported = false)]`).
    grep -rEn --include='*.rs' "fn[[:space:]]+${fn_name}[[:space:]]*\(" \
        "${REPO_ROOT}/crates/frontend/src" 2>/dev/null | while IFS=: read -r file line _; do
        # Look in the 4 lines above for the attribute.
        awk -v line="${line}" -v start="$((line-4))" \
            'NR>=start && NR<line && /#\[wasm_bindgen_test/ {found=1} END {exit found?0:1}' \
            "${file}" && { echo "${file}:${line}"; return 0; }
    done | head -1 | grep -q .
}

check_playwright() {
    local f="$1"
    local path="${REPO_ROOT}/e2e/specs/${f}"
    [[ -s "${path}" ]]
}

check_route() {
    local variant="$1"
    grep -qE "Route::${variant}\b|^[[:space:]]*${variant}[[:space:]]" \
        "${REPO_ROOT}/crates/frontend/src/router.rs"
}

echo "[fvm] Scoring Frontend Verification Matrix from ${MATRIX}"
printf '%-10s %-14s %-22s %-52s %s\n' "ID" "FAMILY" "KIND" "EVIDENCE" "RESULT"

while IFS=$'\t' read -r id family kind ev status; do
    total=$((total+1))
    result=""
    case "${status}" in
        covered)
            case "${kind}" in
                wasm_bindgen_test)
                    if check_wasm_bindgen_test "${ev}"; then
                        covered=$((covered+1)); result="OK"
                    else
                        hard_failures=$((hard_failures+1))
                        result="HARD-FAIL: wasm test '${ev}' not found or not #[wasm_bindgen_test]"
                    fi
                    ;;
                playwright)
                    if check_playwright "${ev}"; then
                        covered=$((covered+1)); result="OK"
                    else
                        hard_failures=$((hard_failures+1))
                        result="HARD-FAIL: Playwright spec '${ev}' missing or empty"
                    fi
                    ;;
                route)
                    if check_route "${ev}"; then
                        covered=$((covered+1)); result="OK"
                    else
                        hard_failures=$((hard_failures+1))
                        result="HARD-FAIL: Route::${ev} not present in router.rs"
                    fi
                    ;;
                *)
                    hard_failures=$((hard_failures+1))
                    result="HARD-FAIL: unknown evidence kind '${kind}'"
                    ;;
            esac
            ;;
        deferred)
            deferred=$((deferred+1))
            result="DEFERRED"
            ;;
        *)
            hard_failures=$((hard_failures+1))
            result="HARD-FAIL: unknown status '${status}' (must be covered|deferred)"
            ;;
    esac
    printf '%-10s %-14s %-22s %-52s %s\n' "${id}" "${family}" "${kind}" "${ev}" "${result}"
done < "${rows_file}"

if [[ ${total} -eq 0 ]]; then
    echo "[fvm] FAILED — no matrix rows parsed from ${MATRIX}" >&2
    exit 1
fi

# Integer percentage (floor of covered/total * 100).
pct=$(( covered * 100 / total ))

echo ""
echo "[fvm] rows=${total} covered=${covered} deferred=${deferred} hard_failures=${hard_failures}"
echo "[fvm] Frontend Verification Matrix score = ${covered}/${total} = ${pct}% (floor ${FLOOR}%)"
echo "[fvm] NOTE: this is a verification-matrix score (covered_rows / total_rows), NOT source-line coverage."

if [[ ${hard_failures} -gt 0 ]]; then
    echo "[fvm] FAILED — ${hard_failures} row(s) marked 'covered' have missing or invalid evidence." >&2
    exit 1
fi

if [[ ${pct} -lt ${FLOOR} ]]; then
    echo "[fvm] FAILED — Frontend Verification Matrix score ${pct}% below floor ${FLOOR}%." >&2
    exit 1
fi

echo "[fvm] OK — ${pct}% Frontend Verification Matrix score (${covered}/${total} rows satisfied) ≥ ${FLOOR}% floor."
