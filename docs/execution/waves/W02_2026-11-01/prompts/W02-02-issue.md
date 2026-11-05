## Audit repair objective

Fix raw Nginx/PHP-FPM config injection through unchecked public/deserializable PHP/proxy fields and document-root strings (domain/validation.rs57, domain/site.rs12-41, config/nginx.rs16,46, php_fpm.rs18-32). Make construction/deserialization/rendering enforce safe validated tokens and paths. Use explicit validated tenant identity mapping instead of silently emitting shared www-data for all pools. Missing Docker currently returns success; E2E prints BLOCKED then assert!(true). Replace false-green cases with executable nonzero dependency gates and meaningful real disposable parser/provision tests when actual prerequisites exist; test adversarial input and invariant bypass.

## Ownership

Exclusive write paths: modules/hosting/. Parent package S09, issue #12. Current verified master base 3a8bf9ef39c35a43b36771c575dbc3b11bc601ba.

## Acceptance and delivery

Preserve feature IDs and selectable workflows. Include meaningful unit, real integration and E2E evidence; missing prerequisites are BLOCKED with nonzero status, not passing placeholders. Provide exact commands, versions, sanitized outputs and cleanup. Create/update a focused PR against master using Refs for this issue and its parent; never auto-close issues, merge, squash, force push or modify production/host services. No product capability acceptance from a merge alone.
