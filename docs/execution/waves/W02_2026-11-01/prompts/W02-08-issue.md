## Audit repair objective

Continue existing OPEN PR69 instead of starting another competing implementation. Fetch branch issue-19-migration-discovery-2421886187897473093 at exact1a16791752fb7f42f5d475cb6472c5d915bbba40; preserve all commits and add only scoped fixes, merge verified master base normally if needed. Planner::create_review_plan can allow decommission for empty inventory through vacuous all(). Fail closed for empty/unknown/unqualified source inventory even when handoff attestations are all true; add explicit blockers and regressions. Keep exact-source registry empty until genuine version-matched sanitized source evidence exists. No guessed CloudPanel schema/support, no actual apply/decommission. Update existing PR69 with additive commits and evidence (no force push); do not duplicate it solely for delivery.

## Ownership

Exclusive write paths: modules/migration/. Parent package S18, issue #19. Current verified master base 3a8bf9ef39c35a43b36771c575dbc3b11bc601ba.

## Acceptance and delivery

Preserve feature IDs and selectable workflows. Include meaningful unit, real integration and E2E evidence; missing prerequisites are BLOCKED with nonzero status, not passing placeholders. Provide exact commands, versions, sanitized outputs and cleanup. Create/update a focused PR against master using Refs for this issue and its parent; never auto-close issues, merge, squash, force push or modify production/host services. No product capability acceptance from a merge alone.
