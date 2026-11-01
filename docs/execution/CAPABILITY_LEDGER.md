# Capability Delivery and Evidence Ledger

All 162 catalog entries retain a row. State is initially PLANNED unless the capability is RESEARCH. Implementation and test claims require artifacts; session completion does not change state automatically. All rows require unit, real integration and E2E evidence with the supported configuration recorded. See [feature preservation](../engineering/FEATURE_PRESERVATION.md).

| Capability | Name | Component owner | Integration owner | E2E owner | Closure owner | State | Evidence |
|---|---|---|---|---|---|---|---|
| HOST-01 | PHP sites | S09 | S26 | S43 | S54 | PARTIAL | PR #88 merged: hosting specifications and Nginx/PHP-FPM templates only. Actual provisioning and serving remain unverified; hosting integration and E2E cases can report BLOCKED while returning success. Next owners: S26 / S43 / S54. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| HOST-02 | Static sites | S09 | S26 | S43 | S54 | PARTIAL | PR #88 merged: hosting specifications and Nginx/PHP-FPM templates only. Actual provisioning and serving remain unverified; hosting integration and E2E cases can report BLOCKED while returning success. Next owners: S26 / S43 / S54. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| HOST-03 | Node.js and Python hosting | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-04 | Nginx and virtual hosts | S09 | S26 | S43 | S54 | PARTIAL | PR #88 merged: hosting specifications and Nginx/PHP-FPM templates only. Actual provisioning and serving remain unverified; hosting integration and E2E cases can report BLOCKED while returning success. Next owners: S26 / S43 / S54. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| HOST-05 | Per-site PHP-FPM pools | S09 | S26 | S43 | S54 | PARTIAL | PR #88 merged: hosting specifications and Nginx/PHP-FPM templates only. Actual provisioning and serving remain unverified; hosting integration and E2E cases can report BLOCKED while returning success. Next owners: S26 / S43 / S54. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| HOST-06 | Let's Encrypt TLS | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-07 | Wildcard certificates | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-08 | Redirects and HSTS | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-09 | FastCGI cache | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-10 | Redis cache | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-11 | File manager | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-12 | Cron jobs | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-13 | Disk quotas | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-14 | Bandwidth limits | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-15 | Atomic provisioning with compensation | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-16 | Image optimization | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| HOST-17 | CDN integration | S09 | S26 | S43 | S54 | PLANNED | Not yet recorded |
| CTR-01 | Native Docker management | S12 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-02 | Docker Compose stacks | S12 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-03 | Application templates | S40 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-04 | Registry and image management | S12 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-05 | Container lifecycle | S12 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-06 | CPU and RAM limits | S12 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-07 | GPU passthrough | S12 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-08 | Image update detection | S40 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-09 | CVE scanning | S40 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-10 | Per-image SBOM | S40 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-11 | Isolation policies | S12 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-12 | Idle container sleep and wake behavior | S12 | S28 | S45 | S55 | PLANNED | Not yet recorded |
| CTR-13 | Horizontal autoscaling | S60 | S60 | S60 | S60 | RESEARCH | Not yet recorded |
| DEP-01 | Git push-to-deploy | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| DEP-02 | Git webhooks | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| DEP-03 | Multi-language builds | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| DEP-04 | Blue-green deployments | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| DEP-05 | Per-commit preview environments | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| DEP-06 | Staging | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| DEP-07 | Release rollback | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| DEP-08 | Scheduled deployments | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| DEP-09 | Two-person deployment approval | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| DEP-10 | Deployment history and progress | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| DEP-11 | Failure diagnostics | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| DEP-12 | GitHub Actions integration | S19 | S30 | S46 | S55 | PLANNED | Not yet recorded |
| SEC-01 | TOTP two-factor authentication | S08 | S25 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-02 | Passkeys and WebAuthn | S08 | S25 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-03 | OAuth and SSO | S08 | S25 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-04 | Role-based access control | S08 | S25 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-05 | Firewall management | S15 | S37 | S52 | S57 | PARTIAL | PR #71 merged: policy validation and inert previews, with 29 unit tests. Host application, real parsers, traffic, rollback, and emergency-access evidence are still required. Next owners: S37 / S52 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| SEC-06 | Brute-force protection | S15 | S37 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-07 | Web application firewall | S15 | S37 | S52 | S57 | PARTIAL | PR #71 merged: policy validation and inert previews, with 29 unit tests. Host application, real parsers, traffic, rollback, and emergency-access evidence are still required. Next owners: S37 / S52 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| SEC-08 | Bot protection | S15 | S37 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-09 | SSH hardening | S15 | S37 | S52 | S57 | PARTIAL | PR #71 merged: policy validation and inert previews, with 29 unit tests. Host application, real parsers, traffic, rollback, and emergency-access evidence are still required. Next owners: S37 / S52 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| SEC-10 | Per-user isolation | S15 | S37 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-11 | Safe command execution | S15 | S37 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-12 | Revocable sessions | S08 | S25 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-13 | Tamper-evident audit integrity | S15 | S37 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-14 | Vulnerability scanning | S15 | S37 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-15 | Credential encryption | S23 | S37 | S52 | S57 | PLANNED | Not yet recorded |
| SEC-16 | Content Security Policy controls | S15 | S37 | S52 | S57 | PARTIAL | PR #71 merged: policy validation and inert previews, with 29 unit tests. Host application, real parsers, traffic, rollback, and emergency-access evidence are still required. Next owners: S37 / S52 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| SEC-17 | Release integrity and provenance | S15 | S37 | S52 | S57 | PLANNED | Not yet recorded |
| BAK-01 | Scheduled backups | S11 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| BAK-02 | File backups | S11 | S27 | S44 | S56 | PARTIAL | PR #86 merged: file archive, checksum or retention component; 1 unit and 3 filesystem integration tests reported passing. Runtime integration and independent real-system E2E remain. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| BAK-03 | Database backups | S11 | S27 | S44 | S56 | PARTIAL | PR #86 merged: database information is metadata only; one isolated file restore cycle was tested. Real database dumps or operator restore drills are not qualified. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| BAK-04 | S3 destinations | S39 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| BAK-05 | SFTP and FTPS destinations | S39 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| BAK-06 | Backblaze B2 and Google Cloud Storage destinations | S39 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| BAK-07 | Backup encryption | S11 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| BAK-08 | Retention policies | S11 | S27 | S44 | S56 | PARTIAL | PR #86 merged: file archive, checksum or retention component; 1 unit and 3 filesystem integration tests reported passing. Runtime integration and independent real-system E2E remain. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| BAK-09 | Selective restore | S11 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| BAK-10 | Backup verification | S11 | S27 | S44 | S56 | PARTIAL | PR #86 merged: file archive, checksum or retention component; 1 unit and 3 filesystem integration tests reported passing. Runtime integration and independent real-system E2E remain. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| BAK-11 | Restore drills | S11 | S27 | S44 | S56 | PARTIAL | PR #86 merged: database information is metadata only; one isolated file restore cycle was tested. Real database dumps or operator restore drills are not qualified. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| BAK-12 | Restic snapshots | S11 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| BAK-13 | Pre-update backups | S11 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| BAK-14 | Restore to another domain | S11 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| BAK-15 | Restore confidence dashboard | S11 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| DB-01 | MariaDB and MySQL management | S10 | S27 | S44 | S56 | PARTIAL | PR #87 merged: adapter operations and 4 unit tests; database daemon integration is ignored/BLOCKED. Real grants, dump and independent restore require S27/S44. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| DB-02 | PostgreSQL management | S10 | S27 | S44 | S56 | PARTIAL | PR #87 merged: adapter operations and 4 unit tests; database daemon integration is ignored/BLOCKED. Real grants, dump and independent restore require S27/S44. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| DB-03 | Database creation | S10 | S27 | S44 | S56 | PARTIAL | PR #87 merged: adapter operations and 4 unit tests; database daemon integration is ignored/BLOCKED. Real grants, dump and independent restore require S27/S44. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| DB-04 | Database users and passwords | S10 | S27 | S44 | S56 | PARTIAL | PR #87 merged: adapter operations and 4 unit tests; database daemon integration is ignored/BLOCKED. Real grants, dump and independent restore require S27/S44. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| DB-05 | SQL browser | S10 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| DB-06 | Schema explorer | S10 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| DB-07 | Protected phpMyAdmin access | S10 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| DB-08 | Database backups | S10 | S27 | S44 | S56 | PARTIAL | PR #87 merged: adapter operations and 4 unit tests; database daemon integration is ignored/BLOCKED. Real grants, dump and independent restore require S27/S44. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| DB-09 | Site and database provisioning integration | S10 | S27 | S44 | S56 | PLANNED | Not yet recorded |
| WP-01 | Automatic installation | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| WP-02 | Plugin management | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| WP-03 | Theme management | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| WP-04 | Automatic updates | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| WP-05 | WordPress staging and production push | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| WP-06 | Vulnerability scanning | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| WP-07 | Core repair | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| WP-08 | Plugin failure diagnostics | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| WP-09 | Restore with URL rewrite | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| WP-10 | Redis object cache | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| WP-11 | WordPress hardening | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| WP-12 | Multiple installation management | S22 | S33 | S47 | S54 | PLANNED | Not yet recorded |
| OBS-01 | CPU, RAM and disk metrics | S16 | S31 | S48 | S57 | PARTIAL | PR #72 merged: module primitives; 9 tests pass. Loopback HTTP/TCP and webhook boundaries are exercised; webhook payload assertions, live collection, scheduling, durable history and outage-to-recovery E2E remain incomplete. Next owners: S31 / S48 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| OBS-02 | Metrics history | S16 | S31 | S48 | S57 | PLANNED | Not yet recorded |
| OBS-03 | Live network traffic | S16 | S31 | S48 | S57 | PARTIAL | PR #72 merged: module primitives; 9 tests pass. Loopback HTTP/TCP and webhook boundaries are exercised; webhook payload assertions, live collection, scheduling, durable history and outage-to-recovery E2E remain incomplete. Next owners: S31 / S48 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| OBS-04 | Per-site metrics | S16 | S31 | S48 | S57 | PARTIAL | PR #72 merged: module primitives; 9 tests pass. Loopback HTTP/TCP and webhook boundaries are exercised; webhook payload assertions, live collection, scheduling, durable history and outage-to-recovery E2E remain incomplete. Next owners: S31 / S48 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| OBS-05 | Per-container metrics | S16 | S31 | S48 | S57 | PARTIAL | PR #72 merged: module primitives; 9 tests pass. Loopback HTTP/TCP and webhook boundaries are exercised; webhook payload assertions, live collection, scheduling, durable history and outage-to-recovery E2E remain incomplete. Next owners: S31 / S48 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| OBS-06 | HTTP uptime checks | S16 | S31 | S48 | S57 | PARTIAL | PR #72 merged: module primitives; 9 tests pass. Loopback HTTP/TCP and webhook boundaries are exercised; webhook payload assertions, live collection, scheduling, durable history and outage-to-recovery E2E remain incomplete. Next owners: S31 / S48 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| OBS-07 | TCP and ping checks | S16 | S31 | S48 | S57 | PARTIAL | PR #72 merged: module primitives; 9 tests pass. Loopback HTTP/TCP and webhook boundaries are exercised; webhook payload assertions, live collection, scheduling, durable history and outage-to-recovery E2E remain incomplete. Next owners: S31 / S48 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| OBS-08 | Prometheus endpoint | S16 | S31 | S48 | S57 | PARTIAL | PR #72 merged: module primitives; 9 tests pass. Loopback HTTP/TCP and webhook boundaries are exercised; webhook payload assertions, live collection, scheduling, durable history and outage-to-recovery E2E remain incomplete. Next owners: S31 / S48 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| OBS-09 | Configurable alerts | S16 | S31 | S48 | S57 | PARTIAL | PR #72 merged: module primitives; 9 tests pass. Loopback HTTP/TCP and webhook boundaries are exercised; webhook payload assertions, live collection, scheduling, durable history and outage-to-recovery E2E remain incomplete. Next owners: S31 / S48 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| OBS-10 | PagerDuty notifications | S16 | S31 | S48 | S57 | PLANNED | Not yet recorded |
| OBS-11 | Slack notifications | S16 | S31 | S48 | S57 | PLANNED | Not yet recorded |
| OBS-12 | Automatic recovery actions | S16 | S31 | S48 | S57 | PLANNED | Not yet recorded |
| OBS-13 | System diagnostics | S16 | S31 | S48 | S57 | PLANNED | Not yet recorded |
| OBS-14 | Public status page | S16 | S31 | S48 | S57 | PLANNED | Not yet recorded |
| OBS-15 | Incident management | S16 | S31 | S48 | S57 | PARTIAL | PR #72 merged: module primitives; 9 tests pass. Loopback HTTP/TCP and webhook boundaries are exercised; webhook payload assertions, live collection, scheduling, durable history and outage-to-recovery E2E remain incomplete. Next owners: S31 / S48 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| NODE-01 | Rust core | S21 | S34 | S49 | S57 | PARTIAL | PR #83 merged; in-memory node lifecycle and placement plans, 11 unit tests passed. Real signed transport, persistence and integration/E2E drivers remain blocked. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| NODE-02 | Lightweight frontend | S21 | S34 | S49 | S57 | PLANNED | Not yet recorded |
| NODE-03 | Agent separation | S21 | S34 | S49 | S57 | PLANNED | Not yet recorded |
| NODE-04 | Restricted privileged surface | S21 | S34 | S49 | S57 | PLANNED | Not yet recorded |
| NODE-05 | Authenticated RPC | S21 | S34 | S49 | S57 | PLANNED | Not yet recorded |
| NODE-06 | Master and worker node management | S21 | S34 | S49 | S57 | PARTIAL | PR #83 merged; in-memory node lifecycle and placement plans, 11 unit tests passed. Real signed transport, persistence and integration/E2E drivers remain blocked. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| NODE-07 | Move and copy sites between nodes | S21 | S34 | S49 | S57 | PARTIAL | PR #83 merged; in-memory node lifecycle and placement plans, 11 unit tests passed. Real signed transport, persistence and integration/E2E drivers remain blocked. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| NODE-08 | Load-aware placement | S21 | S34 | S49 | S57 | PARTIAL | PR #83 merged; in-memory node lifecycle and placement plans, 11 unit tests passed. Real signed transport, persistence and integration/E2E drivers remain blocked. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| NODE-09 | LIFO provisioning compensation | S21 | S34 | S49 | S57 | PLANNED | Not yet recorded |
| NODE-10 | Control-state persistence | S21 | S34 | S49 | S57 | PLANNED | Not yet recorded |
| NODE-11 | Durable jobs | S21 | S34 | S49 | S57 | PLANNED | Not yet recorded |
| NODE-12 | Highly available control plane | S60 | S60 | S60 | S60 | RESEARCH | Not yet recorded |
| NODE-13 | OpenAPI contract | S21 | S34 | S49 | S57 | PLANNED | Not yet recorded |
| NODE-14 | Operational and recovery CLI | S21 | S34 | S49 | S57 | PLANNED | Not yet recorded |
| NODE-15 | AGPL licensing and source compliance | S21 | S34 | S49 | S57 | PLANNED | Not yet recorded |
| NET-01 | DNS zone management | S13/S14 | S29 | S50 | S56 | PARTIAL | PR #85 merged; refined DNS model/provider boundary and 6 unit tests reported passing. PR #84 was closed unmerged as superseded. Authoritative provider integration and real DNS E2E remain blocked. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| NET-02 | DNSSEC | S13/S14 | S29 | S50 | S56 | PARTIAL | PR #85 merged; refined DNS model/provider boundary and 6 unit tests reported passing. PR #84 was closed unmerged as superseded. Authoritative provider integration and real DNS E2E remain blocked. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| NET-03 | DNS templates | S13/S14 | S29 | S50 | S56 | PLANNED | Not yet recorded |
| NET-04 | Hosted mailboxes | S14 | S29 | S50 | S56 | PLANNED | Not yet recorded |
| NET-05 | Webmail | S14 | S29 | S50 | S56 | PLANNED | Not yet recorded |
| NET-06 | Outgoing SMTP | S14 | S29 | S50 | S56 | PLANNED | Not yet recorded |
| NET-07 | DKIM and SPF controls | S14 | S29 | S50 | S56 | PLANNED | Not yet recorded |
| NET-08 | Antispam | S14 | S29 | S50 | S56 | PLANNED | Not yet recorded |
| NET-09 | Mail logs | S14 | S29 | S50 | S56 | PLANNED | Not yet recorded |
| NET-10 | SMTP relay | S14 | S29 | S50 | S56 | PLANNED | Not yet recorded |
| ADM-01 | Web SSH terminal | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-02 | Systemd service management | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-03 | Operating system updates | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-04 | Panel updates and recovery | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-05 | User and role management | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-06 | Reseller hierarchy | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-07 | White-label branding | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-08 | HTTP API | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-09 | API keys | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-10 | API IP allowlists and rate limits | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-11 | Recovery CLI | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-12 | Secret management | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-13 | Infrastructure as Code | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-14 | Care plans | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| ADM-15 | Monthly client reports | S20/S23 | S36/S38 | S51 | S57 | PLANNED | Not yet recorded |
| MIG-01 | CloudPanel import | S18 | S35 | S53 | S57 | PLANNED | Not yet recorded |
| MIG-02 | CloudPanel SQLite discovery | S18 | S35 | S53 | S57 | PARTIAL | PR #69 open: fixture-based discovery/planner slice. Source-schema registry is empty; CloudPanel source compatibility and source-export E2E are blocked. Next owners: S35 / S53 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| MIG-03 | HestiaCP import | S18 | S35 | S53 | S57 | PLANNED | Not yet recorded |
| MIG-04 | cPanel import | S18 | S35 | S53 | S57 | PLANNED | Not yet recorded |
| MIG-05 | Plesk import | S18 | S35 | S53 | S57 | PLANNED | Not yet recorded |
| MIG-06 | SSH migration | S18 | S35 | S53 | S57 | PLANNED | Not yet recorded |
| MIG-07 | Exporter-based migration | S18 | S35 | S53 | S57 | PLANNED | Not yet recorded |
| MIG-08 | Migration dry-run | S18 | S35 | S53 | S57 | PARTIAL | PR #69 open: fixture-based discovery/planner slice. Source-schema registry is empty; CloudPanel source compatibility and source-export E2E are blocked. Next owners: S35 / S53 / S57. See [readiness snapshot](READINESS_SNAPSHOT_2026-11-01.md); no runtime/E2E/closure acceptance. |
| MIG-09 | Same-server migration | S18 | S35 | S53 | S57 | PLANNED | Not yet recorded |
| MIG-10 | Whole-server migration rollback | S18 | S35 | S53 | S57 | PLANNED | Not yet recorded |
| MIG-11 | DNS and mail migration | S18 | S35 | S53 | S57 | PLANNED | Not yet recorded |
| MIG-12 | Migration downtime control | S18 | S35 | S53 | S57 | PLANNED | Not yet recorded |
