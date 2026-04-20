# API Spec — TerraOps

All REST JSON under `/api/v1/`. Served by the same Actix-web binary that serves the Yew SPA at `/` (same TLS origin on `:8443`, no CORS). Standard envelope:

```
{ "error_code": "…", "message": "…", "request_id": "…", "details"?: … }
```

Pagination: `page` (default 1), `page_size` (default 50, max 200). Response: `{items, page, page_size, total}` + `X-Total-Count`. Sorting: `sort_by=field&sort_dir=asc|desc` on list endpoints that expose it (see Contract Rules). Timestamps ISO-8601 with offset in payloads; UI formats MM/DD/YYYY hh:mm AM/PM.

Auth headers: `Authorization: Bearer <access_jwt>` + `X-Requested-With: terraops` for mutations. Refresh token via HttpOnly SameSite=strict cookie `tops_refresh`.

## Auth Classification (authoritative)

Every endpoint carries **exactly one** classification; see `design.md §Permissions`:

- `PUBLIC` — no auth.
- `AUTH` — authenticated; no permission required.
- `PERM(code)` — route-level permission.
- `SELF` — authenticated + object-level owner check.
- `PERM_OR_SELF(code)` — pass if holds `code` OR owns the object.

Signature or query-parameter requirements (for example, the signed-image GET) are expressed in §Contract Rules and in the row's Notes column — never by compounding the auth class. All signed-request endpoints are classified `AUTH`; the signature is an additional contract check verified inside the handler.

All notification endpoints (N1–N9) are classified `SELF`. There is no `notification.read` permission code. Cross-user notification reads are not authorized via any role — an administrator cannot read another user's notifications through these endpoints.

## Login Contract (username-first, audit #10 issue #2)

`POST /api/v1/auth/login` accepts a `username` field only. The backend looks up the account by the `username` column in the `users` table; the previously-permitted email-form fallback has been removed. Attempting to sign in with an email address in the `username` field returns `401 AUTH_INVALID_CREDENTIALS` with no hint that email login is supported. The path `/api/v1/auth/login` and the payload shape `{username, password}` are the single documented sign-in contract for every role (steward, analyst, talent-scorer, admin, SRE, demo) — no alternate `/login-email` or `login_or_email` form exists anywhere in the mounted router.

## Endpoint Inventory

Every row below corresponds to an actual `.route(...)` entry mounted by `handlers::configure` in `crates/backend/src/handlers/mod.rs` plus its feature-family `configure` fns. Paths are written with the full `/api/v1/...` prefix that the mounted `web::scope("/api/v1")` produces. The `scripts/audit_endpoints.sh` Gate 3 script parses both these rows and the Rust handler sources to enforce method+path parity (audit #10 issue #1).

### System / Public
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| S1 | GET | `/api/v1/health` | PUBLIC | liveness |
| S2 | GET | `/api/v1/ready` | PUBLIC | readiness (DB ok) |

### Auth & Session
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| A1 | POST | `/api/v1/auth/login` | PUBLIC | body `{username,password}`; 200 → `{access_token, access_expires_at, user}`; sets `tops_refresh` cookie; username-only (no email fallback — audit #10 issue #2); lockout after 10/15min |
| A2 | POST | `/api/v1/auth/refresh` | PUBLIC | rotates refresh and returns a new access token; handler requires a valid `tops_refresh` HttpOnly cookie (see Contract Rules) |
| A3 | POST | `/api/v1/auth/logout` | AUTH | revokes current session |
| A4 | GET | `/api/v1/auth/me` | AUTH | caller profile (with full decrypted email), roles, perm bitset |
| A5 | POST | `/api/v1/auth/change-password` | SELF | requires current password |

### Users & Roles (Admin console)
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| U1 | GET | `/api/v1/users` | PERM(user.manage) | list; email_mask only |
| U2 | POST | `/api/v1/users` | PERM(user.manage) | create |
| U3 | GET | `/api/v1/users/{id}` | PERM(user.manage) | detail; returns full email (admin-only channel) |
| U4 | PATCH | `/api/v1/users/{id}` | PERM(user.manage) | update status, email, TZ |
| U5 | DELETE | `/api/v1/users/{id}` | PERM(user.manage) | soft-disable |
| U6 | POST | `/api/v1/users/{id}/roles` | PERM(role.assign) | body `{role_ids[]}` (replace set) |
| U7 | POST | `/api/v1/users/{id}/unlock` | PERM(user.manage) | clear lockout |
| U8 | POST | `/api/v1/users/{id}/reset-password` | PERM(user.manage) | force reset on next login |
| U9 | GET | `/api/v1/roles` | PERM(user.manage) | list roles + permission map |
| U10 | GET | `/api/v1/audit` | PERM(user.manage) | query audit entries |

### Security & Ops (Admin)
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| SEC1 | GET | `/api/v1/security/allowlist` | PERM(allowlist.manage) | list |
| SEC2 | POST | `/api/v1/security/allowlist` | PERM(allowlist.manage) | add CIDR |
| SEC3 | DELETE | `/api/v1/security/allowlist/{id}` | PERM(allowlist.manage) | remove |
| SEC4 | GET | `/api/v1/security/device-certs` | PERM(mtls.manage) | list pin set |
| SEC5 | POST | `/api/v1/security/device-certs` | PERM(mtls.manage) | register pre-issued cert (SPKI + label + user assignment) |
| SEC6 | DELETE | `/api/v1/security/device-certs/{id}` | PERM(mtls.manage) | revoke + remove pin (single combined delete route) |
| SEC7 | GET | `/api/v1/security/mtls` | PERM(mtls.manage) | read enforcement state |
| SEC8 | PATCH | `/api/v1/security/mtls` | PERM(mtls.manage) | flip `enforced` |
| SEC9 | GET | `/api/v1/security/mtls/status` | PERM(mtls.manage) | live cert+pin counts |

### Retention (Admin)
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| R1 | GET | `/api/v1/retention` | PERM(retention.manage) | list policies |
| R2 | PATCH | `/api/v1/retention/{domain}` | PERM(retention.manage) | update TTL |
| R3 | POST | `/api/v1/retention/{domain}/run` | PERM(retention.manage) | manual enforce |

### Monitoring (Admin)
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| M1 | GET | `/api/v1/monitoring/latency` | PERM(monitoring.read) | p50/p95/p99 over window |
| M2 | GET | `/api/v1/monitoring/errors` | PERM(monitoring.read) | error rate by route |
| M3 | GET | `/api/v1/monitoring/crash-reports` | PERM(monitoring.read) | list crash reports |
| M4 | POST | `/api/v1/monitoring/crash-report` | AUTH | client-reported crash ingest |

### Reference Data
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| REF1 | GET | `/api/v1/ref/sites` | AUTH | |
| REF2 | GET | `/api/v1/ref/departments` | AUTH | |
| REF3 | GET | `/api/v1/ref/categories` | AUTH | hierarchical |
| REF4 | GET | `/api/v1/ref/brands` | AUTH | |
| REF5 | GET | `/api/v1/ref/units` | AUTH | |
| REF6 | GET | `/api/v1/ref/states` | AUTH | US states for tax rates |
| REF7 | POST | `/api/v1/ref/categories` | PERM(ref.write) | |
| REF8 | POST | `/api/v1/ref/brands` | PERM(ref.write) | |
| REF9 | POST | `/api/v1/ref/units` | PERM(ref.write) | |

### Products
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| P1 | GET | `/api/v1/products` | PERM(product.read) | filter + sort |
| P2 | POST | `/api/v1/products` | PERM(product.write) | create |
| P3 | GET | `/api/v1/products/{id}` | PERM(product.read) | detail + tax rates + images |
| P4 | PATCH | `/api/v1/products/{id}` | PERM(product.write) | partial update |
| P5 | DELETE | `/api/v1/products/{id}` | PERM(product.write) | soft delete |
| P6 | POST | `/api/v1/products/{id}/status` | PERM(product.write) | toggle on/off-shelf |
| P7 | GET | `/api/v1/products/{id}/history` | PERM(product.history.read) | immutable change log |
| P8 | POST | `/api/v1/products/{id}/tax-rates` | PERM(product.write) | add rate |
| P9 | PATCH | `/api/v1/products/{id}/tax-rates/{rid}` | PERM(product.write) | update |
| P10 | DELETE | `/api/v1/products/{id}/tax-rates/{rid}` | PERM(product.write) | remove |
| P11 | POST | `/api/v1/products/{id}/images` | PERM(product.write) | multipart upload |
| P12 | DELETE | `/api/v1/products/{id}/images/{imgid}` | PERM(product.write) | |
| P13 | GET | `/api/v1/images/{imgid}` | AUTH | signed-URL retrieval; requires valid `sig` + `exp` query params (see Contract Rules) |
| P14 | POST | `/api/v1/products/export` | PERM(product.read) | kind=csv\|xlsx, streams |

### Product Imports
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| I1 | POST | `/api/v1/imports` | PERM(product.import) | multipart CSV/XLSX → uploaded batch |
| I2 | GET | `/api/v1/imports` | PERM(product.import) | list batches |
| I3 | GET | `/api/v1/imports/{id}` | PERM(product.import) | batch + rows summary |
| I4 | GET | `/api/v1/imports/{id}/rows` | PERM(product.import) | paginated row validation |
| I5 | POST | `/api/v1/imports/{id}/validate` | PERM(product.import) | validation pass |
| I6 | POST | `/api/v1/imports/{id}/commit` | PERM(product.import) | commit iff 0 errors |
| I7 | POST | `/api/v1/imports/{id}/cancel` | PERM(product.import) | |

### Environmental Sources & Observations
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| E1 | GET | `/api/v1/env/sources` | PERM(metric.read) | list |
| E2 | POST | `/api/v1/env/sources` | PERM(metric.configure) | |
| E3 | PATCH | `/api/v1/env/sources/{id}` | PERM(metric.configure) | |
| E4 | DELETE | `/api/v1/env/sources/{id}` | PERM(metric.configure) | |
| E5 | POST | `/api/v1/env/sources/{id}/observations` | PERM(metric.configure) | bulk ingest |
| E6 | GET | `/api/v1/env/observations` | PERM(metric.read) | filter source, window |

### Metric Definitions & Lineage
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| MD1 | GET | `/api/v1/metrics/definitions` | PERM(metric.read) | |
| MD2 | POST | `/api/v1/metrics/definitions` | PERM(metric.configure) | |
| MD3 | GET | `/api/v1/metrics/definitions/{id}` | PERM(metric.read) | |
| MD4 | PATCH | `/api/v1/metrics/definitions/{id}` | PERM(metric.configure) | |
| MD5 | DELETE | `/api/v1/metrics/definitions/{id}` | PERM(metric.configure) | soft-disable |
| MD6 | GET | `/api/v1/metrics/definitions/{id}/series` | PERM(metric.read) | windowed values |
| MD7 | GET | `/api/v1/metrics/computations/{id}/lineage` | PERM(metric.read) | "why this value" |

### KPIs
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| K1 | GET | `/api/v1/kpi/summary` | PERM(kpi.read) | cycle/funnel/anomaly/efficiency cards |
| K2 | GET | `/api/v1/kpi/cycle-time` | PERM(kpi.read) | sliced/drill |
| K3 | GET | `/api/v1/kpi/funnel` | PERM(kpi.read) | |
| K4 | GET | `/api/v1/kpi/anomalies` | PERM(kpi.read) | |
| K5 | GET | `/api/v1/kpi/efficiency` | PERM(kpi.read) | |
| K6 | GET | `/api/v1/kpi/drill` | PERM(kpi.read) | generic `{site,department,time,category}` drill |

### Alerts
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| AL1 | GET | `/api/v1/alerts/rules` | PERM(alert.manage) | rules are management surface |
| AL2 | POST | `/api/v1/alerts/rules` | PERM(alert.manage) | |
| AL3 | PATCH | `/api/v1/alerts/rules/{id}` | PERM(alert.manage) | |
| AL4 | DELETE | `/api/v1/alerts/rules/{id}` | PERM(alert.manage) | |
| AL5 | GET | `/api/v1/alerts/events` | PERM(kpi.read) | any dashboard-authorized user can see firing events |
| AL6 | POST | `/api/v1/alerts/events/{id}/ack` | PERM(alert.ack) | |

### Reports
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| RP1 | GET | `/api/v1/reports/jobs` | PERM(report.run) | lists caller's own jobs only (service-layer owner filter) |
| RP2 | POST | `/api/v1/reports/jobs` | PERM(report.schedule) | kind, definition, optional cron |
| RP3 | GET | `/api/v1/reports/jobs/{id}` | SELF | owner-only; admin covered via audit log, not this endpoint |
| RP4 | POST | `/api/v1/reports/jobs/{id}/run-now` | SELF | |
| RP5 | POST | `/api/v1/reports/jobs/{id}/cancel` | SELF | |
| RP6 | GET | `/api/v1/reports/jobs/{id}/artifact` | SELF | streams last artifact |

### Talent
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| T1 | GET | `/api/v1/talent/candidates` | PERM(talent.read) | search + filter; audit #10 issue #3 — accepts whitelisted `sort_by` / `sort_dir` |
| T2 | POST | `/api/v1/talent/candidates` | PERM(talent.manage) | create/update |
| T3 | GET | `/api/v1/talent/candidates/{id}` | PERM(talent.read) | |
| T4 | GET | `/api/v1/talent/roles` | PERM(talent.read) | open roles |
| T5 | POST | `/api/v1/talent/roles` | PERM(talent.manage) | |
| T6 | GET | `/api/v1/talent/recommendations` | PERM(talent.read) | cold_start flag in response |
| T7 | GET | `/api/v1/talent/weights` | SELF | caller's weights |
| T8 | PUT | `/api/v1/talent/weights` | SELF | upsert caller's weights (owner only) |
| T9 | POST | `/api/v1/talent/feedback` | PERM(talent.feedback) | owner-scoped record |
| T10 | GET | `/api/v1/talent/watchlists` | SELF | caller's private watchlists |
| T11 | POST | `/api/v1/talent/watchlists` | SELF | |
| T12 | POST | `/api/v1/talent/watchlists/{id}/items` | SELF | add candidate to watchlist |
| T13 | DELETE | `/api/v1/talent/watchlists/{id}/items/{cid}` | SELF | remove candidate from watchlist |
| T14 | GET | `/api/v1/talent/watchlists/{id}/items` | SELF | list candidates on watchlist |

### Notifications
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| N1 | GET | `/api/v1/notifications` | SELF | caller's own; `unread=1` filter |
| N2 | POST | `/api/v1/notifications/{id}/read` | SELF | |
| N3 | POST | `/api/v1/notifications/read-all` | SELF | |
| N4 | GET | `/api/v1/notifications/unread-count` | SELF | returns `{unread: N}` |
| N5 | GET | `/api/v1/notifications/subscriptions` | SELF | |
| N6 | PUT | `/api/v1/notifications/subscriptions` | SELF | upsert topic toggles |
| N7 | GET | `/api/v1/notifications/mailbox-exports` | SELF | list prior exports |
| N8 | POST | `/api/v1/notifications/mailbox-export` | SELF | generate local `.mbox` file |
| N9 | GET | `/api/v1/notifications/mailbox-exports/{id}` | SELF | download artifact |

## Totals

**117 HTTP endpoints.** Every row above has a corresponding test row in `docs/test-coverage.md §Endpoint-to-Test Map`.

Breakdown (matches the inventory tables above, all `## Endpoint Inventory` rows):

- System S1–S2 (2) + Auth A1–A5 (5) + Users U1–U10 (10) + Security SEC1–SEC9 (9) + Retention R1–R3 (3) + Monitoring M1–M4 (4) + Reference Data REF1–REF9 (9) + Notifications N1–N9 (9) = **51 P1 endpoints**.
- Products P1–P14 (14) + Product Imports I1–I7 (7) = **21 P-A endpoints**.
- Env E1–E6 (6) + Metrics MD1–MD7 (7) + KPI K1–K6 (6) + Alerts AL1–AL6 (6) + Reports RP1–RP6 (6) = **31 P-B endpoints**.
- Talent T1–T14 (14) = **14 P-C endpoints**.

51 + 21 + 31 + 14 = **117 total**. This supersedes the old `114` figure that predated:

- the three new notification endpoints (`unread-count`, list-`mailbox-exports`, download `mailbox-exports/{id}` — now N4/N7/N9);
- T14 (`GET /talent/watchlists/{id}/items`); and
- the revised SEC6 mapping (combined DELETE `/security/device-certs/{id}` replaces the previous split revoke/delete pair).

## Contract Rules

- Non-GET handlers that touch multiple tables run in a single DB transaction.
- List endpoints always wrap items + set `X-Total-Count`.
- The frontend client retries only idempotent GETs (exactly once, 3 s per attempt).
- Handler >3 s → `504 TIMEOUT` with `request_id`.
- **Candidate sort contract (applies to T1, audit #10 issue #3)**: `GET /api/v1/talent/candidates` accepts optional `sort_by` + `sort_dir` query parameters. `sort_by` must be one of `last_active_at`, `created_at`, `updated_at`, `full_name`, `years_experience`, `completeness_score`. `sort_dir` must be `asc` or `desc`. Unknown values return `400 VALIDATION`. The handler validates against the whitelist before passing the value down to `talent::candidates::list`, which only interpolates fixed, hard-coded ORDER BY strings — no untrusted identifier ever enters the SQL text. Defaults: `last_active_at` / `desc`, matching pre-audit behavior.
- **Refresh contract (applies to A2)**: although classified `PUBLIC`, the `/auth/refresh` handler requires a valid, non-revoked, non-expired `tops_refresh` cookie (HttpOnly, SameSite=strict, set at login). The handler looks up the cookie's hash in `sessions`, rotates the row (old refresh is marked `revoked`, new refresh is issued), and returns a new access token. Missing/invalid/expired/revoked cookie → `401 AUTH_INVALID_CREDENTIALS`. The cookie check is a handler-level concern, not an auth-classification one, so A2 retains a single canonical class.
- **Login contract (applies to A1, audit #10 issue #2)**: `username` field is the only credential identifier accepted. The handler calls `services::users::find_by_username` and does not fall back to `find_by_email` — an email in the `username` field is rejected with `401 AUTH_INVALID_CREDENTIALS`. The reverse-test `t_a1_login_rejects_email_as_username` exercises this path against the real HTTP endpoint.
- **Signed image URL contract (applies to P13)**: request must include `sig` and `exp` query parameters. `sig = HMAC_SHA256(image_hmac_key, "{path}|{user_id}|{exp}")`. Handler rejects (403) if `exp` is in the past, if `sig` is missing, if `sig` does not match the recomputed HMAC, or if the authenticated caller's user id does not match the one bound in the signature. The `AUTH` classification applies to the route; the signature is an additional handler-level check that lets revoked sessions be refused even if the URL is still within `exp`.
- mTLS pin rejection happens at TLS handshake; it is not an HTTP response. Admin monitoring surfaces the refused-connection counter separately.
- `/security/device-certs` mutations bump an in-memory pin-set generation; the Rustls verifier reloads within ~30 s.

## Static / SPA Serving (not under /api/v1)

- `GET /` → `dist/index.html` (PUBLIC, always).
- `GET /assets/**` → hashed static assets (PUBLIC, cache-friendly headers).
- `GET /<anything-else-without-/api/v1>` → `dist/index.html` (SPA fallback).

These are served by the same `app` binary; they do not appear in the endpoint inventory totals.
