# fusionpanel-domain

This package provides the core domain contract for FusionPanel.

## Domain Contract

- **Identifiers**: `NodeId`, `SiteId`, `TenantId`, and `JobId` are UUID-backed types with serde string encoding.
- **Domain Names**: The `DomainName` type enforces validation rules to guarantee canonical ASCII DNS names. It rejects invalid DNS labels, labels over 63 bytes, overall lengths over 253 bytes, separators, and IP literals. Note that IDNA (Internationalized Domain Names in Applications) is explicitly **unsupported**. Attempting to pass Unicode will be rejected rather than silently accepted or converted.
- **Resources**: `ResourceOwner` models ownership (associated with a `TenantId`). `SiteKind` defines the site type (`Php`, `Static`, `ReverseProxy`).
- **State Management**: `ResourceState` (`Pending`, `Active`, `Suspended`, `Failed`, `Deleting`, `Deleted`) enforces explicit, checked state transitions. Invalid transitions result in typed errors.
