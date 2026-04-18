# Design

This document is the owner-maintained external design reference for the project. Treat it as the authoritative system design and architecture contract once planning begins.

## Bootstrap Notes

- This workspace started from a clean slopmachine bootstrap. Update this file as clarification and planning progress.

## How To Use This File

- Replace placeholders with concrete implementation decisions.
- Do not leave major sections vague, empty, or deferred without an explicit reason.
- Prefer explicit tables, bullet lists, and subsections over broad narrative.
- Make this detailed enough that the accepted scaffold playbook contract and execution checklist in repo/plan.md can be derived directly from it.

## Tech Stack Summary

State the concrete backend, frontend, persistence, auth/security, jobs/async, runtime, and other major stack choices here. If something is intentionally not fixed yet, say so explicitly and explain why.

## Product Overview

Describe what is being built, for whom, and the core business objective in implementation-relevant language.

## System Overview

- What is being built
- Core business objective
- Primary technical shape

## In-Scope Domains

List the major domains or modules that are part of delivery.

## Explicit Out-Of-Scope

List what is intentionally not being built so implementation does not overreach.

## Actors And Roles

Define the user types, admins, operators, and other actors.

## Actor Success Paths

For each important actor, define the concrete path to success for the prompt-critical workflows.

## Architecture And Module Boundaries

Describe the major components, boundaries, and why this structure was chosen.

## Domain Model And Data Model

List the important entities, relationships, supporting records, and persistence constraints.

## Authoritative Business Rules

Define defaults, limits, formulas, uniqueness rules, conflicts, retries, reversals, ownership rules, and other critical domain rules.

## State Machines And Lifecycles

Define the allowed states, transitions, triggers, and illegal transitions where workflow state matters.

## Permissions And Authorization Model

Define roles, scopes, route-level checks, object-level checks, and privileged actions.

## Validation And Error Handling

Define the important validations, normalized errors, and failure behavior.

## Security, Compliance, And Data Governance

Describe authentication, session or token rules, audit expectations, masking, retention, deletion, export restrictions, and other sensitive-data controls when relevant.

## Offline, Queueing, Reliability, And Background Jobs

When relevant, define queue behavior, retries, resumability, conflict handling, job states, observability, and maintenance behavior.

## Reporting, Analytics, Search, And Exports

When relevant, describe KPIs, source of truth, calculations, indexing or search behavior, export rules, and report job lifecycles.

## Runtime, Config, And Ops Contract

Define runtime entrypoints, config flow, environment handling, jobs, observability, and operational expectations.

## Interface Contracts

Describe important APIs, frontend/backend contracts, payload shapes, and user-visible interaction expectations.

## Non-Functional Requirements

Describe performance, deterministic behavior, restart recovery, availability, timezone, backup, and other cross-cutting quality requirements when relevant.

## Verification Strategy

Describe the local verification model, broad-gate verification, and the major risk-to-test expectations.

## Dependency And Parallelism Plan

Identify which work must stay serial because of shared foundations, and which 2 or 3 work packages can safely proceed in parallel once those foundations are settled.

## Implementation Phases

Break the work into realistic phases or major chunks with dependencies and usable outcomes.

## Phase Checkpoints

For each phase, define required artifacts, required working flows, required tests, exit criteria, and what is not allowed to defer past that phase.

## Definition Of Done

Define the hard completion standard that should block fake-complete or scaffold-only delivery.

## Deliverables

List the concrete outputs delivery must produce, such as working application, scripts, tests, docs, bootstrap assets, exports, or admin tools when relevant.

## Assumptions, Dispositions, And Open Items

Keep unresolved items rare. Each section should be either concrete or explicitly marked not applicable with a reason. If something is still open, name it explicitly, explain why it is still open, and state what evidence or decision is needed to close it.
