# Test Coverage Audit

## Scope and Method
- Static inspection only (no runtime execution).
- Evidence sources: backend route declarations, API spec inventory, backend/ frontend test files, `run_tests.sh`, and `README.md`.

## Project Type Detection
- Declared in README: **fullstack** (`README.md:5`).

## Backend Endpoint Inventory
- Source: `docs/api-spec.md` inventory rows.
- Extracted total: **117 endpoints** (`METHOD + full PATH`).

| ID | Endpoint |
|---|---|
| S1 | `GET /api/v1/health` |
| S2 | `GET /api/v1/ready` |
| A1 | `POST /api/v1/auth/login` |
| A2 | `POST /api/v1/auth/refresh` |
| A3 | `POST /api/v1/auth/logout` |
| A4 | `GET /api/v1/auth/me` |
| A5 | `POST /api/v1/auth/change-password` |
| U1 | `GET /api/v1/users` |
| U2 | `POST /api/v1/users` |
| U3 | `GET /api/v1/users/{id}` |
| U4 | `PATCH /api/v1/users/{id}` |
| U5 | `DELETE /api/v1/users/{id}` |
| U6 | `POST /api/v1/users/{id}/roles` |
| U7 | `POST /api/v1/users/{id}/unlock` |
| U8 | `POST /api/v1/users/{id}/reset-password` |
| U9 | `GET /api/v1/roles` |
| U10 | `GET /api/v1/audit` |
| SEC1 | `GET /api/v1/security/allowlist` |
| SEC2 | `POST /api/v1/security/allowlist` |
| SEC3 | `DELETE /api/v1/security/allowlist/{id}` |
| SEC4 | `GET /api/v1/security/device-certs` |
| SEC5 | `POST /api/v1/security/device-certs` |
| SEC6 | `DELETE /api/v1/security/device-certs/{id}` |
| SEC7 | `GET /api/v1/security/mtls` |
| SEC8 | `PATCH /api/v1/security/mtls` |
| SEC9 | `GET /api/v1/security/mtls/status` |
| R1 | `GET /api/v1/retention` |
| R2 | `PATCH /api/v1/retention/{domain}` |
| R3 | `POST /api/v1/retention/{domain}/run` |
| M1 | `GET /api/v1/monitoring/latency` |
| M2 | `GET /api/v1/monitoring/errors` |
| M3 | `GET /api/v1/monitoring/crash-reports` |
| M4 | `POST /api/v1/monitoring/crash-report` |
| REF1 | `GET /api/v1/ref/sites` |
| REF2 | `GET /api/v1/ref/departments` |
| REF3 | `GET /api/v1/ref/categories` |
| REF4 | `GET /api/v1/ref/brands` |
| REF5 | `GET /api/v1/ref/units` |
| REF6 | `GET /api/v1/ref/states` |
| REF7 | `POST /api/v1/ref/categories` |
| REF8 | `POST /api/v1/ref/brands` |
| REF9 | `POST /api/v1/ref/units` |
| P1 | `GET /api/v1/products` |
| P2 | `POST /api/v1/products` |
| P3 | `GET /api/v1/products/{id}` |
| P4 | `PATCH /api/v1/products/{id}` |
| P5 | `DELETE /api/v1/products/{id}` |
| P6 | `POST /api/v1/products/{id}/status` |
| P7 | `GET /api/v1/products/{id}/history` |
| P8 | `POST /api/v1/products/{id}/tax-rates` |
| P9 | `PATCH /api/v1/products/{id}/tax-rates/{rid}` |
| P10 | `DELETE /api/v1/products/{id}/tax-rates/{rid}` |
| P11 | `POST /api/v1/products/{id}/images` |
| P12 | `DELETE /api/v1/products/{id}/images/{imgid}` |
| P13 | `GET /api/v1/images/{imgid}` |
| P14 | `POST /api/v1/products/export` |
| I1 | `POST /api/v1/imports` |
| I2 | `GET /api/v1/imports` |
| I3 | `GET /api/v1/imports/{id}` |
| I4 | `GET /api/v1/imports/{id}/rows` |
| I5 | `POST /api/v1/imports/{id}/validate` |
| I6 | `POST /api/v1/imports/{id}/commit` |
| I7 | `POST /api/v1/imports/{id}/cancel` |
| E1 | `GET /api/v1/env/sources` |
| E2 | `POST /api/v1/env/sources` |
| E3 | `PATCH /api/v1/env/sources/{id}` |
| E4 | `DELETE /api/v1/env/sources/{id}` |
| E5 | `POST /api/v1/env/sources/{id}/observations` |
| E6 | `GET /api/v1/env/observations` |
| MD1 | `GET /api/v1/metrics/definitions` |
| MD2 | `POST /api/v1/metrics/definitions` |
| MD3 | `GET /api/v1/metrics/definitions/{id}` |
| MD4 | `PATCH /api/v1/metrics/definitions/{id}` |
| MD5 | `DELETE /api/v1/metrics/definitions/{id}` |
| MD6 | `GET /api/v1/metrics/definitions/{id}/series` |
| MD7 | `GET /api/v1/metrics/computations/{id}/lineage` |
| K1 | `GET /api/v1/kpi/summary` |
| K2 | `GET /api/v1/kpi/cycle-time` |
| K3 | `GET /api/v1/kpi/funnel` |
| K4 | `GET /api/v1/kpi/anomalies` |
| K5 | `GET /api/v1/kpi/efficiency` |
| K6 | `GET /api/v1/kpi/drill` |
| AL1 | `GET /api/v1/alerts/rules` |
| AL2 | `POST /api/v1/alerts/rules` |
| AL3 | `PATCH /api/v1/alerts/rules/{id}` |
| AL4 | `DELETE /api/v1/alerts/rules/{id}` |
| AL5 | `GET /api/v1/alerts/events` |
| AL6 | `POST /api/v1/alerts/events/{id}/ack` |
| RP1 | `GET /api/v1/reports/jobs` |
| RP2 | `POST /api/v1/reports/jobs` |
| RP3 | `GET /api/v1/reports/jobs/{id}` |
| RP4 | `POST /api/v1/reports/jobs/{id}/run-now` |
| RP5 | `POST /api/v1/reports/jobs/{id}/cancel` |
| RP6 | `GET /api/v1/reports/jobs/{id}/artifact` |
| T1 | `GET /api/v1/talent/candidates` |
| T2 | `POST /api/v1/talent/candidates` |
| T3 | `GET /api/v1/talent/candidates/{id}` |
| T4 | `GET /api/v1/talent/roles` |
| T5 | `POST /api/v1/talent/roles` |
| T6 | `GET /api/v1/talent/recommendations` |
| T7 | `GET /api/v1/talent/weights` |
| T8 | `PUT /api/v1/talent/weights` |
| T9 | `POST /api/v1/talent/feedback` |
| T10 | `GET /api/v1/talent/watchlists` |
| T11 | `POST /api/v1/talent/watchlists` |
| T12 | `POST /api/v1/talent/watchlists/{id}/items` |
| T13 | `DELETE /api/v1/talent/watchlists/{id}/items/{cid}` |
| T14 | `GET /api/v1/talent/watchlists/{id}/items` |
| N1 | `GET /api/v1/notifications` |
| N2 | `POST /api/v1/notifications/{id}/read` |
| N3 | `POST /api/v1/notifications/read-all` |
| N4 | `GET /api/v1/notifications/unread-count` |
| N5 | `GET /api/v1/notifications/subscriptions` |
| N6 | `PUT /api/v1/notifications/subscriptions` |
| N7 | `GET /api/v1/notifications/mailbox-exports` |
| N8 | `POST /api/v1/notifications/mailbox-export` |
| N9 | `GET /api/v1/notifications/mailbox-exports/{id}` |

## API Test Mapping Table
- Matching approach:
  - exact method + normalized path-template matching (`{param}` templates matched against concrete URI segments)
  - query strings stripped before matching
  - helper-driven parity calls resolved to concrete method/path

| ID | Endpoint | Covered | Test Type | Test Files | Evidence (Function + Request Path) |
|---|---|---|---|---|---|
| S1 | `GET /api/v1/health` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs, crates/backend/tests/integration_tests.rs` | `crates/backend/tests/http_p1.rs:36#t_s1_health_ok_unauthenticated() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/health; crates/backend/tests/integration_tests.rs GET /api/v1/health` |
| S2 | `GET /api/v1/ready` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:47#t_s2_ready_reports_db_up() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/ready` |
| A1 | `POST /api/v1/auth/login` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs, crates/backend/tests/integration_tests.rs` | `crates/backend/tests/http_p1.rs:63#t_a1_login_returns_access_token_and_refresh_cookie() {;crates/backend/tests/http_p1.rs:98#t_a1_login_rejects_bad_password() {;crates/backend/tests/http_p1.rs:123#t_a1_login_rejects_email_as_username() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs POST /api/v1/auth/login; crates/backend/tests/http_p1.rs POST /api/v1/auth/login; crates/backend/tests/integration_tests.rs POST /api/v1/auth/login` |
| A2 | `POST /api/v1/auth/refresh` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:143#t_a2_refresh_rotates_cookie_and_issues_new_access_token() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs POST /api/v1/auth/refresh; crates/backend/tests/http_p1.rs POST /api/v1/auth/refresh` |
| A3 | `POST /api/v1/auth/logout` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:186#t_a3_logout_revokes_session() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs POST /api/v1/auth/logout; crates/backend/tests/http_p1.rs POST /api/v1/auth/logout` |
| A4 | `GET /api/v1/auth/me` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:229#t_a4_me_returns_identity_for_bearer_holder() {;crates/backend/tests/http_p1.rs:267#t_a4_me_rejects_missing_bearer() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/auth/me` |
| A5 | `POST /api/v1/auth/change-password` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:276#t_a5_change_password_revokes_old_sessions() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs POST /api/v1/auth/change-password; crates/backend/tests/http_p1.rs POST /api/v1/auth/change-password` |
| U1 | `GET /api/v1/users` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:310#t_u1_list_users_requires_user_manage() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/users` |
| U2 | `POST /api/v1/users` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:340#t_u2_create_user_requires_user_manage_and_returns_id() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs POST /api/v1/users; crates/backend/tests/http_p1.rs POST /api/v1/users` |
| U3 | `GET /api/v1/users/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:368#t_u3_get_user_self_and_admin() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/users/{other}; crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/users/{}; crates/backend/tests/http_p1.rs GET /api/v1/users/{}` |
| U4 | `PATCH /api/v1/users/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:406#t_u4_update_user_self_display_name() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs PATCH /api/v1/users/{victim}; crates/backend/tests/http_p1.rs PATCH /api/v1/users/{}` |
| U5 | `DELETE /api/v1/users/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:433#t_u5_delete_user_soft_deactivates() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs DELETE /api/v1/users/{admin_id}; crates/backend/tests/deep_admin_surface_tests.rs DELETE /api/v1/users/{}; crates/backend/tests/http_p1.rs DELETE /api/v1/users/{}` |
| U6 | `POST /api/v1/users/{id}/roles` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:467#t_u6_unlock_user_clears_lock() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs POST /api/v1/users/{victim}/roles; crates/backend/tests/http_p1.rs POST /api/v1/users/{}/roles` |
| U7 | `POST /api/v1/users/{id}/unlock` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:504#t_u7_assign_roles_replaces_set() {`<br/>`crates/backend/tests/http_p1.rs POST /api/v1/users/{}/unlock` |
| U8 | `POST /api/v1/users/{id}/reset-password` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:544#t_u8_list_roles_requires_role_assign() {`<br/>`crates/backend/tests/http_p1.rs POST /api/v1/users/{}/reset-password` |
| U9 | `GET /api/v1/roles` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:565#t_u9_reset_password_requires_user_manage() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/roles` |
| U10 | `GET /api/v1/audit` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:599#t_u10_audit_list_requires_monitoring_read() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/audit; crates/backend/tests/http_p1.rs GET /api/v1/audit` |
| SEC1 | `GET /api/v1/security/allowlist` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:638#t_sec1_list_allowlist_empty_by_default() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/security/allowlist; crates/backend/tests/http_p1.rs GET /api/v1/security/allowlist` |
| SEC2 | `POST /api/v1/security/allowlist` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:659#t_sec2_create_allowlist_validates_cidr() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs POST /api/v1/security/allowlist; crates/backend/tests/http_p1.rs POST /api/v1/security/allowlist` |
| SEC3 | `DELETE /api/v1/security/allowlist/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:693#t_sec3_delete_allowlist_entry() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs DELETE /api/v1/security/allowlist/{}; crates/backend/tests/http_p1.rs DELETE /api/v1/security/allowlist/{}` |
| SEC4 | `GET /api/v1/security/device-certs` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:725#t_sec4_list_device_certs_admin_only() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/security/device-certs; crates/backend/tests/http_p1.rs GET /api/v1/security/device-certs` |
| SEC5 | `POST /api/v1/security/device-certs` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:758#t_sec5_register_device_cert_validates_pin() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs POST /api/v1/security/device-certs; crates/backend/tests/http_p1.rs POST /api/v1/security/device-certs` |
| SEC6 | `DELETE /api/v1/security/device-certs/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:802#t_sec6_revoke_device_cert() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs DELETE /api/v1/security/device-certs/{}; crates/backend/tests/deep_admin_surface_tests.rs DELETE /api/v1/security/device-certs/{cert_id}; crates/backend/tests/http_p1.rs DELETE /api/v1/security/device-certs/{}` |
| SEC7 | `GET /api/v1/security/mtls` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:831#t_sec7_get_mtls_returns_default_not_enforced() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/security/mtls; crates/backend/tests/http_p1.rs GET /api/v1/security/mtls` |
| SEC8 | `PATCH /api/v1/security/mtls` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:852#t_sec8_patch_mtls_requires_mtls_manage() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs PATCH /api/v1/security/mtls; crates/backend/tests/http_p1.rs PATCH /api/v1/security/mtls` |
| SEC9 | `GET /api/v1/security/mtls/status` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:894#t_sec9_mtls_status_returns_cert_counts() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/security/mtls/status; crates/backend/tests/http_p1.rs GET /api/v1/security/mtls/status` |
| R1 | `GET /api/v1/retention` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:926#t_r1_list_retention_policies_seeded() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/retention` |
| R2 | `PATCH /api/v1/retention/{domain}` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:950#t_r2_patch_retention_updates_ttl() {`<br/>`crates/backend/tests/http_p1.rs PATCH /api/v1/retention/env_raw; crates/backend/tests/http_p1.rs PATCH /api/v1/retention/does_not_exist` |
| R3 | `POST /api/v1/retention/{domain}/run` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs, crates/backend/tests/integration_tests.rs` | `crates/backend/tests/http_p1.rs:992#t_r3_run_retention_reports_enforced_at() {`<br/>`crates/backend/tests/http_p1.rs POST /api/v1/retention/audit/run; crates/backend/tests/integration_tests.rs POST /api/v1/retention/env_raw/run; crates/backend/tests/integration_tests.rs POST /api/v1/retention/kpi/run; crates/backend/tests/integration_tests.rs POST /api/v1/retention/feedback/run; crates/backend/tests/integration_tests.rs POST /api/v1/retention/audit/run` |
| M1 | `GET /api/v1/monitoring/latency` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1018#t_m1_latency_requires_monitoring_read() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/monitoring/latency` |
| M2 | `GET /api/v1/monitoring/errors` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1037#t_m2_errors_requires_monitoring_read() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/monitoring/errors` |
| M3 | `GET /api/v1/monitoring/crash-reports` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1065#t_m3_crash_report_any_authed_user() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/monitoring/crash-reports; crates/backend/tests/http_p1.rs GET /api/v1/monitoring/crash-reports` |
| M4 | `POST /api/v1/monitoring/crash-report` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1086#t_m4_crash_reports_list_admin_only() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs POST /api/v1/monitoring/crash-report; crates/backend/tests/http_p1.rs POST /api/v1/monitoring/crash-report` |
| REF1 | `GET /api/v1/ref/sites` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1132#t_ref1_sites_open_to_any_authed() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/ref/sites; crates/backend/tests/http_p1.rs GET /api/v1/ref/sites` |
| REF2 | `GET /api/v1/ref/departments` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1144#t_ref2_departments_filter_by_site() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/ref/departments; crates/backend/tests/http_p1.rs GET /api/v1/ref/departments` |
| REF3 | `GET /api/v1/ref/categories` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1156#t_ref3_categories_list() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/ref/categories; crates/backend/tests/http_p1.rs GET /api/v1/ref/categories` |
| REF4 | `GET /api/v1/ref/brands` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1168#t_ref4_create_category_requires_ref_write() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/ref/brands; crates/backend/tests/http_p1.rs GET /api/v1/ref/brands` |
| REF5 | `GET /api/v1/ref/units` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1194#t_ref5_brands_list() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/ref/units; crates/backend/tests/http_p1.rs GET /api/v1/ref/units` |
| REF6 | `GET /api/v1/ref/states` | yes | true no-mock HTTP | `crates/backend/tests/deep_admin_surface_tests.rs, crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1206#t_ref6_create_brand_requires_ref_write() {`<br/>`crates/backend/tests/deep_admin_surface_tests.rs GET /api/v1/ref/states; crates/backend/tests/http_p1.rs GET /api/v1/ref/states` |
| REF7 | `POST /api/v1/ref/categories` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1222#t_ref7_units_list() {`<br/>`crates/backend/tests/http_p1.rs POST /api/v1/ref/categories` |
| REF8 | `POST /api/v1/ref/brands` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1234#t_ref8_create_unit_requires_ref_write() {`<br/>`crates/backend/tests/http_p1.rs POST /api/v1/ref/brands` |
| REF9 | `POST /api/v1/ref/units` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1250#t_ref9_states_returns_seeded_list() {`<br/>`crates/backend/tests/http_p1.rs POST /api/v1/ref/units` |
| P1 | `GET /api/v1/products` | yes | true no-mock HTTP | `crates/backend/tests/budget_tests.rs, crates/backend/tests/deep_products_tests.rs, crates/backend/tests/integration_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:64#t_p1_list_products_requires_auth() {`<br/>`crates/backend/tests/budget_tests.rs GET /api/v1/products; crates/backend/tests/deep_products_tests.rs GET /api/v1/products; crates/backend/tests/integration_tests.rs GET /api/v1/products; crates/backend/tests/parity_success_tests.rs GET /api/v1/products; crates/backend/tests/parity_tests.rs GET /api/v1/products` |
| P2 | `POST /api/v1/products` | yes | true no-mock HTTP | `crates/backend/tests/audit9_bundle_tests.rs, crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:86#t_p2_create_product_requires_write_perm() {`<br/>`crates/backend/tests/audit9_bundle_tests.rs POST /api/v1/products; crates/backend/tests/deep_products_tests.rs POST /api/v1/products; crates/backend/tests/parity_success_tests.rs POST /api/v1/products; crates/backend/tests/parity_tests.rs POST /api/v1/products` |
| P3 | `GET /api/v1/products/{id}` | yes | true no-mock HTTP | `crates/backend/tests/audit9_bundle_tests.rs, crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:98#t_p3_get_product_requires_auth() {`<br/>`crates/backend/tests/audit9_bundle_tests.rs GET /api/v1/products/{pid}; crates/backend/tests/deep_products_tests.rs GET /api/v1/products/{pid}; crates/backend/tests/deep_products_tests.rs GET /api/v1/products/{}; crates/backend/tests/parity_success_tests.rs GET /api/v1/products/{pid}; crates/backend/tests/parity_tests.rs GET /api/v1/products/00000000-0000-0000-0000-000000000001` |
| P4 | `PATCH /api/v1/products/{id}` | yes | true no-mock HTTP | `crates/backend/tests/audit9_bundle_tests.rs, crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:104#t_p4_patch_product_requires_write_perm() {`<br/>`crates/backend/tests/audit9_bundle_tests.rs PATCH /api/v1/products/{pid}; crates/backend/tests/deep_products_tests.rs PATCH /api/v1/products/{pid}; crates/backend/tests/deep_products_tests.rs PATCH /api/v1/products/{}; crates/backend/tests/parity_tests.rs PATCH /api/v1/products/00000000-0000-0000-0000-000000000001` |
| P5 | `DELETE /api/v1/products/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:116#t_p5_delete_product_requires_write_perm() {`<br/>`crates/backend/tests/deep_products_tests.rs DELETE /api/v1/products/{pid}; crates/backend/tests/parity_tests.rs DELETE /api/v1/products/00000000-0000-0000-0000-000000000001` |
| P6 | `POST /api/v1/products/{id}/status` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:128#t_p6_toggle_status_requires_write_perm() {`<br/>`crates/backend/tests/deep_products_tests.rs POST /api/v1/products/{pid}/status; crates/backend/tests/deep_products_tests.rs POST /api/v1/products/{}/status; crates/backend/tests/parity_tests.rs POST /api/v1/products/00000000-0000-0000-0000-000000000001/status` |
| P7 | `GET /api/v1/products/{id}/history` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:141#t_p7_history_requires_history_read_perm() {`<br/>`crates/backend/tests/deep_products_tests.rs GET /api/v1/products/{pid}/history; crates/backend/tests/deep_products_tests.rs GET /api/v1/products/{}/history; crates/backend/tests/parity_tests.rs GET /api/v1/products/00000000-0000-0000-0000-000000000001/history` |
| P8 | `POST /api/v1/products/{id}/tax-rates` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:153#t_p8_add_tax_rate_requires_write_perm() {`<br/>`crates/backend/tests/deep_products_tests.rs POST /api/v1/products/{pid}/tax-rates; crates/backend/tests/deep_products_tests.rs POST /api/v1/products/{}/tax-rates; crates/backend/tests/parity_tests.rs POST /api/v1/products/00000000-0000-0000-0000-000000000001/tax-rates` |
| P9 | `PATCH /api/v1/products/{id}/tax-rates/{rid}` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:165#t_p9_update_tax_rate_requires_write_perm() {`<br/>`crates/backend/tests/deep_products_tests.rs PATCH /api/v1/products/{pid}/tax-rates/{rid}; crates/backend/tests/deep_products_tests.rs PATCH /api/v1/products/{pid}/tax-rates/{}; crates/backend/tests/parity_tests.rs PATCH /api/v1/products/00000000-0000-0000-0000-000000000001/tax-rates/00000000-0000-0000-0000-000000000002` |
| P10 | `DELETE /api/v1/products/{id}/tax-rates/{rid}` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:177#t_p10_delete_tax_rate_requires_write_perm() {`<br/>`crates/backend/tests/deep_products_tests.rs DELETE /api/v1/products/{pid}/tax-rates/{rid}; crates/backend/tests/deep_products_tests.rs DELETE /api/v1/products/{}/tax-rates/{rid}; crates/backend/tests/parity_tests.rs DELETE /api/v1/products/00000000-0000-0000-0000-000000000001/tax-rates/00000000-0000-0000-0000-000000000002` |
| P11 | `POST /api/v1/products/{id}/images` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs` | `crates/backend/tests/parity_tests.rs:189#t_p11_upload_image_requires_auth() {`<br/>`crates/backend/tests/deep_products_tests.rs POST /api/v1/products/{pid}/images; crates/backend/tests/deep_products_tests.rs POST /api/v1/products/{}/images` |
| P12 | `DELETE /api/v1/products/{id}/images/{imgid}` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:199#t_p12_delete_image_requires_write_perm() {`<br/>`crates/backend/tests/deep_products_tests.rs DELETE /api/v1/products/{pid}/images/{img_id}; crates/backend/tests/deep_products_tests.rs DELETE /api/v1/products/{}/images/{}; crates/backend/tests/parity_tests.rs DELETE /api/v1/products/00000000-0000-0000-0000-000000000001/images/00000000-0000-0000-0000-000000000002` |
| P13 | `GET /api/v1/images/{imgid}` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:218#t_p13_serve_image_is_signed_capability_only() {`<br/>`crates/backend/tests/deep_products_tests.rs GET /api/v1/images/{img_id}; crates/backend/tests/parity_tests.rs GET /api/v1/images/00000000-0000-0000-0000-000000000001` |
| P14 | `POST /api/v1/products/export` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:229#t_p14_export_products_requires_auth() {`<br/>`crates/backend/tests/deep_products_tests.rs POST /api/v1/products/export; crates/backend/tests/parity_tests.rs POST /api/v1/products/export` |
| I1 | `POST /api/v1/imports` | yes | true no-mock HTTP | `crates/backend/tests/budget_tests.rs, crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:237#t_i1_upload_import_requires_import_perm() {`<br/>`crates/backend/tests/budget_tests.rs POST /api/v1/imports; crates/backend/tests/deep_products_tests.rs POST /api/v1/imports; crates/backend/tests/parity_tests.rs POST /api/v1/imports` |
| I2 | `GET /api/v1/imports` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:243#t_i2_list_imports_requires_import_perm() {`<br/>`crates/backend/tests/deep_products_tests.rs GET /api/v1/imports; crates/backend/tests/parity_success_tests.rs GET /api/v1/imports; crates/backend/tests/parity_tests.rs GET /api/v1/imports` |
| I3 | `GET /api/v1/imports/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:249#t_i3_get_import_requires_auth() {`<br/>`crates/backend/tests/deep_products_tests.rs GET /api/v1/imports/{bid}; crates/backend/tests/deep_products_tests.rs GET /api/v1/imports/{}; crates/backend/tests/parity_tests.rs GET /api/v1/imports/00000000-0000-0000-0000-000000000001` |
| I4 | `GET /api/v1/imports/{id}/rows` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:255#t_i4_list_import_rows_requires_auth() {`<br/>`crates/backend/tests/deep_products_tests.rs GET /api/v1/imports/{bid}/rows; crates/backend/tests/parity_tests.rs GET /api/v1/imports/00000000-0000-0000-0000-000000000001/rows` |
| I5 | `POST /api/v1/imports/{id}/validate` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:261#t_i5_validate_import_requires_import_perm() {`<br/>`crates/backend/tests/deep_products_tests.rs POST /api/v1/imports/{bid}/validate; crates/backend/tests/parity_tests.rs POST /api/v1/imports/00000000-0000-0000-0000-000000000001/validate` |
| I6 | `POST /api/v1/imports/{id}/commit` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:273#t_i6_commit_import_requires_import_perm() {`<br/>`crates/backend/tests/deep_products_tests.rs POST /api/v1/imports/{bid}/commit; crates/backend/tests/parity_tests.rs POST /api/v1/imports/00000000-0000-0000-0000-000000000001/commit` |
| I7 | `POST /api/v1/imports/{id}/cancel` | yes | true no-mock HTTP | `crates/backend/tests/deep_products_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:285#t_i7_cancel_import_requires_import_perm() {`<br/>`crates/backend/tests/deep_products_tests.rs POST /api/v1/imports/{bid}/cancel; crates/backend/tests/parity_tests.rs POST /api/v1/imports/00000000-0000-0000-0000-000000000001/cancel` |
| E1 | `GET /api/v1/env/sources` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/integration_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:299#t_e1_list_env_sources_requires_metric_read() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs GET /api/v1/env/sources; crates/backend/tests/integration_tests.rs GET /api/v1/env/sources; crates/backend/tests/parity_success_tests.rs GET /api/v1/env/sources; crates/backend/tests/parity_tests.rs GET /api/v1/env/sources` |
| E2 | `POST /api/v1/env/sources` | yes | true no-mock HTTP | `crates/backend/tests/audit9_bundle_tests.rs, crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:304#t_e2_create_env_source_requires_metric_configure() {`<br/>`crates/backend/tests/audit9_bundle_tests.rs POST /api/v1/env/sources; crates/backend/tests/deep_metrics_kpi_tests.rs POST /api/v1/env/sources; crates/backend/tests/parity_success_tests.rs POST /api/v1/env/sources; crates/backend/tests/parity_tests.rs POST /api/v1/env/sources` |
| E3 | `PATCH /api/v1/env/sources/{id}` | yes | true no-mock HTTP | `crates/backend/tests/audit9_bundle_tests.rs, crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:316#t_e3_update_env_source_requires_metric_configure() {`<br/>`crates/backend/tests/audit9_bundle_tests.rs PATCH /api/v1/env/sources/{sid}; crates/backend/tests/deep_metrics_kpi_tests.rs PATCH /api/v1/env/sources/{sid}; crates/backend/tests/deep_metrics_kpi_tests.rs PATCH /api/v1/env/sources/{}; crates/backend/tests/parity_tests.rs PATCH /api/v1/env/sources/00000000-0000-0000-0000-000000000001` |
| E4 | `DELETE /api/v1/env/sources/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:328#t_e4_delete_env_source_requires_metric_configure() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs DELETE /api/v1/env/sources/{sid}; crates/backend/tests/parity_tests.rs DELETE /api/v1/env/sources/00000000-0000-0000-0000-000000000001` |
| E5 | `POST /api/v1/env/sources/{id}/observations` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:340#t_e5_bulk_observations_requires_metric_configure() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs POST /api/v1/env/sources/{sid}/observations; crates/backend/tests/deep_metrics_kpi_tests.rs POST /api/v1/env/sources/{}/observations; crates/backend/tests/parity_tests.rs POST /api/v1/env/sources/00000000-0000-0000-0000-000000000001/observations` |
| E6 | `GET /api/v1/env/observations` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:352#t_e6_list_observations_requires_auth() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs GET /api/v1/env/observations; crates/backend/tests/parity_success_tests.rs GET /api/v1/env/observations; crates/backend/tests/parity_tests.rs GET /api/v1/env/observations` |
| MD1 | `GET /api/v1/metrics/definitions` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:362#t_md1_list_definitions_requires_auth() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs GET /api/v1/metrics/definitions; crates/backend/tests/parity_success_tests.rs GET /api/v1/metrics/definitions; crates/backend/tests/parity_tests.rs GET /api/v1/metrics/definitions` |
| MD2 | `POST /api/v1/metrics/definitions` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:370#t_md2_create_definition_requires_configure() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs POST /api/v1/metrics/definitions; crates/backend/tests/parity_success_tests.rs POST /api/v1/metrics/definitions; crates/backend/tests/parity_tests.rs POST /api/v1/metrics/definitions` |
| MD3 | `GET /api/v1/metrics/definitions/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:382#t_md3_get_definition_requires_auth() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs GET /api/v1/metrics/definitions/{id}; crates/backend/tests/deep_metrics_kpi_tests.rs GET /api/v1/metrics/definitions/{}; crates/backend/tests/parity_success_tests.rs GET /api/v1/metrics/definitions/{did}; crates/backend/tests/parity_tests.rs GET /api/v1/metrics/definitions/00000000-0000-0000-0000-000000000001` |
| MD4 | `PATCH /api/v1/metrics/definitions/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:394#t_md4_update_definition_requires_configure() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs PATCH /api/v1/metrics/definitions/{ma_id}; crates/backend/tests/parity_tests.rs PATCH /api/v1/metrics/definitions/00000000-0000-0000-0000-000000000001` |
| MD5 | `DELETE /api/v1/metrics/definitions/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:406#t_md5_delete_definition_requires_configure() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs DELETE /api/v1/metrics/definitions/{roc_id}; crates/backend/tests/parity_tests.rs DELETE /api/v1/metrics/definitions/00000000-0000-0000-0000-000000000001` |
| MD6 | `GET /api/v1/metrics/definitions/{id}/series` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:418#t_md6_series_requires_auth() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs GET /api/v1/metrics/definitions/{id}/series; crates/backend/tests/parity_tests.rs GET /api/v1/metrics/definitions/00000000-0000-0000-0000-000000000001/series` |
| MD7 | `GET /api/v1/metrics/computations/{id}/lineage` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:430#t_md7_lineage_requires_auth() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs GET /api/v1/metrics/computations/{comp_id}/lineage; crates/backend/tests/deep_metrics_kpi_tests.rs GET /api/v1/metrics/computations/{}/lineage; crates/backend/tests/parity_tests.rs GET /api/v1/metrics/computations/00000000-0000-0000-0000-000000000001/lineage` |
| K1 | `GET /api/v1/kpi/summary` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:444#t_k1_summary_requires_auth() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs GET /api/v1/kpi/summary; crates/backend/tests/parity_success_tests.rs GET /api/v1/kpi/summary; crates/backend/tests/parity_tests.rs GET /api/v1/kpi/summary` |
| K2 | `GET /api/v1/kpi/cycle-time` | yes | true no-mock HTTP | `crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:449#t_k2_cycle_time_requires_auth() {`<br/>`crates/backend/tests/parity_success_tests.rs GET /api/v1/kpi/cycle-time; crates/backend/tests/parity_tests.rs GET /api/v1/kpi/cycle-time` |
| K3 | `GET /api/v1/kpi/funnel` | yes | true no-mock HTTP | `crates/backend/tests/deep_metrics_kpi_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:454#t_k3_funnel_requires_auth() {`<br/>`crates/backend/tests/deep_metrics_kpi_tests.rs GET /api/v1/kpi/funnel; crates/backend/tests/parity_tests.rs GET /api/v1/kpi/funnel` |
| K4 | `GET /api/v1/kpi/anomalies` | yes | true no-mock HTTP | `crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:459#t_k4_anomalies_requires_auth() {`<br/>`crates/backend/tests/parity_success_tests.rs GET /api/v1/kpi/anomalies; crates/backend/tests/parity_tests.rs GET /api/v1/kpi/anomalies` |
| K5 | `GET /api/v1/kpi/efficiency` | yes | true no-mock HTTP | `crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:464#t_k5_efficiency_requires_auth() {`<br/>`crates/backend/tests/parity_tests.rs GET /api/v1/kpi/efficiency` |
| K6 | `GET /api/v1/kpi/drill` | yes | true no-mock HTTP | `crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:469#t_k6_drill_requires_auth() {`<br/>`crates/backend/tests/parity_tests.rs GET /api/v1/kpi/drill` |
| AL1 | `GET /api/v1/alerts/rules` | yes | true no-mock HTTP | `crates/backend/tests/csrf_tests.rs, crates/backend/tests/deep_alerts_reports_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:476#t_al1_list_rules_requires_alert_manage() {`<br/>`crates/backend/tests/csrf_tests.rs GET /api/v1/alerts/rules; crates/backend/tests/deep_alerts_reports_tests.rs GET /api/v1/alerts/rules; crates/backend/tests/parity_success_tests.rs GET /api/v1/alerts/rules; crates/backend/tests/parity_tests.rs GET /api/v1/alerts/rules` |
| AL2 | `POST /api/v1/alerts/rules` | yes | true no-mock HTTP | `crates/backend/tests/csrf_tests.rs, crates/backend/tests/deep_alerts_reports_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:482#t_al2_create_rule_requires_alert_manage() {`<br/>`crates/backend/tests/csrf_tests.rs POST /api/v1/alerts/rules; crates/backend/tests/deep_alerts_reports_tests.rs POST /api/v1/alerts/rules; crates/backend/tests/parity_success_tests.rs POST /api/v1/alerts/rules; crates/backend/tests/parity_tests.rs POST /api/v1/alerts/rules` |
| AL3 | `PATCH /api/v1/alerts/rules/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_alerts_reports_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:498#t_al3_update_rule_requires_alert_manage() {`<br/>`crates/backend/tests/deep_alerts_reports_tests.rs PATCH /api/v1/alerts/rules/{rule_id}; crates/backend/tests/deep_alerts_reports_tests.rs PATCH /api/v1/alerts/rules/{}; crates/backend/tests/parity_tests.rs PATCH /api/v1/alerts/rules/00000000-0000-0000-0000-000000000001` |
| AL4 | `DELETE /api/v1/alerts/rules/{id}` | yes | true no-mock HTTP | `crates/backend/tests/deep_alerts_reports_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:510#t_al4_delete_rule_requires_alert_manage() {`<br/>`crates/backend/tests/deep_alerts_reports_tests.rs DELETE /api/v1/alerts/rules/{rule_id}; crates/backend/tests/parity_tests.rs DELETE /api/v1/alerts/rules/00000000-0000-0000-0000-000000000001` |
| AL5 | `GET /api/v1/alerts/events` | yes | true no-mock HTTP | `crates/backend/tests/deep_alerts_reports_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:522#t_al5_list_events_requires_auth() {`<br/>`crates/backend/tests/deep_alerts_reports_tests.rs GET /api/v1/alerts/events; crates/backend/tests/parity_success_tests.rs GET /api/v1/alerts/events; crates/backend/tests/parity_tests.rs GET /api/v1/alerts/events` |
| AL6 | `POST /api/v1/alerts/events/{id}/ack` | yes | true no-mock HTTP | `crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:527#t_al6_ack_event_requires_auth() {`<br/>`crates/backend/tests/parity_tests.rs POST /api/v1/alerts/events/00000000-0000-0000-0000-000000000001/ack` |
| RP1 | `GET /api/v1/reports/jobs` | yes | true no-mock HTTP | `crates/backend/tests/deep_alerts_reports_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:541#t_rp1_list_jobs_requires_auth() {`<br/>`crates/backend/tests/deep_alerts_reports_tests.rs GET /api/v1/reports/jobs; crates/backend/tests/parity_success_tests.rs GET /api/v1/reports/jobs; crates/backend/tests/parity_tests.rs GET /api/v1/reports/jobs` |
| RP2 | `POST /api/v1/reports/jobs` | yes | true no-mock HTTP | `crates/backend/tests/deep_alerts_reports_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:546#t_rp2_create_job_requires_report_schedule() {`<br/>`crates/backend/tests/deep_alerts_reports_tests.rs POST /api/v1/reports/jobs; crates/backend/tests/parity_success_tests.rs POST /api/v1/reports/jobs; crates/backend/tests/parity_tests.rs POST /api/v1/reports/jobs` |
| RP3 | `GET /api/v1/reports/jobs/{id}` | yes | true no-mock HTTP | `crates/backend/tests/audit9_bundle_tests.rs, crates/backend/tests/deep_alerts_reports_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:558#t_rp3_get_job_requires_auth() {`<br/>`crates/backend/tests/audit9_bundle_tests.rs GET /api/v1/reports/jobs/{job_id}; crates/backend/tests/deep_alerts_reports_tests.rs GET /api/v1/reports/jobs/{j1}; crates/backend/tests/deep_alerts_reports_tests.rs GET /api/v1/reports/jobs/{}; crates/backend/tests/deep_alerts_reports_tests.rs GET /api/v1/reports/jobs/{jid}; crates/backend/tests/parity_success_tests.rs GET /api/v1/reports/jobs/{jid}; crates/backend/tests/parity_tests.rs GET /api/v1/reports/jobs/00000000-0000-0000-0000-000000000001` |
| RP4 | `POST /api/v1/reports/jobs/{id}/run-now` | yes | true no-mock HTTP | `crates/backend/tests/audit9_bundle_tests.rs, crates/backend/tests/deep_alerts_reports_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:566#t_rp4_run_now_requires_auth() {`<br/>`crates/backend/tests/audit9_bundle_tests.rs POST /api/v1/reports/jobs/{job_id}/run-now; crates/backend/tests/deep_alerts_reports_tests.rs POST /api/v1/reports/jobs/{j1}/run-now; crates/backend/tests/deep_alerts_reports_tests.rs POST /api/v1/reports/jobs/{jid}/run-now; crates/backend/tests/parity_tests.rs POST /api/v1/reports/jobs/00000000-0000-0000-0000-000000000001/run-now` |
| RP5 | `POST /api/v1/reports/jobs/{id}/cancel` | yes | true no-mock HTTP | `crates/backend/tests/audit9_bundle_tests.rs, crates/backend/tests/deep_alerts_reports_tests.rs, crates/backend/tests/parity_success_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:578#t_rp5_cancel_job_requires_auth() {`<br/>`crates/backend/tests/audit9_bundle_tests.rs POST /api/v1/reports/jobs/{job_id}/cancel; crates/backend/tests/deep_alerts_reports_tests.rs POST /api/v1/reports/jobs/{j1}/cancel; crates/backend/tests/deep_alerts_reports_tests.rs POST /api/v1/reports/jobs/{jid}/cancel; crates/backend/tests/parity_success_tests.rs POST /api/v1/reports/jobs/{jid}/cancel; crates/backend/tests/parity_tests.rs POST /api/v1/reports/jobs/00000000-0000-0000-0000-000000000001/cancel` |
| RP6 | `GET /api/v1/reports/jobs/{id}/artifact` | yes | true no-mock HTTP | `crates/backend/tests/audit9_bundle_tests.rs, crates/backend/tests/deep_alerts_reports_tests.rs, crates/backend/tests/parity_tests.rs` | `crates/backend/tests/parity_tests.rs:590#t_rp6_get_artifact_requires_auth() {`<br/>`crates/backend/tests/audit9_bundle_tests.rs GET /api/v1/reports/jobs/{job_id}/artifact; crates/backend/tests/deep_alerts_reports_tests.rs GET /api/v1/reports/jobs/{jid}/artifact; crates/backend/tests/parity_tests.rs GET /api/v1/reports/jobs/00000000-0000-0000-0000-000000000001/artifact` |
| T1 | `GET /api/v1/talent/candidates` | yes | true no-mock HTTP | `crates/backend/tests/talent_search_tests.rs` | `crates/backend/tests/talent_search_tests.rs:17#t_t1_list_candidates_requires_auth() {;crates/backend/tests/talent_search_tests.rs:28#t_t1_list_candidates_requires_talent_read() {;crates/backend/tests/talent_search_tests.rs:42#t_t1_list_candidates_returns_empty_initially() {;crates/backend/tests/talent_search_tests.rs:57#t_t1_list_candidates_with_search_q() {;crates/backend/tests/talent_search_tests.rs:83#t_t1_list_candidates_x_total_count_header() {`<br/>`crates/backend/tests/talent_search_tests.rs GET /api/v1/talent/candidates` |
| T2 | `POST /api/v1/talent/candidates` | yes | true no-mock HTTP | `crates/backend/tests/talent_search_tests.rs` | `crates/backend/tests/talent_search_tests.rs:114#t_t2_create_candidate_requires_talent_manage() {;crates/backend/tests/talent_search_tests.rs:133#t_t2_create_candidate_admin_ok() {;crates/backend/tests/talent_search_tests.rs:158#t_t2_create_candidate_invalid_completeness() {`<br/>`crates/backend/tests/talent_search_tests.rs POST /api/v1/talent/candidates` |
| T3 | `GET /api/v1/talent/candidates/{id}` | yes | true no-mock HTTP | `crates/backend/tests/talent_search_tests.rs` | `crates/backend/tests/talent_search_tests.rs:177#t_t3_get_candidate_not_found() {;crates/backend/tests/talent_search_tests.rs:190#t_t3_get_candidate_ok() {`<br/>`crates/backend/tests/talent_search_tests.rs GET /api/v1/talent/candidates/00000000-0000-0000-0000-000000000000; crates/backend/tests/talent_search_tests.rs GET /api/v1/talent/candidates/{id}` |
| T4 | `GET /api/v1/talent/roles` | yes | true no-mock HTTP | `crates/backend/tests/talent_recommend_tests.rs` | `crates/backend/tests/talent_recommend_tests.rs:17#t_t4_list_roles_requires_auth() {;crates/backend/tests/talent_recommend_tests.rs:28#t_t4_list_roles_returns_empty_initially() {`<br/>`crates/backend/tests/talent_recommend_tests.rs GET /api/v1/talent/roles` |
| T5 | `POST /api/v1/talent/roles` | yes | true no-mock HTTP | `crates/backend/tests/talent_recommend_tests.rs` | `crates/backend/tests/talent_recommend_tests.rs:45#t_t5_create_role_requires_talent_manage() {;crates/backend/tests/talent_recommend_tests.rs:64#t_t5_create_role_admin_ok() {`<br/>`crates/backend/tests/talent_recommend_tests.rs POST /api/v1/talent/roles` |
| T6 | `GET /api/v1/talent/recommendations` | yes | true no-mock HTTP | `crates/backend/tests/budget_tests.rs, crates/backend/tests/talent_recommend_tests.rs` | `crates/backend/tests/talent_recommend_tests.rs:88#t_t6_recommendations_requires_auth() {;crates/backend/tests/talent_recommend_tests.rs:99#t_t6_recommendations_role_not_found() {;crates/backend/tests/talent_recommend_tests.rs:112#t_t6_recommendations_cold_start() {;crates/backend/tests/talent_recommend_tests.rs:157#t_t6_recommendations_blended_after_10_feedback() {;crates/backend/tests/talent_recommend_tests.rs:260#t_t6_cold_start_is_isolated_per_owner() {;crates/backend/tests/talent_recommend_tests.rs:309#t_t6_cold_start_is_isolated_per_role() {;crates/backend/tests/talent_recommend_tests.rs:367#t_t6_cold_start_only_scoped_feedback_triggers_transition() {`<br/>`crates/backend/tests/budget_tests.rs GET /api/v1/talent/recommendations; crates/backend/tests/talent_recommend_tests.rs GET /api/v1/talent/recommendations` |
| T7 | `GET /api/v1/talent/weights` | yes | true no-mock HTTP | `crates/backend/tests/talent_weights_tests.rs` | `crates/backend/tests/talent_weights_tests.rs:17#t_t7_get_weights_requires_auth() {;crates/backend/tests/talent_weights_tests.rs:28#t_t7_get_weights_returns_defaults_when_none_stored() {;crates/backend/tests/talent_weights_tests.rs:46#t_t7_get_weights_returns_own_only() {;crates/backend/tests/talent_weights_tests.rs:176#t_t7_get_weights_forbidden_without_talent_read() {`<br/>`crates/backend/tests/talent_weights_tests.rs GET /api/v1/talent/weights` |
| T8 | `PUT /api/v1/talent/weights` | yes | true no-mock HTTP | `crates/backend/tests/csrf_tests.rs, crates/backend/tests/talent_weights_tests.rs` | `crates/backend/tests/talent_weights_tests.rs:77#t_t8_put_weights_requires_auth() {;crates/backend/tests/talent_weights_tests.rs:92#t_t8_put_weights_updates_own() {;crates/backend/tests/talent_weights_tests.rs:114#t_t8_put_weights_rejects_sum_not_100() {;crates/backend/tests/talent_weights_tests.rs:133#t_t8_put_weights_scoped_self_only() {;crates/backend/tests/talent_weights_tests.rs:190#t_t8_put_weights_forbidden_without_talent_read() {`<br/>`crates/backend/tests/csrf_tests.rs PUT /api/v1/talent/weights; crates/backend/tests/talent_weights_tests.rs PUT /api/v1/talent/weights` |
| T9 | `POST /api/v1/talent/feedback` | yes | true no-mock HTTP | `crates/backend/tests/talent_feedback_tests.rs` | `crates/backend/tests/talent_feedback_tests.rs:17#t_t9_feedback_requires_auth() {;crates/backend/tests/talent_feedback_tests.rs:32#t_t9_feedback_requires_talent_feedback_perm() {;crates/backend/tests/talent_feedback_tests.rs:57#t_t9_feedback_recruiter_can_submit() {;crates/backend/tests/talent_feedback_tests.rs:88#t_t9_feedback_invalid_thumb() {;crates/backend/tests/talent_feedback_tests.rs:112#t_t9_feedback_candidate_not_found() {;crates/backend/tests/talent_feedback_tests.rs:129#t_t9_feedback_owner_scoped_record() {`<br/>`crates/backend/tests/talent_feedback_tests.rs POST /api/v1/talent/feedback` |
| T10 | `GET /api/v1/talent/watchlists` | yes | true no-mock HTTP | `crates/backend/tests/talent_watchlist_tests.rs, crates/backend/tests/talent_weights_tests.rs` | `crates/backend/tests/talent_watchlist_tests.rs:17#t_t10_list_watchlists_requires_auth() {;crates/backend/tests/talent_watchlist_tests.rs:28#t_t10_list_watchlists_self_scoped() {;crates/backend/tests/talent_weights_tests.rs:208#t_t10_list_watchlists_forbidden_without_talent_read() {`<br/>`crates/backend/tests/talent_watchlist_tests.rs GET /api/v1/talent/watchlists; crates/backend/tests/talent_weights_tests.rs GET /api/v1/talent/watchlists` |
| T11 | `POST /api/v1/talent/watchlists` | yes | true no-mock HTTP | `crates/backend/tests/talent_watchlist_tests.rs, crates/backend/tests/talent_weights_tests.rs` | `crates/backend/tests/talent_watchlist_tests.rs:69#t_t11_create_watchlist_ok() {;crates/backend/tests/talent_watchlist_tests.rs:87#t_t11_create_watchlist_requires_auth() {;crates/backend/tests/talent_weights_tests.rs:222#t_t11_create_watchlist_forbidden_without_talent_read() {`<br/>`crates/backend/tests/talent_watchlist_tests.rs POST /api/v1/talent/watchlists; crates/backend/tests/talent_weights_tests.rs POST /api/v1/talent/watchlists` |
| T12 | `POST /api/v1/talent/watchlists/{id}/items` | yes | true no-mock HTTP | `crates/backend/tests/talent_watchlist_tests.rs` | `crates/backend/tests/talent_watchlist_tests.rs:101#t_t12_add_item_to_own_watchlist() {;crates/backend/tests/talent_watchlist_tests.rs:132#t_t12_add_item_forbidden_for_other_user() {`<br/>`crates/backend/tests/talent_watchlist_tests.rs POST /api/v1/talent/watchlists/{wl_id}/items` |
| T13 | `DELETE /api/v1/talent/watchlists/{id}/items/{cid}` | yes | true no-mock HTTP | `crates/backend/tests/talent_watchlist_tests.rs` | `crates/backend/tests/talent_watchlist_tests.rs:166#t_t13_remove_item_ok() {;crates/backend/tests/talent_watchlist_tests.rs:206#t_t13_remove_item_forbidden_for_other_user() {`<br/>`crates/backend/tests/talent_watchlist_tests.rs DELETE /api/v1/talent/watchlists/{wl_id}/items/{cand_id}` |
| T14 | `GET /api/v1/talent/watchlists/{id}/items` | yes | true no-mock HTTP | `crates/backend/tests/talent_watchlist_tests.rs` | `crates/backend/tests/talent_watchlist_tests.rs:250#t_t14_list_items_owner_scoped() {;crates/backend/tests/talent_watchlist_tests.rs:274#t_t14_list_items_forbidden_for_other_user() {`<br/>`crates/backend/tests/talent_watchlist_tests.rs GET /api/v1/talent/watchlists/{wl_id}/items` |
| N1 | `GET /api/v1/notifications` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs, crates/backend/tests/integration_tests.rs` | `crates/backend/tests/http_p1.rs:1270#t_n1_list_notifications_empty_by_default() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/notifications; crates/backend/tests/integration_tests.rs GET /api/v1/notifications` |
| N2 | `POST /api/v1/notifications/{id}/read` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1285#t_n2_mark_read_self_scope_only() {`<br/>`crates/backend/tests/http_p1.rs POST /api/v1/notifications/{}/read` |
| N3 | `POST /api/v1/notifications/read-all` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1326#t_n3_mark_all_read() {`<br/>`crates/backend/tests/http_p1.rs POST /api/v1/notifications/read-all` |
| N4 | `GET /api/v1/notifications/unread-count` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1358#t_n4_unread_count_is_self_scoped() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/notifications/unread-count` |
| N5 | `GET /api/v1/notifications/subscriptions` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1376#t_n5_subscriptions_list_empty_by_default() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/notifications/subscriptions` |
| N6 | `PUT /api/v1/notifications/subscriptions` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1389#t_n6_upsert_subscriptions_round_trip() {`<br/>`crates/backend/tests/http_p1.rs PUT /api/v1/notifications/subscriptions` |
| N7 | `GET /api/v1/notifications/mailbox-exports` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1418#t_n7_mailbox_exports_self_scoped() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/notifications/mailbox-exports` |
| N8 | `POST /api/v1/notifications/mailbox-export` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1438#t_n8_create_mailbox_export_returns_summary() {`<br/>`crates/backend/tests/http_p1.rs POST /api/v1/notifications/mailbox-export` |
| N9 | `GET /api/v1/notifications/mailbox-exports/{id}` | yes | true no-mock HTTP | `crates/backend/tests/http_p1.rs` | `crates/backend/tests/http_p1.rs:1459#t_n9_download_mailbox_export_self_scoped() {`<br/>`crates/backend/tests/http_p1.rs GET /api/v1/notifications/mailbox-exports/{}` |

## Coverage Summary
- Total endpoints: **117**
- Endpoints with HTTP tests: **117**
- Endpoints with TRUE no-mock tests: **117**
- HTTP coverage: **100.00%**
- True API coverage: **100.00%**

## API Test Classification
1. True No-Mock HTTP
- Real middleware/router stack bootstrapped in `crates/backend/tests/common/mod.rs` (`build_test_app`, `build_test_app_strict`).
- API suites include:
  - `crates/backend/tests/http_p1.rs`
  - `crates/backend/tests/parity_tests.rs`
  - `crates/backend/tests/parity_success_tests.rs`
  - `crates/backend/tests/integration_tests.rs`
  - `crates/backend/tests/deep_*.rs`
  - `crates/backend/tests/talent_*.rs`

2. HTTP with Mocking
- **None detected**.
- No hits for `jest.mock`, `vi.mock`, `sinon.stub`, `mockall`, `wiremock`, `stub(...)` in backend API tests/frontend/e2e.

3. Non-HTTP (unit/integration without HTTP)
- Backend/unit modules present under:
  - `crates/backend/src/talent/scoring.rs`
  - `crates/backend/src/metrics_env/{formula,lineage}.rs`
  - `crates/backend/src/products/import_validator.rs`
  - `crates/backend/src/reports/{csv,pdf,xlsx,cron}.rs`
  - `crates/backend/src/middleware/{csrf,request_id}.rs`
  - `crates/backend/src/crypto/{argon,email,jwt,keys,signed_url}.rs`
- Frontend wasm unit tests:
  - `crates/frontend/src/gate2_tests.rs`

## Unit Test Summary

### Backend Unit Tests
- Test files (backend tests folder):
  - `crates/backend/tests/http_p1.rs`
  - `crates/backend/tests/parity_tests.rs`
  - `crates/backend/tests/parity_success_tests.rs`
  - `crates/backend/tests/integration_tests.rs`
  - `crates/backend/tests/deep_admin_surface_tests.rs`
  - `crates/backend/tests/deep_alerts_reports_tests.rs`
  - `crates/backend/tests/deep_jobs_scheduler_tests.rs`
  - `crates/backend/tests/deep_metrics_kpi_tests.rs`
  - `crates/backend/tests/deep_products_tests.rs`
  - `crates/backend/tests/talent_search_tests.rs`
  - `crates/backend/tests/talent_recommend_tests.rs`
  - `crates/backend/tests/talent_weights_tests.rs`
  - `crates/backend/tests/talent_watchlist_tests.rs`
  - `crates/backend/tests/talent_feedback_tests.rs`
  - `crates/backend/tests/csrf_tests.rs`
  - `crates/backend/tests/budget_tests.rs`
  - `crates/backend/tests/audit9_bundle_tests.rs`
  - `crates/backend/tests/mtls_handshake_tests.rs`

- Modules covered (evidence by suite behavior):
  - controllers/handlers: broad endpoint-level coverage
  - services/repositories: exercised through deep + integration tests
  - auth/guards/middleware: auth/csrf/budget/request-id/security suites

- Important backend modules not strongly unit-focused (mostly integration-verified):
  - `crates/backend/src/app.rs`
  - `crates/backend/src/config.rs`
  - `crates/backend/src/db.rs`
  - `crates/backend/src/spa.rs`
  - `crates/backend/src/seed.rs`

### Frontend Unit Tests (STRICT)
- Frontend test files: `crates/frontend/src/gate2_tests.rs`
- Framework/tool detected: `wasm_bindgen_test`
- Frontend modules directly targeted:
  - `crate::api`
  - `crate::router`
  - `crate::state`
- Important frontend modules/components not directly unit-tested:
  - `crates/frontend/src/pages.rs`
  - `crates/frontend/src/components.rs`
  - `crates/frontend/src/app.rs`
  - `crates/frontend/src/main.rs`

**Frontend unit tests: PRESENT**

### Cross-Layer Observation
- Backend API coverage is exhaustive.
- Frontend has strong logic-level unit tests, but component/page render coverage is thinner.

## API Observability Check
- Strong:
  - `parity_success_tests.rs` adds status + payload shape + envelope assertions.
  - deep/integration suites include response-body and side-effect assertions.
- Weak:
  - `parity_tests.rs` remains mostly auth/status oriented for many endpoints.

## Tests Check
- Success/failure/edge/validation/auth: present.
- Integration boundaries: strong (`integration_tests.rs`, deep suites).
- Over-mocking: none detected.
- `run_tests.sh` Docker-based: **OK** (`docker compose`-driven).

## End-to-End Expectations
- Fullstack FE↔BE e2e specs present:
  - `e2e/specs/login.spec.ts`
  - `e2e/specs/admin_ops.spec.ts`
  - `e2e/specs/products_import.spec.ts`
  - `e2e/specs/analyst_metric_report.spec.ts`
  - `e2e/specs/alert_to_notification.spec.ts`
  - `e2e/specs/talent_recommendations.spec.ts`
  - `e2e/specs/offline_states.spec.ts`
- Gap: several are still navigation/smoke-heavy vs deep business assertion depth.

## Test Coverage Score (0–100)
- **95 / 100**

## Score Rationale
- + 117/117 endpoint coverage with strict method/path evidence
- + real no-mock API-path testing
- + broad auth/validation/integration coverage
- - frontend component/page unit depth still limited
- - e2e business-assertion depth can improve

## Key Gaps
1. Add frontend page/component-level tests (`pages.rs`, `components.rs`, `app.rs`).
2. Deepen e2e assertions (data correctness, workflow effects, negative permission cases).
3. Expand contract-level payload assertions where parity tests are currently status-only.

## Confidence & Assumptions
- Confidence: high on endpoint coverage and no-mock classification.
- Confidence: medium-high on sufficiency scoring.
- Assumption: `docs/api-spec.md` is authoritative endpoint inventory.

---

# README Audit

## README Location
- Found at required path: `repo/README.md`.

## Hard Gates

### Formatting
- PASS
- Clean markdown structure with readable hierarchy and code blocks.

### Startup Instructions (Backend/Fullstack)
- PASS
- Required legacy string present: `docker-compose up` (`README.md:97`).
- Canonical startup present: `docker compose up --build` (`README.md:94`).

### Access Method
- PASS
- URL and port are explicit (`https://localhost:8443`) with probe example.

### Verification Method
- PASS
- Explicit verification section with `./run_tests.sh`.

### Environment Rules (STRICT)
- PASS
- No prohibited runtime install instructions detected (`npm install`, `pip install`, `apt-get`).
- No manual DB setup commands detected (no `./init_db.sh`, no `docker compose exec app terraops-backend seed` command in user workflow).
- README explicitly states Docker-contained startup and no manual DB init required.

### Demo Credentials (Conditional)
- PASS
- Auth exists and README provides username + password context and all roles (`README.md:285+`, role table at `291+`).

## Engineering Quality
- Tech stack clarity: strong.
- Architecture explanation: strong.
- Testing instructions: strong.
- Security/roles/workflows: strong.
- Presentation quality: strong.

## High Priority Issues
- None found.

## Medium Priority Issues
1. README is long and mixes runbook content with historical remediation logs, which may reduce quick-start clarity.

## Low Priority Issues
1. Repetition around startup/coverage narratives can be condensed.

## Hard Gate Failures
- None.

## README Verdict
- **PASS**
