#!/usr/bin/env bash
# audit_endpoints.sh — Gate 3: endpoint parity audit.
#
# This script enforces three parity contracts between the spec file
# `docs/api-spec.md` and the actual mounted Actix-web routes in
# `crates/backend/src/**/*.rs`.
#
#   1. Reverse ID parity (always enforced):
#        every `t_<id>_*` test fn references an ID that appears in the
#        `## Endpoint Inventory` tables of api-spec.md.
#
#   2. Forward ID parity (strict mode only — marker file
#      `crates/backend/tests/.audit_strict` exists):
#        every authoritative ID in api-spec.md has at least one
#        `t_<id>_*` test fn.
#
#   3. Method+path parity (audit #10 issue #1, always enforced):
#        the (METHOD, PATH) set derived from api-spec.md inventory rows
#        matches the (METHOD, PATH) set mounted by the backend router.
#        Drift in either direction is a failure — this is what catches
#        "spec lies about what is mounted" and "mounted route nobody
#        documented" the way pure ID checks cannot.
#
# Parser contracts (per docs/test-coverage.md §Endpoint Audit Rules):
#   - ID regex      : ^[A-Z]{1,5}[0-9]+$
#   - test fn regex : \bfn[[:space:]]+t_([A-Za-z]{1,5}[0-9]+)_[A-Za-z0-9_]*[[:space:]]*\(
#                     (test fn names use lowercase prefixes — e.g. `t_sec3_…`
#                     for SEC3 — so the captured ID is uppercased before the
#                     inventory comparison)
#   - spec row      : `| <ID> | <METHOD> | \`<path>\` | <auth> | <notes> |`
#   - route line    : `.route("<relative-path>", web::<method>().to(...))`
#                     prefixed with the enclosing `web::scope("/<prefix>")`
#                     and the fixed `/api/v1` app-level scope.
#
# Authoritative endpoint total is the single declarative sentence
# `**N HTTP endpoints.**` inside the `## Totals` section of api-spec.md.
#
# Exit codes:
#   0 — pass
#   1 — audit failure (method+path drift, reverse orphans, or
#       strict-mode forward miss)
#   2 — internal error (missing inputs)

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." &>/dev/null && pwd)"
SPEC_FILE="${SPEC_FILE:-${REPO_ROOT}/../docs/api-spec.md}"
TESTS_DIR="${REPO_ROOT}/crates/backend/tests"
HANDLERS_DIR="${REPO_ROOT}/crates/backend/src"
MARKER="${REPO_ROOT}/crates/backend/tests/.audit_strict"
OUT_DIR="${REPO_ROOT}/coverage"
OUT_JSON="${OUT_DIR}/endpoint_audit.json"

if [[ ! -f "${SPEC_FILE}" ]]; then
    echo "audit_endpoints: cannot find api-spec.md at ${SPEC_FILE}" >&2
    exit 2
fi
if [[ ! -d "${HANDLERS_DIR}" ]]; then
    echo "audit_endpoints: cannot find backend src at ${HANDLERS_DIR}" >&2
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

# ---- Inventory IDs + spec (method, path) tuples --------------------------
# We parse every row in `## Endpoint Inventory` that looks like:
#     | <ID> | <METHOD> | `<path>` | <auth> | <notes> |
# and emit two artifacts:
#   * inventory_ids   — one uppercase ID per line
#   * spec_pairs      — "<METHOD> <path>" one per line, uppercase METHOD,
#                       path exactly as written inside the backticks
INVENTORY_AWK='
    BEGIN { in_inv = 0 }
    /^## / {
        # Use substr() prefix match — avoids [[:space:]]*$ anchor differences
        # between mawk (Debian) / gawk (Amazon Linux) / BSD awk (macOS).
        if (substr($0, 1, 22) == "## Endpoint Inventory") { in_inv = 1; next }
        else if (in_inv == 1) { in_inv = 0 }
    }
    in_inv == 1 && /^\|/ {
        # split on "|" and trim each cell
        n = split($0, cells, "|")
        # cells[1] is empty (leading |), cells[2]=id, cells[3]=method,
        # cells[4]=path (backticked), cells[5]=auth, cells[6]=notes
        if (n < 5) next
        id = cells[2]; gsub(/[[:space:]]/, "", id)
        meth = cells[3]; gsub(/[[:space:]]/, "", meth)
        path_cell = cells[4]
        gsub(/`/, "", path_cell)
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", path_cell)
        if (id !~ /^[A-Z]{1,5}[0-9]+$/) next
        if (meth !~ /^(GET|POST|PUT|PATCH|DELETE)$/) next
        if (path_cell !~ /^\/api\/v1\//) next
        printf "ID\t%s\n", id
        printf "PAIR\t%s %s\n", meth, path_cell
    }
'

spec_stream=$(awk "${INVENTORY_AWK}" "${SPEC_FILE}")
inventory_ids=$(printf '%s\n' "${spec_stream}" | awk -F'\t' '$1=="ID"{print $2}' | sort -u)
spec_pairs=$(printf '%s\n' "${spec_stream}" | awk -F'\t' '$1=="PAIR"{print $2}' | sort -u)

inventory_count=$(printf '%s\n' "${inventory_ids}" | grep -c . || true)
spec_pair_count=$(printf '%s\n' "${spec_pairs}"   | grep -c . || true)

# ---- Derive (METHOD, PATH) from the backend router ------------------------
# Strategy:
#   * Start at the app-level scope `/api/v1` (hard-coded — there is exactly
#     one such scope in app.rs, see `web::scope("/api/v1")`).
#   * Walk every handler source file that calls `cfg.service(web::scope(...))`
#     or `cfg.route(...)` directly.
#   * For each file, run a small awk state machine:
#       - when we see `web::scope("<prefix>")` record the active prefix
#         until the enclosing service block closes with ");".
#       - when we see `.route("<p>", web::<m>().to(...))` emit
#         (METHOD, /api/v1 + scope_prefix + p).
#       - when we see a bare `cfg.route("<p>", web::<m>().to(...))`
#         emit (METHOD, /api/v1 + p) — no scope prefix.
#
# This mirrors the real router structure:
#   * handlers/system.rs      → bare cfg.route (no scope)
#   * handlers/users.rs       → web::scope("/users") + bare cfg.route for
#                               "/roles" and "/audit"
#   * handlers/auth.rs etc.   → single web::scope
#   * products/handlers.rs    → web::scope("/products"), bare
#                               cfg.route("/images/{imgid}", ...),
#                               web::scope("/imports")
#
# We also skip `web::scope("/api/v1")` (app-level) and the SPA fallback in
# spa.rs, neither of which belong in the inventory.

ROUTER_AWK='
    function extract_between(s, open_tok, close_tok,    i, j) {
        i = index(s, open_tok)
        if (i == 0) return ""
        i += length(open_tok)
        j = index(substr(s, i), close_tok)
        if (j == 0) return ""
        return substr(s, i, j - 1)
    }
    function extract_method(s,    p) {
        # look for web::<method>()
        if (match(s, /web::(get|post|put|patch|delete)\(\)/)) {
            p = substr(s, RSTART, RLENGTH)
            sub(/^web::/, "", p); sub(/\(\)$/, "", p)
            return toupper(p)
        }
        return ""
    }

    function emit(scope, p, m_u) {
        if (m_u == "") return
        # p may legitimately be "" for `.route("", web::get())` — that means
        # the route is mounted at exactly the scope root. Treat the empty
        # path as a valid suffix and only reject if BOTH scope and p are
        # empty (no route at all).
        if (scope == "" && p == "") return
        printf "%s %s\n", m_u, "/api/v1" scope p
    }
    BEGIN { scope = ""; in_scope = 0; pending = 0; pending_p = "" }
    # Ignore the app-level scope — it only wraps the configure() call.
    /web::scope\("\/api\/v1"\)/ { next }
    # Enter a feature-family scope block.
    /web::scope\("/ {
        scope = extract_between($0, "web::scope(\"", "\"")
        if (scope != "") { in_scope = 1; next }
    }
    # End of the enclosing cfg.service(...) block that held the scope.
    in_scope == 1 && /^[[:space:]]*\);[[:space:]]*$/ {
        scope = ""; in_scope = 0; pending = 0; pending_p = ""
        next
    }
    # Route inside an active scope. Two shapes:
    #   (a) single line:  .route("<p>", web::<m>().to(...))
    #   (b) multi line :  .route(\n  "<p>",\n  web::<m>().to(...),\n)
    # Form (a) has web::<m>() on the same line; form (b) does not.
    in_scope == 1 && /\.route\(/ {
        # On same line if `.route("…"` appears; otherwise multi-line form.
        if (match($0, /\.route\("/)) {
            p = extract_between($0, ".route(\"", "\"")
            m_u = extract_method($0)
            if (m_u != "") {
                emit(scope, p, m_u)
                pending = 0; pending_p = ""
            } else {
                pending = 1; pending_p = p
            }
        } else {
            # Multi-line `.route(` — path comes on a later line.
            pending = 1; pending_p = ""
        }
        next
    }
    # Multi-line form (a) continuation — path on its own line after bare `.route(`
    in_scope == 1 && pending == 1 && /^[[:space:]]*"[^"]*"[[:space:]]*,[[:space:]]*$/ && pending_p == "" {
        line = $0
        sub(/^[[:space:]]*"/, "", line); sub(/"[[:space:]]*,[[:space:]]*$/, "", line)
        pending_p = line
        next
    }
    # Multi-line form (b) continuation — method on a later line.
    in_scope == 1 && pending == 1 {
        m_u = extract_method($0)
        if (m_u != "") {
            emit(scope, pending_p, m_u)
            pending = 0; pending_p = ""
        }
        next
    }
    # Bare cfg.route (no active scope).
    in_scope == 0 && /cfg\.route\("/ {
        p = extract_between($0, "cfg.route(\"", "\"")
        m_u = extract_method($0)
        if (p == "/{tail:.*}") next   # SPA fallback
        if (m_u != "") emit("", p, m_u)
        next
    }
'

router_pairs=$(find "${HANDLERS_DIR}" -type f -name '*.rs' -print0 \
    | xargs -0 awk "${ROUTER_AWK}" \
    | sort -u)
router_pair_count=$(printf '%s\n' "${router_pairs}" | grep -c . || true)

# ---- Covered test IDs ------------------------------------------------------
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

# ---- Set differences -------------------------------------------------------
reverse_orphan=$(comm -13 <(printf '%s\n' "${inventory_ids}") <(printf '%s\n' "${covered_ids}"))
reverse_orphan_count=$(printf '%s\n' "${reverse_orphan}" | grep -c . || true)

spec_only_pairs=$(comm -23 <(printf '%s\n' "${spec_pairs}")   <(printf '%s\n' "${router_pairs}"))
router_only_pairs=$(comm -13 <(printf '%s\n' "${spec_pairs}") <(printf '%s\n' "${router_pairs}"))
spec_only_count=$(printf '%s\n' "${spec_only_pairs}"   | grep -c . || true)
router_only_count=$(printf '%s\n' "${router_only_pairs}" | grep -c . || true)

# ---- Forward ID parity against the authoritative declared total -----------
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
echo " TerraOps endpoint-parity audit (Gate 3)"
echo "==================================================="
echo " spec file              : ${SPEC_FILE}"
echo " handlers dir           : ${HANDLERS_DIR}"
echo " tests dir              : ${TESTS_DIR}"
echo " mode                   : ${mode}"
echo " authoritative total    : ${declared_total}    (from \"## Totals\")"
echo " spec inventory IDs     : ${inventory_count}"
echo " spec (method,path)     : ${spec_pair_count}"
echo " router (method,path)   : ${router_pair_count}"
echo " covered test IDs       : ${covered_count}"
echo " forward parity         : ${covered_count}/${declared_total}  (${forward_pct}%)"
echo " reverse orphans        : ${reverse_orphan_count}"
echo " spec-only routes       : ${spec_only_count}"
echo " router-only routes     : ${router_only_count}"
echo "==================================================="

if [[ ${reverse_orphan_count} -gt 0 ]]; then
    echo "REVERSE ID CHECK FAILED — test IDs with no matching endpoint in inventory:" >&2
    printf '  - %s\n' ${reverse_orphan} >&2
fi

if [[ ${spec_only_count} -gt 0 ]]; then
    echo "METHOD+PATH CHECK FAILED — routes documented in api-spec.md but NOT mounted in handlers:" >&2
    printf '  - %s\n' "${spec_only_pairs}" >&2
fi

if [[ ${router_only_count} -gt 0 ]]; then
    echo "METHOD+PATH CHECK FAILED — routes mounted in handlers but NOT documented in api-spec.md:" >&2
    printf '  - %s\n' "${router_only_pairs}" >&2
fi

if [[ "${mode}" == "strict" && ${forward_missing_count} -gt 0 ]]; then
    echo "FORWARD ID CHECK FAILED — ${forward_missing_count} of ${declared_total} authoritative endpoints have no t_<id>_* test." >&2
fi

# ---- JSON summary (no jq dep) ----
{
    printf '{\n'
    printf '  "mode": "%s",\n' "${mode}"
    printf '  "api_total": %d,\n' "${declared_total}"
    printf '  "spec_inventory_ids": %d,\n' "${inventory_count}"
    printf '  "spec_pairs": %d,\n' "${spec_pair_count}"
    printf '  "router_pairs": %d,\n' "${router_pair_count}"
    printf '  "covered": %d,\n' "${covered_count}"
    printf '  "forward_pct": %s,\n' "${forward_pct}"
    printf '  "forward_missing_count": %d,\n' "${forward_missing_count}"
    printf '  "reverse_orphans": %d,\n' "${reverse_orphan_count}"
    printf '  "spec_only_routes": %d,\n' "${spec_only_count}"
    printf '  "router_only_routes": %d\n' "${router_only_count}"
    printf '}\n'
} > "${OUT_JSON}"

# ---- Exit policy ----
if [[ ${reverse_orphan_count} -gt 0 ]]; then
    exit 1
fi
if [[ ${spec_only_count} -gt 0 || ${router_only_count} -gt 0 ]]; then
    exit 1
fi
if [[ "${mode}" == "strict" && ${forward_missing_count} -gt 0 ]]; then
    exit 1
fi

echo "audit_endpoints: OK (${mode} mode; authoritative total = ${declared_total}; method+path parity clean)"
exit 0
