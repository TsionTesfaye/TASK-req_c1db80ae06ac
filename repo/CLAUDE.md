# Developer Rulebook

This file is the repo-local engineering rulebook for `slopmachine` projects.

## Scope

- Treat the current working directory as the project.
- Ignore parent-directory workflow files unless the user explicitly asks you to use them.
- Do not treat workflow research, session exports, or sibling directories as hidden implementation instructions.
- Do not make the repo depend on parent-directory docs or sibling artifacts for startup, build/preview, configuration, verification, or basic project understanding.

## Working Style

- Operate like a strong senior engineer.
- Read the code before making assumptions.
- Work in meaningful end-to-end workstreams.
- Do not call work complete while it is still shaky.
- Treat `plan.md` as the definitive repo-local execution plan once planning is accepted.
- Use `../docs/design.md` for system design and architecture, and use `plan.md` for the execution file tree, execution order, file ownership, and completion tracking.
- During scaffold, treat the accepted scaffold playbook contract in `plan.md` as binding; do not re-choose the playbook, starter, or bootstrap path unless planning is explicitly reopened.
- Update `plan.md` in place from the main developer lane as implementation items are completed.
- If work is interrupted, resume from the current state of `plan.md` instead of inventing a new hidden plan.
- Respect the execution file tree, file-ownership map, and shared-file contract from `plan.md` when parallel work is in play.
- During parallel work, branch or worktree lanes should report completion back to the main lane, and the main lane should update `plan.md` after merge.
- Once given a bounded objective, keep going autonomously until it is complete or genuinely blocked; do not stop for reassurance or permission when a prompt-faithful default lets you proceed.
- Reuse and extend shared cross-cutting patterns instead of inventing incompatible local ones.
- Before coding, identify the actors or personas touched by the change and the concrete path to success for each one.
- Make important business rules explicit before coding: defaults, limits, allowed transitions, uniqueness, conflicts, reversals, retries, and ownership rules when they matter.
- When the product has meaningful workflow state, define or confirm the relevant state machine before treating the flow as implemented.
- Keep a concrete out-of-scope boundary in mind so you do not overbuild speculative features.
- Do not introduce convenience-based `v1` scope cuts, role simplifications, or workflow omissions unless they were explicitly authorized.
- When backend or fullstack API endpoints are added or changed, prefer real HTTP tests for the exact `METHOD + PATH` over controller or service bypasses when practical.
- If mocked HTTP tests or unit-only tests still exist for an API surface, do not overstate them as equivalent to true no-mock endpoint coverage.

## Requirements Fidelity

- Preserve the full prompt intent, including implied business constraints.
- Do not weaken required actor models, operator flows, security controls, or lifecycle behavior for implementation convenience.
- If a requirement is ambiguous, choose the safest prompt-faithful behavior and keep moving when a defensible default exists; surface the ambiguity only when it is genuinely blocking or materially changes the product contract.
- If the feature depends on business rules, make those rules traceable in code, tests, and `README.md` rather than leaving them implicit.

## Architecture Rules

- For backend or fullstack projects, route configuration through a central config module instead of scattering direct environment reads through business logic.
- Keep database operations, business logic, transport layers, and UI surfaces separated clearly enough for static review.
- Use a shared logging path and shared validation/error-normalization path when the project is large enough for those concerns to matter.
- When a third-party service is required but real integration is not explicitly demanded, prefer an internal stub or adaptor boundary over shipping brittle real credentials or uncontrolled external dependencies.
- If a feature requires auth or privileged access, enforce it across route, controller or handler, and object scope where applicable.

## Runtime And Verification

- Keep one primary documented runtime command and one primary broad test command: repo-root `./run_tests.sh`.
- Follow the original prompt and existing repository first for the runtime stack.
- Prefer the fastest meaningful local verification for the changed area during ordinary iteration.
- Do not rerun broad runtime/test commands on every small change.
- During ordinary `P4` implementation work, do not run Docker runtime commands, browser E2E, Playwright, full test suites, or `./run_tests.sh`.
- Use targeted local tests during ordinary `P4` implementation work and leave browser E2E plus broad-gate commands for the end-of-development gate and fused `P5`.
- When API tests are material, make them hit real endpoints and print simple useful response evidence such as status codes and message/body summaries.
- For web projects, require the runtime contract to be `docker compose up --build`.
- For Android, mobile, desktop, and iOS-targeted projects, also require a meaningful `docker compose up --build` command even when platform-specific runtime proof differs from web semantics.
- For non-web projects, `./run_app.sh` may exist as a helper wrapper, but it does not replace the required Docker command.
- If the project has database dependencies, keep `./init_db.sh` as the only project-standard database initialization path.

## Git Discipline

- Keep the implementation git-backed as work progresses.
- After each feature-complete or otherwise meaningful completed workstream, stage and create a small descriptive progress commit before moving on.
- Prefer many honest small commits over one large end-of-run dump commit.
- Do not commit broken work, secrets, local-only junk, or unrelated noise.
- When `plan.md` marks mutually exclusive file ownership, default to separate branches or worktrees for those sections when support exists, and otherwise still use parallel lanes with the same boundaries.
- Keep `plan.md`, `README.md`, and other shared-file integration in the main lane unless the accepted plan explicitly delegates them.
- When parallel branches or worktrees are used, each one should commit meaningful progress as it goes, and the main lane should also commit shared-file and integration progress.

## Documentation Rules

- Keep `README.md` accurate.
- The README must explain what the project is, what it does, how to run it, how to test it, the main repo contents, and any important information a new developer needs immediately.
- The README must also explain the delivered architecture and major implementation structure clearly enough for review and handoff.
- The README must include project type near the top, startup instructions, access method, verification method, and demo credentials for every role or the exact statement `No authentication required`.
- The README must clearly document the required Docker command `docker compose up --build` and any additional helper runtime wrapper such as `./run_app.sh` when present.
- The README must clearly document repo-root `./run_tests.sh` as the broad test command.
- For backend, fullstack, and web projects, the README should also contain the exact legacy compatibility string `docker-compose up` somewhere in startup guidance without replacing the canonical runtime contract.
- The README must stand on its own for basic codebase use.
- Keep `README.md` as the primary documentation file inside the repo.
- `plan.md` is the explicit allowed exception because it is the repo-local execution plan.
- Treat `README.md` as the primary documentation surface inside the repo.
- The repo should be statically reviewable by a fresh reviewer: entry points, routes, config, test commands, and major module boundaries should be traceable from repository artifacts.
- The README should name the important actors, the main success paths, major limitations or out-of-scope boundaries, and any non-obvious business rules that affect usage.
- If the project uses mock, stub, fake, interception, or local-data behavior, the README must disclose that scope accurately.
- If mock or interception behavior is enabled by default, the README must say so clearly.
- Feature flags, debug/demo surfaces, default enabled states, and mock/interception defaults must be disclosed in `README.md` when they exist.
- Do not let a mock-only or local-data-only project look like undisclosed real backend or production integration.
- Do not hide missing failure handling behind fake-success paths.
- Before final delivery, remove local-only setup traces and host-only dependency assumptions from the README and wrapper scripts.

## Secret And Runtime Rules

- Do not create or keep `.env` files anywhere in the repo.
- Do not rely on `.env`, `.env.local`, `.env.example`, or similar files for project startup.
- Do not hardcode secrets.
- If runtime env-file format is required, generate it ephemerally and do not commit or package it.
- Do not hardcode database connection values or database bootstrap values anywhere in the repo.
- For Dockerized web projects, `docker compose up --build` should work without any manual `export ...` step.
- For Dockerized web projects, prefer a dev-only runtime bootstrap script that is invoked automatically by the Docker startup path to generate or inject local-development runtime values.
- That bootstrap path must not use checked-in `.env` files or hardcoded runtime values.
- Do not pre-seed secret literals in Compose files, config files, Dockerfiles, or startup scripts even if they are labeled dev-only, test-only, or non-production.
- `./run_tests.sh` should use the same startup-value model as `docker compose up --build` rather than a separate pre-seeded test-secret path.
- If local-development runtime values must persist across restarts, keep them only in Docker-managed runtime state rather than committed repo files.
- If such a bootstrap script exists, document in the script and in `README.md` that it is for local development bootstrap only and is not the production secret-management path.
- Do not let `docker compose up --build` or `./run_tests.sh` depend on host-installed packages, SDKs, language runtimes, CLIs, or toolchains beyond Docker and the documented baseline host prerequisites; define those dependencies in Dockerfiles or other repo-controlled container build definitions.

## Product Integrity Rules

- Do not leave placeholder, setup, debug, or demo content in product-facing UI.
- If a real user-facing or admin-facing surface is required, build that surface instead of bypassing it with API shortcuts.
- Treat missing real surfaces as incomplete implementation.
- If multiple roles or personas exist, implement the real role-aware surfaces and permissions rather than collapsing them into a single generic flow.
- Do not replace prompt-required interaction models, lifecycle behavior, or data-integrity rules with easier substitutes unless explicitly authorized.

## Security And Reliability Rules

- When multiple roles or privileged actions exist, use RBAC or stronger scoped authorization as required by the actual product behavior.
- Enforce object-level authorization when users should only see or mutate their own records.
- Keep sensitive data out of logs, errors, screenshots, and seeded runtime values.
- Return normalized user-safe errors; do not expose raw stack traces or internal file paths in normal product surfaces.
- Make request logging, exception logging, and background failure logging meaningful enough for operators to understand what failed without leaking secrets.
- If offline, queueing, retries, jobs, or resumability matter, implement explicit states and recovery behavior instead of vague best-effort logic.

## Rulebook Files

- Do not edit `AGENTS.md` or other workflow/rulebook files unless explicitly asked.
- Do not rewrite or bypass `plan.md` casually once it has been accepted as the implementation checklist.
