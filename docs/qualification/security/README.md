# Security and Secrets Qualification

This module contains end-to-end security and secrets qualification tests.

According to the instructions in `docs/testing/TEST_STRATEGY.md`, E2E tests for security features require actual OS dependencies (like `sshd`, `iptables`, `nftables`, `fusionpanel-api`, `psql`, `unshare`, `openssl`). When such prerequisites are unavailable or when the test lacks sufficient privileges (e.g. creating network namespaces via `unshare`), the tests execute a real dependency check and exit with a `RESULT=BLOCKED` panic status, explicitly recording the missing dependency or permission rather than silently passing. Tests successfully attempt to execute real dependency logic where available and panic where unavailable.

## Test Cases
- **SEC-09: Emergency Lockout**: Probes for `sshd`, `iptables` and `unshare`. Tests the ability to spawn a network namespace. If successful, executes real firewall drop rules and attempts to verify lockout behavior before falling back. If not, reports `RESULT=BLOCKED` since an isolated test network namespace with root privileges is required.
- **SEC-12: Token Revocation**: Checks for `fusionpanel-api` and `psql` availability. Validates connectivity to the PostgreSQL backend. If dependencies are present, executes real API commands to mimic token creation, utilization and revocation, asserting expected rejection. Otherwise, reports `RESULT=BLOCKED` as it requires an actual API instance and database backend configured for testing.
- **SEC-05: Firewall Drop Rules**: Checks for `iptables` and `nftables`. Probes ability to list firewall rules inside a new network namespace. If available, adds, lists, and drops actual rules to verify execution paths. Else reports `RESULT=BLOCKED` due to missing root privileges/isolated test network.
- **SEC-15: AES-GCM Envelope Secret Encryption Validation**: Checks for `openssl` capability to execute the AES-256-GCM cipher. When present, executes actual shell openssl enc/dec cycles on a generated secret. If missing, reports `RESULT=BLOCKED` as it requires a fully prepared cryptographic subsystem and keystore fixture.

To run the qualification definitions, execute:
```bash
cargo test --manifest-path tests/e2e/security/Cargo.toml
```

All E2E test cases currently fail with the expected `RESULT=BLOCKED` indicator in standard unprivileged environments. This correctly signals that the features are not fully E2E qualified due to missing privileged dependencies or required full-OS isolation, preventing a false sense of security while satisfying real test execution criteria.
