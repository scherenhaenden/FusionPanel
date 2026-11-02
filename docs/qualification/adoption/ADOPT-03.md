# ADOPT-03: Reviewed External Document-Root and Volume Bindings

## Objective
Establish a qualification test suite and strict validation rules for external document-root and volume bindings.

## Background
In FusionPanel, while managed roots follow a strict prefix structure (e.g. `/srv/managed/`), operators may need to bind external volumes or document roots. These external bindings require rigorous validation to prevent path traversals, access to protected paths, owner ambiguities, and unsafe symlink escapes or races.

## Requirements

1. **Validation Separation**: The global managed-root validator remains strict, only accepting paths inside designated prefixes without traversals or protected targets. External bindings have their own dedicated validator that accepts specific paths but applies thorough checks.
2. **Review Records**: Each external binding must record:
   - `source_path`
   - `canonical_path`
   - `target_path`
   - `owner`
   - `purpose`
   - `read_write_policy` ("ro" or "rw")
   - `mount_review` (if applicable)
   - `symlink_review` (if applicable)
   - `operator_review_reference` (e.g. a ticketing system reference)
3. **Strict Constraints**:
   - **Path Traversal**: No `..` references allowed in any path.
   - **Protected Targets**: Paths cannot start with `/etc`, `/proc`, or `/sys`.
   - **Owner Ambiguity**: `owner` field must be present and non-empty.
   - **Symlink Escapes**: If `source_path` and `canonical_path` differ, a valid `symlink_review` marked "APPROVED" must be present.
   - **Unreviewed Mounts**: An explicit "APPROVED" review must be provided for external mounts if no symlink review is applicable.
   - **Review Reference**: An explicit `operator_review_reference` must be provided for auditability.

## Implementation Details

The validation logic is implemented as a standalone Rust crate at `tests/qualification/adoption/adopt03/`. The suite ensures that external bindings are thoroughly vetted while keeping the managed-root operations unmodified and safe.

See `tests/qualification/adoption/adopt03/src/lib.rs` for the specific structural representation and unit tests verifying the rejection of unauthorized or unsafe bindings.
