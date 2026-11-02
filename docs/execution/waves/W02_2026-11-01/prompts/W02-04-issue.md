## Audit repair objective

restore.rs validates lexical paths but follows pre-existing parent/leaf symlinks, may write outside target, truncates valid destination before checksum and deletes it on failure. It returns success when manifest-listed members are absent and ignores expected sizes/duplicate manifests/members. Stage and validate complete archives before committing target changes, preserve existing data on failure, enforce symlink/source containment, reject nonexistent sources and output paths within source tree. Retention must prove backup ownership instead of deleting arbitrary .gz files. Add real filesystem tests for malicious links, missing/duplicate entries, corruption rollback, existing target preservation, source/output boundaries and retention ownership. DB metadata alone is not a DB backup; no encryption/selective restore/Restic claims without implementation.

## Ownership

Exclusive write paths: modules/backups/. Parent package S11, issue #15. Current verified master base 3a8bf9ef39c35a43b36771c575dbc3b11bc601ba.

## Acceptance and delivery

Preserve feature IDs and selectable workflows. Include meaningful unit, real integration and E2E evidence; missing prerequisites are BLOCKED with nonzero status, not passing placeholders. Provide exact commands, versions, sanitized outputs and cleanup. Create/update a focused PR against master using Refs for this issue and its parent; never auto-close issues, merge, squash, force push or modify production/host services. No product capability acceptance from a merge alone.
