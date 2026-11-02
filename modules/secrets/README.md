# Secrets Module

This package implements encrypted secret storage, key rotation, scoped access, and redaction for the FusionPanel domain.

## Capability Ownership Tracker

- SEC-15: Credential encryption [IMPLEMENTED]
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
1. Adding `"modules/secrets"` to the root `Cargo.toml` `members` array. (Presently done via a local override for testing compilation within this independent module scope).
2. Incorporating `SecretError` into the global application error boundary if necessary.
