# Mail Module

Provides configuration rendering for Postfix and Dovecot.

## APIs
- Postfix virtual mailbox domains and maps configuration rendering.
- Dovecot SQL passdb configuration rendering.

## Tests
- `unit_config.rs`: Tests for safe rendering and escaping of configuration files.

## Supported Features
- Configuration generation for mail servers.
- Prevention of shell injection and format string vulnerabilities.

## Unsupported Features
- Runtime process management (handled elsewhere).
