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
