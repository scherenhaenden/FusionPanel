# Hosting Module

Independent implementation for S09 Hosting plans and configuration.

## Features

- Domain models for HostingSpec (Static, Php, ReverseProxy).
- Nginx config generation.
- PHP-FPM pool config generation.

## Test Strategy

- T0: Unit testing domain rules, validations, and generation.
- T1/T2: Nginx and PHP-FPM config validation using disposable Docker containers.
- E2E: Defined, but BLOCKED on agent capability to perform actual provision.

## Shared Type Proposals

These should map to `crates/domain` in a later integration phase:
- `TenantId`, `SiteId`, `JobId`
- `DomainName` (with IDNA unsupported explicitly)
- `SiteKind`
- `ResourceState` (Pending, Active, Suspended, Failed, Deleting, Deleted)

## Validation Updates (Refs #91)
- Strict validation added via "parse, don't validate" pattern: `DocumentRoot`, `PoolName`, `PhpVersion`, `MemoryLimit`, and `UpstreamUrl`.
- Deserialization automatically enforced via `TryFrom` in `serde`.
- `www-data` default pools explicitly replaced by strict `tenant_x` identity mappings to preserve tenant isolation.
- Integration tests robustly handle BLOCKED environmental limitations and explicit negative checking against configuration bypasses (e.g. multiline injection).
