# Product readiness snapshot — 10 November 2026

**Source baseline** [`3e7862c`](https://github.com/scherenhaenden/FusionPanel/commit/3e7862c5edbed38c50d04ff88beb43e5c7233a91) (`3e7862c5edbed38c50d04ff88beb43e5c7233a91`). Local and GitHub `master` matched at capture. This is the baseline before development moves local on **11 November 2026**. The comparison starts at the [8 November catalog](READINESS_CATALOG_2026-11-08.md).

| Measure | At this cut |
|---|---|
| First-parent history since 8 November | 102 PR merge commits (101 on 10 November, Europe/Berlin) and one direct commit. Merges #562 and #563 have no path changes relative to their first parents |
| Net paths changed | 177, including 74 under test directories |
| Package scopes with net path changes | 26 of 63. 124 of 162 feature rows have at least one owner in a changed scope |
| Package tracking issues | 38 open and 25 closed. GitHub showed no open PRs at capture |
| CI for the baseline | [queued](https://github.com/scherenhaenden/FusionPanel/actions/runs/42799836254) at capture |
| Latest completed scoring | [6 November](READINESS_SNAPSHOT_2026-11-06.json), 8.02% features and 43.49% packages, with 3 of 63 packages accepted then |

No local product tests ran for this snapshot. Test files in the source diff are implementation evidence. The historical scores remain tied to their own SHA and cannot be used as current acceptance scores.

## Regressions that affect the handoff

- [#513](https://github.com/scherenhaenden/FusionPanel/pull/513) ([`f63909d`](https://github.com/scherenhaenden/FusionPanel/commit/f63909d6ec9f06f497f97b437f0c5fae91b56211)) added GeoIP filtering and deleted **100 paths** compared with its first parent. **98 are still absent** at the baseline. The two recovered paths are the port knocker module and its unit test file, restored by [`3e7862c`](https://github.com/scherenhaenden/FusionPanel/commit/3e7862c5edbed38c50d04ff88beb43e5c7233a91). The affected areas include administration, backup destinations, containers, databases, DNS, hosting, mail, nodes, and security. The machine-readable snapshot lists all paths and the catalog maps them to 21 package scopes.
- [#462](https://github.com/scherenhaenden/FusionPanel/pull/462) merged before [#513](https://github.com/scherenhaenden/FusionPanel/pull/513). The current qualification workflow has lost its PR cancellation group, matrix limits, job timeouts and discovery deduplication. The package workflow retains its own PR concurrency group, while the checkout trust command is back inside the tag-only step. The [successful earlier PR package run](https://github.com/scherenhaenden/FusionPanel/actions/runs/42497860770) belongs to `5c307a8` and does not qualify the current package workflow.
- The release tool and package workflow use `yyyy.MM.dd.HH.mm.sss.g<12-character-commit-prefix>`. GitHub has **zero published releases** and only the legacy annotated tag at capture. The earlier PR artifact includes the API and agent. A local source archive or a verified artifact is required to exercise the installer.

## Selected component changes present at this cut

| Catalog context | Source present | Next proof |
|---|---|---|
| [SEC-02](features/sec-02.md) | [WebAuthn credential store](../../modules/security/src/webauthn_store.rs) ([#532](https://github.com/scherenhaenden/FusionPanel/pull/532)) | Browser registration and sign-in |
| [NET-02](features/net-02.md) | [DNSSEC key rollover planner](../../modules/dns/src/dnssec_key_rollover.rs) ([#547](https://github.com/scherenhaenden/FusionPanel/pull/547)) | Signed zone rollover on a real authoritative server |
| [NODE-06](features/node-06.md) | [Peer heartbeat ring](../../modules/nodes/src/heartbeat_ring.rs) ([#545](https://github.com/scherenhaenden/FusionPanel/pull/545)) | Multi-node transport and failover |
| [DB-01](features/db-01.md) | [MySQL slow query parser](../../modules/databases/src/mysql_slow_log_parser.rs) ([#554](https://github.com/scherenhaenden/FusionPanel/pull/554)) | Tenant-scoped MariaDB operations through the main API |
| [BAK-08](features/bak-08.md) | [Retention tiers](../../modules/backups/src/retention_tiers.rs) ([#561](https://github.com/scherenhaenden/FusionPanel/pull/561)) | Real retention execution and independent restore |
| [HOST-04](features/host-04.md) | [GeoIP filtering](../../modules/hosting/src/geoip_filter.rs) ([#513](https://github.com/scherenhaenden/FusionPanel/pull/513)) | Live Nginx validation and served traffic |

## Product journeys

| Journey | State | Current evidence | Next proof |
|---|---|---|---|
| Sign in and use the dashboard | BLOCKED | The main API mounts health and administrative Bearer-token capabilities. Identity login and logout live in a separate router. Login can generate a tenant ID when there is no stored binding. | Compose identity into the main API and verify browser login, persisted membership, logout, revocation and restart. |
| Install and administer on Debian | UNQUALIFIED | The packaging pipeline and date/hash versioning from #459 are present. A Debian-compatible x86_64 PR artifact with API and agent was generated at the earlier #462 PR head. GitHub Releases is empty at this cut. | Build a local archive, install it in a clean disposable system, start the services and exercise restart and rollback. Confirm the agent URL contract and mounted API endpoints. |
| Manage a tenant MariaDB database | BLOCKED | The database and tenant handlers exist separately. The main API does not mount the data router. This cut adds database component source and test source. | Use an authenticated browser against the actual API and MariaDB, and demonstrate scoped create, read/write, restart, restore and isolation. |
| Provision and serve a website | UNQUALIFIED | Hosting, TLS and runtime component changes are present. The CI status and merged test files do not demonstrate a served site at this SHA. | Serve tenant content over real HTTP and TLS, then verify recovery from failed provisioning. |

For the local handoff, compare the missing paths with their last reviewed implementations before assigning component work. The shortest product acceptance checkpoint remains a real browser sign-in, live status and sign-out against one persistent local API. Record the exact source SHA, commands and results for each local check.

The last documented release decision is [NO-RELEASE](waves/W04_2026-11-03/final/RELEASE_DECISION.md).

[Complete feature and Arbeitspaket tables](READINESS_CATALOG_2026-11-10.md) · [Machine-readable snapshot](READINESS_SNAPSHOT_2026-11-10.json)
