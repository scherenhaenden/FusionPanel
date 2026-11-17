# Sprint 7 Plan — DNS Manager & DKIM Sync

**Base branch:** `sprint-7` (branched from `master` at `65f3afa`)  
**Target release:** `0.8.0-dev.1`

## Goal
Enable operators to manage tenant-scoped authoritative DNS zones and records (`A`, `AAAA`, `CNAME`, `MX`, `TXT`), sync DKIM TXT records (`<selector>._domainkey`), apply zones atomically to an authoritative DNS server (BIND9), and probe live DNS/DKIM propagation from the control plane and UI.

## Atomic Branches (`-> sprint-7`)

### 1. `feat/sprint-7-dns-api`
- `crates/agent-protocol`: Add `Operation::DnsZoneApply` (`dns.zone.apply`), `Operation::DnsZoneDelete` (`dns.zone.delete`), `Operation::DnsProbe` (`dns.probe`), `DnsRecordType` (`A` | `AAAA` | `CNAME` | `MX` | `TXT`), `DnsRecordSpec`, `DnsZoneApplyPayload`, `DnsZoneDeletePayload`, `DnsProbePayload`, and strict ASCII DNS domain/record/TTL/value validation.
- `crates/control-store`: Schema v8 (`dns_zones`, `dns_records`, `dns_operations`) with tenant-scoped state machine (`provisioning` -> `ready` | `failed` | `deleting`), monotonic SOA serial increment on every zone/record mutation, DKIM TXT upsert helper, and idempotent reservation/completion.
- `crates/api`: Mount `GET/POST /v1/dns/zones`, `GET/DELETE /v1/dns/zones/{id}`, `POST /v1/dns/zones/{id}/records`, `PUT/DELETE /v1/dns/zones/{id}/records/{record_id}`, `POST /v1/dns/zones/{id}/dkim`, and `POST /v1/dns/zones/{id}/probe` (plus `/api/v1/dns/...`) with `Permission::DnsManage`, tenant isolation, and `node_jobs.rs` completion wiring.

### 2. `feat/sprint-7-dns-zone-worker`
- `crates/agent`: Implement `dns_driver.rs` supporting:
  - Authoritative BIND9 zone file rendering (`$ORIGIN`, `$TTL`, `SOA` with monotonic serial, `NS`, `A`, `AAAA`, `CNAME`, `MX`, `TXT` escaping) + `named.conf.fusionpanel` atomic write + reload hook (`FUSIONPANEL_DNS_RELOAD_CMD` / `rndc reload`).
  - Safe zone file removal and config reload on `dns.zone.delete`.
  - Pure-Rust UDP DNS wire query & response parser for `dns.probe` (`A`, `AAAA`, `CNAME`, `MX`, `TXT` / DKIM verification against `FUSIONPANEL_DNS_PROBE_ADDR`).
- Wire `DnsZoneApply`, `DnsZoneDelete`, and `DnsProbe` in `crates/agent/src/worker.rs`.

### 3. `feat/sprint-7-dns-ui`
- `frontend`: Add `/dns` route (`App.tsx`) and sidebar navigation (`DashboardLayout`), typed DNS API methods with `Idempotency-Key` headers in `frontend/src/api/index.ts`, `DnsView` (`frontend/src/views/Dns/`) with zone management, record CRUD (`A`, `AAAA`, `CNAME`, `MX`, `TXT`), DKIM TXT sync modal/form, propagation probe trigger/status, and Jest unit tests in `Dns.test.tsx`.

### 4. `test/sprint-7-dns-e2e-fixture`
- `tests/e2e/hosting/fixtures/sprint7/`: Bounded Docker Compose fixture (`ubuntu/bind9:9.18-22.04_beta` + `api` + `node`) + Playwright spec creating a DNS zone + `A` record + DKIM `TXT` record via UI, querying BIND9 directly, running propagation probe, updating/deleting a record, and verifying BIND9 reflects the change.
