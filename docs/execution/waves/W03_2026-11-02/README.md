# Wave W03 — 60-Session Master Plan and Dispatch Register

**Repository:** `scherenhaenden/FusionPanel`  
**Base:** `master`  
**Dispatched:** 2026-11-02  
**Coordinator:** Jules Administrator (Gemini / Antigravity)  
**Policy:** Strict deduplication, zero-overlap exclusive write paths, max 15 concurrent sessions, and **every session and PR is explicitly bound to a GitHub issue**.

---

## 1. Batch 1 Register (W03-01 to W03-15) — All Merged / Active

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
| **W03-11** | S17 | Dashboard & Metrics Widgets | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `4740648715193034086` | [Session 4740648715193034086](https://jules.google.com/session/4740648715193034086) | Active (Planning) |
| **W03-12** | S17 | Websites Management UI | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `4761356683868390003` | [PR #128](https://github.com/scherenhaenden/FusionPanel/pull/128) (`b116d6c`) | Merged |
| **W03-13** | S17 | Databases Management UI | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `979988589079175966` | [PR #107](https://github.com/scherenhaenden/FusionPanel/pull/107) (`a0ed4ad`) | Merged |
| **W03-14** | S17 | Backup & Restore UI | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `17799395005981997214` | [PR #105](https://github.com/scherenhaenden/FusionPanel/pull/105) (`5f5aa40`) | Merged |
| **W03-15** | S17 | Security & Firewall UI | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) | `5421757727225165028` | [PR #104](https://github.com/scherenhaenden/FusionPanel/pull/104) (`44a188e`) | Merged |

---

## 2. Batch 2 Register (W03-16 to W03-29) — All Completed in Jules

| Slot | Package | Target Area | GitHub Issue | Jules Session ID | Direct Session Link | Jules Status | Exclusive Paths |
|---|---|---|---|---|---|---|---|
| **W03-16** | S19 | Deployments Domain & State Machine | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | `12064912187721672863` | [Session 12064912187721672863](https://jules.google.com/session/12064912187721672863) | Completed | `modules/deployments/src/domain/`, `modules/deployments/Cargo.toml`, `modules/deployments/tests/unit_domain.rs` |
| **W03-17** | S19 | Git Webhook Verification & HMAC | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | `18381417889390761015` | [Session 18381417889390761015](https://jules.google.com/session/18381417889390761015) | Completed | `modules/deployments/src/webhooks/`, `modules/deployments/tests/unit_webhooks.rs` |
| **W03-18** | S19 | Build Hooks & Pipeline Runner | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | `6629849626619255315` | [Session 6629849626619255315](https://jules.google.com/session/6629849626619255315) | Completed | `modules/deployments/src/builder/`, `modules/deployments/tests/unit_builder.rs` |
| **W03-19** | S19 | Zero-Downtime Atomic Symlink Switcher | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | `3590335955884585339` | [Session 3590335955884585339](https://jules.google.com/session/3590335955884585339) | Completed | `modules/deployments/src/switcher/`, `modules/deployments/tests/unit_switcher.rs` |
| **W03-20** | S19 | Rollback Manager & Release Inventory | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) | `15127921283247017699` | [Session 15127921283247017699](https://jules.google.com/session/15127921283247017699) | Completed | `modules/deployments/src/rollback/`, `modules/deployments/tests/unit_rollback.rs` |
| **W03-21** | S20 | Tenant & Organization Account Models | [#21](https://github.com/scherenhaenden/FusionPanel/issues/21) | `16830530553260854924` | [Session 16830530553260854924](https://jules.google.com/session/16830530553260854924) | Completed | `modules/administration/src/tenants/`, `modules/administration/Cargo.toml`, `modules/administration/tests/unit_tenants.rs` |
| **W03-22** | S20 | Quota Enforcement Engine | [#21](https://github.com/scherenhaenden/FusionPanel/issues/21) | `309367718475350135` | [Session 309367718475350135](https://jules.google.com/session/309367718475350135) | Completed | `modules/administration/src/quotas/`, `modules/administration/tests/unit_quotas.rs` |
| **W03-23** | S20 | Immutable Audit Logging Subsystem | [#21](https://github.com/scherenhaenden/FusionPanel/issues/21) | `4082645572900380697` | [Session 4082645572900380697](https://jules.google.com/session/4082645572900380697) | Completed | `modules/administration/src/audit/`, `modules/administration/tests/unit_audit.rs` |
| **W03-24** | S20 | Team Invitations & Session Revocation | [#21](https://github.com/scherenhaenden/FusionPanel/issues/21) | `8000927545476276971` | [Session 8000927545476276971](https://jules.google.com/session/8000927545476276971) | Completed | `modules/administration/src/teams/`, `modules/administration/tests/unit_teams.rs` |
| **W03-25** | S22 | WordPress Installer Orchestrator | [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) | `16007520896407336243` | [Session 16007520896407336243](https://jules.google.com/session/16007520896407336243) | Completed | `modules/wordpress/src/install/`, `modules/wordpress/Cargo.toml`, `modules/wordpress/tests/unit_install.rs` |
| **W03-26** | S22 | WP-CLI Execution Wrapper | [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) | `9032267810977640862` | [Session 9032267810977640862](https://jules.google.com/session/9032267810977640862) | Completed | `modules/wordpress/src/wp_cli/`, `modules/wordpress/tests/unit_wp_cli.rs` |
| **W03-27** | S22 | Plugin & Theme Lifecycle Manager | [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) | `14405568485447545300` | [Session 14405568485447545300](https://jules.google.com/session/14405568485447545300) | Completed | `modules/wordpress/src/plugins/`, `modules/wordpress/tests/unit_plugins.rs` |
| **W03-28** | S22 | Automated Cron & Maintenance Mode | [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) | `9366822882557400835` | [Session 9366822882557400835](https://jules.google.com/session/9366822882557400835) | Completed | `modules/wordpress/src/maintenance/`, `modules/wordpress/tests/unit_maintenance.rs` |
| **W03-29** | S22 | Security Hardening & Integrity Verifier | [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) | `10740439738794473175` | [Session 10740439738794473175](https://jules.google.com/session/10740439738794473175) | Completed | `modules/wordpress/src/hardening/`, `modules/wordpress/tests/unit_hardening.rs` |

---

## 3. Batch 3 Register (W03-30 to W03-43) — Currently Active in Jules

| Slot | Package | Target Area | GitHub Issue | Jules Session ID | Direct Session Link | Status | Exclusive Paths |
|---|---|---|---|---|---|---|---|
| **W03-30** | S39 | Storage Provider Abstraction Trait | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | `6931300648232475142` | [Session 6931300648232475142](https://jules.google.com/session/6931300648232475142) | Active | `modules/backup-destinations/src/traits/`, `modules/backup-destinations/Cargo.toml` |
| **W03-31** | S39 | S3 & Cloudflare R2 Multipart Adapter | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | `623386852888805824` | [Session 623386852888805824](https://jules.google.com/session/623386852888805824) | Active | `modules/backup-destinations/src/s3/`, `modules/backup-destinations/tests/unit_s3.rs` |
| **W03-32** | S39 | SFTP Remote Storage Adapter | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | `12171226112271889871` | [Session 12171226112271889871](https://jules.google.com/session/12171226112271889871) | Active | `modules/backup-destinations/src/sftp/`, `modules/backup-destinations/tests/unit_sftp.rs` |
| **W03-33** | S39 | Backup Sync & Manifest Verifier | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) | `13180409079065058350` | [Session 13180409079065058350](https://jules.google.com/session/13180409079065058350) | Active | `modules/backup-destinations/src/sync/`, `modules/backup-destinations/tests/unit_sync.rs` |
| **W03-34** | S40 | Application Catalog Template Schemas | [#43](https://github.com/scherenhaenden/FusionPanel/issues/43) | `7377129580795990828` | [Session 7377129580795990828](https://jules.google.com/session/7377129580795990828) | Active | `modules/app-catalog/src/schema/`, `modules/app-catalog/Cargo.toml` |
| **W03-35** | S40 | One-Click Compose Installer Service | [#43](https://github.com/scherenhaenden/FusionPanel/issues/43) | `11344501225775572989` | [Session 11344501225775572989](https://jules.google.com/session/11344501225775572989) | Active | `modules/app-catalog/src/installer/`, `modules/app-catalog/tests/unit_installer.rs` |
| **W03-36** | S40 | Container Image Vulnerability Scanner | [#43](https://github.com/scherenhaenden/FusionPanel/issues/43) | `5631393194054650549` | [Session 5631393194054650549](https://jules.google.com/session/5631393194054650549) | Active | `modules/app-catalog/src/scanner/`, `modules/app-catalog/tests/unit_scanner.rs` |
| **W03-37** | S38 | Zero-Dependency Host Installer Script | [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) | `16592631466483636733` | [Session 16592631466483636733](https://jules.google.com/session/16592631466483636733) | Active | `packaging/install.sh`, `packaging/tests/test_installer.sh` |
| **W03-38** | S38 | Systemd Service Definitions & Hardening | [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) | `3858375561761490003` | [Session 3858375561761490003](https://jules.google.com/session/3858375561761490003) | Active | `packaging/systemd/`, `packaging/tests/test_systemd.sh` |
| **W03-39** | S38 | In-Place Self-Updater & Rollback Guard | [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) | `15368396485086819994` | [Session 15368396485086819994](https://jules.google.com/session/15368396485086819994) | Active | `packaging/updater/`, `packaging/tests/unit_updater.rs` |
| **W03-40** | S24 | Node Agent Loop & Heartbeat Transport | [#26](https://github.com/scherenhaenden/FusionPanel/issues/26) | `15091886940994976566` | [Session 15091886940994976566](https://jules.google.com/session/15091886940994976566) | Active | `crates/agent/src/daemon.rs`, `crates/agent/src/heartbeat.rs` |
| **W03-41** | S24 | Sandboxed Command Executor | [#26](https://github.com/scherenhaenden/FusionPanel/issues/26) | `2735019309886798193` | [Session 2735019309886798193](https://jules.google.com/session/2735019309886798193) | Active | `crates/agent/src/executor.rs`, `crates/agent/tests/unit_executor.rs` |
| **W03-42** | S24 | Procfs System Metrics Collector | [#26](https://github.com/scherenhaenden/FusionPanel/issues/26) | `291445812890982161` | [Session 291445812890982161](https://jules.google.com/session/291445812890982161) | Active | `crates/agent/src/metrics.rs`, `crates/agent/tests/unit_metrics.rs` |
| **W03-43** | S24 | Atomic File & Permissions Manager | [#26](https://github.com/scherenhaenden/FusionPanel/issues/26) | `10483686126943246086` | [Session 10483686126943246086](https://jules.google.com/session/10483686126943246086) | Active | `crates/agent/src/files.rs`, `crates/agent/tests/unit_files.rs` |

---

## 4. Master Queue: Next 17 Sessions (W03-44 to W03-60)

| Slot | Package | Target Area | GitHub Issue | Status | Exclusive Paths |
|---|---|---|---|---|---|
| **W03-44** | S25 | Axum Authentication Middleware | [#31](https://github.com/scherenhaenden/FusionPanel/issues/31) | Queued | `integration/identity/src/middleware.rs` |
| **W03-45** | S25 | Principal & RBAC Permission Extractors | [#31](https://github.com/scherenhaenden/FusionPanel/issues/31) | Queued | `integration/identity/src/extractors.rs` |
| **W03-46** | S25 | Login, Session Refresh & Logout API | [#31](https://github.com/scherenhaenden/FusionPanel/issues/31) | Queued | `integration/identity/src/routes.rs` |
| **W03-47** | S26 | Virtual Host Provisioning Worker | [#28](https://github.com/scherenhaenden/FusionPanel/issues/28) | Queued | `integration/hosting/src/worker.rs` |
| **W03-48** | S26 | ACME HTTP-01 Let's Encrypt Automation | [#28](https://github.com/scherenhaenden/FusionPanel/issues/28) | Queued | `integration/hosting/src/acme.rs` |
| **W03-49** | S26 | Real Nginx/PHP-FPM Reload Test Suite | [#28](https://github.com/scherenhaenden/FusionPanel/issues/28) | Queued | `integration/hosting/tests/live_hosting.rs` |
| **W03-50** | S27 | Database Lifecycle API Handlers | [#29](https://github.com/scherenhaenden/FusionPanel/issues/29) | Queued | `integration/data/src/db_routes.rs` |
| **W03-51** | S27 | Automated Database Dump Scheduler | [#29](https://github.com/scherenhaenden/FusionPanel/issues/29) | Queued | `integration/data/src/backup_scheduler.rs` |
| **W03-52** | S27 | Disposable DB Dump/Restore Verification | [#29](https://github.com/scherenhaenden/FusionPanel/issues/29) | Queued | `integration/data/tests/disposable_db.rs` |
| **W03-53** | S28 | Container Lifecycle Management API | [#30](https://github.com/scherenhaenden/FusionPanel/issues/30) | Queued | `integration/containers/src/routes.rs` |
| **W03-54** | S28 | Port Allocation & Conflict Detector | [#30](https://github.com/scherenhaenden/FusionPanel/issues/30) | Queued | `integration/containers/src/ports.rs` |
| **W03-55** | S29 | Authoritative DNS Zone Provisioning API | [#32](https://github.com/scherenhaenden/FusionPanel/issues/32) | Queued | `integration/network-services/src/dns_routes.rs` |
| **W03-56** | S29 | Dovecot/Postfix Virtual Mailbox Worker | [#32](https://github.com/scherenhaenden/FusionPanel/issues/32) | Queued | `integration/network-services/src/mail_worker.rs` |
| **W03-57** | S42 | E2E Suite: Foundation & Identity Auth | [#46](https://github.com/scherenhaenden/FusionPanel/issues/46) | Queued | `tests/e2e/foundation/` |
| **W03-58** | S43 | E2E Suite: Hosting & Reverse Proxy | [#45](https://github.com/scherenhaenden/FusionPanel/issues/45) | Queued | `tests/e2e/hosting/` |
| **W03-59** | S44 | E2E Suite: Multi-DB Export & Restore | [#47](https://github.com/scherenhaenden/FusionPanel/issues/47) | Queued | `tests/e2e/data/` |
| **W03-60** | S52 | E2E Suite: Security Emergency Lockout | [#55](https://github.com/scherenhaenden/FusionPanel/issues/55) | Queued | `tests/e2e/security/` |

---

## 5. Concurrency and Governance Invariants
1. **Concurrency Cap:** Exactly 15 active sessions running concurrently in Jules.
2. **Issue Binding Rule:** Every task dispatched to Jules explicitly references `Implement https://github.com/scherenhaenden/FusionPanel/issues/<N>`, guaranteeing all branches and PRs link directly to their tracker issue.
3. **Non-overlapping Paths:** Each package and submodule operates in isolated directories without shared write paths.
4. **Regular Merges Only:** PRs are qualified with real test runs, formatted, and integrated with regular two-parent merge commits (`--merge`). No squash merging.
