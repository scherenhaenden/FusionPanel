# Test and Acceptance Plan

Map tests and limitations to capability IDs. Record the tested build, configuration, fixture and observed outcomes. Use isolated synthetic or sanitized data.

## Foundation acceptance

- Domain validation rejects malformed identifiers and enforces lifecycle rules.
- API tests exercise health, bearer authentication, structured errors and secret redaction.
- Protocol tests reject unsupported versions, operations, unsafe paths and malformed inputs.
- Job-store tests cover legal transitions, atomic claims, persistence across reopen and conflicting workers.
- CLI tests verify parsing, HTTP behavior, token redaction, JSON output and failure exit codes against a local HTTP fixture.

Each session owns tests within its allowed package. Integration owns the root Cargo.lock, workspace checks and continuous integration. Incomplete dependencies or unsupported operations must produce explicit errors rather than fabricated success.

## Product acceptance

Every committed capability requires successful normal, permission-denied and relevant failure journeys. Cover real site provisioning, database integrity, TLS renewal, scheduled jobs, deployments, container volumes, mail send/receive, DNS changes, alert delivery, backup restoration and node failure recovery as modules become available.

Disposable full OS VMs are required for host integration, quotas, firewall, systemd, installation and reboot testing. Container smoke results alone do not certify those behaviors. No existing FusionPanel implementation test results are claimed by this initial scaffold.

## Required detailed strategy

[TEST_STRATEGY.md](docs/testing/TEST_STRATEGY.md) defines unit, actual-dependency integration, browser/process E2E, VM and external qualification tiers. [DELIVERY_PLAN.md](docs/execution/DELIVERY_PLAN.md) assigns test sessions and gates; [CAPABILITY_LEDGER.md](docs/execution/CAPABILITY_LEDGER.md) records individual capability evidence. Missing prerequisites never count as passing tests.
