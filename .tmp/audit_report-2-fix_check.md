# TerraOps Issue Re-Check (From Scratch)

## Scope / Method
- Static re-audit only (code + docs + tests).
- Did **not** run project, Docker, or tests.
- Re-checked all 5 previously reported findings directly against current repository state.

## Results Summary
- High #1 (notification producer bypass): **Fixed**
- High #2 (cold-start global count): **Fixed**
- High #3 (P13 contract contradiction): **Fixed**
- Medium #4 (`X-Requested-With` doc-only): **Fixed**
- Low #5 (budget comment contradiction): **Fixed**

---

## 1) High — Notification producer paths bypass shared `emit` contract
**Previous status:** Fail  
**Current status:** **Fixed**

### Current evidence
- Shared contract still defines `emit` as sole publisher path with subscription + delivery-attempt logic:  
  `crates/backend/src/services/notifications.rs:3-5`, `:21-32`, `:46-55`
- Alert producer now routes through `notif_svc::emit(...)` (no raw notification insert):  
  `crates/backend/src/alerts/evaluator.rs:203-217`
- Report producer now routes through `services::notifications::emit(...)`:  
  `crates/backend/src/reports/scheduler.rs:366-380`
- Source-wide check shows no runtime raw producer insert remains (only seed path + service insert):  
  `crates/backend/src/seed.rs:555`, `crates/backend/src/services/notifications.rs:35`

### Supporting test evidence (static)
- Producer-path regression tests added for both alert/report emit path + opt-out behavior + delivery attempts:  
  `crates/backend/tests/deep_jobs_scheduler_tests.rs:575-582`, `:585-707`, `:711-838`

### Conclusion
- **Fixed** based on code-path and test coverage updates.

---

## 2) High — Cold-start threshold uses global feedback count
**Previous status:** Fail  
**Current status:** **Fixed**

### Current evidence
- Design contract still requires scoped signal:  
  `docs/design.md:192`
- Recommendations now use scoped count by `(user, role)`:  
  `crates/backend/src/talent/handlers.rs:325-332`
- Feedback repository now provides scoped counter (`count_scoped`) instead of global `count_total`:  
  `crates/backend/src/talent/feedback.rs:59-79`

### Supporting test evidence (static)
- Existing blended transition test updated to use same owner/role-scoped rows:  
  `crates/backend/tests/talent_recommend_tests.rs:187-204`, `:215`
- New isolation tests added for owner scope, role scope, and threshold boundary:  
  `crates/backend/tests/talent_recommend_tests.rs:226-233`, `:260-306`, `:309-360`, `:367-459`

### Conclusion
- **Fixed** with matching docs/code/tests semantics.

---

## 3) High — P13 signed-image auth contract internally contradictory
**Previous status:** Fail  
**Current status:** **Fixed**

### Current evidence
- API spec now consistently defines `SIGNED` as explicit auth class and P13 as capability URL (no bearer):  
  `docs/api-spec.md:23`, `docs/api-spec.md:120`, `docs/api-spec.md:243`
- Handler remains intentionally non-bearer and signature-gated:  
  `crates/backend/src/products/images.rs:141-149`, `:163-167`
- Parity test expectation aligned to SIGNED behavior (403 when missing signed params, not 401):  
  `crates/backend/tests/parity_tests.rs:210-225`

### Supporting test evidence (static)
- Signed URL negative-path integration tests exist for forged/expired/tampered params:  
  `crates/backend/tests/integration_tests.rs:854`, `:933`

### Conclusion
- **Fixed** with docs + handler + parity tests aligned to one canonical contract.

---

## 4) Medium — Mutation header contract documented but not implemented
**Previous status:** Fail  
**Current status:** **Fixed**

### Current evidence
- Docs now explicitly state enforcement and client attachment:  
  `docs/api-spec.md:11`, `docs/design.md:266`
- Backend CSRF middleware implemented (`CsrfMw`) for mutations with expected header/value:  
  `crates/backend/src/middleware/csrf.rs:5-11`, `:41-43`, `:90-111`
- Middleware is registered in app stack:  
  `crates/backend/src/app.rs:110`
- Frontend mutation helpers attach `X-Requested-With: terraops`:  
  `crates/frontend/src/api.rs:294-297`, `:320-321`

### Supporting test evidence (static)
- Dedicated CSRF contract tests cover missing/wrong/correct header, method scope, and auth precedence:  
  `crates/backend/tests/csrf_tests.rs:30-58`, `:60-84`, `:86-123`, `:151-168`, `:170-213`

### Conclusion
- **Fixed** end-to-end in docs, backend, frontend, and tests.

---

## 5) Low — Budget comment contradicts middleware scope
**Previous status:** Partial Fail  
**Current status:** **Fixed**

### Current evidence
- Budget comment now explicitly states it applies to every route including `/health` and `/ready`, and clarifies no exemption logic:  
  `crates/backend/src/middleware/budget.rs:4-13`
- App still wraps globally (consistent with updated comment):  
  `crates/backend/src/app.rs:109`

### Conclusion
- **Fixed** (documentation/commentary now matches implementation behavior).

---

## Final Determination
All five previously reported findings appear **resolved** in the current repository state based on static evidence.

## Boundary Note
Runtime behavior (actual request responses, middleware ordering under live server load, and test execution outcomes) remains **Manual Verification Required** because this review intentionally did not execute code.
