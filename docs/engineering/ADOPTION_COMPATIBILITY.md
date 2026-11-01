# Adoption Compatibility and Acceptance Gates

## Purpose and evidence status

FusionPanel must preserve useful existing workloads while introducing its own management model. The cases below turn reported adoption observations into product contracts and synthetic regression fixtures. They are not refreshed proof of any current production server state. Revalidate the actual source inventory and runtime configuration before a migration or decommission decision.

Use reserved synthetic domains such as `shop.example.test`, generated fixture identities and temporary paths. Keep operational addresses, production inventories, data, credentials and private configuration outside this public repository.

## ADOPT-01 — Reconcile metadata with effective runtime configuration

**Behavior:** discovery captures separate observations from panel metadata, effective web-server configuration, application configuration and actual upstream listeners. Preserve each observation's origin, snapshot/version and confidence. Produce an explicit conflict when site type, document root, upstream scheme/port, aliases or route maps differ. Automatic apply remains blocked until an authorized operator reviews the conflict and records the chosen mapping. Neither database metadata nor a single configuration file is universally authoritative.

Preserve custom locations, HTTP methods, headers, redirects, legacy rewrites, discovery endpoints, OAuth paths and streaming/WebSocket behavior when present. A generic proxy root must not silently replace a richer route map. A successful parser is not successful runtime preservation.

**Unit:** conflicting PHP/proxy observations; stale upstream URL; missing observation; ambiguous aliases; deterministic conflict and resolved-plan serialization.

**Real integration:** actual SQLite fixture plus effective Nginx configuration and real HTTP/HTTPS upstreams deliberately disagree; discovery reports every difference without changing source or target. Syntax-check the resulting reviewed configuration.

**E2E:** import a reviewed synthetic site with distinct ordinary, discovery and streaming routes. Assert actual response content, redirects, headers and upstream target. An unresolved conflict must refuse apply and leave existing service behavior unchanged.

## ADOPT-02 — Distinguish managed and external resource ownership

**Behavior:** every account, process/service, container, path and database has explicit ownership: panel-owned, adopted-with-declared-management-boundary, or externally managed. A site may proxy to an external service without owning its account, home directory, cgroup or container runtime. Suspension/deletion operates only on resources the panel owns and is authorized to stop/remove. Never use UID-wide termination or account/home deletion against adopted or external identities. References between resources prevent accidental cascading deletion.

For panel-owned resources, lifecycle actions still require scope/ownership revalidation and known dependency checks. Prefer the specific owned service/cgroup/process identity rather than broad account-level actions. Show the effect and remaining external dependencies in the operation plan and audit result.

**Unit:** ownership combinations, shared references, retained external dependencies, denied destructive plans and explicit deletion boundaries.

**Real integration:** disposable OS fixture with a managed web service and an independently supervised/rootless workload under a distinct or adopted identity. Suspend/delete the web site and verify the external workload, account and data remain intact.

**E2E:** perform site suspend/resume/delete through the actual API/agent path and probe the external application afterward. Attempt unauthorized account/process removal; it must fail without side effects.

## ADOPT-03 — Preserve external path and volume bindings safely

**Behavior:** managed roots retain the managed-root validator. An imported external root is represented by a separate, explicit binding with allowed canonical path, owner, purpose, read/write policy and required operator review. Do not relax global path validation or silently relocate existing content. Inventory bind mounts, symlinks, runtime volumes, writable uploads and deployment scripts before proposing a move.

A binding change is a durable planned operation with permission checks, canonical-path/symlink validation, resource locking, data-movement/backup requirements, actual service checks and a documented recovery boundary. Reject traversal, unexpected mount targets, symlink escape/races and bindings to protected system paths. Backup/restore semantics must distinguish preserving a link from copying referenced data.

**Unit:** managed versus external binding schemas, traversal/NUL/relative paths, forbidden targets, conflicting owners and unsafe symlink/mount maps.

**Real integration:** disposable filesystem with valid bind mounts, symlinks, permissions and an external upload tree. Verify that only explicitly registered bindings are usable and that content remains reachable without relocation.

**E2E:** import a proxy/static fixture with existing external uploads; read/write through the real application, restart services and restore to a separate target. Confirm exact hashes/ownership and unchanged external paths unless an explicit reviewed move was requested.

## ADOPT-04 — Preserve one-to-many database associations

**Behavior:** a site can reference multiple databases, and database ownership is independently represented. Discovery and export enumerate all relevant database associations. A preferred/default database is an explicit optional reference, not a substitute for the collection. Never truncate with a first-item lookup. Ambiguous or cross-environment associations require a reviewed mapping; do not guess ownership from database names.

A source limitation may exist in application queries/import logic even when the storage schema supports multiple rows. Verify both schema and runtime behavior before asserting a structural constraint. Import and restore must retain all declared schemas, users, grants, secrets references and application connection mappings.

**Unit:** zero/one/multiple databases; stable ordering; duplicate/shared references; explicit default; ambiguous mapping and partial failures.

**Real integration:** create multiple real databases with distinct content under one synthetic site, export/import each, and compare schema/content/grants. Detect an intentionally omitted export and fail verification.

**E2E:** exercise an application that reads/writes two databases, then independently restore and verify both. Deleting a site must respect database retention/shared-resource policies rather than remove unowned databases.

## ADOPT-05 — Replace backup and certificate-renewal ownership before decommission

**Behavior:** discover both per-user and system-level scheduled work, including panel-owned jobs. Before disabling the incumbent scheduler/control plane, establish a single explicit owner for every required backup, retention policy and certificate renewal. A configuration or recreated cron line alone does not establish continuity. Require a successful scheduled backup, independent restore and a real renewal-path test in the qualification environment.

Record schedule timezone, environment, credentials references, database exclusions, destination, retention and last observed successful run. Avoid duplicate active owners. The handoff plan states when the new owner starts, how success is verified and when the old owner may stop. Decommission remains blocked when any critical job or renewal owner is unknown, failing or untested.

**Unit:** schedule parsing/timezone, exclusions, owner conflicts, missing credentials/destinations and decommission gate evaluation.

**Real integration:** actual scheduler executes a safe backup fixture; verify artifact/data and retention. Exercise certificate renewal against controlled staging/local test infrastructure and observe reload without replacing real behavior with a mocked success.

**E2E:** transfer ownership in a disposable deployment, stop the incumbent fixture scheduler, then observe subsequent backup execution and renewed certificate serving. Prove recovery from the independently stored backup. Missing public-domain/provider prerequisites remain explicitly blocked for external qualification.

## ADOPT-06 — Preserve valid operating-system usernames with explicit mappings

**Behavior:** accept the declared supported platform's valid account names, including ordinary hyphenated names, while rejecting option-like names, invalid delimiters and protected/reserved identities. Validate names and pass them as safely separated arguments to system tools. The identifier model must distinguish account name, numeric identity and resource ownership.

If a target policy requires renaming, present an explicit collision-checked mapping. Never silently normalize hyphens to underscores: two distinct source names may collide. Existing accounts require ownership/adoption review; mere existence does not authorize reuse or deletion. Generated names must remain deterministic and collision-safe within platform length constraints.

**Unit:** hyphenated names, underscore/hyphen collisions, truncation collisions, invalid delimiters, reserved identities and distinct UID/name mappings.

**Real integration:** create disposable allowed accounts on the supported OS, verify identity lookups and safe argument handling, and reject attempted adoption of a protected account without side effects.

**E2E:** import two synthetic users whose names would collide under naive normalization; preserve their ownership and independent data, then verify scoped lifecycle operations cannot affect the other identity.

## ADOPT-07 — Make upstream TLS trust and SNI explicit

**Behavior:** distinguish public-facing certificate management from upstream TLS policy. An HTTPS upstream has explicit verification mode, trust material, expected server name and SNI behavior. Prefer verified system/internal-CA trust. If legacy compatibility without verification is supported, make it a narrowly scoped, visibly insecure, authorized mode with audit and a migration path to verified trust. Never introduce an invisible global verification bypass.

Nginx documents `proxy_ssl_verify off` and `proxy_ssl_server_name off` as defaults. Therefore an observed upstream failure cannot be attributed to default certificate verification without checking effective inherited configuration and handshake evidence. The name used for certificate verification and SNI must match the intended upstream identity; a fixed name must not be applied indiscriminately. See the [official Nginx upstream TLS directives](https://nginx.org/en/docs/http/ngx_http_proxy_module.html#proxy_ssl_verify).

**Unit:** trust/SNI mode schema, missing trust references, incorrect expected names, incompatible mode combinations and explicit compatibility selection.

**Real integration:** actual Nginx proxies to HTTPS fixture servers using a controlled private CA, valid/invalid names, untrusted certificates and an SNI-dependent endpoint. Assert actual handshake/HTTP outcomes and inspect effective configuration.

**E2E:** select verified upstream TLS through UI/API, serve the intended application and reject an untrusted/name-mismatched target. Change to an explicitly permitted compatibility mode through a reviewed job, verify visibility/audit, then migrate back to verified trust without losing route maps.

## Variant preservation and acceptance

Expose useful choices as descriptive product modes with stable IDs, explicit scope, persisted selections, prerequisites, conflicts, change policy and mode-specific test IDs. Preserve managed versus external bindings, managed versus external workload ownership and upstream trust modes without presenting unsafe combinations as valid choices. Import/export, backups and IaC must retain these selections.

All applicable unit, real integration and E2E cases above require evidence before adoption qualification. A narrow foundation library may merge while host execution remains unimplemented, but that does not accept these runtime gates. A blocked case stays visible and prevents the corresponding migration/decommission claim. Production observations must be refreshed before any actual execution authorization.
