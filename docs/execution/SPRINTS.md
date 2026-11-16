# FusionPanel — Project Delivery Roadmap & Sprint Status

Source of truth for active engineering execution, sprint governance, quality gates, and end-to-end product readiness. Updated at the close of each sprint.

---

## 1. Workflow & Governance Rules (Mandatory)

```text
master                                (verified sprint releases only)
  └── sprint-<N>                      (sprint integration branch; NO direct feature commits)
        ├── feat/sprint-<N>-<topic-1> ──(local unit tests only)──▶ PR into sprint-<N>
        ├── feat/sprint-<N>-<topic-2> ──(local unit tests only)──▶ PR into sprint-<N>
        └── fix/sprint-<N>-<bug>      ──(local unit tests only)──▶ PR into sprint-<N>
              │
              ▼  [All atomic sprint PRs merged into sprint-<N>]
        Run full integration + live E2E + quality gates on sprint-<N> (+ fix PRs if needed)
        Update docs/execution/SPRINTS.md & related docs
              │
              ▼
        Single PR: sprint-<N> ──▶ master
```

1. **Sprint Integration Branch (`sprint-<N>`)**: Each sprint starts by branching `sprint-<N>` from `master` (and pushing `origin/sprint-<N>`). Feature code must **never** be committed directly onto `sprint-<N>`.
2. **Atomic Feature/Fix Branches (`feat/*`, `fix/*`, `docs/*`) -> `sprint-<N>`**:
   - Plan each sprint into small, independent atomic branches created from `sprint-<N>`.
   - On each atomic branch, run **only local unit tests** for the touched crate/module/view (do not run heavy workspace/E2E suites for every small change unless specifically needed).
   - Once unit tests pass, open an atomic PR **targeting `sprint-<N>`** (never targeting `master`).
3. **Sprint Close, Quality Gates & PR to `master`**:
   - Once all planned atomic PRs are merged into `sprint-<N>`, execute the full **workspace integration tests + live E2E verification + quality gates** on `sprint-<N>`. If failures occur, land fix PRs into `sprint-<N>` until green.
   - Update this document with exact verification evidence and feature readiness status.
   - Only when `sprint-<N>` is 100% green and documented, open the single sprint release PR **`sprint-<N>` -> `master`**.
4. **Operational Safety**: Never `push`, `--force`, or `merge` pre-existing/remote user branches without explicit approval.
5. **Priority Scope**: Deliver 100% end-to-end usable core infrastructure features first. **PHP management is explicitly deprioritized.**

---

## 2. Critical Path & Sprint Dependencies

```text
[Sprint 1: Auth + Unified API + UI + DKIM Prober] (DONE - PR #574)
        │
        ▼  (unlocks real host execution)
[Sprint 2: Node Agent + Jobs Runtime] ──────────────────────────────┐
        │                                                           │
        ├──▶ [Sprint 3: Websites + Reverse Proxy]                   ├──▶ [Sprint 8: File Manager + Cron]
        │           │                                               │
        │           ▼                                               ├──▶ [Sprint 9: RBAC Multi-user + Audit + MFA]
        │    [Sprint 4: TLS / SSL]                                  │
        │           │                                               └──▶ [Sprint 10: Observability & Alerts]
        │           ▼
        ├──▶ [Sprint 5: Databases (PG & MariaDB)] ──▶ [Sprint 6: Backups & Restore]
        │
        └──▶ [Sprint 7: DNS + DKIM Sync] ──▶ [Sprint 11: Mail Core + Debian Packaging]
```

**Why this sequencing:** Without **Sprint 2** (`agent` + `jobs`), the control-plane API cannot apply real changes to any host; UI views would remain disconnected. From Sprint 3 onward, each sprint delivers a complete, usable end-to-end capability while keeping PHP out of the critical path.

---

## 3. Verified Sprint Gates

| Sprint | Branch / PR | Scope (100% E2E Goal) | Status | Quality & Verification Gate |
| :--- | :--- | :--- | :--- | :--- |
| **Sprint 1** | `sprint-1` -> `master` ([#574](https://github.com/scherenhaenden/FusionPanel/pull/574)) | **P0 + P1 + Mail DKIM Prober**: Workspace recovery (PR #513 modules), unified `fusionpanel-api` + SQLite auth/RBAC + `/v1/auth/me` + static SPA serving, sprint governance docs, and `dkim_prober` merge (`dac4ab4`). | **DONE (100% E2E)** | - `cargo test --locked --workspace`: all crates PASS (`fusionpanel-api` 7/7, `integration-identity` 22/22, `identity` 22/22, `fusionpanel-cli` 11/11, `fusionpanel-agent-protocol` 14/14, `fusionpanel-control-store` 6/6, `fusionpanel-domain` 2/2, `fusionpanel-jobs` OK)<br>- `modules/mail`: unit + local integration PASS (`unit_dkim_prober` 6/6; 1 SMTP/IMAP live network test `ignored` pending Postfix/Dovecot container)<br>- Frontend: 5 suites / 17 tests PASS + `vite build` PASS<br>- Live E2E (no Docker): CLI `bootstrap operator` -> API + SPA `/` & `/login` -> cookie login -> `/v1/auth/me` (`SuperAdmin`) -> `/v1/capabilities` -> API restart preserving SQLite session -> `logout` revocation (`401`) |
| **Sprint 2** | `sprint-2` ([release PR #590](https://github.com/scherenhaenden/FusionPanel/pull/590) open to `master`) | Node registration, local agent job execution, Nodes/Jobs UI, restart persistence, and a bundled install package. | **Final package, clean-install Docker smoke, Sprint 2 E2E, workspace, agent, jobs, and frontend checks pass at `2e2e8c7`; PR #590 remains open and UNSTABLE because the full qualification matrix is not green** | - PRs #602–#610 are merged into `sprint-2`.<br>- Final package workflow [43826668194](https://github.com/scherenhaenden/FusionPanel/actions/runs/43826668194) passed; its actual artifact passed a network-isolated Docker install smoke (CLI/API/agent/UI, health, and HTTP assets).<br>- GitHub and local `act` Docker/Playwright E2E passed 1/1, including API/agent restart and node/job/log/session persistence.<br>- Local `act` root Cargo, agent (16 library + 5 integration), jobs (9 tests), frontend (6 suites / 26 tests + build), and browser npm jobs passed. `integration/nodes` has 5 passing tests and one migration test blocked pending full-machine disposable VMs.<br>- Full GitHub qualification run [43826668270](https://github.com/scherenhaenden/FusionPanel/actions/runs/43826668270) ended cancelled after 31 successes and 43 failures; the unrelated hosting job was cancelled to save runner time; all failed job names were present in the 46-failure master baseline, with no new failed job names. This is not a green release gate.<br>- Cargo workspace version remains internal metadata `0.3.0-dev.1`. No date/hash release tag, published release artifact, or product-owner acceptance is recorded; issue #24 remains open. See [Sprint 2 plan](SPRINT_02_PLAN.md), [handoff](SPRINT_02_HANDOFF_2026-11-15.md), and [PR evidence tracker](PR_ISSUE_TRACKER.md). |

### Sprint 2 Upgrade Compatibility

The Sprint 2 agent defaults to HTTP mode. Existing deployments that use the legacy WebSocket transport must set `FUSIONPANEL_AGENT_MODE=legacy-ws` and retain `FUSIONPANEL_AGENT_URL` and `FUSIONPANEL_AGENT_SECRET`; the heartbeat URL is optional. Legacy WebSocket variables without an explicit mode now cause startup to exit with a configuration error.

---

## 4. Master Sprint Plan (Sprints 2 – 11)

| Sprint | Depends On | User-Facing E2E Value | Planned Atomic Branches (`-> sprint-<N>`) | Explicit Out of Scope | Definition of Done (Sprint Gate: Integration + E2E) | Environment / Docker | Primary Risk & Mitigation | Estimate |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Sprint 2**<br>*Node Agent & Jobs Runtime* | Sprint 1 | Register local node, enqueue jobs via API/UI, execute them on `agent`, and inspect persistent status & logs across restarts. | 1. `feat/sprint-2-node-jobs-api`<br>2. `feat/sprint-2-agent-job-worker`<br>3. `feat/sprint-2-nodes-jobs-ui` | Multi-node WAN clustering, distributed mTLS, autoscaling. | Bootstrap -> start API + local Agent -> trigger job from UI/API -> restart API/Agent -> verify `succeeded` status & logs in UI. | **Yes for release gates** (disposable Docker E2E and clean-install smoke; development can run native binaries + SQLite) | **Risk:** Concurrent job claim races/duplicates.<br>**Mitigation:** Atomic SQLite claim in `ControlStore` with lease timeout and idempotency key. | **L (8 pts)** |
| **Sprint 3**<br>*Websites & Reverse Proxy* | Sprint 2 | Create, list, and delete static or reverse-proxy sites on `/v1/websites` via UI and serve real HTTP traffic. | 1. `feat/sprint-3-websites-api`<br>2. `feat/sprint-3-proxy-config-apply`<br>3. `feat/sprint-3-websites-ui` | PHP-FPM, advanced ModSecurity WAF rules, custom HTTP/3 tuning. | Create vhost `app.local` in UI -> agent renders config & reloads proxy -> `curl` returns `200 OK` -> delete reverts config & returns `404`. | **Optional** (ephemeral Nginx/Caddy Docker container to keep host clean) | **Risk:** Invalid config breaks global proxy.<br>**Mitigation:** Mandatory `nginx -t` pre-flight validation + automatic rollback before reload. | **L (8 pts)** |
| **Sprint 4**<br>*TLS / SSL Core* | Sprint 3 | Issue/import certificates (ACME or custom PEM) per site and serve HTTPS with automatic renewal. | 1. `feat/sprint-4-tls-api`<br>2. `feat/sprint-4-acme-runner`<br>3. `feat/sprint-4-tls-ui` | Multi-provider DNS-01 wildcards, custom OCSP stapling. | Enable HTTPS on Sprint 3 site -> ACME issuance against Pebble -> `curl --cacert` `200 OK` on `443` -> renewal without downtime. | **Yes** (Pebble ACME + Nginx in Docker) | **Risk:** Challenge failure disrupts active HTTP vhost.<br>**Mitigation:** Isolate HTTP-01 challenge location and promote cert only after TLS validation. | **M (5 pts)** |
| **Sprint 5**<br>*Databases Core (PG & MariaDB)* | Sprint 2 | Provision tenant-scoped PostgreSQL and MariaDB databases and users from UI/API. | 1. `feat/sprint-5-databases-api`<br>2. `feat/sprint-5-db-agent-driver`<br>3. `feat/sprint-5-databases-ui` | Multi-node replication, PgBouncer tuning UI, slow-log analyzer UI. | Create DB + role in UI -> connect via real `psql`/`mysql` client -> verify cross-tenant isolation -> clean drop. | **Yes** (PostgreSQL + MariaDB containers) | **Risk:** Credential leakage in logs or SQL identifier injection.<br>**Mitigation:** Strict identifier validation, secret redaction in job logs, and secure rotation. | **L (8 pts)** |
| **Sprint 6**<br>*Backups & Restore Core* | Sprint 3, Sprint 5 | Run consistent snapshots (site files + DB dump) to local or S3 storage and restore on demand. | 1. `feat/sprint-6-backups-api`<br>2. `feat/sprint-6-backup-s3-worker`<br>3. `feat/sprint-6-backups-ui` | Glacier transitions, multi-tier retention policies, external KMS encryption. | Populate site file + DB row -> snapshot to MinIO S3 -> delete file & DB row -> restore from UI -> verify byte/row integrity. | **Yes** (MinIO S3 + PG/MariaDB in Docker) | **Risk:** Partial restore leaves files and DB out of sync.<br>**Mitigation:** Per-artifact SHA-256 manifest + staged restore before atomic swap. | **L (8 pts)** |
| **Sprint 7**<br>*DNS Manager & DKIM Sync* | Sprint 2 | Manage DNS zones and records (`A`, `AAAA`, `CNAME`, `MX`, `TXT`) plus DKIM TXT sync & propagation probing. | 1. `feat/sprint-7-dns-api`<br>2. `feat/sprint-7-dns-zone-worker`<br>3. `feat/sprint-7-dns-ui` | Anycast probes, Route53/Cloudflare cloud sync, automated DNSSEC rollover. | Create zone + DKIM TXT in UI -> apply to PowerDNS/CoreDNS -> verify via `dig` and `dkim_prober` -> update/delete record. | **Yes** (PowerDNS or CoreDNS in Docker) | **Risk:** Drift between panel DB and authoritative zone.<br>**Mitigation:** Monotonic SOA serial increments + post-apply local DNS query verification. | **M (5 pts)** |
| **Sprint 8**<br>*File Manager & Cron Jobs* | Sprint 2, Sprint 3 | Upload, edit, rename, and download site files and schedule per-tenant Cron jobs. | 1. `feat/sprint-8-files-api`<br>2. `feat/sprint-8-cron-api-agent`<br>3. `feat/sprint-8-files-cron-ui` | Embedded SFTP server, WebDAV, full web IDE features. | Upload `index.html` via UI into Sprint 3 vhost -> served over HTTP; schedule cron -> verify execution + reject `../` traversal. | **No** (local filesystem sandbox) | **Risk:** Path traversal or symlink escape outside tenant root.<br>**Mitigation:** Strict path canonicalization inside tenant jail and external symlink rejection. | **M (5 pts)** |
| **Sprint 9**<br>*Multi-User RBAC, Audit & MFA* | Sprint 1, Sprint 2 | Create operators/tenants, assign scoped roles, issue API tokens, inspect Audit Log, and enable TOTP/Passkeys. | 1. `feat/sprint-9-rbac-users-api`<br>2. `feat/sprint-9-audit-tokens-api`<br>3. `feat/sprint-9-security-admin-ui` | Enterprise OIDC/SAML SSO, external LDAP. | Non-admin gets `403` on foreign tenant resource; Audit Log records mutations; scoped API token works. | **No** | **Risk:** Horizontal privilege escalation across tenants.<br>**Mitigation:** Mandatory `RequirePermission` / tenant-scope extractors + negative tests per endpoint. | **L (8 pts)** |
| **Sprint 10**<br>*Observability, Metrics & Alerts* | Sprint 2 | Display real host/service telemetry (CPU, RAM, disk, service/job health) on Dashboard and trigger signed webhooks. | 1. `feat/sprint-10-metrics-agent`<br>2. `feat/sprint-10-alerts-webhook-api`<br>3. `feat/sprint-10-dashboard-metrics-ui` | Full Prometheus/Grafana stack, distributed OpenTelemetry tracing. | Dashboard renders live host metrics -> simulated threshold triggers signed webhook (`webhook_signer`) & records alert event. | **No** | **Risk:** High-frequency polling saturating SQLite.<br>**Mitigation:** In-memory ring buffer + bounded retention + batched SQLite writes. | **M (5 pts)** |
| **Sprint 11**<br>*Mail Core & Debian Packaging* | Sprint 4, Sprint 7 | Manage mail domains/mailboxes/aliases/DKIM E2E and build a reproducible `.deb` installer for clean Debian hosts. | 1. `feat/sprint-11-mail-api-ui`<br>2. `feat/sprint-11-mail-agent-sync`<br>3. `feat/sprint-11-deb-packaging` | ML antispam tuning, multi-node IMAP clustering, PHP. | Real SMTP/IMAP send/receive against Postfix+Dovecot + clean `.deb` install in fresh Debian container running API+Agent+UI under `systemd`. | **Yes** (Postfix/Dovecot + clean Debian container) | **Risk:** `.deb` permissions/systemd paths diverge from dev.<br>**Mitigation:** Automated `.deb` clean-install smoke test inside disposable Debian container. | **XL (13 pts)** |

### Deprioritized Backlog
* **PHP Management** (`php_pool`, `fpm_profiler`, PHP extensions/versions): explicitly deprioritized per project owner direction.

---

## 5. What Works Today (Sprint 1 Baseline)

* **Bootstrap Admin Operator**: `fusionpanel-cli bootstrap operator --database-url <db> --username <user> --password <pass>`
* **Unified Control Plane Server**: `fusionpanel-api` serves:
  * `/v1/health`, `/v1/capabilities` (both `Bearer $FUSIONPANEL_ADMIN_TOKEN` for CLI and `SESSION_ID` cookie for browser)
  * `/v1/auth/login`, `/v1/auth/refresh`, `/v1/auth/logout`, `/v1/auth/me` backed by persistent SQLite (`SqliteIdentityStore` + `ControlStore`)
  * Built React SPA from `FUSIONPANEL_FRONTEND_DIR` (or `./frontend/dist`) on `/` and client-side routes

---

## 6. Next Sprint Backlog Ready to Start (Sprint 3: Websites & Reverse Proxy)

Sprint 2 implementation and integration gates are documented above. The Sprint 3 handoff plan is in [PR #585](https://github.com/scherenhaenden/FusionPanel/pull/585), which targets `sprint-3`; it is planning documentation only, not website/proxy implementation or live Nginx acceptance. Sprint 3 implementation remains a separate set of atomic PRs to `sprint-3`.
