# W04 — 50-session delivery queue — 3 November 2026

Repository: `scherenhaenden/FusionPanel`. Verified remote master: `5f78a323903b5adb893edd6c0a7643c8165be710` (PR #214). All scheduling dates use Europe/Berlin.

## Delivery intent

Use the next 50 Jules sessions for recoverable bug fixes, real runtime implementation and genuine user-workflow qualification. This is a prioritized engineering queue for today, not a claim that 50 sessions or all 162 capabilities can finish today. Exact feature/mode acceptance remains evidence-gated. The first six are independently runnable on the verified master. The remaining 44 wait for actual available capacity, reviewed prerequisite code and explicit write-path handoff. Task numbers are stable identifiers, not a mandatory launch order.

## Initial six reservations

| Task | Focus | Primary issue | Exclusive paths | Session |
|---|---|---|---|---|
| W04-01 | Fail-closed migration and source-retirement decisions | [#19](https://github.com/scherenhaenden/FusionPanel/issues/19) | `modules/migration/` | [12940082909438083420](https://jules.google.com/session/12940082909438083420) ([PR #216](https://github.com/scherenhaenden/FusionPanel/pull/216)) — **MERGED** |
| W04-02 | Executable, restart-aware and recoverable signed updater | [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) | `packaging/updater/`; `packaging/tests/unit_updater.rs` | [3524266206398097751](https://jules.google.com/session/3524266206398097751) ([PR #217](https://github.com/scherenhaenden/FusionPanel/pull/217)) — **MERGED** |
| W04-03 | Crash-recoverable secret-key rotation and scoped storage | [#25](https://github.com/scherenhaenden/FusionPanel/issues/25) | `modules/secrets/` | [9133192636994590980](https://jules.google.com/session/9133192636994590980) ([PR #218](https://github.com/scherenhaenden/FusionPanel/pull/218)) — **MERGED** |
| W04-04 | Reproducible React frontend build and reachable resource views | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `frontend/` | [15842338730015189448](https://jules.google.com/session/15842338730015189448) ([PR #219](https://github.com/scherenhaenden/FusionPanel/pull/219)) — **MERGED** |
| W04-05 | Authenticated real S3 multipart and storage-adapter implementation | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | `modules/backup-destinations/` | [11664730654098869586](https://jules.google.com/session/11664730654098869586) ([PR #220](https://github.com/scherenhaenden/FusionPanel/pull/220)) — **MERGED** |
| W04-06 | Recover and requalify the deleted real test laboratory | [#10](https://github.com/scherenhaenden/FusionPanel/issues/10) | `tests/.gitignore`; `tests/README.md`; `tests/runner.py`; `tests/test_e2e.py`; `tests/test_integration.py`; `tests/test_unit.py`; `tests/fixtures/compose.py`; `tests/fixtures/docker-compose.yml`; `tests/fixtures/nginx/index.html`; `tests/lab/` | [1501694325006079677](https://jules.google.com/session/1501694325006079677) — *Awaiting User Feedback* |

The current owner reports exactly six free slots. The CLI list does not expose an authoritative capacity/quota counter; do not invent one from Completed/Awaiting rows. Only these six are authorized for immediate creation in this dispatch. No seventh session is created just because one finishes quickly.

## Second dispatch wave — 15 parallel sessions (Base: `44ded76`)

Dispatched after merging the initial completed PRs (#216, #217, #218, #219, #220). All 15 tasks have strictly disjoint write paths (zero collisions) and run in parallel against verified `master` (`44ded76`):

| Task | Focus | Primary issue | Exclusive paths | Session & Status |
|---|---|---|---|---|
| W04-08 | Hosting provisioning compensation for every failure path | [#28](https://github.com/scherenhaenden/FusionPanel/issues/28) | `integration/hosting/` | [14652888502581845168](https://jules.google.com/session/14652888502581845168) ([PR #234](https://github.com/scherenhaenden/FusionPanel/pull/234)) — **MERGED** (failure markers, rollback compensation, dynamic ports) |
| W04-10 | Contain release IDs and make deployment state transitions durable-safe | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | `modules/deployments/` | [9051421364286810987](https://jules.google.com/session/9051421364286810987) ([PR #223](https://github.com/scherenhaenden/FusionPanel/pull/223)) — **MERGED** |
| W04-11 | Transactional backup restore and ownership-aware retention | [#15](https://github.com/scherenhaenden/FusionPanel/issues/15) | `modules/backups/` | [12220664641739891816](https://jules.google.com/session/12220664641739891816) ([PR #222](https://github.com/scherenhaenden/FusionPanel/pull/222)) — **MERGED** (crash-safe journal & recovery added) |
| W04-13 | Persisted principal/session adapter and authenticated operation boundary | [#31](https://github.com/scherenhaenden/FusionPanel/issues/31) | `integration/identity/` | [18017673997336866514](https://jules.google.com/session/18017673997336866514) ([PR #224](https://github.com/scherenhaenden/FusionPanel/pull/224)) — **MERGED** |
| W04-15 | Concrete Docker engine with volumes, networks and bounded lifecycle | [#14](https://github.com/scherenhaenden/FusionPanel/issues/14) | `modules/containers/` | [11029762586239046050](https://jules.google.com/session/11029762586239046050) ([PR #226](https://github.com/scherenhaenden/FusionPanel/pull/226)) — **MERGED** (tokio async timeout, symlink containment & serde fix) |
| W04-16 | Authoritative DNS backend and complete zone lifecycle | [#16](https://github.com/scherenhaenden/FusionPanel/issues/16) | `modules/dns/` | [10193962684488937249](https://jules.google.com/session/10193962684488937249) ([PR #225](https://github.com/scherenhaenden/FusionPanel/pull/225)) — *HELD / CHANGES_REQUESTED (mock HashMap & empty DNSSEC)* |
| W04-20 | Runnable authenticated agent and bounded operation dispatcher | [#26](https://github.com/scherenhaenden/FusionPanel/issues/26) | `crates/agent/` | [17778664549880071640](https://jules.google.com/session/17778664549880071640) — *Awaiting Feedback (empty diff)* |
| W04-21 | Durable tenant-owned control-resource store | [#24](https://github.com/scherenhaenden/FusionPanel/issues/24) | `crates/control-store/` | [12903357500547035184](https://jules.google.com/session/12903357500547035184) ([PR #227](https://github.com/scherenhaenden/FusionPanel/pull/227)) — **MERGED** (strict tenant isolation boundary added) |
| W04-24 | Durable administration, teams, quotas and audit API | [#39](https://github.com/scherenhaenden/FusionPanel/issues/39) | `modules/administration/`; `integration/administration/` | [13674076978658296012](https://jules.google.com/session/13674076978658296012) ([PR #228](https://github.com/scherenhaenden/FusionPanel/pull/228)) — **MERGED** (constant-time token auth, durable revocation, server-time) |
| W04-26 | Persistent multi-node enrollment, placement and reconnection | [#37](https://github.com/scherenhaenden/FusionPanel/issues/37) | `modules/nodes/`; `integration/nodes/` | [1826829466674584062](https://jules.google.com/session/1826829466674584062) — *Failed (missing foundation/contracts)* |
| W04-28 | WordPress provisioning and maintenance runtime | [#36](https://github.com/scherenhaenden/FusionPanel/issues/36) | `modules/wordpress/`; `integration/wordpress/` | [14764772786549647809](https://jules.google.com/session/14764772786549647809) — *Failed (missing storage/runtime dependencies)* |
| W04-30 | Actual SFTP backend with credential redaction and independent restore | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | `modules/backup-destinations/` | [13964025231246527373](https://jules.google.com/session/13964025231246527373) — *Failed (missing SSH/user dependencies)* |
| W04-31 | Real template installation and vulnerability scanner execution | [#43](https://github.com/scherenhaenden/FusionPanel/issues/43) | `modules/app-catalog/` | [11546784311494421002](https://jules.google.com/session/11546784311494421002) ([PR #235](https://github.com/scherenhaenden/FusionPanel/pull/235)) — **MERGED** (hardware validation, port/secret allocation, deploy lifecycle) |
| W04-34 | Real release installer and systemd package reconciliation | [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) | `packaging/install.sh`; `packaging/systemd/`; `packaging/tests/test_installer.sh`; `packaging/tests/test_systemd.sh`; `packaging/README.md` | [16428568201938412283](https://jules.google.com/session/16428568201938412283) ([PR #230](https://github.com/scherenhaenden/FusionPanel/pull/230)) — **MERGED** (fail-closed missing binary check & 0755 enforcement) |
| W04-46 | Discover every package and enforce real CI qualification lanes | [#44](https://github.com/scherenhaenden/FusionPanel/issues/44) | `scripts/ci/`; `.github/workflows/`; `docs/testing/W04_CI_MATRIX.md` | [6581141872018750](https://jules.google.com/session/6581141872018750) ([PR #229](https://github.com/scherenhaenden/FusionPanel/pull/229)) — **MERGED** (accurate multi-suite test accounting) |

## Existing work and external handoffs

Current open-PR capture is empty. Local reconciliation `32c69cf` exists but is not merged into the verified remote master; agy owns its PR workflow. The old pending foundation/login/DNS/security/observability results must be recovered and reviewed, not relaunched as duplicate implementations.

| Gate | Existing work / required handoff |
|---|---|
| X_RECONCILIATION | Owner/agy reviews and integrates candidate 32c69cf (or a reviewed equivalent) restoring domain/observability/security and missing integration manifests. Not merged at capture; W04 does not merge it. |
| X_FOUNDATION | Recover/review existing session 7302772204230979493; explicit handoff and exact published foundation base. Do not start duplicate foundation implementation while it owns paths. |
| X_LOGIN | Recover/review existing login/session routes session 521914698062635906 and publish accepted routes before handing integration/identity to W04-13. |
| X_DNS | Recover/review pending DNS repair 234909097995010988 then hand off modules/dns. |
| X_SECURITY | Recover/review pending firewall/CSP repair 17870708631564350179 then hand off modules/security. |
| X_OBSERVABILITY | Recovered and integrated probe/webhook repair session 11993838793472947743 via [PR #221](https://github.com/scherenhaenden/FusionPanel/pull/221) (**MERGED** at `db3372c`). All 11 integration tests pass. |

Pending database sessions 15834990470205652182 and 1885562045966245180 overlap the already merged DB repairs; compare exact residual deltas rather than replaying an old full-tree patch. Session 3198712460135711531 is historical identity delivery; its duplicate PR #215 was closed as superseded. Unknown/blank provider states are unresolved, never assumed free ownership. Read-only pull results were inspected without applying any of these patches.

## All 50 bounded sessions

Every task has a saved complete prompt with its issue, allowed paths, concrete behavior, negative/real test requirements, delivery rules and capability IDs. First-six prompts contain the exact base SHA. Queued prompt files deliberately refuse dispatch until a coordinator pins a reviewed base containing their prerequisites.

| ID | Session objective | Issue | Prerequisites / handoff | Paths |
|---|---|---|---|---|
| [W04-01](prompts/W04-01.md) | Fail-closed migration and source-retirement decisions | [#19](https://github.com/scherenhaenden/FusionPanel/issues/19) | Independent | `modules/migration/` |
| [W04-02](prompts/W04-02.md) | Executable, restart-aware and recoverable signed updater | [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) | Independent | `packaging/updater/`; `packaging/tests/unit_updater.rs` |
| [W04-03](prompts/W04-03.md) | Crash-recoverable secret-key rotation and scoped storage | [#25](https://github.com/scherenhaenden/FusionPanel/issues/25) | Independent | `modules/secrets/` |
| [W04-04](prompts/W04-04.md) | Reproducible React frontend build and reachable resource views | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | Independent | `frontend/` |
| [W04-05](prompts/W04-05.md) | Authenticated real S3 multipart and storage-adapter implementation | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | Independent | `modules/backup-destinations/` |
| [W04-06](prompts/W04-06.md) | Recover and requalify the deleted real test laboratory | [#10](https://github.com/scherenhaenden/FusionPanel/issues/10) | Independent | `tests/.gitignore`; `tests/README.md`; `tests/runner.py`; `tests/test_e2e.py`; `tests/test_integration.py`; `tests/test_unit.py`; `tests/fixtures/compose.py`; `tests/fixtures/docker-compose.yml`; `tests/fixtures/nginx/index.html`; `tests/lab/` |
| [W04-07](prompts/W04-07.md) | Reconcile existing foundation result and freeze core runtime contracts | [#6](https://github.com/scherenhaenden/FusionPanel/issues/6) | X_RECONCILIATION, X_FOUNDATION, W04-06 | `Cargo.toml`; `Cargo.lock`; `.gitignore`; `crates/domain/`; `crates/agent-protocol/`; `crates/jobs/`; `crates/cli/`; `docs/foundation-integration/` |
| [W04-08](prompts/W04-08.md) | Hosting provisioning compensation for every failure path | [#28](https://github.com/scherenhaenden/FusionPanel/issues/28) | X_RECONCILIATION | `integration/hosting/` |
| [W04-09](prompts/W04-09.md) | Complete bounded ACME order and certificate lifecycle | [#28](https://github.com/scherenhaenden/FusionPanel/issues/28) | W04-08, W04-03 | `integration/hosting/src/acme.rs`; `integration/hosting/tests/unit_acme.rs`; `integration/hosting/tests/acme_lifecycle.rs` |
| [W04-10](prompts/W04-10.md) | Contain release IDs and make deployment state transitions durable-safe | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | Independent | `modules/deployments/` |
| [W04-11](prompts/W04-11.md) | Transactional backup restore and ownership-aware retention | [#15](https://github.com/scherenhaenden/FusionPanel/issues/15) | Independent | `modules/backups/` |
| [W04-12](prompts/W04-12.md) | Replace fabricated database HTTP handlers with real operations | [#29](https://github.com/scherenhaenden/FusionPanel/issues/29) | X_RECONCILIATION, W04-13, W04-21, W04-03 | `integration/data/` |
| [W04-13](prompts/W04-13.md) | Persisted principal/session adapter and authenticated operation boundary | [#31](https://github.com/scherenhaenden/FusionPanel/issues/31) | X_RECONCILIATION, X_LOGIN, W04-07 | `integration/identity/` |
| [W04-14](prompts/W04-14.md) | Principal-owned container router and durable resource mapping | [#30](https://github.com/scherenhaenden/FusionPanel/issues/30) | X_RECONCILIATION, W04-13, W04-21, W04-15 | `integration/containers/` |
| [W04-15](prompts/W04-15.md) | Concrete Docker engine with volumes, networks and bounded lifecycle | [#14](https://github.com/scherenhaenden/FusionPanel/issues/14) | Independent | `modules/containers/` |
| [W04-16](prompts/W04-16.md) | Authoritative DNS backend and complete zone lifecycle | [#16](https://github.com/scherenhaenden/FusionPanel/issues/16) | X_DNS | `modules/dns/` |
| [W04-17](prompts/W04-17.md) | Recoverable Postfix/Dovecot account and alias worker | [#32](https://github.com/scherenhaenden/FusionPanel/issues/32) | X_RECONCILIATION, W04-03 | `modules/mail/`; `integration/network-services/src/mail_worker.rs`; `integration/network-services/tests/unit_mail_worker.rs`; `integration/network-services/Cargo.toml`; `integration/network-services/src/lib.rs` |
| [W04-18](prompts/W04-18.md) | Authenticated DNS and mail resource API | [#32](https://github.com/scherenhaenden/FusionPanel/issues/32) | W04-13, W04-16, W04-17, W04-21 | `integration/network-services/src/dns_routes.rs`; `integration/network-services/src/routes.rs`; `integration/network-services/tests/unit_dns_routes.rs`; `integration/network-services/tests/resource_routes.rs`; `integration/network-services/src/lib.rs`; `integration/network-services/Cargo.toml` |
| [W04-19](prompts/W04-19.md) | Compose the running API with real authenticated resource routers | [#6](https://github.com/scherenhaenden/FusionPanel/issues/6) | W04-07, W04-08, W04-12, W04-13, W04-14, W04-18, W04-20, W04-21, W04-22, W04-23, W04-24, W04-25, W04-26, W04-27, W04-28, W04-32 | `crates/api/`; `Cargo.toml`; `Cargo.lock` |
| [W04-20](prompts/W04-20.md) | Runnable authenticated agent and bounded operation dispatcher | [#26](https://github.com/scherenhaenden/FusionPanel/issues/26) | X_RECONCILIATION, W04-07 | `crates/agent/` |
| [W04-21](prompts/W04-21.md) | Durable tenant-owned control-resource store | [#24](https://github.com/scherenhaenden/FusionPanel/issues/24) | W04-07 | `crates/control-store/` |
| [W04-22](prompts/W04-22.md) | Durable operation workers, cancellation and restart reconciliation | [#6](https://github.com/scherenhaenden/FusionPanel/issues/6) | W04-07, W04-20, W04-21 | `integration/jobs/` |
| [W04-23](prompts/W04-23.md) | Apply and recover host security policy through the agent | [#40](https://github.com/scherenhaenden/FusionPanel/issues/40) | X_RECONCILIATION, X_SECURITY, W04-13, W04-20 | `modules/security/`; `integration/security/` |
| [W04-24](prompts/W04-24.md) | Durable administration, teams, quotas and audit API | [#39](https://github.com/scherenhaenden/FusionPanel/issues/39) | W04-13, W04-21, W04-22 | `modules/administration/`; `integration/administration/` |
| [W04-25](prompts/W04-25.md) | Real monitoring collectors, durable incidents and alerts API | [#34](https://github.com/scherenhaenden/FusionPanel/issues/34) | X_RECONCILIATION, X_OBSERVABILITY, W04-13, W04-20, W04-21 | `modules/observability/`; `integration/observability/` |
| [W04-26](prompts/W04-26.md) | Persistent multi-node enrollment, placement and reconnection | [#37](https://github.com/scherenhaenden/FusionPanel/issues/37) | W04-13, W04-20, W04-21, W04-22 | `modules/nodes/`; `integration/nodes/` |
| [W04-27](prompts/W04-27.md) | Durable repository-to-release deployment runtime | [#33](https://github.com/scherenhaenden/FusionPanel/issues/33) | W04-10, W04-13, W04-15, W04-21, W04-22 | `modules/deployments/`; `integration/deployments/` |
| [W04-28](prompts/W04-28.md) | WordPress provisioning and maintenance runtime | [#36](https://github.com/scherenhaenden/FusionPanel/issues/36) | W04-08, W04-12, W04-13, W04-21, W04-22 | `modules/wordpress/`; `integration/wordpress/` |
| [W04-29](prompts/W04-29.md) | Real migration execution with reversible checkpoints | [#38](https://github.com/scherenhaenden/FusionPanel/issues/38) | W04-01, W04-03, W04-08, W04-11, W04-12, W04-21, W04-22 | `modules/migration/`; `integration/migration/` |
| [W04-30](prompts/W04-30.md) | Actual SFTP backend with credential redaction and independent restore | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | W04-05 | `modules/backup-destinations/` |
| [W04-31](prompts/W04-31.md) | Real template installation and vulnerability scanner execution | [#43](https://github.com/scherenhaenden/FusionPanel/issues/43) | W04-13, W04-15, W04-21, W04-22 | `modules/app-catalog/` |
| [W04-32](prompts/W04-32.md) | Encrypted scheduled backup and independent restore runtime | [#29](https://github.com/scherenhaenden/FusionPanel/issues/29) | W04-03, W04-05, W04-11, W04-12, W04-21, W04-22 | `integration/backups/`; `integration/data/src/backup_scheduler.rs`; `integration/data/tests/unit_backup_scheduler.rs` |
| [W04-33](prompts/W04-33.md) | Adoption and decommission attestation across real resources | [#56](https://github.com/scherenhaenden/FusionPanel/issues/56) | W04-01, W04-21, W04-29, W04-32 | `modules/adoption/`; `tests/qualification/adoption/`; `docs/qualification/adoption/` |
| [W04-34](prompts/W04-34.md) | Real release installer and systemd package reconciliation | [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) | W04-02, W04-07, W04-19, W04-20 | `packaging/install.sh`; `packaging/systemd/`; `packaging/tests/test_installer.sh`; `packaging/tests/test_systemd.sh`; `packaging/README.md` |
| [W04-35](prompts/W04-35.md) | Live frontend operations against the actual authenticated API | [#35](https://github.com/scherenhaenden/FusionPanel/issues/35) | W04-04, W04-19, W04-24, W04-25 | `frontend/` |
| [W04-36](prompts/W04-36.md) | Actual foundation, login and permission E2E | [#46](https://github.com/scherenhaenden/FusionPanel/issues/46) | W04-06, W04-13, W04-19, W04-21 | `tests/e2e/foundation/`; `tests/e2e/identity/` |
| [W04-37](prompts/W04-37.md) | Real hosting, certificate and compensation E2E | [#45](https://github.com/scherenhaenden/FusionPanel/issues/45) | W04-06, W04-08, W04-09, W04-19, W04-23, W04-36 | `tests/e2e/hosting/`; `integration/hosting/tests/live_hosting.rs` |
| [W04-38](prompts/W04-38.md) | Independent database and encrypted backup recovery E2E | [#47](https://github.com/scherenhaenden/FusionPanel/issues/47) | W04-03, W04-06, W04-11, W04-12, W04-19, W04-32, W04-36 | `tests/e2e/data/`; `tests/e2e/backups/`; `integration/data/tests/disposable_db.rs` |
| [W04-39](prompts/W04-39.md) | Docker and application catalog lifecycle E2E | [#48](https://github.com/scherenhaenden/FusionPanel/issues/48) | W04-06, W04-14, W04-15, W04-19, W04-31, W04-36 | `tests/e2e/containers/` |
| [W04-40](prompts/W04-40.md) | Live deployment and rollback continuity E2E | [#49](https://github.com/scherenhaenden/FusionPanel/issues/49) | W04-06, W04-19, W04-27, W04-36 | `tests/e2e/deployments/` |
| [W04-41](prompts/W04-41.md) | Actual WordPress lifecycle and recovery E2E | [#50](https://github.com/scherenhaenden/FusionPanel/issues/50) | W04-06, W04-19, W04-28, W04-36 | `tests/e2e/wordpress/` |
| [W04-42](prompts/W04-42.md) | Authoritative DNS and actual mailbox/relay E2E | [#54](https://github.com/scherenhaenden/FusionPanel/issues/54) | W04-06, W04-16, W04-17, W04-18, W04-19, W04-36 | `tests/e2e/network-services/`; `tests/e2e/network/` |
| [W04-43](prompts/W04-43.md) | Administration and observability outage/recovery E2E | [#53](https://github.com/scherenhaenden/FusionPanel/issues/53) | W04-06, W04-19, W04-24, W04-25, W04-36 | `tests/e2e/administration/`; `tests/e2e/observability/` |
| [W04-44](prompts/W04-44.md) | Agent and multi-node fault/reconciliation E2E | [#52](https://github.com/scherenhaenden/FusionPanel/issues/52) | W04-06, W04-19, W04-20, W04-22, W04-26, W04-36 | `tests/e2e/agent/`; `tests/e2e/nodes/` |
| [W04-45](prompts/W04-45.md) | Real security, secret injection and emergency recovery qualification | [#55](https://github.com/scherenhaenden/FusionPanel/issues/55) | W04-03, W04-06, W04-19, W04-23, W04-34, W04-36 | `tests/e2e/security/`; `docs/qualification/security/` |
| [W04-46](prompts/W04-46.md) | Discover every package and enforce real CI qualification lanes | [#44](https://github.com/scherenhaenden/FusionPanel/issues/44) | W04-04, W04-06, W04-07, W04-19 | `scripts/ci/`; `.github/workflows/`; `docs/testing/W04_CI_MATRIX.md` |
| [W04-47](prompts/W04-47.md) | Browser and supported variant compatibility qualification | [#35](https://github.com/scherenhaenden/FusionPanel/issues/35) | W04-19, W04-35, W04-36, W04-37, W04-38, W04-39, W04-42, W04-46 | `tests/e2e/variants/`; `tests/e2e/browser/`; `docs/qualification/variants/` |
| [W04-48](prompts/W04-48.md) | Replace artificial closure greens with capability-linked evidence | [#58](https://github.com/scherenhaenden/FusionPanel/issues/58) | W04-33, W04-36, W04-37, W04-38, W04-39, W04-40, W04-41, W04-42, W04-43, W04-44, W04-45, W04-47 | `tests/closure/`; `docs/qualification/containers/`; `docs/qualification/data-network/`; `docs/qualification/operations/`; `docs/qualification/hosting/` |
| [W04-49](prompts/W04-49.md) | Measure real performance and failure/recovery budgets | [#62](https://github.com/scherenhaenden/FusionPanel/issues/62) | W04-19, W04-34, W04-36, W04-38, W04-39, W04-40, W04-44, W04-46 | `tests/qualification/performance/`; `docs/qualification/performance/` |
| [W04-50](prompts/W04-50.md) | Integrated release, independent machine recovery and evidence handoff | [#63](https://github.com/scherenhaenden/FusionPanel/issues/63) | W04-33, W04-34, W04-36, W04-37, W04-38, W04-39, W04-40, W04-41, W04-42, W04-43, W04-44, W04-45, W04-46, W04-47, W04-48, W04-49 | `scripts/release/`; `docs/releases/`; `docs/operations/`; `docs/execution/waves/W04_2026-11-03/final/` |

## Scheduling policy after the first six

1. Refresh provider session states and actual owner-reported free capacity. Recover completed artifacts rather than creating another session for a missing PR. A creation timeout/error is ambiguous until reconciled against the provider list; never blindly retry it.
2. Select only undispatched tasks whose prerequisites are reviewed and present at one exact candidate/master SHA, and whose allowed paths are free. Completed/PR-open is not merged or accepted. Do not auto-merge prerequisites or bypass reviews.
3. Recheck the current remote base, published callable contracts and path ownership, replace the queued prompt base paragraph with the exact SHA, then record the resulting prompt hash. Do not use the unresolved queued templates directly.
4. Prefer ready tasks with the greatest critical-path impact; do not follow numeric order when a lower-numbered task is waiting for a higher-numbered prerequisite. Independently ready follow-ups include W04-10 (release containment), W04-11 (restore transaction) and W04-15 (real Docker engine). These are queued, not dispatched.
5. Never run two owners of a shared path together. The JSON lists all ten shared-path pairs and transitive dependency handoffs. Root manifests/lock have explicit sequential owners (W04-07 then W04-19); CI does not silently rewrite them.
6. W04-19 is the composition gate after the domain routers/agent/store/workers have been implemented. Its prompt integrates published interfaces, not a second implementation of each domain. W04-35/36 onward then exercise the running product.
7. Fixtures use task-owned package test directories. Full OS, two-node, public domain/provider and GPU cases remain explicit blockers without the required lab; do not spend on new hosting/providers or touch production to make a lane green.
8. Keep all results as branches/PRs, exact SHAs, retained patches and actual test logs. Owner review/agy handles integration. Sessions never merge or close issues. Shared ledgers are updated by coordination after evidence review.

## Critical dependency chains

- Recovery/lab + foundation handoff → core contracts → durable store / authenticated agent → operation workers.
- Existing login handoff → current persisted principal contract → real tenant-owned domain APIs.
- Real adapters + durable resource ownership + real workers → W04-19 running API → live frontend and actual E2E.
- Real restore, migration and scoped ownership → adoption/decommission attestations.
- Real E2E + CI + variants → capability-linked closure + measured failure budgets → signed install/recovery release gate.

## Acceptance and scope continuity

A passing compile/unit test is component evidence. A genuine resource assertion after API/job/agent/service execution is runtime/E2E evidence. Source-only, expected BLOCKED panics, assert(true), ignored prerequisites and HTTP 201 without effects cannot close a capability. Keep each canonical catalog feature and useful mode visible; unimplemented breadth (including unsupported HA/autoscaling and unqualified provider/hardware variants) remains open. This wave does not waive the original 162-feature commitment or silently reinterpret 50 sessions as 100% delivery.

## Artifacts

- [Machine-readable queue and dispatch register](tasks.json).
- [All full prompts](prompts/).
- [Provider status before dispatch](sessions-before.txt), [open PR capture](open-prs-before.json), [open issues](open-issues-before.json).
- Per-session creation logs are stored beside this file; session status means provider state, not product acceptance.
