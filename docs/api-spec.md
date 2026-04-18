# API Spec — TerraOps

All REST JSON under `/api/v1/`. Served by the same Actix-web binary that serves the Yew SPA at `/` (same TLS origin on `:8443`, no CORS). Standard envelope:

```
{ "error_code": "…", "message": "…", "request_id": "…", "details"?: … }
```

Pagination: `page` (default 1), `page_size` (default 50, max 200). Response: `{items, page, page_size, total}` + `X-Total-Count`. Sorting: `sort=field:asc,other:desc`. Timestamps ISO-8601 with offset in payloads; UI formats MM/DD/YYYY hh:mm AM/PM.

Auth headers: `Authorization: Bearer <access_jwt>` + `X-Requested-With: terraops` for mutations. Refresh token via HttpOnly SameSite=strict cookie `tops_refresh`.

## Auth Classification (authoritative)

Every endpoint carries **exactly one** classification; see `design.md §Permissions`:

- `PUBLIC` — no auth.
- `AUTH` — authenticated; no permission required.
- `PERM(code)` — route-level permission.
- `SELF` — authenticated + object-level owner check.
- `PERM_OR_SELF(code)` — pass if holds `code` OR owns the object.

Signature or query-parameter requirements (for example, the signed-image GET) are expressed in §Contract Rules and in the row's Notes column — never by compounding the auth class. All signed-request endpoints are classified `AUTH`; the signature is an additional contract check verified inside the handler.

All notification endpoints (N1–N7) are classified `SELF`. There is no `notification.read` permission code. Cross-user notification reads are not authorized via any role — an administrator cannot read another user's notifications through these endpoints.

## Endpoint Inventory

### System / Public
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| S1 | GET | `/api/v1/health` | PUBLIC | liveness |
| S2 | GET | `/api/v1/ready` | PUBLIC | readiness (DB ok) |

### Auth & Session
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| A1 | POST | `/api/v1/auth/login` | PUBLIC | body `{username,password}`; 200 → `{access_token, access_expires_at, user}`; sets `tops_refresh` cookie; lockout after 10/15min |
| A2 | POST | `/api/v1/auth/refresh` | PUBLIC | rotates refresh and returns a new access token; handler requires a valid `tops_refresh` HttpOnly cookie (see Contract Rules) — the cookie is not an auth-classification concern |
| A3 | POST | `/api/v1/auth/logout` | AUTH | revokes current session |
| A4 | GET | `/api/v1/auth/me` | AUTH | caller profile (with full decrypted email), roles, perm bitset |
| A5 | POST | `/api/v1/auth/change-password` | SELF | requires current password |

### Users & Roles (Admin console)
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| U1 | GET | `/api/v1/admin/users` | PERM(user.manage) | list; email_mask only |
| U2 | POST | `/api/v1/admin/users` | PERM(user.manage) | create |
| U3 | GET | `/api/v1/admin/users/{id}` | PERM(user.manage) | detail; returns full email (admin-only channel) |
| U4 | PATCH | `/api/v1/admin/users/{id}` | PERM(user.manage) | update status, email, TZ |
| U5 | DELETE | `/api/v1/admin/users/{id}` | PERM(user.manage) | soft-disable |
| U6 | POST | `/api/v1/admin/users/{id}/roles` | PERM(role.assign) | body `{role_ids[]}` (replace set) |
| U7 | POST | `/api/v1/admin/users/{id}/unlock` | PERM(user.manage) | clear lockout |
| U8 | POST | `/api/v1/admin/users/{id}/reset-password` | PERM(user.manage) | force reset on next login |
| U9 | GET | `/api/v1/admin/roles` | PERM(user.manage) | list roles + permission map |
| U10 | GET | `/api/v1/admin/audit-log` | PERM(user.manage) | query audit entries |

### Security & Ops (Admin)
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| SEC1 | GET | `/api/v1/admin/allowlist` | PERM(allowlist.manage) | list |
| SEC2 | POST | `/api/v1/admin/allowlist` | PERM(allowlist.manage) | add CIDR |
| SEC3 | DELETE | `/api/v1/admin/allowlist/{id}` | PERM(allowlist.manage) | remove |
| SEC4 | GET | `/api/v1/admin/device-certs` | PERM(mtls.manage) | list pin set |
| SEC5 | POST | `/api/v1/admin/device-certs` | PERM(mtls.manage) | register pre-issued cert (SPKI + label + user assignment) |
| SEC6 | POST | `/api/v1/admin/device-certs/{id}/revoke` | PERM(mtls.manage) | revoke pin |
| SEC7 | DELETE | `/api/v1/admin/device-certs/{id}` | PERM(mtls.manage) | delete record |
| SEC8 | GET | `/api/v1/admin/mtls-config` | PERM(mtls.manage) | read enforcement state |
| SEC9 | PUT | `/api/v1/admin/mtls-config` | PERM(mtls.manage) | flip `enforced` |
| R1 | GET | `/api/v1/admin/retention` | PERM(retention.manage) | list policies |
| R2 | PUT | `/api/v1/admin/retention/{domain}` | PERM(retention.manage) | update TTL |
| R3 | POST | `/api/v1/admin/retention/{domain}/run` | PERM(retention.manage) | manual enforce |

### Monitoring (Admin)
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| M1 | GET | `/api/v1/monitoring/latency` | PERM(monitoring.read) | p50/p95/p99 over window |
| M2 | GET | `/api/v1/monitoring/errors` | PERM(monitoring.read) | error rate by route |
| M3 | GET | `/api/v1/monitoring/crashes` | PERM(monitoring.read) | list crash reports |
| M4 | POST | `/api/v1/monitoring/crashes` | AUTH | client-reported crash ingest |

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
| I2 | GET | `/api/v1/imports` | PERM(product.import) | list batches (steward sees all; could add SELF filter) |
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
| T1 | GET | `/api/v1/talent/candidates` | PERM(talent.read) | search + filter + sort |
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
| T12 | POST | `/api/v1/talent/watchlists/{id}/items` | SELF | |
| T13 | DELETE | `/api/v1/talent/watchlists/{id}/items/{cid}` | SELF | |

### Notifications
| # | Method | Path | Auth | Notes |
|---|---|---|---|---|
| N1 | GET | `/api/v1/notifications` | SELF | caller's own; `unread=1` filter |
| N2 | POST | `/api/v1/notifications/{id}/read` | SELF | |
| N3 | POST | `/api/v1/notifications/read-all` | SELF | |
| N4 | GET | `/api/v1/notifications/subscriptions` | SELF | |
| N5 | PUT | `/api/v1/notifications/subscriptions` | SELF | upsert topic toggles |
| N6 | POST | `/api/v1/notifications/mailbox-export` | SELF | generates local `.mbox` file |
| N7 | GET | `/api/v1/notifications/mailbox-exports/{id}` | SELF | download artifact |

## Totals

**77 HTTP endpoints.** Every row above has a corresponding test row in `docs/test-coverage.md §Endpoint-to-Test Map`.

## Contract Rules

- Non-GET handlers that touch multiple tables run in a single DB transaction.
- List endpoints always wrap items + set `X-Total-Count`.
- The frontend client retries only idempotent GETs (exactly once, 3 s per attempt).
- Handler >3 s → `504 TIMEOUT` with `request_id`.
- **Refresh contract (applies to A2)**: although classified `PUBLIC`, the `/auth/refresh` handler requires a valid, non-revoked, non-expired `tops_refresh` cookie (HttpOnly, SameSite=strict, set at login). The handler looks up the cookie's hash in `sessions`, rotates the row (old refresh is marked `revoked`, new refresh is issued), and returns a new access token. Missing/invalid/expired/revoked cookie → `401 AUTH_INVALID_CREDENTIALS`. The cookie check is a handler-level concern, not an auth-classification one, so A2 retains a single canonical class.
- **Signed image URL contract (applies to P13)**: request must include `sig` and `exp` query parameters. `sig = HMAC_SHA256(image_hmac_key, "{path}|{user_id}|{exp}")`. Handler rejects (403) if `exp` is in the past, if `sig` is missing, if `sig` does not match the recomputed HMAC, or if the authenticated caller's user id does not match the one bound in the signature. The `AUTH` classification applies to the route; the signature is an additional handler-level check that lets revoked sessions be refused even if the URL is still within `exp`.
- mTLS pin rejection happens at TLS handshake; it is not an HTTP response. Admin monitoring surfaces the refused-connection counter separately.
- `/admin/device-certs` mutations bump an in-memory pin-set generation; the Rustls verifier reloads within 1 s.

## Static / SPA Serving (not under /api/v1)

- `GET /` → `dist/index.html` (PUBLIC, always).
- `GET /assets/**` → hashed static assets (PUBLIC, cache-friendly headers).
- `GET /<anything-else-without-/api/v1>` → `dist/index.html` (SPA fallback).

These are served by the same `app` binary; they do not appear in the endpoint inventory totals.
