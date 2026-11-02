# Wave W03 — 60-Session Master Plan and Dispatch Register

**Repository:** `scherenhaenden/FusionPanel`  
**Base:** `master`  
**Dispatched:** 2026-11-02  
**Coordinator:** Jules Administrator (Gemini / Antigravity)  
**Policy:** Strict deduplication, zero-overlap exclusive write paths, max 15 concurrent sessions, and **every session and PR is explicitly bound to a GitHub issue**.

---

## 1. Batch 1 Register (W03-01 to W03-15) — All Merged

| Slot | Package | Target Area | GitHub Issue | Jules Session ID | PR & Merge Commit | Status |
|---|---|---|---|---|---|---|
| **W03-01** | S12 | Container Domain & Engine Trait | [#14](https://github.com/scherenhaenden/FusionPanel/issues/14) | `6707431377717910594` | [PR #118](https://github.com/scherenhaenden/FusionPanel/pull/118) (`55fee9a`) | Merged |
| **W03-02** | S12 | Compose Parser & Validator | [#14](https://github.com/scherenhaenden/FusionPanel/issues/14) | `14026426912816606450` | [PR #119](https://github.com/scherenhaenden/FusionPanel/pull/119) (`d223c78`) | Merged |
| **W03-03** | S12 | Volumes & Resource Quotas | [#14](https://github.com/scherenhaenden/FusionPanel/issues/14) | `14125981171241757598` | [PR #120](https://github.com/scherenhaenden/FusionPanel/pull/120) (`c07268c`) | Merged |
| **W03-04** | S12 | Health Checker & Log Tailer | [#14](https://github.com/scherenhaenden/FusionPanel/issues/14) | `16424821635117386360` | [PR #121](https://github.com/scherenhaenden/FusionPanel/pull/121) (`c3a7460`) | Merged |
| **W03-05** | S14 | Mail Domain & Mailboxes | [#17](https://github.com/scherenhaenden/FusionPanel/issues/17) | `8065715395383877443` | [PR #122](https://github.com/scherenhaenden/FusionPanel/pull/122) (`df0a879`) | Merged |
| **W03-06** | S14 | Postfix & Dovecot Configs | [#17](https://github.com/scherenhaenden/FusionPanel/issues/17) | `11773659808044753093` | [PR #123](https://github.com/scherenhaenden/FusionPanel/pull/123) (`c3593d0`) | Merged |
| **W03-07** | S14 | Mail DNS & DKIM Generator | [#17](https://github.com/scherenhaenden/FusionPanel/issues/17) | `15977681818578176017` | [PR #124](https://github.com/scherenhaenden/FusionPanel/pull/124) (`fd7f80d`) | Merged |
| **W03-08** | S14 | Aliases, Forwards & Catch-All | [#17](https://github.com/scherenhaenden/FusionPanel/issues/17) | `15480425042024234109` | [PR #125](https://github.com/scherenhaenden/FusionPanel/pull/125) (`a32c9bc`) | Merged |
| **W03-09** | S17 | Frontend React + Vite Scaffolding | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `2165485427729566278` | [PR #126](https://github.com/scherenhaenden/FusionPanel/pull/126) (`16fdf17`) | Merged |
| **W03-10** | S17 | Typed API Client & Auth Store | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `15711557563974781875` | [PR #127](https://github.com/scherenhaenden/FusionPanel/pull/127) (`2c15075`) | Merged |
| **W03-11** | S17 | Dashboard & Metrics Widgets | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `4740648715193034086` | [Session 4740648715193034086](https://jules.google.com/session/4740648715193034086) | Archived |
| **W03-12** | S17 | Websites Management UI | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `4761356683868390003` | [PR #128](https://github.com/scherenhaenden/FusionPanel/pull/128) (`b116d6c`) | Merged |
| **W03-13** | S17 | Databases Management UI | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `979988589079175966` | [PR #107](https://github.com/scherenhaenden/FusionPanel/pull/107) (`a0ed4ad`) | Merged |
| **W03-14** | S17 | Backup & Restore UI | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `17799395005981997214` | [PR #105](https://github.com/scherenhaenden/FusionPanel/pull/105) (`5f5aa40`) | Merged |
| **W03-15** | S17 | Security & Firewall UI | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `5421757727225165028` | [PR #104](https://github.com/scherenhaenden/FusionPanel/pull/104) (`44a188e`) | Merged |

---

## 2. Batch 2 Register (W03-16 to W03-29) — All Merged

| Slot | Package | Target Area | GitHub Issue | Jules Session ID | PR & Merge Commit | Status | Tests Verified |
|---|---|---|---|---|---|---|---|
| **W03-16** | S19 | Deployments Domain & State Machine | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | `12064912187721672863` | [PR #134](https://github.com/scherenhaenden/FusionPanel/pull/134) (`295b5c4`) | Merged | 3 passed |
| **W03-17** | S19 | Git Webhook Verification & HMAC | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | `18381417889390761015` | [PR #136](https://github.com/scherenhaenden/FusionPanel/pull/136) (`2d03e95`) | Merged | 4 passed |
| **W03-18** | S19 | Build Hooks & Pipeline Runner | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | `6629849626619255315` | [PR #136](https://github.com/scherenhaenden/FusionPanel/pull/136) (`2d03e95`) | Merged | 5 passed |
| **W03-19** | S19 | Zero-Downtime Atomic Symlink Switcher | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | `3590335955884585339` | [PR #136](https://github.com/scherenhaenden/FusionPanel/pull/136) (`2d03e95`) | Merged | 1 passed |
| **W03-20** | S19 | Rollback Manager & Release Inventory | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | `15127921283247017699` | [PR #136](https://github.com/scherenhaenden/FusionPanel/pull/136) (`2d03e95`) | Merged | 3 passed |
| **W03-21** | S20 | Tenant & Organization Account Models | [#21](https://github.com/scherenhaenden/FusionPanel/issues/21) | `16830530553260854924` | [PR #137](https://github.com/scherenhaenden/FusionPanel/pull/137) (`5b3eb32`) | Merged | 3 passed |
| **W03-22** | S20 | Quota Enforcement Engine | [#21](https://github.com/scherenhaenden/FusionPanel/issues/21) | `309367718475350135` | [PR #137](https://github.com/scherenhaenden/FusionPanel/pull/137) (`5b3eb32`) | Merged | 7 passed |
| **W03-23** | S20 | Immutable Audit Logging Subsystem | [#21](https://github.com/scherenhaenden/FusionPanel/issues/21) | `4082645572900380697` | [PR #137](https://github.com/scherenhaenden/FusionPanel/pull/137) (`5b3eb32`) | Merged | 3 passed |
| **W03-24** | S20 | Team Invitations & Session Revocation | [#21](https://github.com/scherenhaenden/FusionPanel/issues/21) | `8000927545476276971` | [PR #137](https://github.com/scherenhaenden/FusionPanel/pull/137) (`5b3eb32`) | Merged | 5 passed |
| **W03-25** | S22 | WordPress Installer Orchestrator | [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) | `16007520896407336243` | [PR #138](https://github.com/scherenhaenden/FusionPanel/pull/138) (`f15e33e`) | Merged | 4 passed |
| **W03-26** | S22 | WP-CLI Execution Wrapper | [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) | `9032267810977640862` | [PR #138](https://github.com/scherenhaenden/FusionPanel/pull/138) (`f15e33e`) | Merged | 5 passed |
| **W03-27** | S22 | Plugin & Theme Lifecycle Manager | [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) | `14405568485447545300` | [PR #138](https://github.com/scherenhaenden/FusionPanel/pull/138) (`f15e33e`) | Merged | 13 passed |
| **W03-28** | S22 | Automated Cron & Maintenance Mode | [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) | `9366822882557400835` | [PR #138](https://github.com/scherenhaenden/FusionPanel/pull/138) (`f15e33e`) | Merged | 4 passed |
| **W03-29** | S22 | Security Hardening & Integrity Verifier | [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) | `10740439738794473175` | [PR #138](https://github.com/scherenhaenden/FusionPanel/pull/138) (`f15e33e`) | Merged | 5 passed |

---

## 3. Batch 3 Register (W03-30 to W03-43) — All Merged

| Slot | Package | Target Area | GitHub Issue | Jules Session ID | PR & Merge Commit | Status | Tests Verified |
|---|---|---|---|---|---|---|---|
| **W03-30** | S39 | Storage Provider Abstraction Trait | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | `6931300648232475142` | [PR #139](https://github.com/scherenhaenden/FusionPanel/pull/139) (`c2eb4ed`) | Merged | 11 passed |
| **W03-31** | S39 | S3 & Cloudflare R2 Multipart Adapter | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | `623386852888805824` | [PR #139](https://github.com/scherenhaenden/FusionPanel/pull/139) (`c2eb4ed`) | Merged | 9 passed |
| **W03-32** | S39 | SFTP Remote Storage Adapter | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | `12171226112271889871` | [PR #139](https://github.com/scherenhaenden/FusionPanel/pull/139) (`c2eb4ed`) | Merged | 4 passed |
| **W03-33** | S39 | Backup Sync & Manifest Verifier | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | `13180409079065058350` | [PR #139](https://github.com/scherenhaenden/FusionPanel/pull/139) (`c2eb4ed`) | Merged | 4 passed |
| **W03-34** | S40 | Application Catalog Template Schemas | [#43](https://github.com/scherenhaenden/FusionPanel/issues/43) | `7377129580795990828` | [PR #135](https://github.com/scherenhaenden/FusionPanel/pull/135) (`8256530`) | Merged | 5 passed |
| **W03-35** | S40 | One-Click Compose Installer Service | [#43](https://github.com/scherenhaenden/FusionPanel/issues/43) | `11344501225775572989` | [PR #133](https://github.com/scherenhaenden/FusionPanel/pull/133) (`bca062c`) | Merged | 4 passed |
| **W03-36** | S40 | Container Image Vulnerability Scanner | [#43](https://github.com/scherenhaenden/FusionPanel/issues/43) | `5631393194054650549` | [PR #140](https://github.com/scherenhaenden/FusionPanel/pull/140) (`a517a3e`) | Merged | 4 passed |
| **W03-37** | S38 | Zero-Dependency Host Installer Script | [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) | `16592631466483636733` | [PR #132](https://github.com/scherenhaenden/FusionPanel/pull/132) (`ea32d54`) | Merged | Shell passed |
| **W03-38** | S38 | Systemd Service Definitions & Hardening | [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) | `3858375561761490003` | [PR #141](https://github.com/scherenhaenden/FusionPanel/pull/141) (`b949872`) | Merged | Systemd-analyze verify |
| **W03-39** | S38 | In-Place Self-Updater & Rollback Guard | [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) | `15368396485086819994` | [PR #131](https://github.com/scherenhaenden/FusionPanel/pull/131) (`26deafd`) | Merged | 3 passed |
| **W03-40** | S24 | Node Agent Loop & Heartbeat Transport | [#26](https://github.com/scherenhaenden/FusionPanel/issues/26) | `15091886940994976566` | [PR #130](https://github.com/scherenhaenden/FusionPanel/pull/130) (`d1d005e`) | Merged | 3 passed |
| **W03-41** | S24 | Sandboxed Command Executor | [#26](https://github.com/scherenhaenden/FusionPanel/issues/26) | `2735019309886798193` | [PR #142](https://github.com/scherenhaenden/FusionPanel/pull/142) (`ec79533`) | Merged | 4 passed |
| **W03-42** | S24 | Procfs System Metrics Collector | [#26](https://github.com/scherenhaenden/FusionPanel/issues/26) | `291445812890982161` | [PR #129](https://github.com/scherenhaenden/FusionPanel/pull/129) (`2269695`) | Merged | 1 passed |
| **W03-43** | S24 | Atomic File & Permissions Manager | [#26](https://github.com/scherenhaenden/FusionPanel/issues/26) | `10483686126943246086` | [PR #142](https://github.com/scherenhaenden/FusionPanel/pull/142) (`ec79533`) | Merged | 6 passed |

---

## 4. Batch 4 Register (W03-44 to W03-57) — Ready for Pull & Integration

| Slot | Package | Target Area | GitHub Issue | Jules Session ID | Direct Session Link | Jules Status | Exclusive Paths |
|---|---|---|---|---|---|---|---|
| **W03-44** | S25 | Axum Authentication Middleware | [#31](https://github.com/scherenhaenden/FusionPanel/issues/31) | `5944340483820829324` | [Session 5944340483820829324](https://jules.google.com/session/5944340483820829324) | Completed | `integration/identity/src/middleware.rs` |
| **W03-45** | S25 | Principal & RBAC Permission Extractors | [#31](https://github.com/scherenhaenden/FusionPanel/issues/31) | `2000124174540998410` | [Session 2000124174540998410](https://jules.google.com/session/2000124174540998410) | Completed | `integration/identity/src/extractors.rs` |
| **W03-46** | S25 | Login, Session Refresh & Logout API | [#31](https://github.com/scherenhaenden/FusionPanel/issues/31) | `521914698062635906` | [Session 521914698062635906](https://jules.google.com/session/521914698062635906) | Awaiting | `integration/identity/src/routes.rs` |
| **W03-47** | S26 | Virtual Host Provisioning Worker | [#28](https://github.com/scherenhaenden/FusionPanel/issues/28) | `9640868149248492827` | [Session 9640868149248492827](https://jules.google.com/session/9640868149248492827) | Completed | `integration/hosting/src/worker.rs` |
| **W03-48** | S26 | ACME HTTP-01 Let's Encrypt Automation | [#28](https://github.com/scherenhaenden/FusionPanel/issues/28) | `16633911755693231490` | [Session 16633911755693231490](https://jules.google.com/session/16633911755693231490) | Completed | `integration/hosting/src/acme.rs` |
| **W03-49** | S26 | Real Nginx/PHP-FPM Reload Test Suite | [#28](https://github.com/scherenhaenden/FusionPanel/issues/28) | `9895857453157935083` | [Session 9895857453157935083](https://jules.google.com/session/9895857453157935083) | Completed | `integration/hosting/tests/live_hosting.rs` |
| **W03-50** | S27 | Database Lifecycle API Handlers | [#29](https://github.com/scherenhaenden/FusionPanel/issues/29) | `14941793878173461840` | [Session 14941793878173461840](https://jules.google.com/session/14941793878173461840) | Completed | `integration/data/src/db_routes.rs` |
| **W03-51** | S27 | Automated Database Dump Scheduler | [#29](https://github.com/scherenhaenden/FusionPanel/issues/29) | `3497453922540298791` | [Session 3497453922540298791](https://jules.google.com/session/3497453922540298791) | Completed | `integration/data/src/backup_scheduler.rs` |
| **W03-52** | S27 | Disposable DB Dump/Restore Verification | [#29](https://github.com/scherenhaenden/FusionPanel/issues/29) | `15624981679065060806` | [Session 15624981679065060806](https://jules.google.com/session/15624981679065060806) | Completed | `integration/data/tests/disposable_db.rs` |
| **W03-53** | S28 | Container Lifecycle Management API | [#30](https://github.com/scherenhaenden/FusionPanel/issues/30) | `3673089590692342583` | [Session 3673089590692342583](https://jules.google.com/session/3673089590692342583) | Completed | `integration/containers/src/routes.rs` |
| **W03-54** | S28 | Port Allocation & Conflict Detector | [#30](https://github.com/scherenhaenden/FusionPanel/issues/30) | `4035471101206234350` | [Session 4035471101206234350](https://jules.google.com/session/4035471101206234350) | Completed | `integration/containers/src/ports.rs` |
| **W03-55** | S29 | Authoritative DNS Zone Provisioning API | [#32](https://github.com/scherenhaenden/FusionPanel/issues/32) | `10741532246894175995` | [Session 10741532246894175995](https://jules.google.com/session/10741532246894175995) | Completed | `integration/network-services/src/dns_routes.rs` |
| **W03-56** | S29 | Dovecot/Postfix Virtual Mailbox Worker | [#32](https://github.com/scherenhaenden/FusionPanel/issues/32) | `8503113406841753593` | [Session 8503113406841753593](https://jules.google.com/session/8503113406841753593) | Completed | `integration/network-services/src/mail_worker.rs` |
| **W03-57** | S42 | E2E Suite: Foundation & Identity Auth | [#46](https://github.com/scherenhaenden/FusionPanel/issues/46) | `16192902824867794107` | [Session 16192902824867794107](https://jules.google.com/session/16192902824867794107) | Completed | `tests/e2e/foundation/` |

---

## 5. Batch 5 Register (W03-58 to W03-70) — Dispatched & Active in Jules

| Slot | Package | Target Area | GitHub Issue | Jules Session ID | Direct Session Link | Jules Status | Exclusive Paths |
|---|---|---|---|---|---|---|---|
| **W03-58** | S43 | E2E Suite: Hosting & Reverse Proxy | [#45](https://github.com/scherenhaenden/FusionPanel/issues/45) | `2507150338887815722` | [Session 2507150338887815722](https://jules.google.com/session/2507150338887815722) | Completed | `tests/e2e/hosting/` |
| **W03-59** | S44 | E2E Suite: Multi-DB & Containers | [#47](https://github.com/scherenhaenden/FusionPanel/issues/47) | `17589056097592185599` | [Session 17589056097592185599](https://jules.google.com/session/17589056097592185599) | Completed | `tests/e2e/data/` |
| **W03-60** | S52 | E2E Suite: Security Emergency Lockout | [#55](https://github.com/scherenhaenden/FusionPanel/issues/55) | `9888000411726813432` | [Session 9888000411726813432](https://jules.google.com/session/9888000411726813432) | Completed | `tests/e2e/security/` |
| **W03-61** | S45 | E2E Suite: Zero-Downtime Deployments | [#48](https://github.com/scherenhaenden/FusionPanel/issues/48) | `5308925537194527914` | [Session 5308925537194527914](https://jules.google.com/session/5308925537194527914) | Completed | `tests/e2e/deployments/` |
| **W03-62** | S46 | E2E Suite: WordPress Lifecycle | [#49](https://github.com/scherenhaenden/FusionPanel/issues/49) | `4997095032005739898` | [Session 4997095032005739898](https://jules.google.com/session/4997095032005739898) | Completed | `tests/e2e/wordpress/` |
| **W03-63** | S47 | E2E Suite: Backup Recovery | [#50](https://github.com/scherenhaenden/FusionPanel/issues/50) | `7252582386228411838` | [Session 7252582386228411838](https://jules.google.com/session/7252582386228411838) | Completed | `tests/e2e/backups/` |
| **W03-64** | S48 | E2E Suite: Node Agent Reconnect | [#51](https://github.com/scherenhaenden/FusionPanel/issues/51) | `1937858837463565545` | [Session 1937858837463565545](https://jules.google.com/session/1937858837463565545) | Completed | `tests/e2e/agent/` |
| **W03-65** | S49 | E2E Suite: Multi-Node Failure & Reschedule | [#52](https://github.com/scherenhaenden/FusionPanel/issues/52) | `14519425713916536351` | [Session 14519425713916536351](https://jules.google.com/session/14519425713916536351) | Completed | `tests/e2e/nodes/` |
| **W03-66** | S50 | E2E Suite: DNS & Mail Forwarding | [#54](https://github.com/scherenhaenden/FusionPanel/issues/54) | `7385074516211862461` | [Session 7385074516211862461](https://jules.google.com/session/7385074516211862461) | Re-queue | `tests/e2e/network/` |
| **W03-67** | S51 | E2E Suite: Admin Quotas & Updater | [#53](https://github.com/scherenhaenden/FusionPanel/issues/53) | `8149208741547436572` | [Session 8149208741547436572](https://jules.google.com/session/8149208741547436572) | Completed | `tests/e2e/admin/` |
| **W03-68** | Audit | S09 Hosting Validation & Port Conflicts | [#91](https://github.com/scherenhaenden/FusionPanel/issues/91) | `86234627996270397` | [Session 86234627996270397](https://jules.google.com/session/86234627996270397) | Completed | `modules/hosting/` |
| **W03-69** | Audit | S10 Database Collation & Transaction Isolation | [#92](https://github.com/scherenhaenden/FusionPanel/issues/92) | `15834990470205652182` | [Session 15834990470205652182](https://jules.google.com/session/15834990470205652182) | In Progress | `modules/databases/` |
| **W03-70** | Audit | S11 Backup Containment & Compression | [#93](https://github.com/scherenhaenden/FusionPanel/issues/93) | `1025120005518045429` | [Session 1025120005518045429](https://jules.google.com/session/1025120005518045429) | Completed | `modules/backups/` |

---

## 6. Concurrency and Governance Invariants
1. **Concurrency Cap:** Exactly 15 active sessions running concurrently in Jules.
2. **Issue Binding Rule:** Every task dispatched to Jules explicitly references `Implement https://github.com/scherenhaenden/FusionPanel/issues/<N>`, guaranteeing all branches and PRs link directly to their tracker issue.
3. **Non-overlapping Paths:** Each package and submodule operates in isolated directories without shared write paths.
4. **Regular Merges Only:** PRs are qualified with real test runs, formatted, and integrated with regular two-parent merge commits (`--merge`). No squash merging.
