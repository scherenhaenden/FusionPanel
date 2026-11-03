# ADOPT-02: Resource Ownership Boundaries

This document describes the qualification test suite for adopted and external resource ownership boundaries.

## Objective

To define and test the boundaries of resource ownership within the FusionPanel ecosystem, particularly distinguishing between fully managed, adopted, and externally managed resources.

## Ownership Tiers

FusionPanel categorizes resources into three primary ownership tiers:

1.  **Panel-Owned (`PanelOwned`)**:
    *   Resources created and entirely managed by FusionPanel.
    *   Examples: Sites created via the panel, local databases provisioned by the panel.
    *   Lifecycle: Full lifecycle management is permitted (suspend, restart, delete).

2.  **Adopted with Boundary (`AdoptedWithBoundary`)**:
    *   Resources that existed before FusionPanel but have been brought under partial or full panel management.
    *   Examples: An existing system service where the panel manages the configuration, but the user retains data ownership.
    *   Lifecycle: The panel can perform lifecycle operations within defined boundaries. For testing purposes, these currently allow standard operations but might have specific constraints in production.

3.  **Externally Managed (`ExternallyManaged`)**:
    *   Resources recognized by FusionPanel but explicitly managed outside of its scope.
    *   Examples: External database servers, self-managed user accounts, external containers.
    *   Lifecycle: FusionPanel is strictly forbidden from modifying the lifecycle state of these resources. Actions like suspend, restart, or delete are rejected.

## Test Suite

The test suite (`tests/qualification/adoption/adopt02/`) simulates these ownership models and validates the enforcement of lifecycle boundaries.

### Key Scenarios Validated

*   **Panel-Owned Lifecycle**: Ensures that `PanelOwned` resources can be successfully suspended, restarted, and deleted.
*   **Externally Managed Lifecycle**: Ensures that lifecycle operations (suspend, restart, delete) are strictly rejected for `ExternallyManaged` resources, protecting them from unintended interference.
*   **Adopted with Boundary Lifecycle**: Validates operations on adopted resources.
*   **Boundary Enforcement on Deletion**: Confirms that deleting a panel-managed resource (e.g., a managed site) does not cascade deletion or modification to associated `ExternallyManaged` resources.

## Implementation Details

*   Implemented as a standalone Rust crate with an empty `[workspace]` directive in `Cargo.toml`.
*   Uses `#![forbid(unsafe_code)]` to guarantee memory safety.
*   Tests can be executed via: `cargo test --manifest-path tests/qualification/adoption/adopt02/Cargo.toml`.
