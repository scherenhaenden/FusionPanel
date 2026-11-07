# Atomic Readiness Snapshot — 6 November 2026

**Baseline** `ab7cbf27d10abb7635730a6daeba8a590a65711e` (verified GitHub `master`). **Previous baseline** `0afd416bb5b74b50a46de8ab676a857b5cacd801`. **Timezone** Europe/Berlin.

| Scope | Maturity | Previous | Change | Fully accepted |
|---|---:|---:|---:|---:|
| Capabilities | 8.02% | 7.95% | +0.07 pp | 0/162 |
| Packages | 43.49% | 42.86% | +0.63 pp | 3/63 |

## Gate changes

| Row | Score | Evidence |
|---|---:|---|
| S06 Foundation integration | 20 → 30 | Root domain and agent reconciliation plus foundation documentation provide partial implementation. Standalone tests and mocked CLI do not prove integrated API/CLI operation. |
| S23 Secrets and encryption | 50 → 70 | The encrypted store met the package's file integration gate. The disposable probe verified rotation, reopen, scoped reads and backup-key recovery. Service injection and machine restore remain pending. |
| S54 Hosting/WordPress capability closure | 20 → 30 | The HOST/WP matrix and harness provide a partial audit artifact. Generic blockers and expected panics do not qualify any capability closure. |
| SEC-15 Credential encryption | 12.5 → 25 | Promoted the encrypted credential component from partial to verified using real encrypted files, scope checks, rotation, reopen and backup-key recovery. Runtime credential injection and product acceptance remain pending. |

## Product acceptance

- The running fusionpanel-api mounts /v1/health and /v1/capabilities. It does not compose the login or database routers that the MariaDB pilot needs.
- The MariaDB pilot still needs durable tenant-owned database state through the running API, a scoped SQL account, reload persistence, a cross-tenant denial check and a real SQL write/read in the disposable fixture.
- Docker socket access is blocked in this environment. Live database, mail, WordPress, and remote-backup journeys remain unqualified.
- Both CI audit scripts failed locally. Closure suites still contain expected panics and cannot qualify product behavior.
- Real CloudPanel SQLite fixture, actual migration and independent rollback, live agent movement, and isolated service injection/machine restore remain unverified.

## Merged changes

| PR or commit | Scope | Qualification |
|---|---|---|
| [#272](https://github.com/scherenhaenden/FusionPanel/pull/272) | S01, S03, S06 | Restores a domain implementation in the root workspace and revises agent protocol types. Root tests pass. The running API still lacks product router composition. |
| [#273](https://github.com/scherenhaenden/FusionPanel/pull/273) | S54, S55, S56, S57 | Adds or revises closure matrices and expected BLOCKED panics. Matrix rows and expected panics do not establish product behavior. |
| [#274](https://github.com/scherenhaenden/FusionPanel/pull/274) | S15, S37, SEC-05, SEC-16 | Refines CSP and firewall modules and their test stubs. Live host enforcement was not exercised. |
| [#275](https://github.com/scherenhaenden/FusionPanel/pull/275) | S47 | Adds WP-CLI lifecycle cases. PHP, WP-CLI, disposable database and a served WordPress journey were not qualified. |
| [#276](https://github.com/scherenhaenden/FusionPanel/pull/276) | S10, S27, DB-01, DB-02, DB-03, DB-04 | MariaDB and PostgreSQL adapters query observed database and account state. The adapter unit tests do not exercise live engines. |
| [#277](https://github.com/scherenhaenden/FusionPanel/pull/277) | S07, S44, DB-03 | Adds isolated fixture lifecycle, client invocation and generated credentials. Docker socket access is blocked locally. |
| [#278](https://github.com/scherenhaenden/FusionPanel/pull/278) | S32, DB-03, DB-04 | Adds database list/create and SQL-account UI behavior. The tests intercept HTTP with axios-mock-adapter. The main API does not mount the routes. |
| [#279](https://github.com/scherenhaenden/FusionPanel/pull/279) | S39, BAK-04, BAK-05 | Expands local, S3 and SFTP operations. Local filesystem and S3 unit cases pass. No real remote backup/restore or FTPS acceptance. |
| [#280](https://github.com/scherenhaenden/FusionPanel/pull/280) | S21, S34, S49, NODE-06, NODE-07, NODE-08 | Adds lifecycle, placement and movement planning behavior. Node library tests pass. Two real agents and site transfer remain unverified. |
| [#281](https://github.com/scherenhaenden/FusionPanel/pull/281) | S44, DB-01, DB-02, DB-04, DB-08, BAK-11 | Adds dual-engine database isolation and restore cases. Follow-up 77e08af fails closed on database connection errors. Docker access is blocked, and the suite does not establish the browser/API journey. |
| `77e08af` | S44, DB-01, DB-02, DB-04, DB-08, BAK-11 | Adds dual-engine database isolation and restore cases. Follow-up 77e08af fails closed on database connection errors. Docker access is blocked, and the suite does not establish the browser/API journey. |
| [#282](https://github.com/scherenhaenden/FusionPanel/pull/282) | S23, S52, SEC-15 | Adds AES-GCM envelope changes. File permissions, scoped access, reopening after key rotation and backup-key recovery were verified in a disposable directory. |
| [#283](https://github.com/scherenhaenden/FusionPanel/pull/283) | S41 | Adds CI lanes and validation scripts. Both new audit commands return exit 1 on the current tree. No CI qualification credit. |
| [#284](https://github.com/scherenhaenden/FusionPanel/pull/284) | S53, MIG-02, MIG-08, MIG-10, MIG-12 | Adds CloudPanel SQLite discovery and dry-run harness with an absent fixture. ReviewEvidence booleans do not perform a live migration or rollback. |
| [#285](https://github.com/scherenhaenden/FusionPanel/pull/285) | S14, S29, S50, NET-04, NET-06 | Adds recoverable account and alias synchronization with SQLite and daemon coordination. Four worker unit cases pass after ab7cbf2. Docker-backed SMTP/IMAP is blocked. |
| `ab7cbf2` | S14, S29, S50, NET-04, NET-06 | Adds recoverable account and alias synchronization with SQLite and daemon coordination. Four worker unit cases pass after ab7cbf2. Docker-backed SMTP/IMAP is blocked. |

## Checks run

| Check | Result | Evidence |
|---|---|---|
| Frontend | PASS | 12 tests across 4 suites. The database tests mock HTTP. |
| Frontend build | PASS | TypeScript and Vite production build. |
| Root Rust workspace | PASS | 41 tests across seven targets. The first sandboxed run blocked eight CLI cases at loopback bind. All passed when rerun with loopback access. CLI tests use Wiremock. |
| Database adapters | PASS | 5 unit tests. No running engine in this command. |
| Node library | PASS | 18 unit tests. No live agent transport or site movement. |
| Secrets | PASS | 14 passed and 1 service-injection placeholder filtered. File and crypto tests ran. |
| Secrets rotation probe | PASS | 1 temporary test in the archived tree. Created an encrypted 0600 record, rotated and reopened with the new key, denied a different tenant and the old key, then recovered with the backup key. The probe was outside the repository. |
| Mail worker | PASS | 4 unit tests, including one blocked-daemon behavior check. |
| Disposable mail | BLOCKED | The integration target compiled and exited on the Docker prerequisite before running SMTP or IMAP. |
| Backup destinations | PARTIAL | Local 5/5, S3 11/11, SFTP 9 reported pass and 1 failed on sandbox socket permission. Some green SFTP cases skip I/O or ignore its result. Remote storage remains unqualified. |
| Unsafe-code audit | FAIL | Exit 1. Reports missing forbid(unsafe_code) declarations in targets including the HA research tests. |
| Mock-bypass audit | FAIL | Exit 1. The script treats an #[ignore] mention in a comment in modules/migration/tests/e2e.rs as an ignored test. |
| Docker prerequisite | BLOCKED | Docker API socket permission denied. No database/WordPress/mail product journey was credited. |

## Capture and scoring

GitHub master independently matched the audited commit using git ls-remote and the GitHub compare endpoint. The first-parent delta contains 14 PR merges and two direct commits. Checks ran on an archive of the committed tree, with an existing node_modules cache for frontend checks and offline Cargo caches. Existing working-tree edits were excluded. Historical evidence fields are inherited from the previous catalog. Each snapshot_review records the current delta.

Features use component, runtime integration, real E2E and closure gates worth 25 points each, with 12.5 for a partial component. Packages use scope, implementation, unit, real integration and E2E gates worth 20 points each, with 10 for a partial implementation artifact. Averages span all catalog rows. Previous evidence is carried forward unless this delta changes it. Mocked, skipped and BLOCKED cases do not qualify real integration, E2E or closure. Delta is the difference between displayed averages.

The committed W04 registry lists 34 merged, 15 dispatched and 1 with changes requested. It lags some matching merges in this snapshot. Those entries describe the committed registry, and live Jules session status was outside this review.

[Machine-readable catalog, test commands and provenance](READINESS_SNAPSHOT_2026-11-06.json) · [Previous snapshot](READINESS_SNAPSHOT_2026-11-05.md)
