# Real-System Test Strategy

## Evidence rules

Unit tests verify domain invariants and negative paths. Integration tests verify actual dependency behavior across process/storage boundaries. End-to-end tests (E2E, also called n2n here) exercise actual user workflows from browser or CLI through API, authorization, jobs, node agent and real service/data effects. Do not label a request against a fake server as product E2E.

Every run records commit, OS/container/tool versions, fixture seed/hash, mode, commands, timestamps, expected/observed outcomes, stdout/stderr/logs with redaction, exit status, cleanup and artifact locations. PASS, FAIL, BLOCKED and NOT_RUN are distinct. A missing binary, Docker socket, VM, credential or domain is BLOCKED; mandatory lanes fail qualification when blocked. No skip-on-error test is allowed to turn an unavailable dependency into a green release.

## Unit suite

Validation, tenant ownership, lifecycle transitions, idempotency, safe paths, serialization, config generation, policy decisions and crypto boundary misuse. Include malformed Unicode/labels, oversized input, null/traversal/symlink cases, duplicate claims, terminal-state transitions and redaction canaries. Test expected outcomes rather than reproducing implementation code.

## Real integration suite

Use actual SQLite reopen/concurrent connections; real MariaDB/PostgreSQL; Nginx/PHP syntax and serving; real Docker daemon/Compose, filesystem archives and independent restore; MinIO/SFTP/FTPS as applicable; authoritative DNS queries; SMTP/IMAP roundtrip; HTTP/TCP outage targets; actual CLI/API processes and sockets. Pin service versions/digests when fixtures are implemented and preserve their logs.

Mocks are appropriate for deterministic API failure/retry/client parsing cases. Label those as boundary tests. A local SMTP capture service demonstrates message submission only, not hosted mailbox IMAP support, DKIM validation or external deliverability. S3 compatibility proves the tested compatible endpoint only; native provider support requires provider-specific evidence.

## Full OS and browser suite

Use disposable Debian VMs for systemd, root agent, firewall, SSH, kernel quotas, reboot, install/update and machine restore. Never run host mutation tests on the operator workstation or production. Browser tests use the actual API and backend; no request interception supplying fake success in an acceptance lane. Assert visible outcomes plus real HTTP content, DB rows, files, ports, permissions, certificates and logs where relevant.

## Failure and recovery suite

Interrupt process/node/network during operations; restart workers; exhaust a bounded fixture disk; corrupt a backup; revoke credentials; fail release health checks; inject unsafe paths; cancel jobs; restore to another target. Verify no duplicate external effects, unauthorized access or silent data loss. Measure RTO/RPO and explain their configuration/hardware context. Whole-server rollback needs independent machine recovery and post-cutover-write reconciliation.

## Test environment tiers

| Tier | Dependencies | Purpose |
|---|---|---|
| T0 | Rust/JS/Python tooling | Fast unit and static/contract checks. |
| T1 | Actual binaries and local temp stores/sockets | API/CLI/job/protocol integration. |
| T2 | Disposable Docker/Compose services | Real database, web, containers, DNS/mail/store adapters. |
| T3 | Disposable full OS VMs, two nodes for cluster tests | Privileges, services, reboot, quotas, firewall and recovery. |
| T4 | Controlled public domains/providers/hardware | Public TLS, external mail/DNS/cloud/GPU qualification. |

T4 requires owner-provided resources and bounded credentials; do not silently substitute a mock or incur unapproved costs. Record exact missing prerequisites. GPU qualification is hardware/vendor-specific. Public CA tests use staging first and respect issuer limits.

## Required artifacts

JUnit or equivalent machine-readable test reports, sanitized service logs, screenshots/traces for browser failures, DB/file comparison reports, restore manifests, capability/mode evidence links and measured failure/recovery times. Upload bounded artifacts in CI; do not commit secrets, dumps or personal data to the public repository. Use synthetic example names and isolated generated credentials.

## Release gate

Every committed capability and mode has applicable unit, real integration and E2E evidence. No unresolved critical authorization/data-integrity/recovery defect; clean install/update/recovery verified; unsupported behavior explicit; no success claim based solely on stub response, mock, upstream description or session status. Remaining gaps stay visible and prevent blanket full-coverage claims.
