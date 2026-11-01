# DNS Module

## Capability Ownership
- **NET-01: DNS zone management** (Implemented: Types, Validation, Provider Interface; Blocked: Real Provider Backend E2E)
- **NET-02: DNSSEC** (Partial: Contracts defined; Blocked: Provider execution)
- **NET-03: DNS templates** (Partial: Record struct template concepts; Blocked: Real application logic)

## Supported/Unsupported Features
- Supported: ASCII DomainName validation, standard record types (A, AAAA, CNAME, MX, TXT).
- Unsupported: IDNA/Unicode domains are explicitly rejected.

## Proposed Shared Changes
- Integration will need to merge this package into the root workspace and lockfile.
