# Secrets Module

This recovered draft provides an experimental encrypted-file store and cryptographic primitives. Security and recovery defects listed below block runtime use and package acceptance.

## Capability Ownership Tracker

- SEC-15: Credential encryption [PARTIAL / NOT ACCEPTED] - AEAD primitives exist; key redaction, authenticated metadata, store safety and recovery remain unresolved.
- ADM-12: Secret management [PARTIAL] - Requires E2E integration with actual runtime components (web interface, configuration injection). Follow-on work required for integration into the control plane and UI.
- Backup-key recovery: [PARTIAL] - Key rotation works at the file level. Offline backup key restore needs explicit integration with `BAK-*` capabilities.

*Remaining domains (ADM-01..15 minus ADM-12) are out of scope for this module's direct storage logic but are noted as dependencies for the overarching platform. Specific E2E tests for these domains are BLOCKED pending the integration environment.*

## Domain Types & Error Model

The module adheres to strict stable error types (`SecretError`) distinguishing between `InvalidInput`, `Unauthorized`, `Conflict`, `CryptoError`, and `ReconciliationRequired`. Request and Response structures separate Intent (`SecretPlan`), Application (`AppliedResult`), and State (`ObservedState`).

Sensitive material is wrapped in a zeroizing `SecretMaterial` struct which guarantees `[REDACTED]` output for standard Debug printing, preventing credentials from entering logs.

## Modes & Features

- **AEAD Encryption**: ChaCha20Poly1305 with random nonces for payload confidentiality and integrity.
- **Scoped Access**: Read operations must provide the matching tenant and scope identifier, rejecting cross-tenant unauthorized reads.
- **Key Rotation**: `rotate_master_key` re-encrypts all file records on disk using a new master key.
- **Unsupported**: Direct injection into isolated systemd/docker services is currently deferred (BLOCKED) pending real environment testing tiers.

## Verification
- **Unit Verification**: Extensive checks on AEAD misuse (wrong key, tampered ciphertext, invalid key sizes), redaction validation, and isolated file-system invariants (access control boundaries).
- **Integration Verification**: Disposable `tempfile` fixtures test real serialized JSON files, reopening the store after instantiation, and executing complete key rotation cycles to ensure older keys fail decryption after a successful rotation.
- **E2E Verification**: The `test_e2e_inject_secret_into_isolated_service` case is defined but reports BLOCKED due to the missing required runtime infrastructure (isolated systemd environment for true blackbox testing).

## Proposed Shared Changes

No shared files outside of `modules/secrets/` have been modified in this iteration. Future workspace integration will require:
1. Adding `"modules/secrets"` to the root `Cargo.toml` members only after explicit integration ownership. This recovery was tested through the standalone module manifest and did not edit the root workspace.
2. Incorporating `SecretError` into the global application error boundary if necessary.


## Recovered result and acceptance blockers

Recovered from Jules session `1788723557430535400` onto base `3a8bf9ef39c35a43b36771c575dbc3b11bc601ba`. The original patch contained 775 generated build-artifact paths; recovery excludes all `target/` output and Cargo.lock. Only this module is delivered. The source was formatted, while substantive defects are retained for visible draft review.

Static audit found these blockers:

1. `CryptoEngine` derives Debug while holding the raw key (`src/crypto.rs`), exposing key bytes through debugging; its key also lacks an explicit zeroizing owner.
2. `SecretStore::file_path` accepts an unchecked string ID and joins it to the storage directory. Validate canonical UUID IDs and enforce directory containment, including symlink handling.
3. Scope authorization concatenates tenant and scope with a colon, so distinct pairs can collide (for example tenant `a:b` / scope `c` and tenant `a` / scope `b:c`). Metadata is not authenticated as AEAD associated data, and `get_state` has no tenant/scope authorization parameter.
4. Key rotation overwrites records one by one without atomic commit, locking, a recovery journal or rollback. An interruption or write failure can leave mixed-key records while the in-memory engine still holds the old key. Ordinary record writes are also non-atomic, and filesystem permissions are not explicitly restricted.
5. The key-rotation test confirms that the old key fails but never reopens with the new key to prove successful recovery.
6. The E2E injection test prints BLOCKED and returns success. It is excluded from the verification command below and is not accepted as a passing runtime test. It needs an executable real-runtime driver and a nonzero missing-dependency gate.

Verification executed during recovery:

- `cargo fmt --manifest-path modules/secrets/Cargo.toml` — completed.
- `CARGO_TARGET_DIR=/tmp/fusionpanel-secrets-recovery-target cargo test --offline --manifest-path modules/secrets/Cargo.toml --lib -- --skip test_e2e_inject_secret_into_isolated_service` — 8 passed, 0 failed, 1 filtered out. These exercise real ChaCha20Poly1305, file persistence/reopen and the limited rotation path; they do not prove the unresolved properties above.
- No isolated service injection, key-backup recovery drill or full-machine restore E2E was executed.

Keep issue #25 open. This result must remain a draft until the blockers are addressed and independent verification passes. No release or product version change is made (`+semver: none`).
