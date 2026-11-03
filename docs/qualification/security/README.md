# Security and Secrets Qualification

This module contains end-to-end security and secrets qualification tests.

According to the instructions in `docs/execution/sessions/S52.md` and `docs/testing/TEST_STRATEGY.md`, E2E tests for security features require actual OS dependencies (like `sshd`, `iptables`, `nftables`, `fusionpanel-api`, `openssl`). When such prerequisites are unavailable in the test environment, the tests are implemented as executable case definitions that probe the system and exit with a `BLOCKED` status, explicitly recording the missing dependency rather than silently passing.

## Test Cases
- **SEC-09: Emergency Lockout**: Checks for `sshd` and `iptables` and reports `BLOCKED` since an isolated test network namespace and firewall adapter are required.
- **SEC-12: Token Revocation**: Checks for `fusionpanel-api` availability and reports `BLOCKED` as it requires an actual API instance and database backend.
- **SEC-05: Firewall Drop Rules**: Checks for `iptables` and `nftables` and reports `BLOCKED` due to missing isolated test network.
- **SEC-15: AES-GCM Envelope Secret Encryption Validation**: Checks for `openssl` and reports `BLOCKED` as it needs a cryptographic subsystem and keystore fixture.

To run the qualification definitions, execute:
```bash
cargo test --manifest-path tests/e2e/security/Cargo.toml
```

All E2E test cases currently fail with the expected `BLOCKED` indicator. This correctly signals that the features are not fully E2E qualified due to missing dependencies, preventing a false sense of security.
