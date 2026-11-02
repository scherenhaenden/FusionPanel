# Administration

Independent module for administration capabilities (ADM-01 through ADM-15).

## Capabilities

Implemented:
- ADM-05: User and role management (Partial: Team member invitations and role delegation)

Partial:
- None

Blocked/Missing:
- ADM-01: Web SSH terminal
- ADM-02: Systemd service management
- ADM-03: Operating system updates
- ADM-04: Panel updates and recovery
- ADM-05: User and role management (Missing: full profile management, password reset, etc)
- ADM-06: Reseller hierarchy
- ADM-07: White-label branding
- ADM-08: HTTP API
- ADM-09: API keys
- ADM-10: API IP allowlists and rate limits
- ADM-11: Recovery CLI
- ADM-12: Secret management
- ADM-13: Infrastructure as Code
- ADM-14: Care plans
- ADM-15: Monthly client reports

## Tests
Unit tests exist for invitation token generation, expiry (bounded TTL), accepted status, and role assignment.
Run tests using: `cargo test --manifest-path Cargo.toml`

## Proposed Shared Changes
- None
