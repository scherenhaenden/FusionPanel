# S61: Release Integration and Regression

## Execution Output
- **Objective**: Collect reviewed outputs, dependency provenance, locked build, full regression and known gaps.
- **Dependencies**: S54, S55, S56, S57, S58, S59. (Considered available per branch state and instructions).

## Capability Status
- **DEP-07: Release rollback**: PARTIAL. The updater stages a copy beside the target, preserves the old file with a synced backup, and atomically replaces the live path. Its target-specific recovery marker can restore the old file or remove a first-install candidate. The recovery API is not called from startup or a CLI, so machine-level recovery remains unintegrated. E2E is BLOCKED pending agent/CLI dependencies.
- **SEC-17: Release integrity and provenance**: PARTIAL. Release manifest schema validation implemented, ensuring component hashes and signatures can be preserved.

## Testing and Verification
- **Unit Testing**: Verified manifest structure handling, required fields, empty components rejection and schema integrity.
```text
running 5 tests
test manifest::tests::test_empty_components ... ok
test manifest::tests::test_empty_version ... ok
test manifest::tests::test_invalid_component ... ok
test manifest::tests::test_serialization ... ok
test manifest::tests::test_valid_manifest ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- **Integration**: Clean build lane verified against the root `Cargo.toml`. Atomic update and mock binary execution tested.
```text
running 3 tests
test test_updater_atomic_swap ... ok
test test_updater_execute_mock ... ok
test test_clean_build_root_workspace ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- **Updater transaction/recovery repair (5 Nov 2026)**: The focused updater unit suite passed 7 tests, including failed replacement with the old target still present, interrupted replacement rollback, first-install cleanup, and legacy-marker handling. The filesystem integration subset passed 4 tests for atomic swap, interrupted replacement recovery, initial installation, and mock execution. These tests exercise the library only; they do not prove an installed service invokes recovery at startup.

- **E2E / Browser**: BLOCKED. Missing dependency: `fusionpanel-cli`. The E2E test attempts to interact with the executable, fails predictably, and outputs `BLOCKED: Missing dependency 'fusionpanel-cli'` ensuring the missing case is visibly recorded rather than mocked.
```text
running 1 test
test test_e2e_release ... FAILED

failures:

---- test_e2e_release stdout ----
thread 'test_e2e_release' panicked at tests/e2e.rs:11:9:
BLOCKED: Missing dependency 'fusionpanel-cli'
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

## Outstanding Follow-on Work
- Root workspace integration of `scripts/release` when it hands over.
- Fully resolve E2E capabilities once `fusionpanel-cli` and node agents are integrated and deployed.
