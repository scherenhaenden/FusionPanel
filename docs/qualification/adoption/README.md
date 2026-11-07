# Adoption Qualification Evidence

This directory contains the qualification schemas and integration constraints for Adoption features.

- `ADOPT-01.md`: Reconciling Effective Nginx and Application Behavior
- `ADOPT-02.md`: Resource Ownership Boundaries
- `ADOPT-03.md`: Reviewed External Document-Root and Volume Bindings
- `ADOPT-04.md`: Multi-Database Export, Restore, and Ownership
- `ADOPT-05.md`: Backup and Certificate-Renewal Ownership Handoff
- `ADOPT-06.md`: OS Username Preservation and Identity Mapping
- `ADOPT-07.md`: Upstream TLS Trust and SNI Verification

Real disposable integration tests asserting the above boundaries are located in `tests/qualification/adoption/integration`.
They ensure:
- External file preservation on teardowns (`tempfile`-based tests).
- Pre-decommission verifications across trust policies (expired tokens/missing steps deny retirement).
- Strict reporting for missing test labs (T3).
