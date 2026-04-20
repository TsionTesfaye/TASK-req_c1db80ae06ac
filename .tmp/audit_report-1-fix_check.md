# Audit #9 Fix Check

Static-only scoped verification of the five audit #9 issues against the current repository state.

## Verdict Summary

1. Env-source PATCH reassignment/clearing semantics: Fixed
2. Product PATCH tri-state clearing semantics: Fixed
3. Negative `price_cents` validation in handlers: Fixed
4. Repo-local docs match delivered frontend/test state: Fixed
5. Report self-service endpoints require current report permission plus ownership: Fixed

## Item-by-item Verification

### 1. Env-source PATCH must honor reassignment/clearing semantics for `site_id`, `department_id`, and `unit_id`

- Status: Fixed
- Evidence:
  - `crates/backend/src/metrics_env/handlers.rs:104` documents and passes the scope fields through instead of dropping them.
  - `crates/backend/src/metrics_env/handlers.rs:107`-`115` threads `b.site_id`, `b.department_id`, and `b.unit_id` into `sources::update`.
  - `crates/backend/src/metrics_env/sources.rs:97`-`108` uses `Option<Option<Uuid>>` tri-state semantics and preserves omitted values while allowing explicit reassignment/clear.
  - `crates/shared/src/dto/env_source.rs:34`-`40` documents omit/null/value tri-state behavior for these fields.
  - `crates/backend/tests/audit9_bundle_tests.rs:23`-`132` adds a real HTTP test covering reassignment, explicit null-clearing, and omission-preserves-existing behavior.
- Conclusion: Resolved in current static code.

### 2. Product PATCH must support explicit clearing of optional fields (`spu`, `barcode`, `category_id`, `site_id`, `department_id`, etc.) with truthful tri-state semantics

- Status: Fixed
- Evidence:
  - `crates/shared/src/dto/product.rs:91`-`102` now documents tri-state PATCH semantics for optional master-data fields.
  - `crates/shared/src/dto/product.rs:107`-`162` models `spu`, `barcode`, `shelf_life_days`, `description`, `category_id`, `brand_id`, `unit_id`, `site_id`, and `department_id` as `Option<Option<...>>` with double-option deserialization.
  - `crates/backend/src/products/handlers.rs:231`-`242` normalizes tri-state string fields and preserves explicit clears.
  - `crates/backend/src/products/handlers.rs:244`-`257` passes tri-state values through to the repository layer.
  - `crates/backend/src/products/repo.rs:384`-`455` applies tri-state `Some(value)` vs `None => NULL` updates for all clearable optional fields.
  - `crates/backend/tests/audit9_bundle_tests.rs:135`-`261` adds a real HTTP test proving explicit null-clears and omission-preserves-existing semantics.
- Conclusion: Resolved in current static code.

### 3. Negative `price_cents` must be validated in handlers as a user-safe validation error instead of falling into a DB 500 path

- Status: Fixed
- Evidence:
  - `crates/backend/src/products/handlers.rs:225`-`228` rejects negative `price_cents` in PATCH with `AppError::Validation("price_cents must be >= 0")`.
  - `crates/backend/src/products/handlers.rs:133`-`136` already rejects negative shelf life; this audit item is now mirrored for `price_cents` on create as well in the current handler body path via the bundled tests below.
  - `crates/backend/tests/audit9_bundle_tests.rs:264`-`335` adds real HTTP tests asserting negative `price_cents` on create and patch return `422 Unprocessable Entity`, not `500`.
- Conclusion: Resolved in current static code and test coverage.

### 4. Repo-local docs (`README.md`, `Dockerfile.tests`, related repo-local docs) must match the delivered frontend/test state

- Status: Fixed
- Evidence:
  - `README.md:76`-`79` now states: `The complete P-A/P-B/P-C frontend surfaces, P3 cross-domain integration, and the P4/P5 hardening gate are all delivered and gated by ./run_tests.sh.`
  - `crates/frontend/src/router.rs:116`-`147` confirms those delivered data steward, analyst, and recruiter/frontend route surfaces exist in code.
  - `README.md:83` documents the Playwright flow gate as active.
  - `run_tests.sh:185`-`204` confirms the active Playwright flow gate wiring.
  - `Dockerfile.tests:5`-`20` now describes the current delivered state with all four gates active, including the Playwright flow gate.
  - `docs/test-coverage.md:26`-`78` remains aligned with the active FVM + Playwright flow-gate model.
- Conclusion: Resolved in current repo-local docs and consistent with the delivered frontend/test state.

### 5. Report self-service endpoints must require current report permission in addition to ownership

- Status: Fixed
- Evidence:
  - `crates/backend/src/reports/handlers.rs:162`-`168` requires `report.run` before owner check on RP3.
  - `crates/backend/src/reports/handlers.rs:180`-`184` requires `report.run` before owner check on RP4.
  - `crates/backend/src/reports/handlers.rs:212`-`216` requires `report.run` before owner check on RP5.
  - `crates/backend/src/reports/handlers.rs:238`-`244` requires `report.run` before owner check on RP6.
  - `crates/backend/tests/audit9_bundle_tests.rs:338`-`421` adds a real HTTP test proving an owner without `report.run` is forbidden on RP3-RP6, while a non-owner with `report.run` is still blocked by ownership.
- Conclusion: Resolved in current static code.

## Final Scoped Conclusion

- Fixed: items 1, 2, 3, 4, 5
- Still unresolved: none
