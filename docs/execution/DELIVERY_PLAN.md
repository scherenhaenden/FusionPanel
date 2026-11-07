# Delivery Plan: 1–11 November 2026

Use the [product delivery plan](PRODUCT_DELIVERY_PLAN.md) to schedule the next user journeys. This document retains the package ownership, dependencies, and original November targets.

## Objective and deadline

Use Jules through 11 November 2026 to implement and qualify FusionPanel's combined capability scope. All dates use Europe/Berlin. The provider's exact access cutoff is unknown: target the final complete candidate and recoverable artifacts for **10 November, 18:00**, and reserve 11 November for correction and handoff. No claim is made that 162 broad capabilities can be proven production-ready within this window merely by launching more sessions.

The planning baseline is 63 reviewable work packages. Some may require more than one bounded Jules session if observed complexity or failures exceed the prompt scope. Every such split retains the original capability owner and acceptance requirements. Dates are target gates, not evidence of completion.

Track every implementation PR against its package issue in the [PR and issue tracker](PR_ISSUE_TRACKER.md). That tracker records merged SHAs and qualification gaps; it does not replace the 162-capability evidence ledger.

## Coordination model

The coordinator owns architecture, feature preservation, interfaces, task specifications, dependencies, issue/session traceability, review and integration proposals. Jules sessions implement bounded code and tests. Only dispatch a dependent task after its prerequisite artifacts are reviewed and available on its explicit base branch/SHA. Sessions never merge their own PRs. Integration merges remain pending explicit owner authorization under current project policy; without them, dependent sessions are blocked rather than pointed at absent code.

Remote commands use the local Git/GitHub/Jules CLIs outside the sandbox. Maintain one session per active task; retrieve a completed patch rather than launching duplicates because a PR is missing. Pin source commits and exact accepted contracts at dispatch. The `master` branch is canonical. The frozen legacy `main` branch exists temporarily for five sessions launched before the branch change; retarget their PRs to `master` and retire the legacy branch only after those results are recovered.

## Parallelism and critical path

Start five foundation packages plus independent domain modules, UI and lab work. New domain modules are standalone Rust packages with their own `[workspace]` until integration; they must not edit root Cargo.toml/Cargo.lock. This prevents workspace conflicts while preserving independently runnable tests. Integration packages wait for approved contracts and working runtimes. End-to-end suites start only when actual binaries/services exist; missing prerequisites are blocked, never a passing test.

Critical path: S01–S05 → S06 → S24/S25 → S26/S27/S28 → S32/S33/S35 → S43–S53 → S54–S59 → S61/S62. S07 must be usable before the real integration and E2E waves. Provision disposable full-OS lab capacity early; a container does not qualify kernel/firewall/reboot behavior.

Bound dispatch by actual Jules account availability. Initially add ten independent tasks to the existing five. If the provider rejects a request, record its error and leave the task queued; do not blindly retry ambiguous creates. Keep two scheduling slots free once real limits are known for repair and integration work. No task writes another live task's directory. Later tasks owning existing directories require an explicit handoff after earlier owners finish.

## Daily gates

| Date | Focus | Required gate / response if missed |
|---|---|---|
| Nov 1 | Foundation plus ten independent module/lab/UI tasks | Record IDs, contracts and actual status; recover completed API patch. |
| Nov 2 | Review foundation; complete initial module adapters | Agree interfaces and real fixture evidence. Repair missing contracts before wiring modules. |
| Nov 3 | Remaining independent modules; source/behavior characterization | Identity, secrets, deployment, node and WordPress contracts ready; no silent capability omissions. |
| Nov 4 | Node runtime, auth integration, CI and disposable OS lab | Real authenticated API-to-agent action and durable jobs; CI captures evidence. |
| Nov 5 | Hosting/data/container/DNS/mail vertical slices | Real service effects and persisted state, not successful stub responses. |
| Nov 6 | UI, deployment, observability, multi-node, WordPress and migration integration | Browser/API/CLI operate the same actual resources. |
| Nov 7 | Real E2E: foundation, hosting, data, containers, deployments and WordPress | Real processes; assert served content/data and negative cases. |
| Nov 8 | Node faults, DNS/mail, admin, security and full migration/recovery | Record actual failure/recovery timing and every blocker. |
| Nov 9 | Capability closure, variants, performance and gap repairs | Review all 162 entries; open defects with owners instead of deleting scope. |
| Nov 10 | Release regression, clean install, independent restore and handbook | Complete candidate and exported artifacts by 18:00 target. |
| Nov 11 | Correction reserve and final handoff | Targeted retest; preserve branches/patches/test evidence before Jules access ends. |

## Full session table

Each row links to a detailed specification with unit, integration and E2E requirements. The machine-readable source is [sessions.json](sessions.json). Task dates are earliest target finish dates subject to dependencies, not permission to bypass a failed gate.

### Foundation

| ID | Target | Work package | Waits for | Exclusive paths |
|---|---|---|---|---|
| [S01](sessions/S01.md) | Nov 1 | Domain model | Independent | `crates/domain/` |
| [S02](sessions/S02.md) | Nov 1 | Control-plane API | Independent | `crates/api/` |
| [S03](sessions/S03.md) | Nov 1 | Agent protocol | Independent | `crates/agent-protocol/` |
| [S04](sessions/S04.md) | Nov 1 | Persistent jobs | Independent | `crates/jobs/` |
| [S05](sessions/S05.md) | Nov 1 | Administrative CLI | Independent | `crates/cli/` |
| [S06](sessions/S06.md) | Nov 2 | Foundation integration | S01, S02, S03, S04, S05 | `Cargo.toml` `Cargo.lock` `crates/api/` `crates/cli/` `crates/domain/` `crates/agent-protocol/` `crates/jobs/` |

### Independent module and laboratory work

| ID | Target | Work package | Waits for | Exclusive paths |
|---|---|---|---|---|
| [S07](sessions/S07.md) | Nov 1 | Real test laboratory | Independent | `tests/` |
| [S08](sessions/S08.md) | Nov 2 | Identity and permissions | Independent | `modules/identity/` |
| [S09](sessions/S09.md) | Nov 2 | Hosting plans and configuration | Independent | `modules/hosting/` |
| [S10](sessions/S10.md) | Nov 2 | Database adapters | Independent | `modules/databases/` |
| [S11](sessions/S11.md) | Nov 2 | Backup and recovery core | Independent | `modules/backups/` |
| [S12](sessions/S12.md) | Nov 2 | Container management | Independent | `modules/containers/` |
| [S13](sessions/S13.md) | Nov 2 | DNS management | Independent | `modules/dns/` |
| [S14](sessions/S14.md) | Nov 2 | Mail management | Independent | `modules/mail/` |
| [S15](sessions/S15.md) | Nov 2 | Host security controls | Independent | `modules/security/` |
| [S16](sessions/S16.md) | Nov 2 | Metrics, alerts and incidents | Independent | `modules/observability/` |
| [S17](sessions/S17.md) | Nov 2 | Unified administration frontend | Independent | `frontend/` |
| [S18](sessions/S18.md) | Nov 2 | Migration discovery and planner | Independent | `modules/migration/` |
| [S19](sessions/S19.md) | Nov 3 | Deployment workflows | Independent | `modules/deployments/` |
| [S20](sessions/S20.md) | Nov 3 | Administration and customer workflows | Independent | `modules/administration/` |
| [S21](sessions/S21.md) | Nov 3 | Node management | Independent | `modules/nodes/` |
| [S22](sessions/S22.md) | Nov 3 | WordPress management | Independent | `modules/wordpress/` |
| [S23](sessions/S23.md) | Nov 3 | Secrets and encryption | Independent | `modules/secrets/` |

### Integration and operational implementation

| ID | Target | Work package | Waits for | Exclusive paths |
|---|---|---|---|---|
| [S24](sessions/S24.md) | Nov 4 | Real node runtime | S06, S09, S15, S23 | `crates/agent/` `integration/node-runtime/` |
| [S25](sessions/S25.md) | Nov 4 | Identity/API integration | S06, S08, S23 | `integration/identity/` `crates/api/` |
| [S26](sessions/S26.md) | Nov 5 | Hosting runtime/API | S24, S25 | `integration/hosting/` `modules/hosting/` |
| [S27](sessions/S27.md) | Nov 5 | Database/backup runtime | S10, S11, S23, S24 | `integration/data/` `modules/databases/` `modules/backups/` |
| [S28](sessions/S28.md) | Nov 5 | Container runtime/API | S12, S24, S25 | `integration/containers/` `modules/containers/` |
| [S29](sessions/S29.md) | Nov 5 | DNS/mail runtime/API | S13, S14, S24, S25 | `integration/network-services/` `modules/dns/` `modules/mail/` |
| [S30](sessions/S30.md) | Nov 6 | Deployments runtime/API | S19, S28 | `integration/deployments/` `modules/deployments/` |
| [S31](sessions/S31.md) | Nov 6 | Monitoring runtime/API | S16, S24, S25 | `integration/observability/` `modules/observability/` |
| [S32](sessions/S32.md) | Nov 6 | Frontend live workflows | S17, S25, S26, S28 | `frontend/` |
| [S33](sessions/S33.md) | Nov 6 | WordPress runtime | S22, S26, S27 | `integration/wordpress/` `modules/wordpress/` |
| [S34](sessions/S34.md) | Nov 6 | Multi-node runtime | S21, S24 | `integration/nodes/` `modules/nodes/` |
| [S35](sessions/S35.md) | Nov 6 | Migration execution | S18, S26, S27 | `integration/migration/` `modules/migration/` |
| [S36](sessions/S36.md) | Nov 6 | Administration runtime | S20, S25 | `integration/administration/` `modules/administration/` |
| [S37](sessions/S37.md) | Nov 6 | Security runtime | S15, S24, S25 | `integration/security/` `modules/security/` |
| [S38](sessions/S38.md) | Nov 6 | Installer, update and recovery | S06, S24, S25 | `packaging/` |
| [S39](sessions/S39.md) | Nov 6 | Remote backup destinations | S11, S23 | `modules/backup-destinations/` |
| [S40](sessions/S40.md) | Nov 6 | Application templates and image scanning | S12, S28 | `modules/app-catalog/` |
| [S41](sessions/S41.md) | Nov 4 | CI and build qualification | S06, S07 | `scripts/ci/` `.github/workflows/` |

### End-to-end qualification

| ID | Target | Work package | Waits for | Exclusive paths |
|---|---|---|---|---|
| [S42](sessions/S42.md) | Nov 7 | Foundation and identity E2E | S07, S24, S25 | `tests/e2e/foundation/` `tests/e2e/identity/` |
| [S43](sessions/S43.md) | Nov 7 | Hosting and runtime E2E | S07, S26, S37 | `tests/e2e/hosting/` |
| [S44](sessions/S44.md) | Nov 7 | Database and restore E2E | S07, S27, S39 | `tests/e2e/data/` |
| [S45](sessions/S45.md) | Nov 7 | Containers and app catalog E2E | S07, S28, S40 | `tests/e2e/containers/` |
| [S46](sessions/S46.md) | Nov 7 | CI/CD release E2E | S07, S30 | `tests/e2e/deployments/` |
| [S47](sessions/S47.md) | Nov 7 | WordPress E2E | S07, S33 | `tests/e2e/wordpress/` |
| [S48](sessions/S48.md) | Nov 7 | Observability E2E | S07, S31 | `tests/e2e/observability/` |
| [S49](sessions/S49.md) | Nov 8 | Multi-node failure E2E | S07, S34 | `tests/e2e/nodes/` |
| [S50](sessions/S50.md) | Nov 8 | DNS and mail E2E | S07, S29 | `tests/e2e/network-services/` |
| [S51](sessions/S51.md) | Nov 8 | Administration and update E2E | S07, S36, S38 | `tests/e2e/administration/` |
| [S52](sessions/S52.md) | Nov 8 | Security and secrets qualification | S07, S23, S25, S37 | `tests/e2e/security/` `docs/qualification/security/` |
| [S53](sessions/S53.md) | Nov 8 | Migration and rollback qualification | S07, S35, S38 | `tests/e2e/migration/` `docs/qualification/migration/` |

### Coverage closure and deadline reserve

| ID | Target | Work package | Waits for | Exclusive paths |
|---|---|---|---|---|
| [S54](sessions/S54.md) | Nov 9 | Hosting/WordPress capability closure | S43, S47 | `docs/qualification/hosting/` `tests/closure/hosting/` |
| [S55](sessions/S55.md) | Nov 9 | Containers/deployments capability closure | S45, S46 | `docs/qualification/containers/` `tests/closure/containers/` |
| [S56](sessions/S56.md) | Nov 9 | Data/network capability closure | S44, S50 | `docs/qualification/data-network/` `tests/closure/data-network/` |
| [S57](sessions/S57.md) | Nov 9 | Operations/administration capability closure | S48, S49, S51, S52 | `docs/qualification/operations/` `tests/closure/operations/` |
| [S58](sessions/S58.md) | Nov 9 | Variant compatibility qualification | S32, S43, S44, S45, S46, S50 | `tests/e2e/variants/` `docs/qualification/variants/` |
| [S59](sessions/S59.md) | Nov 9 | Performance and failure budget | S42, S43, S44, S45, S49 | `tests/qualification/performance/` `docs/qualification/performance/` |
| [S60](sessions/S60.md) | Nov 9 | HA and autoscaling feasibility | S34, S49 | `docs/decisions/research/` `tests/research/` |
| [S61](sessions/S61.md) | Nov 10 | Release integration and regression | S54, S55, S56, S57, S58, S59 | `docs/releases/` `scripts/release/` |
| [S62](sessions/S62.md) | Nov 10 | Final recovery and operator handbook | S53, S61 | `docs/operations/` `docs/releases/recovery/` |
| [S63](sessions/S63.md) | Nov 11 | Final Jules correction reserve | S61, S62 | `Assigned per defect with exclusive paths` |

## Cutoff and incomplete work

On Nov 9, prioritize unresolved critical-path defects and tested feature completion over starting speculative breadth. Keep every missing feature visible in the capability ledger. On Nov 10, retrieve every completed result and save patches, branches, lockfiles, fixtures, reports and operator notes. On Nov 11, avoid long new sessions without a credible bounded completion path. The deadline does not waive security, data-integrity or recovery gates and does not authorize production changes.
