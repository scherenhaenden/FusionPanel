# DNS Module

## Capability status

- **NET-01 — DNS zone management: partial.** This package validates domain and record values, enforces record-owner containment when building or deserializing a zone, and defines provider contracts. It does not include an authoritative DNS provider, zone export/restore, or real DNS changes. Those actions are not implemented or claimed as successful. Follow-up runtime integration is owned by [S29](../../docs/execution/sessions/S29.md); real DNS end-to-end qualification is owned by [S50](../../docs/execution/sessions/S50.md).
- **NET-02 — DNSSEC: partial.** Public DNSKEY and DS material has a typed, explicitly encoded contract. Key generation, signing, rollover, and provider execution are not implemented.
- **NET-03 — DNS templates: blocked.** There is no template model, renderer, or application workflow in this package.

The root [capability ledger](../../docs/execution/CAPABILITY_LEDGER.md) remains the release acceptance record. A module test passing does not change a capability from `PLANNED` or qualify a live provider.

## Validation and supported scope

- Domain input must be ASCII. Unicode U-label conversion is not implemented. ASCII `xn--` A-label strings are accepted as DNS labels, but this package does not validate their IDNA conversion or round trip.
- IPv4 literals, legacy numeric IPv4 forms, and IPv6 literals are rejected as domain names.
- Record validation supports A, AAAA, CNAME, MX, and TXT data. TTL zero is valid; values above `2^31 - 1` are rejected. `MX 0 .` is supported as Null MX; root targets with another priority are rejected.
- Constructor, serde deserialization, and zone insertion paths enforce the same domain, record, and zone-owner checks.
- A zone currently permits records only at its apex or beneath its apex. Authoritative SOA/NS generation and provider-specific record-set rules remain future work.

## Provider boundary

`Provider` defines planning, application, observation, and DNSSEC status operations. It is only an interface. There is no concrete provider implementation; implementations must report unsupported operations explicitly and must not return fabricated success.

## Test status

Unit validation tests run with:

```sh
cargo test --manifest-path modules/dns/Cargo.toml
```

The real authoritative DNS journey is **BLOCKED**, not passed. The exact missing fixture is: a concrete `Provider` implementation plus an isolated disposable authoritative DNS service, a test zone, and a reachable fixture endpoint. Without these, the S13 create/update → query expected answer → export/restore check cannot run. No service image/version, credentials, zone, endpoint, or teardown was available or provisioned for this run. The executable blocked case is ignored in the default unit suite and, when explicitly run, exits with a `BLOCKED` diagnostic:

```sh
cargo test --manifest-path modules/dns/Cargo.toml -- --ignored --nocapture
```

Do not treat the ignored case or the passing unit suite as integration/E2E evidence. The integration owner must select and pin a server image, create isolated test-only credentials and zone data, query the resulting records, verify export/restore, and record fixture teardown before NET-01 can be accepted.

## Workspace integration

This crate has its own Cargo workspace for isolated development. Root workspace membership and lockfile updates belong to the integration owner.
