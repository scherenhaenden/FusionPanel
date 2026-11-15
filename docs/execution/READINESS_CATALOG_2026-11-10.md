# FusionPanel features and Arbeitspakete — 10 November 2026

This inventory follows [`3e7862c`](https://github.com/scherenhaenden/FusionPanel/commit/3e7862c5edbed38c50d04ff88beb43e5c7233a91), compared with [`95c5329`](https://github.com/scherenhaenden/FusionPanel/commit/95c5329cba2d1e7b924935e863af3209ecb23775). See the [snapshot](READINESS_SNAPSHOT_2026-11-10.md) for the local handoff, regressions and product journeys. All scores and individual feature sheets refer to the 6 November scoring baseline, with the sheets reviewed on 7 November. Changed owners and path counts are navigation aids. An owner match does not show that an individual feature changed or works end to end.

## Features (162)

Gates are historical component, integration, real E2E and closure. The two owner columns use the net diff and the list of paths still absent after [#513](https://github.com/scherenhaenden/FusionPanel/pull/513), respectively.

| Feature | Capability | 6 Nov gates | 6 Nov score | Owners with net changes | Owners with missing paths |
|---|---|---|---:|---|---|
| [HOST-01](features/host-01.md) | PHP sites | 12.5 / 0 / 0 / 0 | 12.5% | S09, S26 | S09, S26 |
| [HOST-02](features/host-02.md) | Static sites | 12.5 / 0 / 0 / 0 | 12.5% | S09, S26 | S09, S26 |
| [HOST-03](features/host-03.md) | Node.js and Python hosting | 0 / 0 / 0 / 0 | 0% | S09, S26 | S09, S26 |
| [HOST-04](features/host-04.md) | Nginx and virtual hosts | 12.5 / 0 / 0 / 0 | 12.5% | S09, S26 | S09, S26 |
| [HOST-05](features/host-05.md) | Per-site PHP-FPM pools | 12.5 / 0 / 0 / 0 | 12.5% | S09, S26 | S09, S26 |
| [HOST-06](features/host-06.md) | Let's Encrypt TLS | 12.5 / 0 / 0 / 0 | 12.5% | S09, S26 | S09, S26 |
| [HOST-07](features/host-07.md) | Wildcard certificates | 0 / 0 / 0 / 0 | 0% | S09, S26 | S09, S26 |
| [HOST-08](features/host-08.md) | Redirects and HSTS | 0 / 0 / 0 / 0 | 0% | S09, S26 | S09, S26 |
| [HOST-09](features/host-09.md) | FastCGI cache | 0 / 0 / 0 / 0 | 0% | S09, S26 | S09, S26 |
| [HOST-10](features/host-10.md) | Redis cache | 0 / 0 / 0 / 0 | 0% | S09, S26 | S09, S26 |
| [HOST-11](features/host-11.md) | File manager | 0 / 0 / 0 / 0 | 0% | S09, S26 | S09, S26 |
| [HOST-12](features/host-12.md) | Cron jobs | 0 / 0 / 0 / 0 | 0% | S09, S26 | S09, S26 |
| [HOST-13](features/host-13.md) | Disk quotas | 12.5 / 0 / 0 / 0 | 12.5% | S09, S26 | S09, S26 |
| [HOST-14](features/host-14.md) | Bandwidth limits | 0 / 0 / 0 / 0 | 0% | S09, S26 | S09, S26 |
| [HOST-15](features/host-15.md) | Atomic provisioning with compensation | 12.5 / 0 / 0 / 0 | 12.5% | S09, S26 | S09, S26 |
| [HOST-16](features/host-16.md) | Image optimization | 0 / 0 / 0 / 0 | 0% | S09, S26 | S09, S26 |
| [HOST-17](features/host-17.md) | CDN integration | 0 / 0 / 0 / 0 | 0% | S09, S26 | S09, S26 |
| [CTR-01](features/ctr-01.md) | Native Docker management | 12.5 / 0 / 0 / 0 | 12.5% | S12, S28 | S12, S28 |
| [CTR-02](features/ctr-02.md) | Docker Compose stacks | 12.5 / 0 / 0 / 0 | 12.5% | S12, S28 | S12, S28 |
| [CTR-03](features/ctr-03.md) | Application templates | 12.5 / 0 / 0 / 0 | 12.5% | S28 | S40, S28 |
| [CTR-04](features/ctr-04.md) | Registry and image management | 0 / 0 / 0 / 0 | 0% | S12, S28 | S12, S28 |
| [CTR-05](features/ctr-05.md) | Container lifecycle | 12.5 / 0 / 0 / 0 | 12.5% | S12, S28 | S12, S28 |
| [CTR-06](features/ctr-06.md) | CPU and RAM limits | 12.5 / 0 / 0 / 0 | 12.5% | S12, S28 | S12, S28 |
| [CTR-07](features/ctr-07.md) | GPU passthrough | 0 / 0 / 0 / 0 | 0% | S12, S28 | S12, S28 |
| [CTR-08](features/ctr-08.md) | Image update detection | 0 / 0 / 0 / 0 | 0% | S28 | S40, S28 |
| [CTR-09](features/ctr-09.md) | CVE scanning | 12.5 / 0 / 0 / 0 | 12.5% | S28 | S40, S28 |
| [CTR-10](features/ctr-10.md) | Per-image SBOM | 0 / 0 / 0 / 0 | 0% | S28 | S40, S28 |
| [CTR-11](features/ctr-11.md) | Isolation policies | 12.5 / 0 / 0 / 0 | 12.5% | S12, S28 | S12, S28 |
| [CTR-12](features/ctr-12.md) | Idle container sleep and wake behavior | 0 / 0 / 0 / 0 | 0% | S12, S28 | S12, S28 |
| [CTR-13](features/ctr-13.md) | Horizontal autoscaling | 0 / 0 / 0 / 0 | 0% | — | — |
| [DEP-01](features/dep-01.md) | Git push-to-deploy | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [DEP-02](features/dep-02.md) | Git webhooks | 25 / 0 / 0 / 0 | 25% | — | — |
| [DEP-03](features/dep-03.md) | Multi-language builds | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [DEP-04](features/dep-04.md) | Blue-green deployments | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [DEP-05](features/dep-05.md) | Per-commit preview environments | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [DEP-06](features/dep-06.md) | Staging | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [DEP-07](features/dep-07.md) | Release rollback | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [DEP-08](features/dep-08.md) | Scheduled deployments | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [DEP-09](features/dep-09.md) | Two-person deployment approval | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [DEP-10](features/dep-10.md) | Deployment history and progress | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [DEP-11](features/dep-11.md) | Failure diagnostics | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [DEP-12](features/dep-12.md) | GitHub Actions integration | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [SEC-01](features/sec-01.md) | TOTP two-factor authentication | 0 / 0 / 0 / 0 | 0% | S25 | — |
| [SEC-02](features/sec-02.md) | Passkeys and WebAuthn | 0 / 0 / 0 / 0 | 0% | S25 | — |
| [SEC-03](features/sec-03.md) | OAuth and SSO | 0 / 0 / 0 / 0 | 0% | S25 | — |
| [SEC-04](features/sec-04.md) | Role-based access control | 25 / 0 / 0 / 0 | 25% | S25 | — |
| [SEC-05](features/sec-05.md) | Firewall management | 25 / 0 / 0 / 0 | 25% | S15, S37 | S15, S37 |
| [SEC-06](features/sec-06.md) | Brute-force protection | 0 / 0 / 0 / 0 | 0% | S15, S37 | S15, S37 |
| [SEC-07](features/sec-07.md) | Web application firewall | 25 / 0 / 0 / 0 | 25% | S15, S37 | S15, S37 |
| [SEC-08](features/sec-08.md) | Bot protection | 0 / 0 / 0 / 0 | 0% | S15, S37 | S15, S37 |
| [SEC-09](features/sec-09.md) | SSH hardening | 25 / 0 / 0 / 0 | 25% | S15, S37 | S15, S37 |
| [SEC-10](features/sec-10.md) | Per-user isolation | 0 / 0 / 0 / 0 | 0% | S15, S37 | S15, S37 |
| [SEC-11](features/sec-11.md) | Safe command execution | 12.5 / 0 / 0 / 0 | 12.5% | S15, S37 | S15, S37 |
| [SEC-12](features/sec-12.md) | Revocable sessions | 25 / 0 / 0 / 0 | 25% | S25 | — |
| [SEC-13](features/sec-13.md) | Tamper-evident audit integrity | 12.5 / 0 / 0 / 0 | 12.5% | S15, S37 | S15, S37 |
| [SEC-14](features/sec-14.md) | Vulnerability scanning | 12.5 / 0 / 0 / 0 | 12.5% | S15, S37 | S15, S37 |
| [SEC-15](features/sec-15.md) | Credential encryption | 25 / 0 / 0 / 0 | 25% | S37 | S23, S37 |
| [SEC-16](features/sec-16.md) | Content Security Policy controls | 25 / 0 / 0 / 0 | 25% | S15, S37 | S15, S37 |
| [SEC-17](features/sec-17.md) | Release integrity and provenance | 12.5 / 0 / 0 / 0 | 12.5% | S15, S37 | S15, S37 |
| [BAK-01](features/bak-01.md) | Scheduled backups | 12.5 / 0 / 0 / 0 | 12.5% | S11, S27 | S27 |
| [BAK-02](features/bak-02.md) | File backups | 25 / 0 / 0 / 0 | 25% | S11, S27 | S27 |
| [BAK-03](features/bak-03.md) | Database backups | 12.5 / 0 / 0 / 0 | 12.5% | S11, S27 | S27 |
| [BAK-04](features/bak-04.md) | S3 destinations | 12.5 / 0 / 0 / 0 | 12.5% | S39, S27 | S39, S27 |
| [BAK-05](features/bak-05.md) | SFTP and FTPS destinations | 12.5 / 0 / 0 / 0 | 12.5% | S39, S27 | S39, S27 |
| [BAK-06](features/bak-06.md) | Backblaze B2 and Google Cloud Storage destinations | 0 / 0 / 0 / 0 | 0% | S39, S27 | S39, S27 |
| [BAK-07](features/bak-07.md) | Backup encryption | 0 / 0 / 0 / 0 | 0% | S11, S27 | S27 |
| [BAK-08](features/bak-08.md) | Retention policies | 25 / 0 / 0 / 0 | 25% | S11, S27 | S27 |
| [BAK-09](features/bak-09.md) | Selective restore | 12.5 / 0 / 0 / 0 | 12.5% | S11, S27 | S27 |
| [BAK-10](features/bak-10.md) | Backup verification | 25 / 0 / 0 / 0 | 25% | S11, S27 | S27 |
| [BAK-11](features/bak-11.md) | Restore drills | 12.5 / 0 / 0 / 0 | 12.5% | S11, S27 | S27 |
| [BAK-12](features/bak-12.md) | Restic snapshots | 0 / 0 / 0 / 0 | 0% | S11, S27 | S27 |
| [BAK-13](features/bak-13.md) | Pre-update backups | 0 / 0 / 0 / 0 | 0% | S11, S27 | S27 |
| [BAK-14](features/bak-14.md) | Restore to another domain | 0 / 0 / 0 / 0 | 0% | S11, S27 | S27 |
| [BAK-15](features/bak-15.md) | Restore confidence dashboard | 0 / 0 / 0 / 0 | 0% | S11, S27 | S27 |
| [DB-01](features/db-01.md) | MariaDB and MySQL management | 12.5 / 0 / 0 / 0 | 12.5% | S10, S27 | S10, S27 |
| [DB-02](features/db-02.md) | PostgreSQL management | 12.5 / 0 / 0 / 0 | 12.5% | S10, S27 | S10, S27 |
| [DB-03](features/db-03.md) | Database creation | 12.5 / 0 / 0 / 0 | 12.5% | S10, S27 | S10, S27 |
| [DB-04](features/db-04.md) | Database users and passwords | 12.5 / 0 / 0 / 0 | 12.5% | S10, S27 | S10, S27 |
| [DB-05](features/db-05.md) | SQL browser | 0 / 0 / 0 / 0 | 0% | S10, S27 | S10, S27 |
| [DB-06](features/db-06.md) | Schema explorer | 0 / 0 / 0 / 0 | 0% | S10, S27 | S10, S27 |
| [DB-07](features/db-07.md) | Protected phpMyAdmin access | 0 / 0 / 0 / 0 | 0% | S10, S27 | S10, S27 |
| [DB-08](features/db-08.md) | Database backups | 12.5 / 0 / 0 / 0 | 12.5% | S10, S27 | S10, S27 |
| [DB-09](features/db-09.md) | Site and database provisioning integration | 0 / 0 / 0 / 0 | 0% | S10, S27 | S10, S27 |
| [WP-01](features/wp-01.md) | Automatic installation | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [WP-02](features/wp-02.md) | Plugin management | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [WP-03](features/wp-03.md) | Theme management | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [WP-04](features/wp-04.md) | Automatic updates | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [WP-05](features/wp-05.md) | WordPress staging and production push | 0 / 0 / 0 / 0 | 0% | — | — |
| [WP-06](features/wp-06.md) | Vulnerability scanning | 0 / 0 / 0 / 0 | 0% | — | — |
| [WP-07](features/wp-07.md) | Core repair | 0 / 0 / 0 / 0 | 0% | — | — |
| [WP-08](features/wp-08.md) | Plugin failure diagnostics | 0 / 0 / 0 / 0 | 0% | — | — |
| [WP-09](features/wp-09.md) | Restore with URL rewrite | 0 / 0 / 0 / 0 | 0% | — | — |
| [WP-10](features/wp-10.md) | Redis object cache | 0 / 0 / 0 / 0 | 0% | — | — |
| [WP-11](features/wp-11.md) | WordPress hardening | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [WP-12](features/wp-12.md) | Multiple installation management | 0 / 0 / 0 / 0 | 0% | — | — |
| [OBS-01](features/obs-01.md) | CPU, RAM and disk metrics | 25 / 0 / 0 / 0 | 25% | S16, S31 | S16, S31 |
| [OBS-02](features/obs-02.md) | Metrics history | 0 / 0 / 0 / 0 | 0% | S16, S31 | S16, S31 |
| [OBS-03](features/obs-03.md) | Live network traffic | 25 / 0 / 0 / 0 | 25% | S16, S31 | S16, S31 |
| [OBS-04](features/obs-04.md) | Per-site metrics | 12.5 / 0 / 0 / 0 | 12.5% | S16, S31 | S16, S31 |
| [OBS-05](features/obs-05.md) | Per-container metrics | 12.5 / 0 / 0 / 0 | 12.5% | S16, S31 | S16, S31 |
| [OBS-06](features/obs-06.md) | HTTP uptime checks | 25 / 0 / 0 / 0 | 25% | S16, S31 | S16, S31 |
| [OBS-07](features/obs-07.md) | TCP and ping checks | 12.5 / 0 / 0 / 0 | 12.5% | S16, S31 | S16, S31 |
| [OBS-08](features/obs-08.md) | Prometheus endpoint | 25 / 0 / 0 / 0 | 25% | S16, S31 | S16, S31 |
| [OBS-09](features/obs-09.md) | Configurable alerts | 25 / 0 / 0 / 0 | 25% | S16, S31 | S16, S31 |
| [OBS-10](features/obs-10.md) | PagerDuty notifications | 0 / 0 / 0 / 0 | 0% | S16, S31 | S16, S31 |
| [OBS-11](features/obs-11.md) | Slack notifications | 0 / 0 / 0 / 0 | 0% | S16, S31 | S16, S31 |
| [OBS-12](features/obs-12.md) | Automatic recovery actions | 0 / 0 / 0 / 0 | 0% | S16, S31 | S16, S31 |
| [OBS-13](features/obs-13.md) | System diagnostics | 0 / 0 / 0 / 0 | 0% | S16, S31 | S16, S31 |
| [OBS-14](features/obs-14.md) | Public status page | 0 / 0 / 0 / 0 | 0% | S16, S31 | S16, S31 |
| [OBS-15](features/obs-15.md) | Incident management | 25 / 0 / 0 / 0 | 25% | S16, S31 | S16, S31 |
| [NODE-01](features/node-01.md) | Rust core | 12.5 / 0 / 0 / 0 | 12.5% | S21, S34 | S21, S34 |
| [NODE-02](features/node-02.md) | Lightweight frontend | 12.5 / 0 / 0 / 0 | 12.5% | S21, S34 | S21, S34 |
| [NODE-03](features/node-03.md) | Agent separation | 12.5 / 0 / 0 / 0 | 12.5% | S21, S34 | S21, S34 |
| [NODE-04](features/node-04.md) | Restricted privileged surface | 12.5 / 0 / 0 / 0 | 12.5% | S21, S34 | S21, S34 |
| [NODE-05](features/node-05.md) | Authenticated RPC | 12.5 / 0 / 0 / 0 | 12.5% | S21, S34 | S21, S34 |
| [NODE-06](features/node-06.md) | Master and worker node management | 12.5 / 0 / 0 / 0 | 12.5% | S21, S34 | S21, S34 |
| [NODE-07](features/node-07.md) | Move and copy sites between nodes | 12.5 / 0 / 0 / 0 | 12.5% | S21, S34 | S21, S34 |
| [NODE-08](features/node-08.md) | Load-aware placement | 12.5 / 0 / 0 / 0 | 12.5% | S21, S34 | S21, S34 |
| [NODE-09](features/node-09.md) | LIFO provisioning compensation | 0 / 0 / 0 / 0 | 0% | S21, S34 | S21, S34 |
| [NODE-10](features/node-10.md) | Control-state persistence | 12.5 / 0 / 0 / 0 | 12.5% | S21, S34 | S21, S34 |
| [NODE-11](features/node-11.md) | Durable jobs | 25 / 0 / 0 / 0 | 25% | S21, S34 | S21, S34 |
| [NODE-12](features/node-12.md) | Highly available control plane | 0 / 0 / 0 / 0 | 0% | — | — |
| [NODE-13](features/node-13.md) | OpenAPI contract | 0 / 0 / 0 / 0 | 0% | S21, S34 | S21, S34 |
| [NODE-14](features/node-14.md) | Operational and recovery CLI | 0 / 0 / 0 / 0 | 0% | S21, S34 | S21, S34 |
| [NODE-15](features/node-15.md) | AGPL licensing and source compliance | 0 / 0 / 0 / 0 | 0% | S21, S34 | S21, S34 |
| [NET-01](features/net-01.md) | DNS zone management | 12.5 / 0 / 0 / 0 | 12.5% | S13, S14, S29 | S13, S14, S29 |
| [NET-02](features/net-02.md) | DNSSEC | 12.5 / 0 / 0 / 0 | 12.5% | S13, S14, S29 | S13, S14, S29 |
| [NET-03](features/net-03.md) | DNS templates | 0 / 0 / 0 / 0 | 0% | S13, S14, S29 | S13, S14, S29 |
| [NET-04](features/net-04.md) | Hosted mailboxes | 12.5 / 0 / 0 / 0 | 12.5% | S14, S29 | S14, S29 |
| [NET-05](features/net-05.md) | Webmail | 0 / 0 / 0 / 0 | 0% | S14, S29 | S14, S29 |
| [NET-06](features/net-06.md) | Outgoing SMTP | 12.5 / 0 / 0 / 0 | 12.5% | S14, S29 | S14, S29 |
| [NET-07](features/net-07.md) | DKIM and SPF controls | 12.5 / 0 / 0 / 0 | 12.5% | S14, S29 | S14, S29 |
| [NET-08](features/net-08.md) | Antispam | 0 / 0 / 0 / 0 | 0% | S14, S29 | S14, S29 |
| [NET-09](features/net-09.md) | Mail logs | 0 / 0 / 0 / 0 | 0% | S14, S29 | S14, S29 |
| [NET-10](features/net-10.md) | SMTP relay | 0 / 0 / 0 / 0 | 0% | S14, S29 | S14, S29 |
| [ADM-01](features/adm-01.md) | Web SSH terminal | 0 / 0 / 0 / 0 | 0% | S20, S36, S38 | S20, S23, S36 |
| [ADM-02](features/adm-02.md) | Systemd service management | 0 / 0 / 0 / 0 | 0% | S20, S36, S38 | S20, S23, S36 |
| [ADM-03](features/adm-03.md) | Operating system updates | 0 / 0 / 0 / 0 | 0% | S20, S36, S38 | S20, S23, S36 |
| [ADM-04](features/adm-04.md) | Panel updates and recovery | 12.5 / 0 / 0 / 0 | 12.5% | S20, S36, S38 | S20, S23, S36 |
| [ADM-05](features/adm-05.md) | User and role management | 12.5 / 0 / 0 / 0 | 12.5% | S20, S36, S38 | S20, S23, S36 |
| [ADM-06](features/adm-06.md) | Reseller hierarchy | 0 / 0 / 0 / 0 | 0% | S20, S36, S38 | S20, S23, S36 |
| [ADM-07](features/adm-07.md) | White-label branding | 0 / 0 / 0 / 0 | 0% | S20, S36, S38 | S20, S23, S36 |
| [ADM-08](features/adm-08.md) | HTTP API | 12.5 / 0 / 0 / 0 | 12.5% | S20, S36, S38 | S20, S23, S36 |
| [ADM-09](features/adm-09.md) | API keys | 0 / 0 / 0 / 0 | 0% | S20, S36, S38 | S20, S23, S36 |
| [ADM-10](features/adm-10.md) | API IP allowlists and rate limits | 0 / 0 / 0 / 0 | 0% | S20, S36, S38 | S20, S23, S36 |
| [ADM-11](features/adm-11.md) | Recovery CLI | 0 / 0 / 0 / 0 | 0% | S20, S36, S38 | S20, S23, S36 |
| [ADM-12](features/adm-12.md) | Secret management | 12.5 / 0 / 0 / 0 | 12.5% | S20, S36, S38 | S20, S23, S36 |
| [ADM-13](features/adm-13.md) | Infrastructure as Code | 0 / 0 / 0 / 0 | 0% | S20, S36, S38 | S20, S23, S36 |
| [ADM-14](features/adm-14.md) | Care plans | 0 / 0 / 0 / 0 | 0% | S20, S36, S38 | S20, S23, S36 |
| [ADM-15](features/adm-15.md) | Monthly client reports | 0 / 0 / 0 / 0 | 0% | S20, S36, S38 | S20, S23, S36 |
| [MIG-01](features/mig-01.md) | CloudPanel import | 0 / 0 / 0 / 0 | 0% | — | — |
| [MIG-02](features/mig-02.md) | CloudPanel SQLite discovery | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [MIG-03](features/mig-03.md) | HestiaCP import | 0 / 0 / 0 / 0 | 0% | — | — |
| [MIG-04](features/mig-04.md) | cPanel import | 0 / 0 / 0 / 0 | 0% | — | — |
| [MIG-05](features/mig-05.md) | Plesk import | 0 / 0 / 0 / 0 | 0% | — | — |
| [MIG-06](features/mig-06.md) | SSH migration | 0 / 0 / 0 / 0 | 0% | — | — |
| [MIG-07](features/mig-07.md) | Exporter-based migration | 0 / 0 / 0 / 0 | 0% | — | — |
| [MIG-08](features/mig-08.md) | Migration dry-run | 12.5 / 0 / 0 / 0 | 12.5% | — | — |
| [MIG-09](features/mig-09.md) | Same-server migration | 0 / 0 / 0 / 0 | 0% | — | — |
| [MIG-10](features/mig-10.md) | Whole-server migration rollback | 0 / 0 / 0 / 0 | 0% | — | — |
| [MIG-11](features/mig-11.md) | DNS and mail migration | 0 / 0 / 0 / 0 | 0% | — | — |
| [MIG-12](features/mig-12.md) | Migration downtime control | 0 / 0 / 0 / 0 | 0% | — | — |

## Arbeitspakete (63)

Paths match the session scope, with explicit adjacent mappings for monitoring, backup integration and versioning. Shared paths can appear under multiple packages. Missing paths may have been added and removed between the two cuts and therefore may not appear in the net diff. Issue state is independent of readiness.

| Package | Scope | Issue | 6 Nov score | Net paths | Paths still absent after #513 | Next acceptance evidence |
|---|---|---|---:|---:|---:|---|
| [S01](sessions/S01.md) | Domain model | [#3](https://github.com/scherenhaenden/FusionPanel/issues/3) · CLOSED | 60% | 0 | 0 | Exercise validated resource lifecycle through the integrated API |
| [S02](sessions/S02.md) | Control-plane API | [#1](https://github.com/scherenhaenden/FusionPanel/issues/1) · CLOSED | 100% | 1 | 0 | Real API binary plus real CLI, valid/invalid credentials |
| [S03](sessions/S03.md) | Agent protocol | [#2](https://github.com/scherenhaenden/FusionPanel/issues/2) · CLOSED | 60% | 0 | 0 | Real agent socket through control plane after S24 |
| [S04](sessions/S04.md) | Persistent jobs | [#5](https://github.com/scherenhaenden/FusionPanel/issues/5) · CLOSED | 80% | 0 | 0 | Kill/restart worker; reconcile running jobs without duplicate work |
| [S05](sessions/S05.md) | Administrative CLI | [#4](https://github.com/scherenhaenden/FusionPanel/issues/4) · CLOSED | 100% | 0 | 0 | Real CLI against real API process |
| [S06](sessions/S06.md) | Foundation integration | [#6](https://github.com/scherenhaenden/FusionPanel/issues/6) · OPEN | 30% | 1 | 0 | Run test harness against built binaries |
| [S07](sessions/S07.md) | Real test laboratory | [#10](https://github.com/scherenhaenden/FusionPanel/issues/10) · CLOSED | 100% | 0 | 0 | API/CLI process journey; later browser/VM suites must fail when prerequisite missing |
| [S08](sessions/S08.md) | Identity and permissions | [#11](https://github.com/scherenhaenden/FusionPanel/issues/11) · CLOSED | 80% | 0 | 0 | Denied cross-tenant API requests after S25 |
| [S09](sessions/S09.md) | Hosting plans and configuration | [#12](https://github.com/scherenhaenden/FusionPanel/issues/12) · OPEN | 60% | 14 | 10 | Create domain, serve uploaded content and undo failed provision after S26 |
| [S10](sessions/S10.md) | Database adapters | [#13](https://github.com/scherenhaenden/FusionPanel/issues/13) · OPEN | 60% | 31 | 14 | App writes data then restores it on a separate DB instance |
| [S11](sessions/S11.md) | Backup and recovery core | [#15](https://github.com/scherenhaenden/FusionPanel/issues/15) · CLOSED | 80% | 3 | 0 | Restore to a separate target and verify file hashes and application records |
| [S12](sessions/S12.md) | Container management | [#14](https://github.com/scherenhaenden/FusionPanel/issues/14) · CLOSED | 60% | 27 | 12 | Deploy stateful service, restart it and prove volume data survives |
| [S13](sessions/S13.md) | DNS management | [#16](https://github.com/scherenhaenden/FusionPanel/issues/16) · OPEN | 60% | 14 | 10 | Create/update record, resolve via DNS and verify exported zone restoration |
| [S14](sessions/S14.md) | Mail management | [#17](https://github.com/scherenhaenden/FusionPanel/issues/17) · OPEN | 60% | 21 | 12 | Mailbox-to-mailbox message delivery and retrieval; external deliverability remains separate |
| [S15](sessions/S15.md) | Host security controls | [#23](https://github.com/scherenhaenden/FusionPanel/issues/23) · OPEN | 60% | 15 | 6 | Benign requests succeed, malicious requests blocked, emergency access preserved |
| [S16](sessions/S16.md) | Metrics, alerts and incidents | [#22](https://github.com/scherenhaenden/FusionPanel/issues/22) · CLOSED | 70% | 5 | 6 | Trigger a real fixture outage, receive alert and resolve incident |
| [S17](sessions/S17.md) | Unified administration frontend | [#18](https://github.com/scherenhaenden/FusionPanel/issues/18) · CLOSED | 50% | 0 | 0 | Playwright against real API, including unauthorized and unavailable states |
| [S18](sessions/S18.md) | Migration discovery and planner | [#19](https://github.com/scherenhaenden/FusionPanel/issues/19) · OPEN | 50% | 0 | 0 | Read sanitized server copy, plan import without changing source or target |
| [S19](sessions/S19.md) | Deployment workflows | [#20](https://github.com/scherenhaenden/FusionPanel/issues/20) · CLOSED | 60% | 0 | 0 | Deploy known commit, serve it, rollback and verify prior content |
| [S20](sessions/S20.md) | Administration and customer workflows | [#21](https://github.com/scherenhaenden/FusionPanel/issues/21) · CLOSED | 50% | 7 | 8 | Customer scopes and scheduled report delivery through integrated UI |
| [S21](sessions/S21.md) | Node management | [#24](https://github.com/scherenhaenden/FusionPanel/issues/24) · OPEN | 60% | 15 | 8 | Enrol two disposable nodes, revoke one and reject its further requests |
| [S22](sessions/S22.md) | WordPress management | [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) · OPEN | 50% | 0 | 0 | Update/stage/restore real WordPress and inspect content/database |
| [S23](sessions/S23.md) | Secrets and encryption | [#25](https://github.com/scherenhaenden/FusionPanel/issues/25) · OPEN | 70% | 0 | 2 | Inject secret into isolated service without logging it; recover after restore |
| [S24](sessions/S24.md) | Real node runtime | [#26](https://github.com/scherenhaenden/FusionPanel/issues/26) · CLOSED | 50% | 2 | 0 | Control plane performs real agent operation; interrupted work reconciles |
| [S25](sessions/S25.md) | Identity/API integration | [#31](https://github.com/scherenhaenden/FusionPanel/issues/31) · OPEN | 30% | 1 | 0 | Browser/CLI cannot access another tenant or retain revoked permission |
| [S26](sessions/S26.md) | Hosting runtime/API | [#28](https://github.com/scherenhaenden/FusionPanel/issues/28) · CLOSED | 30% | 14 | 10 | Serve multiple real domains, deploy writeable app and test failed provision rollback |
| [S27](sessions/S27.md) | Database/backup runtime | [#29](https://github.com/scherenhaenden/FusionPanel/issues/29) · OPEN | 30% | 34 | 14 | Create/write/backup/delete/restore and compare exact data |
| [S28](sessions/S28.md) | Container runtime/API | [#30](https://github.com/scherenhaenden/FusionPanel/issues/30) · OPEN | 30% | 27 | 12 | Create stateful app via API, inspect real service and retain data on restart |
| [S29](sessions/S29.md) | DNS/mail runtime/API | [#32](https://github.com/scherenhaenden/FusionPanel/issues/32) · OPEN | 30% | 35 | 22 | Create domain/mailbox through API, resolve records and send/retrieve actual mail |
| [S30](sessions/S30.md) | Deployments runtime/API | [#33](https://github.com/scherenhaenden/FusionPanel/issues/33) · OPEN | 30% | 0 | 0 | Change service content, intentionally fail release and rollback |
| [S31](sessions/S31.md) | Monitoring runtime/API | [#34](https://github.com/scherenhaenden/FusionPanel/issues/34) · OPEN | 30% | 5 | 6 | Kill fixture service and measure actual alert/incident recovery |
| [S32](sessions/S32.md) | Frontend live workflows | [#35](https://github.com/scherenhaenden/FusionPanel/issues/35) · OPEN | 30% | 0 | 0 | Provision and operate a real site/container from browser |
| [S33](sessions/S33.md) | WordPress runtime | [#36](https://github.com/scherenhaenden/FusionPanel/issues/36) · OPEN | 20% | 0 | 0 | Stage change, push, update, intentionally break plugin, recover real site |
| [S34](sessions/S34.md) | Multi-node runtime | [#37](https://github.com/scherenhaenden/FusionPanel/issues/37) · OPEN | 30% | 15 | 8 | Move site and its data between nodes, isolate failed node |
| [S35](sessions/S35.md) | Migration execution | [#38](https://github.com/scherenhaenden/FusionPanel/issues/38) · OPEN | 20% | 0 | 0 | Import sanitized CloudPanel workload and compare served content/data |
| [S36](sessions/S36.md) | Administration runtime | [#39](https://github.com/scherenhaenden/FusionPanel/issues/39) · CLOSED | 30% | 7 | 8 | Tenant-specific branding/limits and report delivered via live workflow |
| [S37](sessions/S37.md) | Security runtime | [#40](https://github.com/scherenhaenden/FusionPanel/issues/40) · OPEN | 30% | 15 | 6 | Block attack traffic while keeping administrative recovery working |
| [S38](sessions/S38.md) | Installer, update and recovery | [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) · OPEN | 50% | 5 | 0 | Failed update returns working services; local recovery works without UI |
| [S39](sessions/S39.md) | Remote backup destinations | [#41](https://github.com/scherenhaenden/FusionPanel/issues/41) · OPEN | 50% | 7 | 6 | Upload/verify/restore off-host; fail corrupt artifact and inaccessible key |
| [S40](sessions/S40.md) | Application templates and image scanning | [#43](https://github.com/scherenhaenden/FusionPanel/issues/43) · CLOSED | 50% | 0 | 2 | Launch representative stateful/stateless templates, gate vulnerable image |
| [S41](sessions/S41.md) | CI and build qualification | [#44](https://github.com/scherenhaenden/FusionPanel/issues/44) · OPEN | 30% | 1 | 1 | CI fails on deliberate broken fixture; evidence artifacts are preserved |
| [S42](sessions/S42.md) | Foundation and identity E2E | [#46](https://github.com/scherenhaenden/FusionPanel/issues/46) · OPEN | 30% | 0 | 0 | Auth success/deny/revoke, worker interruption and CLI exit behavior |
| [S43](sessions/S43.md) | Hosting and runtime E2E | [#45](https://github.com/scherenhaenden/FusionPanel/issues/45) · OPEN | 30% | 0 | 0 | Read/write/upload/cache/redirect, domain mismatch and rollback |
| [S44](sessions/S44.md) | Database and restore E2E | [#47](https://github.com/scherenhaenden/FusionPanel/issues/47) · OPEN | 30% | 0 | 0 | Exact content/schema/permission verification after destructive fixture loss |
| [S45](sessions/S45.md) | Containers and app catalog E2E | [#48](https://github.com/scherenhaenden/FusionPanel/issues/48) · OPEN | 30% | 0 | 0 | Stateful app restart/update, limits and isolation checked on real daemon |
| [S46](sessions/S46.md) | CI/CD release E2E | [#49](https://github.com/scherenhaenden/FusionPanel/issues/49) · OPEN | 30% | 0 | 0 | Observe changed response content; reject unsafe volume blue-green transition |
| [S47](sessions/S47.md) | WordPress E2E | [#50](https://github.com/scherenhaenden/FusionPanel/issues/50) · OPEN | 30% | 0 | 0 | Backup before update, repair and verify user content after restore |
| [S48](sessions/S48.md) | Observability E2E | [#51](https://github.com/scherenhaenden/FusionPanel/issues/51) · OPEN | 30% | 0 | 0 | Measured outage causes alert and recovery resolves incident without false healthy |
| [S49](sessions/S49.md) | Multi-node failure E2E | [#52](https://github.com/scherenhaenden/FusionPanel/issues/52) · OPEN | 30% | 0 | 0 | Kill node during move; preserve data and reject revoked node |
| [S50](sessions/S50.md) | DNS and mail E2E | [#54](https://github.com/scherenhaenden/FusionPanel/issues/54) · OPEN | 20% | 0 | 0 | Roundtrip message and zone restore; external reputation separately qualified |
| [S51](sessions/S51.md) | Administration and update E2E | [#53](https://github.com/scherenhaenden/FusionPanel/issues/53) · OPEN | 20% | 0 | 0 | Reboot/update failure plus emergency local recovery |
| [S52](sessions/S52.md) | Security and secrets qualification | [#55](https://github.com/scherenhaenden/FusionPanel/issues/55) · OPEN | 30% | 0 | 0 | Forbidden actions fail at real boundary and no canary secret leaks |
| [S53](sessions/S53.md) | Migration and rollback qualification | [#56](https://github.com/scherenhaenden/FusionPanel/issues/56) · OPEN | 30% | 0 | 0 | Compare files/data/cron/TLS; recover independently and measure interruption |
| [S54](sessions/S54.md) | Hosting/WordPress capability closure | [#57](https://github.com/scherenhaenden/FusionPanel/issues/57) · OPEN | 30% | 0 | 0 | Every committed capability has artifact or explicit unresolved blocker |
| [S55](sessions/S55.md) | Containers/deployments capability closure | [#58](https://github.com/scherenhaenden/FusionPanel/issues/58) · CLOSED | 30% | 0 | 0 | No feature quietly removed; unsupported hardware/auto-scale remains explicit |
| [S56](sessions/S56.md) | Data/network capability closure | [#59](https://github.com/scherenhaenden/FusionPanel/issues/59) · CLOSED | 30% | 0 | 0 | All modes have data-integrity evidence and recovery limits |
| [S57](sessions/S57.md) | Operations/administration capability closure | [#60](https://github.com/scherenhaenden/FusionPanel/issues/60) · CLOSED | 30% | 0 | 0 | Document each missing capability with owner and retest gate |
| [S58](sessions/S58.md) | Variant compatibility qualification | [#61](https://github.com/scherenhaenden/FusionPanel/issues/61) · CLOSED | 30% | 0 | 0 | Change mode safely with preview, export/backup and rollback when required |
| [S59](sessions/S59.md) | Performance and failure budget | [#62](https://github.com/scherenhaenden/FusionPanel/issues/62) · CLOSED | 30% | 0 | 0 | Published hardware/config, p95/p99, data integrity after interrupted operations |
| [S60](sessions/S60.md) | HA and autoscaling feasibility | [#63](https://github.com/scherenhaenden/FusionPanel/issues/63) · CLOSED | 30% | 0 | 0 | No HA/zero-downtime claim without measured failover and data safety |
| [S61](sessions/S61.md) | Release integration and regression | [#66](https://github.com/scherenhaenden/FusionPanel/issues/66) · CLOSED | 30% | 2 | 0 | Fresh machine install and selected full browser/API/CLI journeys |
| [S62](sessions/S62.md) | Final recovery and operator handbook | [#64](https://github.com/scherenhaenden/FusionPanel/issues/64) · CLOSED | 30% | 0 | 0 | Operator follows documented commands and serves recovered application/data |
| [S63](sessions/S63.md) | Final Jules correction reserve | [#65](https://github.com/scherenhaenden/FusionPanel/issues/65) · OPEN | 20% | 0 | 0 | Release status honest: accepted, blocked and deferred with no hidden losses |

## Paths still absent after [#513](https://github.com/scherenhaenden/FusionPanel/pull/513)

Paths in the parent tree that [#513](https://github.com/scherenhaenden/FusionPanel/pull/513) deleted and that are absent from [`3e7862c`](https://github.com/scherenhaenden/FusionPanel/commit/3e7862c5edbed38c50d04ff88beb43e5c7233a91). Consult the parent for restoration evidence. This list is separate from the net path inventory.

- `modules/administration/src/cron_guard.rs`
- `modules/administration/src/disk_quota.rs`
- `modules/administration/src/sysctl_tuner.rs`
- `modules/administration/src/systemd_profile.rs`
- `modules/administration/tests/unit_cron_guard.rs`
- `modules/administration/tests/unit_disk_quota.rs`
- `modules/administration/tests/unit_sysctl_tuner.rs`
- `modules/administration/tests/unit_systemd_profile.rs`
- `modules/app-catalog/src/dependency_solver.rs`
- `modules/app-catalog/tests/unit_dependency_solver.rs`
- `modules/backup-destinations/src/azure_sas.rs`
- `modules/backup-destinations/src/glacier_transition.rs`
- `modules/backup-destinations/src/s3_version_pruner.rs`
- `modules/backup-destinations/tests/unit_azure_sas.rs`
- `modules/backup-destinations/tests/unit_glacier_transition.rs`
- `modules/backup-destinations/tests/unit_s3_version_pruner.rs`
- `modules/containers/src/cpu_pinning.rs`
- `modules/containers/src/oom_priority.rs`
- `modules/containers/src/overlay_pruner.rs`
- `modules/containers/src/seccomp_profile.rs`
- `modules/containers/src/traffic_shaper.rs`
- `modules/containers/src/volume_cloner.rs`
- `modules/containers/tests/unit_cpu_pinning.rs`
- `modules/containers/tests/unit_oom_priority.rs`
- `modules/containers/tests/unit_overlay_pruner.rs`
- `modules/containers/tests/unit_seccomp_profile.rs`
- `modules/containers/tests/unit_traffic_shaper.rs`
- `modules/containers/tests/unit_volume_cloner.rs`
- `modules/databases/src/gtid_monitor.rs`
- `modules/databases/src/innodb_advisor.rs`
- `modules/databases/src/pgbouncer_config.rs`
- `modules/databases/src/postgres_freeze_guard.rs`
- `modules/databases/src/postgres_pool_guard.rs`
- `modules/databases/src/privilege_auditor.rs`
- `modules/databases/src/slow_query_analyzer.rs`
- `modules/databases/tests/unit_gtid_monitor.rs`
- `modules/databases/tests/unit_innodb_advisor.rs`
- `modules/databases/tests/unit_pgbouncer_config.rs`
- `modules/databases/tests/unit_postgres_freeze_guard.rs`
- `modules/databases/tests/unit_postgres_pool_guard.rs`
- `modules/databases/tests/unit_privilege_auditor.rs`
- `modules/databases/tests/unit_slow_query_analyzer.rs`
- `modules/dns/src/anycast_probe.rs`
- `modules/dns/src/cloudflare_sync.rs`
- `modules/dns/src/powerdns_sync.rs`
- `modules/dns/src/route53_sync.rs`
- `modules/dns/src/rpz_compiler.rs`
- `modules/dns/tests/unit_anycast_probe.rs`
- `modules/dns/tests/unit_cloudflare_sync.rs`
- `modules/dns/tests/unit_powerdns_sync.rs`
- `modules/dns/tests/unit_route53_sync.rs`
- `modules/dns/tests/unit_rpz_compiler.rs`
- `modules/hosting/src/brotli_dynamic.rs`
- `modules/hosting/src/fpm_profiler.rs`
- `modules/hosting/src/http3_quic.rs`
- `modules/hosting/src/modsecurity_rules.rs`
- `modules/hosting/src/wildcard_cert.rs`
- `modules/hosting/tests/unit_brotli_dynamic.rs`
- `modules/hosting/tests/unit_fpm_profiler.rs`
- `modules/hosting/tests/unit_http3_quic.rs`
- `modules/hosting/tests/unit_modsecurity_rules.rs`
- `modules/hosting/tests/unit_wildcard_cert.rs`
- `modules/mail/src/clamav_scanner.rs`
- `modules/mail/src/lmtp_routing.rs`
- `modules/mail/src/milter_headers.rs`
- `modules/mail/src/relay_transport.rs`
- `modules/mail/src/rspamd_rules.rs`
- `modules/mail/src/sasl_submission.rs`
- `modules/mail/tests/unit_clamav_scanner.rs`
- `modules/mail/tests/unit_lmtp_routing.rs`
- `modules/mail/tests/unit_milter_headers.rs`
- `modules/mail/tests/unit_relay_transport.rs`
- `modules/mail/tests/unit_rspamd_rules.rs`
- `modules/mail/tests/unit_sasl_submission.rs`
- `modules/monitoring/src/io_psi.rs`
- `modules/monitoring/src/socket_monitor.rs`
- `modules/monitoring/tests/unit_io_psi.rs`
- `modules/monitoring/tests/unit_socket_monitor.rs`
- `modules/nodes/src/affinity_evaluator.rs`
- `modules/nodes/src/ping_matrix.rs`
- `modules/nodes/src/raft_state.rs`
- `modules/nodes/src/wireguard_mesh.rs`
- `modules/nodes/tests/unit_affinity_evaluator.rs`
- `modules/nodes/tests/unit_ping_matrix.rs`
- `modules/nodes/tests/unit_raft_state.rs`
- `modules/nodes/tests/unit_wireguard_mesh.rs`
- `modules/observability/src/webhook_signer.rs`
- `modules/observability/tests/unit_webhook_signer.rs`
- `modules/secrets/src/key_rotation.rs`
- `modules/secrets/tests/unit_key_rotation.rs`
- `modules/security/src/apparmor_profile.rs`
- `modules/security/src/auditd_parser.rs`
- `modules/security/src/bot_jail.rs`
- `modules/security/tests/unit_apparmor_profile.rs`
- `modules/security/tests/unit_auditd_parser.rs`
- `modules/security/tests/unit_bot_jail.rs`
- `scripts/ci/tests/test_discover.py`
- `update_lib.patch`

## First-parent history

| Commit | PR | Subject | Path changes | Package path matches |
|---|---|---|---:|---|
| [`3e7862c`](https://github.com/scherenhaenden/FusionPanel/commit/3e7862c5edbed38c50d04ff88beb43e5c7233a91) | — | fix(security): restore port_knocker module and unit tests | 2 | S15, S37 |
| [`d4cc496`](https://github.com/scherenhaenden/FusionPanel/commit/d4cc496e0b19d5a1778536e7c10af310c9d1a9f5) | [#543](https://github.com/scherenhaenden/FusionPanel/pull/543) | feature-apparmor-seccomp-enforcement-11681019375372735549 | 19 | S12, S28 |
| [`304e925`](https://github.com/scherenhaenden/FusionPanel/commit/304e9255de9e31141bda4fa4805e2bf29f67cc2d) | [#563](https://github.com/scherenhaenden/FusionPanel/pull/563) | feat/dnssec-key-rollover-planner-10980647979519042231 | 0 | — |
| [`e5eda39`](https://github.com/scherenhaenden/FusionPanel/commit/e5eda395bfe8405a0f0811acf99c4013aead008c) | [#562](https://github.com/scherenhaenden/FusionPanel/pull/562) | feat/cluster-peer-heartbeat-ring-6606554404379807153 | 0 | — |
| [`f63909d`](https://github.com/scherenhaenden/FusionPanel/commit/f63909d6ec9f06f497f97b437f0c5fae91b56211) | [#513](https://github.com/scherenhaenden/FusionPanel/pull/513) | feat-geoip-filter-2231018939930381381 | 130 | S09, S10, S12, S13, S14, S15, S16, S20, S21, S23, S26, S27, S28, S29, S31, S34, S36, S37, S39, S40, S41 |
| [`bb3c2f3`](https://github.com/scherenhaenden/FusionPanel/commit/bb3c2f36d8d7387ba07b2ce2596e9c97f264fb70) | [#471](https://github.com/scherenhaenden/FusionPanel/pull/471) | feat-opcache-advisor-15276425657055218398 | 3 | S09, S26 |
| [`cfdbfe2`](https://github.com/scherenhaenden/FusionPanel/commit/cfdbfe2813cc1881b2ef270b9ff9b9788bd9a254) | [#561](https://github.com/scherenhaenden/FusionPanel/pull/561) | feature-gfs-retention-planner-10646283032682581421 | 3 | S11, S27 |
| [`65be3cc`](https://github.com/scherenhaenden/FusionPanel/commit/65be3cc84aab04f9bc47de726c2b22d6543eb9a4) | [#560](https://github.com/scherenhaenden/FusionPanel/pull/560) | feature-etckeeper-guard-8265824384959626622 | 3 | S20, S36 |
| [`c60c12d`](https://github.com/scherenhaenden/FusionPanel/commit/c60c12d842b776d36ead499c2ab727cc1f7db766) | [#559](https://github.com/scherenhaenden/FusionPanel/pull/559) | feat-pci-dss-auditor-7485329532891010892 | 3 | S15, S37 |
| [`776fe57`](https://github.com/scherenhaenden/FusionPanel/commit/776fe573d6c010c3b8cf6236085682381d4bffd0) | [#558](https://github.com/scherenhaenden/FusionPanel/pull/558) | implement-mesh-rekey-scheduler-13345317471899908581 | 9 | S21, S34 |
| [`311833d`](https://github.com/scherenhaenden/FusionPanel/commit/311833d707d574d71bccfecb2af87def41753e06) | [#557](https://github.com/scherenhaenden/FusionPanel/pull/557) | feat-tls-ticket-rotation-12846405295575459951 | 4 | S09, S26 |
| [`0e1b2e4`](https://github.com/scherenhaenden/FusionPanel/commit/0e1b2e4180289654cdb9b0b10558ec86d269cefb) | [#556](https://github.com/scherenhaenden/FusionPanel/pull/556) | feature-logrotate-generator-3703361915982178185 | 3 | S20, S36 |
| [`d092749`](https://github.com/scherenhaenden/FusionPanel/commit/d092749730931e6cdc542eba65373fa01132ad78) | [#554](https://github.com/scherenhaenden/FusionPanel/pull/554) | feat-mysql-slow-query-parser-11105659302654442916 | 4 | S10, S27 |
| [`cb829d7`](https://github.com/scherenhaenden/FusionPanel/commit/cb829d72c5e949d6bc874950f6dcc92e3c559feb) | [#553](https://github.com/scherenhaenden/FusionPanel/pull/553) | feat-ssh-key-enforcer-8250135918205503674 | 3 | S15, S37 |
| [`cf7ebb5`](https://github.com/scherenhaenden/FusionPanel/commit/cf7ebb5c5543cb91f63f33041fda8ec74471ef5f) | [#552](https://github.com/scherenhaenden/FusionPanel/pull/552) | feat-suid-sgid-scanner-13876688975515368148 | 3 | S15, S37 |
| [`4f9b0e5`](https://github.com/scherenhaenden/FusionPanel/commit/4f9b0e5a6caf731385a96870793af4575006fee0) | [#551](https://github.com/scherenhaenden/FusionPanel/pull/551) | feat-restic-repo-guard-3670638335935455542 | 3 | S39 |
| [`b8c4ecf`](https://github.com/scherenhaenden/FusionPanel/commit/b8c4ecf790f8f281e350a31cad4aa155040cf86d) | [#550](https://github.com/scherenhaenden/FusionPanel/pull/550) | feature-brotli-compression-tuner-3775271974061958427 | 3 | S09, S26 |
| [`91ca096`](https://github.com/scherenhaenden/FusionPanel/commit/91ca0960c8680030c4f898da5a64d4fd5c8a7929) | [#549](https://github.com/scherenhaenden/FusionPanel/pull/549) | feat-pg-replication-slot-monitor-3206880478217886693 | 3 | S10, S27 |
| [`4b362b0`](https://github.com/scherenhaenden/FusionPanel/commit/4b362b0abecf83171f5cd7f883a50c5b080eef25) | [#548](https://github.com/scherenhaenden/FusionPanel/pull/548) | feature-arc-seal-validator-14439891420000300329 | 3 | S14, S29 |
| [`8526c78`](https://github.com/scherenhaenden/FusionPanel/commit/8526c78a774cfe1d766396476e80ba610c253c05) | [#547](https://github.com/scherenhaenden/FusionPanel/pull/547) | feat-dnssec-key-rollover-planner-10980647979519042231 | 3 | S13, S29 |
| [`ae27f4a`](https://github.com/scherenhaenden/FusionPanel/commit/ae27f4a818fba69e1fff075a2118336b9b3c8e39) | [#546](https://github.com/scherenhaenden/FusionPanel/pull/546) | feat-cgroup-v2-psi-monitor-10008328234210983387 | 3 | S12, S28 |
| [`8da5de3`](https://github.com/scherenhaenden/FusionPanel/commit/8da5de3bedd7a16f243eab6ca80c4d6871ce26bb) | [#545](https://github.com/scherenhaenden/FusionPanel/pull/545) | feat-cluster-peer-heartbeat-ring-6606554404379807153 | 3 | S21, S34 |
| [`70e6bb1`](https://github.com/scherenhaenden/FusionPanel/commit/70e6bb1f9beb69d225146ccd4035662068c9a5f6) | [#544](https://github.com/scherenhaenden/FusionPanel/pull/544) | feature-sieve-vacation-generator-9633993941947735195 | 13 | S14, S29 |
| [`79c04a1`](https://github.com/scherenhaenden/FusionPanel/commit/79c04a19a4b2fde71edf87ea3d3402a6d1b5e900) | [#542](https://github.com/scherenhaenden/FusionPanel/pull/542) | feat-http2-stream-graph-1744383911121876417 | 3 | S09, S26 |
| [`20c0391`](https://github.com/scherenhaenden/FusionPanel/commit/20c03913437caa460e69b588bcec07008d991cb8) | [#541](https://github.com/scherenhaenden/FusionPanel/pull/541) | feat-thread-pool-guard-10908047068922169822 | 3 | S10, S27 |
| [`df3d01a`](https://github.com/scherenhaenden/FusionPanel/commit/df3d01acba503030832d4f083c9f7bd9e2d7f93c) | [#540](https://github.com/scherenhaenden/FusionPanel/pull/540) | feat-stat-statements-analyzer-17352149652393006119 | 18 | S10, S27 |
| [`60e7c60`](https://github.com/scherenhaenden/FusionPanel/commit/60e7c6071849281b428efaab7f568758928400d5) | [#539](https://github.com/scherenhaenden/FusionPanel/pull/539) | feat-rrl-config-13160597031516906921 | 3 | S13, S29 |
| [`7d1a811`](https://github.com/scherenhaenden/FusionPanel/commit/7d1a811f2806d42df55e2447961706ca1a6cb564) | [#538](https://github.com/scherenhaenden/FusionPanel/pull/538) | feat-fts-indexer-1055130148497942399 | 3 | S14, S29 |
| [`5a12425`](https://github.com/scherenhaenden/FusionPanel/commit/5a12425110181c6bd93899a37abc84669e5598ed) | [#537](https://github.com/scherenhaenden/FusionPanel/pull/537) | feat-notify-engine-8463130667903862700 | 3 | S13, S29 |
| [`43bce99`](https://github.com/scherenhaenden/FusionPanel/commit/43bce9901882c1a1696441e3516f2c72493ec859) | [#536](https://github.com/scherenhaenden/FusionPanel/pull/536) | feat-readonly-guard-16432752078260118968 | 3 | S12, S28 |
| [`4f912fb`](https://github.com/scherenhaenden/FusionPanel/commit/4f912fbdc2f7e0d1c8eea4144f78dfc8dbc8858f) | [#535](https://github.com/scherenhaenden/FusionPanel/pull/535) | feat-mtu-clamper-864201113504420316 | 3 | S12, S28 |
| [`8fcbe3c`](https://github.com/scherenhaenden/FusionPanel/commit/8fcbe3c5663c14a7cad88d836833f93b80e43052) | [#534](https://github.com/scherenhaenden/FusionPanel/pull/534) | feat-gossip-membership-15508155181340036497 | 3 | S21, S34 |
| [`858acfd`](https://github.com/scherenhaenden/FusionPanel/commit/858acfdcfd0e9e1ef5bd47489d63d00ca2a86263) | [#533](https://github.com/scherenhaenden/FusionPanel/pull/533) | feat-xdp-filter-361710831812166404 | 3 | S15, S37 |
| [`3a2f15d`](https://github.com/scherenhaenden/FusionPanel/commit/3a2f15d51d78e89dcb71a1d9ed0bd6d8f256eb1c) | [#532](https://github.com/scherenhaenden/FusionPanel/pull/532) | feat-webauthn-store-5512340523623077872 | 3 | S15, S37 |
| [`9727e18`](https://github.com/scherenhaenden/FusionPanel/commit/9727e18f4939574121976f3281cb00a4362104ab) | [#531](https://github.com/scherenhaenden/FusionPanel/pull/531) | feat-journald-tuner-14822404806349552859 | 3 | S20, S36 |
| [`6b936d8`](https://github.com/scherenhaenden/FusionPanel/commit/6b936d8f294d8e0aeb0dc30013ca18e48b5e8986) | [#530](https://github.com/scherenhaenden/FusionPanel/pull/530) | feat-s3-compat-matrix-1742363632920917360 | 3 | S39 |
| [`157ba15`](https://github.com/scherenhaenden/FusionPanel/commit/157ba15a638b9c417a7073281f5f1bef312ee81e) | [#529](https://github.com/scherenhaenden/FusionPanel/pull/529) | feature-fastcgi-keepalive-tuner-11277446159263573201 | 3 | S09, S26 |
| [`8298c50`](https://github.com/scherenhaenden/FusionPanel/commit/8298c50fbacd43aa7faafc35a7519abaf8000d02) | [#528](https://github.com/scherenhaenden/FusionPanel/pull/528) | feat-postgres-checkpointer-tuner-5436280462720864360 | 3 | S10, S27 |
| [`b260ce2`](https://github.com/scherenhaenden/FusionPanel/commit/b260ce2df3427b53cbd016d37b23777535bcc72d) | [#527](https://github.com/scherenhaenden/FusionPanel/pull/527) | feat-bbr-advisor-12016124823322572550 | 3 | S16, S31 |
| [`94672fb`](https://github.com/scherenhaenden/FusionPanel/commit/94672fb50a178e7fa0638c368476a0c21d0fe8a5) | [#526](https://github.com/scherenhaenden/FusionPanel/pull/526) | feat-redis-sentinel-11836622587518905407 | 3 | S10, S27 |
| [`8ea37bf`](https://github.com/scherenhaenden/FusionPanel/commit/8ea37bfb8ffea7dca277bfad8de1a2ca48ee440a) | [#525](https://github.com/scherenhaenden/FusionPanel/pull/525) | feature-postscreen-dnsbl-aggregator-13887847749100190830 | 3 | S14, S29 |
| [`cd01239`](https://github.com/scherenhaenden/FusionPanel/commit/cd01239ba439aaa73854cedc5cec987813fc12d2) | [#524](https://github.com/scherenhaenden/FusionPanel/pull/524) | feat-dns-over-tls-forwarding-11972480072133115196 | 3 | S13, S29 |
| [`acd5cb5`](https://github.com/scherenhaenden/FusionPanel/commit/acd5cb52eb159a7d8efa074fc77e56819fee5163) | [#523](https://github.com/scherenhaenden/FusionPanel/pull/523) | feat-idn-punycode-converter-5764798841007224079 | 4 | S13, S29 |
| [`1e4087b`](https://github.com/scherenhaenden/FusionPanel/commit/1e4087baf31a37a5dc9ea10b94f2b87a9d8b1f5c) | [#522](https://github.com/scherenhaenden/FusionPanel/pull/522) | containers-circuit-breaker-4712041475262764298 | 3 | S12, S28 |
| [`0d7d3af`](https://github.com/scherenhaenden/FusionPanel/commit/0d7d3af8e44348f1d2be2a5a20fa30ff74756faf) | [#521](https://github.com/scherenhaenden/FusionPanel/pull/521) | feat-subnet-escalator-16155079358726485112 | 4 | S15, S37 |
| [`9ef8cde`](https://github.com/scherenhaenden/FusionPanel/commit/9ef8cdeac95b78c2254d4d52a012a5f46922acb1) | [#520](https://github.com/scherenhaenden/FusionPanel/pull/520) | feat-psi-parser-2348198365153439219 | 3 | S16, S31 |
| [`56c8732`](https://github.com/scherenhaenden/FusionPanel/commit/56c8732c099689e8f7b9686794ab8b984ddb042c) | [#519](https://github.com/scherenhaenden/FusionPanel/pull/519) | feat-tsig-keyring-10268205592486424301 | 3 | S13, S29 |
| [`8c22a96`](https://github.com/scherenhaenden/FusionPanel/commit/8c22a9673419512ceee4715774578f06a8e8ddae) | [#518](https://github.com/scherenhaenden/FusionPanel/pull/518) | feat-maildir-flags-16517902653243462585 | 3 | S14, S29 |
| [`9aaa544`](https://github.com/scherenhaenden/FusionPanel/commit/9aaa54472bdadc5be69222c6fb742600ffef832a) | [#517](https://github.com/scherenhaenden/FusionPanel/pull/517) | feat-postgres-pitr-1008283577261733557 | 3 | S10, S27 |
| [`8cef194`](https://github.com/scherenhaenden/FusionPanel/commit/8cef19434928023de8aa42a5a1298b46f3396b91) | [#516](https://github.com/scherenhaenden/FusionPanel/pull/516) | feat-ephemeral-storage-3514821381974609761 | 3 | S12, S28 |
| [`973b7a0`](https://github.com/scherenhaenden/FusionPanel/commit/973b7a071d1bd5ff90375564b0847ed1e2b7d874) | [#515](https://github.com/scherenhaenden/FusionPanel/pull/515) | feat-task-retry-16409972641168590458 | 3 | S21, S34 |
| [`6e59292`](https://github.com/scherenhaenden/FusionPanel/commit/6e592928e42701a061faed269798bc540429f853) | [#514](https://github.com/scherenhaenden/FusionPanel/pull/514) | feat-presigned-url-4475426211885725962 | 3 | S39 |
| [`34d1055`](https://github.com/scherenhaenden/FusionPanel/commit/34d105552ce84ffb1d851a582d606e112c59fe25) | [#512](https://github.com/scherenhaenden/FusionPanel/pull/512) | feat-privilege-auditor-2070126655676796877 | 3 | S10, S27 |
| [`19e5410`](https://github.com/scherenhaenden/FusionPanel/commit/19e541050679e2d46f4d78a8b9cb6889035a69d9) | [#511](https://github.com/scherenhaenden/FusionPanel/pull/511) | feat-cpu-pinning-15827905945041075896 | 17 | S12, S28 |
| [`0f1b00b`](https://github.com/scherenhaenden/FusionPanel/commit/0f1b00b18b78dc470e612d5446601909caef4fd2) | [#510](https://github.com/scherenhaenden/FusionPanel/pull/510) | feat-milter-headers-13510836917297753446 | 3 | S14, S29 |
| [`c72bf57`](https://github.com/scherenhaenden/FusionPanel/commit/c72bf57f279e44c338f643fd5540bd2ecdb0c7fe) | [#509](https://github.com/scherenhaenden/FusionPanel/pull/509) | feat-bot-jail-6276082908907441609 | 3 | S15, S37 |
| [`dcaa65a`](https://github.com/scherenhaenden/FusionPanel/commit/dcaa65a477daec8d9c949f1bd08f4753245b7cce) | [#508](https://github.com/scherenhaenden/FusionPanel/pull/508) | feat-raft-state-18015781409357511500 | 3 | S21, S34 |
| [`b9c2bf0`](https://github.com/scherenhaenden/FusionPanel/commit/b9c2bf07480a43dbc1b01fae36d02832aada17a7) | [#507](https://github.com/scherenhaenden/FusionPanel/pull/507) | feat-systemd-profile-8211687768260064507 | 3 | S20, S36 |
| [`6469957`](https://github.com/scherenhaenden/FusionPanel/commit/646995774df11d3057291610cfd494f6896264c7) | [#506](https://github.com/scherenhaenden/FusionPanel/pull/506) | feat-cloudflare-sync-10777489175945898073 | 3 | S13, S29 |
| [`963fc73`](https://github.com/scherenhaenden/FusionPanel/commit/963fc73f6077d71d06f5c1445d250584039ab42e) | [#505](https://github.com/scherenhaenden/FusionPanel/pull/505) | feat-glacier-transition-7560231898375284491 | 3 | S39 |
| [`ead06af`](https://github.com/scherenhaenden/FusionPanel/commit/ead06affce56835876ff9ad9477132be8f32436e) | [#504](https://github.com/scherenhaenden/FusionPanel/pull/504) | feat-io-psi-10764498654153205174 | 3 | S16, S31 |
| [`a7416bc`](https://github.com/scherenhaenden/FusionPanel/commit/a7416bc04f1f087e290278643bdc58e8e7321d4a) | [#503](https://github.com/scherenhaenden/FusionPanel/pull/503) | feat-wildcard-cert-9125696981338313320 | 3 | S09, S26 |
| [`eb66f74`](https://github.com/scherenhaenden/FusionPanel/commit/eb66f7403e7f1305186fa5e99a850eff07547a67) | [#502](https://github.com/scherenhaenden/FusionPanel/pull/502) | feat-postgres-pool-guard-5192211643122899197 | 3 | S10, S27 |
| [`544acfb`](https://github.com/scherenhaenden/FusionPanel/commit/544acfb0f612b40b7ccfa677a76b4e84f8f62ca4) | [#501](https://github.com/scherenhaenden/FusionPanel/pull/501) | feat-s3-version-pruner-4853875392885789723 | 3 | S39 |
| [`41a64e5`](https://github.com/scherenhaenden/FusionPanel/commit/41a64e5e12426fa88de73349b29942efa14279ce) | [#500](https://github.com/scherenhaenden/FusionPanel/pull/500) | feat-lmtp-routing-18317220272427888542 | 3 | S14, S29 |
| [`e19e635`](https://github.com/scherenhaenden/FusionPanel/commit/e19e63573198dfae662112d687e029a265296360) | [#499](https://github.com/scherenhaenden/FusionPanel/pull/499) | feat-volume-cloner-9304041631599912816 | 3 | S12, S28 |
| [`7cbdcc1`](https://github.com/scherenhaenden/FusionPanel/commit/7cbdcc1bc867eebb7c581908afa943eb74c2db9c) | [#498](https://github.com/scherenhaenden/FusionPanel/pull/498) | feat-disk-quota-14075380488488213298 | 3 | S20, S36 |
| [`80e2faf`](https://github.com/scherenhaenden/FusionPanel/commit/80e2faf6b3c0c74e813f3f3dd6e2795f24039474) | [#497](https://github.com/scherenhaenden/FusionPanel/pull/497) | feat-ping-matrix-10777489175945898631 | 3 | S21, S34 |
| [`6f12df4`](https://github.com/scherenhaenden/FusionPanel/commit/6f12df426d6daed3ddb2c3d957fc12ce2c563f36) | [#496](https://github.com/scherenhaenden/FusionPanel/pull/496) | feat-webhook-signer-15909132215857630153 | 3 | S16, S31 |
| [`e860444`](https://github.com/scherenhaenden/FusionPanel/commit/e8604445cd5c3eb508a4e902f9b32d5f54e644fe) | [#495](https://github.com/scherenhaenden/FusionPanel/pull/495) | feat-slow-query-analyzer-5119063009506332351 | 3 | S10, S27 |
| [`6b5fd81`](https://github.com/scherenhaenden/FusionPanel/commit/6b5fd81d1d9f428138168267c189807d20371add) | [#494](https://github.com/scherenhaenden/FusionPanel/pull/494) | feat-modsecurity-rules-14766132670256904267 | 3 | S09, S26 |
| [`afe0fa5`](https://github.com/scherenhaenden/FusionPanel/commit/afe0fa52419f86b5ee868e19102f8a51fb006d9a) | [#493](https://github.com/scherenhaenden/FusionPanel/pull/493) | feat-http3-quic-18205107536396526512 | 3 | S09, S26 |
| [`89fbcc8`](https://github.com/scherenhaenden/FusionPanel/commit/89fbcc82505ebea377202e47cae1bf0a7e181c00) | [#492](https://github.com/scherenhaenden/FusionPanel/pull/492) | feat-affinity-evaluator-1139921280432412740 | 3 | S21, S34 |
| [`8164d84`](https://github.com/scherenhaenden/FusionPanel/commit/8164d844c7c470a6af5f58202986fa267fdb99cf) | [#491](https://github.com/scherenhaenden/FusionPanel/pull/491) | feat-fpm-profiler-13068136758291919729 | 3 | S09, S26 |
| [`c1e25fa`](https://github.com/scherenhaenden/FusionPanel/commit/c1e25fae7be74a29ae54b2d847fa314720734a12) | [#490](https://github.com/scherenhaenden/FusionPanel/pull/490) | feat-key-rotation-7453504326100388843 | 3 | S23 |
| [`7a4576b`](https://github.com/scherenhaenden/FusionPanel/commit/7a4576b131b77f934906e3f74bac150e6b433395) | [#489](https://github.com/scherenhaenden/FusionPanel/pull/489) | feat-dependency-solver-129417008352044174 | 3 | S40 |
| [`15203c0`](https://github.com/scherenhaenden/FusionPanel/commit/15203c078afa206e9d0bca07ca5144b72bbb5f7d) | [#488](https://github.com/scherenhaenden/FusionPanel/pull/488) | feat-innodb-advisor-17014878596270994524 | 3 | S10, S27 |
| [`122d495`](https://github.com/scherenhaenden/FusionPanel/commit/122d4956ff61f8726af06f5f35280a036cc262a3) | [#487](https://github.com/scherenhaenden/FusionPanel/pull/487) | feat-postgres-freeze-guard-2228759770189095190 | 3 | S10, S27 |
| [`069bdd0`](https://github.com/scherenhaenden/FusionPanel/commit/069bdd016e0063a512d7fc9aa27996b7389b9bc7) | [#486](https://github.com/scherenhaenden/FusionPanel/pull/486) | feat-relay-transport-2291230800722767679 | 3 | S14, S29 |
| [`611d60c`](https://github.com/scherenhaenden/FusionPanel/commit/611d60cc1c663b4c2aea8e1cf2b2103e1b00ad98) | [#485](https://github.com/scherenhaenden/FusionPanel/pull/485) | feat-rspamd-rules-11709727394547030456 | 3 | S14, S29 |
| [`c71eae3`](https://github.com/scherenhaenden/FusionPanel/commit/c71eae367404e9ccc6924cbfe5e942a673497e26) | [#484](https://github.com/scherenhaenden/FusionPanel/pull/484) | feat-overlay-pruner-5020351163078483705 | 3 | S12, S28 |
| [`15a7d25`](https://github.com/scherenhaenden/FusionPanel/commit/15a7d25d20f4608926267c35b01d7dcfbaa1e75d) | [#483](https://github.com/scherenhaenden/FusionPanel/pull/483) | feat-port-knocker-9066525518240239116 | 3 | S15, S37 |
| [`e01b814`](https://github.com/scherenhaenden/FusionPanel/commit/e01b814948fa4a4121fde0f1746e8ae512a80ce9) | [#482](https://github.com/scherenhaenden/FusionPanel/pull/482) | feat-route53-sync-10947149470500392888 | 3 | S13, S29 |
| [`389fca0`](https://github.com/scherenhaenden/FusionPanel/commit/389fca00aa698870fb7f0c51daad9deed20df5ac) | [#481](https://github.com/scherenhaenden/FusionPanel/pull/481) | feat-rpz-compiler-14626648806012498337 | 3 | S13, S29 |
| [`6e1b525`](https://github.com/scherenhaenden/FusionPanel/commit/6e1b525b73e7e7c148421465105f9a8fc2dbad1c) | [#480](https://github.com/scherenhaenden/FusionPanel/pull/480) | feat-seccomp-profile-9423020060855820019 | 3 | S12, S28 |
| [`7a98388`](https://github.com/scherenhaenden/FusionPanel/commit/7a9838884a187d684da948f2b94e4e396b248ba6) | [#479](https://github.com/scherenhaenden/FusionPanel/pull/479) | feat-auditd-parser-8174800788173923495 | 3 | S15, S37 |
| [`19e35dc`](https://github.com/scherenhaenden/FusionPanel/commit/19e35dca54fad9c401a5df45510d00ad5ec08e97) | [#478](https://github.com/scherenhaenden/FusionPanel/pull/478) | feat-sysctl-tuner-17069344411873414415 | 3 | S20, S36 |
| [`17ca3ec`](https://github.com/scherenhaenden/FusionPanel/commit/17ca3ec8e09f05e24536fd4a29b877bb9c7083f3) | [#477](https://github.com/scherenhaenden/FusionPanel/pull/477) | feat-azure-sas-5852312052243377672 | 3 | S39 |
| [`e5e4806`](https://github.com/scherenhaenden/FusionPanel/commit/e5e4806a3d5de46628690b1cf67155653e2fc086) | [#476](https://github.com/scherenhaenden/FusionPanel/pull/476) | feat-socket-monitor-8490157197337157423 | 3 | S16, S31 |
| [`5091145`](https://github.com/scherenhaenden/FusionPanel/commit/5091145d01b216eef1904f4aa62551f6e74913f2) | [#475](https://github.com/scherenhaenden/FusionPanel/pull/475) | feat-gtid-monitor-3087394270700325899 | 3 | S10, S27 |
| [`4135dd9`](https://github.com/scherenhaenden/FusionPanel/commit/4135dd9b23fdfa8c4ac30c0dc609a7595b81e57a) | [#474](https://github.com/scherenhaenden/FusionPanel/pull/474) | feat-wireguard-mesh-14560488091646215764 | 3 | S21, S34 |
| [`7659faa`](https://github.com/scherenhaenden/FusionPanel/commit/7659faa6c6c8cd8bcf2a70af7f2d424abda0dfa5) | [#473](https://github.com/scherenhaenden/FusionPanel/pull/473) | feat-cron-guard-7810111985278871012 | 3 | S20, S36 |
| [`fde62e1`](https://github.com/scherenhaenden/FusionPanel/commit/fde62e1aec88b8d2a6f0ac1f20cd8eae64c41ec3) | [#472](https://github.com/scherenhaenden/FusionPanel/pull/472) | feat-brotli-dynamic-12070334988055174236 | 3 | S09, S26 |
| [`8166f15`](https://github.com/scherenhaenden/FusionPanel/commit/8166f15b1dbcc481d474bd712b8e245e59a73045) | [#470](https://github.com/scherenhaenden/FusionPanel/pull/470) | feat-pgbouncer-config-4285491412387264481 | 3 | S10, S27 |
| [`da72e11`](https://github.com/scherenhaenden/FusionPanel/commit/da72e1172259299e2ecf078f57b556d2b8e15059) | [#469](https://github.com/scherenhaenden/FusionPanel/pull/469) | feat-powerdns-sync-14471845844954110857 | 3 | S13, S29 |
| [`8dc9700`](https://github.com/scherenhaenden/FusionPanel/commit/8dc97009dd975166e3db896a9f082e0df37bef31) | [#468](https://github.com/scherenhaenden/FusionPanel/pull/468) | feat-sasl-submission-6505939454026859971 | 3 | S14, S29 |
| [`168e7a8`](https://github.com/scherenhaenden/FusionPanel/commit/168e7a8009c01da415a8af3608a4143b0264d703) | [#467](https://github.com/scherenhaenden/FusionPanel/pull/467) | feat-clamav-scanner-5832978399992902112 | 4 | S14, S29 |
| [`5f3184a`](https://github.com/scherenhaenden/FusionPanel/commit/5f3184aa38a75b045df7bc4e980cd97b5e52ad0f) | [#466](https://github.com/scherenhaenden/FusionPanel/pull/466) | feat-anycast-probe-14471845844954110360 | 3 | S13, S29 |
| [`12c8352`](https://github.com/scherenhaenden/FusionPanel/commit/12c83529b82455e7296fb350d91fc95358e8d817) | [#465](https://github.com/scherenhaenden/FusionPanel/pull/465) | feat-traffic-shaper-17830573011447086803 | 3 | S12, S28 |
| [`6a12ab7`](https://github.com/scherenhaenden/FusionPanel/commit/6a12ab7e77d7f18761d0d632e9ad0f944111fb5c) | [#464](https://github.com/scherenhaenden/FusionPanel/pull/464) | feat-oom-priority-7022218188586691300 | 3 | S12, S28 |
| [`72e2d1d`](https://github.com/scherenhaenden/FusionPanel/commit/72e2d1dc0e34d82e98422cd01e63588388677ff1) | [#463](https://github.com/scherenhaenden/FusionPanel/pull/463) | feat-apparmor-profile-10455875540694088089 | 3 | S15, S37 |
| [`9a8efd3`](https://github.com/scherenhaenden/FusionPanel/commit/9a8efd31e13d729b624e2c326867bb80d6251839) | [#462](https://github.com/scherenhaenden/FusionPanel/pull/462) | codex/workflow-ci-efficiency | 4 | S41 |
| [`494d9a2`](https://github.com/scherenhaenden/FusionPanel/commit/494d9a2aa4f224b5ca5eb1199e1d01af9cab65a1) | [#459](https://github.com/scherenhaenden/FusionPanel/pull/459) | codex/package-ci | 14 | S02, S06, S24, S25, S38, S41, S61 |

[Full per-commit and per-package path data](READINESS_SNAPSHOT_2026-11-10.json)
