# Foundation Contracts, Version 1

All packages in this batch are independent. Do not depend on unfinished sibling packages. Follow-up integration will adapt package types explicitly and review boundaries.

## HTTP contract

Bind to `127.0.0.1:8080` by default; configure using `FUSIONPANEL_LISTEN`. The executable is `fusionpanel-api`. Require nonempty `FUSIONPANEL_ADMIN_TOKEN` at startup. Public `GET /v1/health` returns HTTP 200 JSON `{"status":"ok","service":"fusionpanel-api","version":"<workspace package version>"}`. Authenticated `GET /v1/capabilities` requires `Authorization: Bearer <token>` and returns `{"capabilities":["control-plane.health.read","control-plane.capabilities.read"]}`. Invalid/missing authentication returns 401 JSON `{"error":{"code":"unauthorized","message":"Authentication required"}}`. Unknown routes return JSON error code `not_found`, HTTP 404. These endpoints do not claim hosting support. Tokens never appear in logs or response bodies. No CORS wildcard. Graceful shutdown on Ctrl-C.

## Domain contract

Expose NodeId, SiteId, TenantId and JobId as UUID-backed types with serde string encoding; DomainName as validated canonical ASCII DNS name; ResourceOwner with tenant_id; SiteKind (Php, Static, ReverseProxy); ResourceState (Pending, Active, Suspended, Failed, Deleting, Deleted) with explicit checked transitions. Reject invalid DNS labels, labels over 63 bytes, overlong names, separators and IP literals; allow ordinary multi-label ASCII hostnames. Document IDNA handling as unsupported rather than silently accepting Unicode.

## Agent protocol contract

Version 1 JSON request: `{"version":1,"request_id":"UUID","operation":"system.info","payload":{}}`. Response includes version, request_id and either result or an error object, never both. Support only typed `system.info` and `site.validate` in the foundation protocol; the latter validates a domain and absolute document root under `/srv/fusionpanel/sites/`. Reject traversal, NUL, relative paths and unsupported operations/versions. No process execution, daemon, SSH, root actions or transport authentication in this package. Define validation APIs and JSON fixtures, not a fake successful site provisioning operation.

## Job-store contract

Expose a SQLite store with create/get/list/claim/transition operations; string UUID identifiers, job kind, JSON payload, timestamps and bounded optional error text. States are queued, running, succeeded, failed, cancelled. Only queued -> running/cancelled and running -> succeeded/failed/cancelled are allowed. Terminal states are immutable. Claim oldest queued job atomically with a worker identifier; two claimers cannot own one job. Reopen preserves all records. Running jobs are not automatically retried after a crash: expose them for explicit reconciliation. Use transactions and SQL parameters; return typed errors. Store no credentials in fixtures.

## CLI contract

Executable `fusionpanel-cli`: `health` and `capabilities` subcommands, `--server` default `http://127.0.0.1:8080`, `--json`, token from `FUSIONPANEL_TOKEN` only. Use the HTTP contract above. Health requires no token, capabilities requires one. Set finite connect/request timeouts; non-2xx/network/parse errors exit nonzero and print useful redacted errors. Never print request authorization headers or tokens. HTTP localhost is allowed for development; never disable TLS verification. No mutation commands in this milestone.
