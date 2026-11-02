# MIG-02: CloudPanel SQLite Discovery Qualification

This document outlines the qualification logic and tests for discovering and validating CloudPanel SQLite databases for migration.

## Scope

The purpose of this module is to ensure that a given SQLite database file is a supported CloudPanel configuration database before proceeding with migration tools.

## Discovery Logic

The discovery process verifies the schema fingerprint and data integrity using the following checks:

### 1. Schema Fingerprinting

We check for the existence of the following required tables:
- `site`
- `php_settings`
- `database`
- `database_user`

If any table is missing, an `UnsupportedVersion` error is returned.

### 2. Column Validation

We verify the existence of specific columns in key tables:
- `site`: Must have `domain_name`
- `database`: Must have `database_name` and `site_id`
- `php_settings`: Must have `site_id` and `setting_name`

If any of these columns are missing or renamed, an `UnsupportedVersion` error is returned.

### 3. Data Integrity Checks (Negative Tests)

The following negative conditions are explicitly checked to prevent bad data from causing issues during migration:

- **Dangling Foreign Keys:** Verified using `PRAGMA foreign_key_check`. Returns `DanglingForeignKeys` if found.
- **Multiple Databases Per Site:** FusionPanel currently maps one database per site. If a site has multiple databases, we return `MultipleDatabasesPerSite`.
- **Ambiguous Settings:** If there are multiple entries for the same `setting_name` on a single `site_id` in `php_settings`, we return `AmbiguousSettings`.

## Testing

The test suite runs standard schema validations and explicit negative tests for all edge cases mentioned above.
Run tests using:
```bash
cargo test --manifest-path tests/qualification/migration/mig02/Cargo.toml
```
