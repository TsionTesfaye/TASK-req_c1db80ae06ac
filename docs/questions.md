# Questions

This file records only prompt items that needed interpretation because they were unclear, incomplete, or materially ambiguous.

### 1. Local Deployment Scope
- Question: Does “single local workstation site” mean localhost only, or a local single-node deployment that can still serve approved LAN devices?
- My Understanding: The prompt simultaneously requires fully offline operation, TLS on local LAN, endpoint allowlists, and certificate pinning. That implies a single local deployment unit, but not necessarily localhost-only access.
- Solution: Treat the product as a single-node local deployment that can serve the host machine and, when configured by administrators, a limited internal LAN allowlist over HTTPS.

### 2. Role Separation Model
- Question: Should the five named roles be hard-separated, loosely cosmetic, or explicitly permissioned with possible overlap?
- My Understanding: The prompt asks for role-based workspaces across materially different responsibilities. Cosmetic role labels would underdeliver, while forcing exactly one role per user would be unnecessarily restrictive.
- Solution: Implement explicit RBAC with route-level and object/action checks, and allow administrators to assign multiple roles to one user when needed.

### 3. Dashboard Landing Behavior
- Question: What is the correct default landing experience immediately after authentication?
- My Understanding: The prompt says users land on KPI dashboards after sign-in, but the specific dashboard behavior across multiple roles is unstated.
- Solution: Send users to a role-aware dashboard home with KPI cards, alerts, and shortcuts filtered to the modules they are authorized to use.

### 4. Dashboard Filtering Depth
- Question: Should slice-and-drill support one active filter at a time or composable filtering across dimensions?
- My Understanding: The prompt names multiple dimensions—site, department, time window, and category—which implies users need combined filtering for meaningful analysis.
- Solution: Support composable filters and drill-down views across those dimensions, with sane per-user defaults where helpful.

### 5. Alert Rule Flexibility
- Question: Are the comfort-index and on-shelf examples the only alert rules required, or examples of a more general threshold system?
- My Understanding: The examples are illustrative rather than exhaustive, and the notification center plus dashboard alerts imply reusable alert-rule records.
- Solution: Model configurable local alert rules with threshold, duration/window, target metric, and recipient/subscription behavior instead of hard-coding only the examples.

### 6. Scheduled Report Semantics
- Question: Does scheduled reporting require real persisted jobs or just downloadable exports triggered from the UI?
- My Understanding: The prompt explicitly calls for scheduled generation and local retry behavior elsewhere, which points to real persisted report jobs.
- Solution: Treat scheduled reporting as a real local workflow with durable job state and visible status/failure handling, not as a UI-only shortcut for immediate downloads.

### 7. Product Change History Depth
- Question: What level of master-data history is required for “visible change history showing who changed what and when”?
- My Understanding: The phrase implies more than a last-modified field; import operations, image updates, and status changes also need traceability.
- Solution: Keep immutable audit/change-history entries capturing actor, timestamp, before/after summary, action type, and batch context where applicable.

### 8. Import Validation Behavior
- Question: Should bulk imports validate before persistence or accept bad rows and report issues later?
- My Understanding: The prompt explicitly requires preview and row-level validation feedback, which implies pre-commit staging.
- Solution: Validate rows during a preview step before final import commit, so users can see row-level issues before data becomes authoritative.

### 9. Derived Metric Definition Persistence
- Question: Are environmental metrics fixed features or user-configurable reusable definitions?
- My Understanding: Analysts are asked to configure sources, alignment rules, and confidence labels, which implies persisted reusable definitions rather than ad hoc front-end calculations.
- Solution: Persist metric-definition records including source selections, alignment/windowing parameters, formulas, and confidence metadata so computations are reproducible.

### 10. Lineage Explainability Depth
- Question: How detailed must the “why this value” panel be?
- My Understanding: Because the prompt explicitly mentions computation lineage, source IDs, alignment parameters, and confidence, the explanation must be evidence-rich.
- Solution: Show contributing source observations, applied windows/alignment rules, formula components, confidence labels, and timestamps that explain the derived value.

### 11. Cold-Start Recommendation Logic
- Question: How should talent recommendation ranking behave until at least 10 feedback events exist?
- My Understanding: The prompt already points to recency and completeness, but planning needs that locked as the true cold-start baseline rather than a weak placeholder.
- Solution: Use a cold-start weighted score prioritizing recency and completeness until at least 10 relevant feedback events exist, then blend user preference weights and feedback signals into the ranking.

### 12. Watchlist And Feedback Visibility
- Question: Are watchlists and thumbs-up/down reactions shared globally or scoped?
- My Understanding: The prompt mentions personal tuning and watchlists without describing teamwide visibility. Global exposure would unnecessarily expand privacy and UX scope.
- Solution: Keep watchlists private to their creator while making feedback auditable and available for role-scoped recommendation tuning.

### 13. Offline Search Boundary
- Question: What search approach fits intelligent matching without internet dependencies?
- My Understanding: The portal must remain fully local, so search should not depend on cloud services or internet-connected vector tooling.
- Solution: Keep search and recommendation matching fully local and offline-capable, with no runtime dependence on cloud or internet services.

### 16. Image Protection Strategy
- Question: What is the best interpretation of hotlink protection for product images in a local portal?
- My Understanding: The requirement for signed URLs expiring after 10 minutes implies authorized image serving rather than raw static file exposure.
- Solution: Store images in managed local storage and serve them only through signed, expiring, authorization-checked URLs.

### 17. Retention Policy Enforcement
- Question: Are retention settings just visible configuration or active policy enforcement?
- My Understanding: The prompt frames them as privacy controls administrators can set, which implies actual purge behavior.
- Solution: Implement scheduled local retention jobs that purge expired raw environmental readings and stale user feedback while preserving aggregated KPIs and audit history per policy.

### 18. Local Notification Center Behavior
- Question: What functionality is required beyond simple in-app messages?
- My Understanding: The prompt includes subscriptions, read/unread state, retry queues, and mailbox-file exports, so the system needs real delivery-attempt tracking.
- Solution: Model notifications with subscription rules, delivery attempts, retry state, read/unread tracking, and local mailbox-export generation without any outbound sending.

### 19. Offline Placeholder Semantics
- Question: Should unavailable data be hidden, shown as stale, or replaced with placeholders?
- My Understanding: The prompt asks for placeholder states when data is unavailable and tiered caching, which means the UI must be explicit about freshness.
- Solution: Show clear loading/empty/error placeholders and cached last-known summaries only when marked as such; never present unavailable fresh data as current.

### 20. Monitoring Scope
- Question: How deep should built-in monitoring go for administrators?
- My Understanding: The prompt explicitly calls out API latency, error rate, and client crash reports, and the local retry/job features add more operational surfaces worth tracking.
- Solution: Deliver built-in, fully local administrator monitoring that at minimum covers API latency, error rate, and client crash reports, while leaving detailed telemetry design to planning.

### 21. Timestamp Standardization
- Question: How should timezone storage and display formatting be reconciled consistently across the system?
- My Understanding: The prompt requires timezone-aware storage plus MM/DD/YYYY 12-hour display, so the system needs one clear storage/display rule.
- Solution: Store timestamps as timezone-aware values in PostgreSQL, keep timezone context available where calculations require it, and present/export them consistently in MM/DD/YYYY, 12-hour time.
