# WordPress Module

This is an independent package providing automation for WordPress self-hosting.

## Capabilities

* **Maintenance Mode Coordinator:** Automatically create and remove the `.maintenance` file to put WordPress sites into and out of maintenance mode.
* **Cron Offloading:** Modifies `wp-config.php` to set `DISABLE_WP_CRON`, allowing system cron to take over background tasks for improved performance.

## Tests

This module is independently testable:

```bash
cargo test --manifest-path modules/wordpress/Cargo.toml
```

## Modes

The maintenance coordinator allows setting WordPress to run in maintenance mode or standard production mode. Cron offloading can similarly be enabled or disabled based on operator preference.

## Features

- **Implemented:**
  - Automated maintenance mode toggling
  - WordPress cron offloading configuration manipulation
- **Unsupported/Blocked:**
  - Currently unsupported: complex PHP-based AST parsing of `wp-config.php`. The module relies on line-based modification for `DISABLE_WP_CRON`.

## ZIP archive consistency

The installer requires `zip >= 9.0.5, < 10` and validates local/central header consistency before extracting any entry. Ambiguous filenames, compression metadata, duplicate names and unlisted local entries are rejected through `InstallError::ZipError`; the staging guard cleans up failed installs. Stored and Deflate archives remain supported. The minimum patched zip release requires Rust 1.88 or newer.

The local parser/installer boundary regressions run without WordPress network access:

```bash
cargo test --manifest-path modules/wordpress/Cargo.toml --test zip_header_consistency
```

These tests exercise the real ZIP parser and temporary filesystem through local HTTP fixtures; they do not qualify a served WordPress installation.
