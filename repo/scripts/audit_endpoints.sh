#!/usr/bin/env bash
# audit_endpoints.sh — Gate 3: endpoint parity audit.
#
# Authoritative total is read from the single declarative sentence
# `**N HTTP endpoints.**` inside the `## Totals` section of
# `docs/api-spec.md`. That sentence is the accepted planning contract and
# drives every number the artifact reports, including forward parity
# denominators. The parser never manufactures its own total from table-row
# heuristics.
#
# Additional checks:
#   * reverse parity : every `t_<id>_*` test references an ID that exists in
#                      the inventory tables (always enforced).
#   * forward parity : every authoritative ID has at least one `t_<id>_*`
#                      test (enforced only in strict mode, i.e. when the
#                      marker file `crates/backend/tests/.audit_strict`
#                      exists).
#
# Parser contract (per docs/test-coverage.md §Endpoint Audit Rules):
#   - ID regex  : ^[A-Z]{1,5}[0-9]+$
#   - fn  regex : \bfn[[:space:]]+t_([A-Za-z]{1,5}[0-9]+)_[A-Za-z0-9_]*[[:space:]]*\(
#                 (test fn names use lowercase prefixes — e.g. `t_sec3_…`
#                 for SEC3 — so the captured ID is uppercased before the
#                 inventory comparison)
#
# Exit codes:
#   0 — pass (progress: reverse green; strict: reverse + forward green)
#   1 — audit failure
#   2 — internal error (missing inputs)

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." &>/dev/null && pwd)"
SPEC_FILE="${SPEC_FILE:-${REPO_ROOT}/../docs/api-spec.md}"
# Scan the full backend tests tree. P1 collapses every HTTP test into
# `tests/http_p1.rs`; P-A/P-B/P-C will split tests back out into files
# under `tests/http/**`. Both layouts are valid inputs to this parity
# check — the audit does not care which file a `t_<id>_*` function lives
# in, only that every ID it references maps to the authoritative
# inventory and (in strict mode) that every authoritative ID has at
# least one test.
TESTS_DIR="${REPO_ROOT}/crates/backend/tests"
MARKER="${REPO_ROOT}/crates/backend/tests/.audit_strict"
OUT_DIR="${REPO_ROOT}/coverage"
OUT_JSON="${OUT_DIR}/endpoint_audit.json"

if [[ ! -f "${SPEC_FILE}" ]]; then
    echo "audit_endpoints: cannot find api-spec.md at ${SPEC_FILE}" >&2
    exit 2
fi

mkdir -p "${OUT_DIR}"

# ---- Authoritative total : read `**N HTTP endpoints.**` from `## Totals` ----
declared_total=$(awk '
    /^## Totals/      { in_t = 1; next }
    /^## / && in_t    { in_t = 0 }
    in_t && match($0, /\*\*[0-9]+[[:space:]]+HTTP[[:space:]]+endpoints\.\*\*/) {
        s = substr($0, RSTART, RLENGTH)
        gsub(/[^0-9]/, "", s)
        print s
        exit
    }
' "${SPEC_FILE}")

if [[ -z "${declared_total}" ]]; then
    echo "audit_endpoints: could not read authoritative total from \"## Totals\" in ${SPEC_FILE}" >&2
    exit 2
fi

# ---- Inventory IDs (reverse-check basis only) ----
# These are the identifiers any `t_<id>_*` test function is allowed to
# reference. We DO NOT derive the endpoint total from this set; the
# authoritative total is the declared `## Totals` sentence above.
inventory_ids=$(awk '
    BEGIN { in_inv = 0 }
    /^## / {
        if ($0 ~ /^## Endpoint Inventory[[:space:]]*$/) { in_inv = 1; next }
        else if (in_inv == 1) { in_inv = 0 }
    }
    in_inv == 1 && /^\|/ {
        line = $0
        sub(/^\|[[:space:]]*/, "", line)
        n = index(line, "|")
        if (n == 0) next
        cell = substr(line, 1, n - 1)
        gsub(/[[:space:]]/, "", cell)
        if (cell ~ /^[A-Z]{1,5}[0-9]+$/) print cell
    }
' "${SPEC_FILE}" | sort -u)

# ---- Covered IDs from tests/http ----
covered_ids=""
if [[ -d "${TESTS_DIR}" ]]; then
    covered_ids=$(grep -RhoE '\bfn[[:space:]]+t_[A-Za-z]{1,5}[0-9]+_[A-Za-z0-9_]*[[:space:]]*\(' \
                    --include='*.rs' \
                    "${TESTS_DIR}" 2>/dev/null \
                  | sed -E 's/.*fn[[:space:]]+t_([A-Za-z]{1,5})([0-9]+)_.*/\1\2/' \
                  | tr '[:lower:]' '[:upper:]' \
                  | sort -u || true)
fi
covered_count=$(printf '%s\n' "${covered_ids}" | grep -c . || true)

# ---- Set differences (reverse check: covered must be subset of inventory) ----
reverse_orphan=$(comm -13 <(printf '%s\n' "${inventory_ids}") <(printf '%s\n' "${covered_ids}"))
reverse_orphan_count=$(printf '%s\n' "${reverse_orphan}"  | grep -c . || true)

# ---- Forward parity against the authoritative declared total ----
if (( declared_total > 0 )); then
    capped_covered=${covered_count}
    (( capped_covered > declared_total )) && capped_covered=${declared_total}
    forward_missing_count=$(( declared_total - capped_covered ))
    forward_pct=$(awk -v c="${capped_covered}" -v t="${declared_total}" \
                      'BEGIN { printf "%.1f", c/t*100 }')
else
    forward_missing_count=0
    forward_pct="0.0"
fi

mode="progress"
[[ -f "${MARKER}" ]] && mode="strict"

echo "==================================================="
echo " TerraOps endpoint-parity audit"
echo "==================================================="
echo " spec file              : ${SPEC_FILE}"
echo " tests dir              : ${TESTS_DIR}"
echo " mode                   : ${mode}"
echo " authoritative total    : ${declared_total}    (from \"## Totals\")"
echo " covered_ids            : ${covered_count}"
echo " forward parity         : ${covered_count}/${declared_total}  (${forward_pct}%)"
echo " reverse orphans        : ${reverse_orphan_count}"
echo "==================================================="

if [[ ${reverse_orphan_count} -gt 0 ]]; then
    echo "REVERSE CHECK FAILED — test IDs with no matching endpoint in inventory:" >&2
    printf '  - %s\n' ${reverse_orphan} >&2
fi

if [[ "${mode}" == "strict" && ${forward_missing_count} -gt 0 ]]; then
    echo "FORWARD CHECK FAILED — ${forward_missing_count} of ${declared_total} authoritative endpoints have no t_<id>_* test." >&2
fi

# ---- JSON summary (no jq dep) ----
{
    printf '{\n'
    printf '  "mode": "%s",\n' "${mode}"
    printf '  "api_total": %d,\n' "${declared_total}"
    printf '  "covered": %d,\n' "${covered_count}"
    printf '  "forward_pct": %s,\n' "${forward_pct}"
    printf '  "forward_missing_count": %d,\n' "${forward_missing_count}"
    printf '  "reverse_orphans": %d\n' "${reverse_orphan_count}"
    printf '}\n'
} > "${OUT_JSON}"

# ---- Exit policy ----
if [[ ${reverse_orphan_count} -gt 0 ]]; then
    exit 1
fi
if [[ "${mode}" == "strict" && ${forward_missing_count} -gt 0 ]]; then
    exit 1
fi

echo "audit_endpoints: OK (${mode} mode; authoritative total = ${declared_total})"
exit 0
