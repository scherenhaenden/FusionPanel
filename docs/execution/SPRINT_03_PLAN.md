# Sprint 3 Plan & Handoff Guide — Websites & Reverse Proxy

- **Sprint branch**: `sprint-3` (currently based on `master` at `6b5adf5`)
- **Dependency**: Sprint 2 release and its node/job API + agent worker contracts
- **Target release**: `sprint-3` -> `master` only after the Sprint 3 close gate
- **Docker**: Optional for local feature work; required for the disposable Nginx E2E gate
- **Estimate**: L (8 story points)

> Sprint 2 is still in review. The current `sprint-3` branch is a planning base only. Before starting dependent implementation branches, refresh it from the verified Sprint 2 release on `master`; do not implement against guessed or unmerged agent/API interfaces.

## 1. Sprint goal

Create, inspect, and delete static and reverse-proxy websites through FusionPanel's API and UI. The agent must apply a real Nginx virtual host on a disposable test host, verify the full configuration before reload, and restore the previous configuration if validation or reload fails.

At Sprint 3 close:

1. An authenticated operator can create a static or reverse-proxy site in the UI.
2. The control plane persists the requested site and queues a typed agent job; the UI reports `provisioning`, `active`, or `failed` from persisted API state.
3. The agent renders a managed Nginx config, writes it without following untrusted paths, runs `nginx -t`, and reloads only after successful validation.
4. Delete removes only that site's managed config and returns the site to `404` without changing unrelated virtual hosts.
5. A disposable end-to-end run proves real HTTP traffic for both static and reverse-proxy sites, then proves deletion and rollback behavior.

## 2. Shared API and data contract

The API task owns the request and response types and publishes them before the agent and UI tasks depend on them.

### Create

`POST /v1/websites` requires operator authentication and an `Idempotency-Key` header.

Static request:

```json
{
  "domain": "app.local",
  "kind": "static",
  "document_root": "/srv/fusionpanel/sites/app.local"
}
```

Reverse-proxy request:

```json
{
  "domain": "app.local",
  "kind": "reverse_proxy",
  "upstream_url": "http://127.0.0.1:8080"
}
```

The API returns `202 Accepted` with `{ "site": ..., "job": ... }`. A repeated idempotency key with the same request returns the original operation; reusing it for a different request returns `409 Conflict`.

### Read and delete

- `GET /v1/websites` returns `{ "websites": [...] }`.
- `GET /v1/websites/:id` returns `{ "website": ... }`.
- `DELETE /v1/websites/:id` requires an idempotency key and returns `202 Accepted` with the deleting site and job.

Each site record includes `id`, `domain`, `kind`, its kind-specific config, `state` (`provisioning`, `active`, `failed`, or `deleting`), `last_error`, `created_at`, and `updated_at`. The UI must display the server state; an enqueued job is not a successful site creation.

### Input and ownership rules

- Accept strict ASCII DNS names only; reject IP literals, malformed labels, control characters, and Nginx directives.
- A static document root must remain under `/srv/fusionpanel/sites/<domain>` after canonicalization; reject symlink escapes and traversal.
- A reverse-proxy upstream must be an absolute `http` or `https` URL with a host and valid port. Reject userinfo, fragments, query strings, whitespace/control characters, and config delimiters.
- The agent protocol must represent create/delete as typed operations. Do not accept arbitrary shell commands or arbitrary Nginx text from the API/UI.
- Operators may see only the existing session/admin-authorized site records; agents authenticate with their node token and may update only the job/site they own.

## 3. Atomic implementation branches

Each branch starts from the refreshed `sprint-3` integration branch, contains one bounded change, runs only its local unit tests, and opens a PR into `sprint-3`. Do not merge these PRs directly to `master`.

### Task 3.1 — `feat/sprint-3-websites-api`

**Ownership**: API routes and site persistence, plus the shared contract above.

- Add create/list/detail/delete endpoints, authentication, strict validation, idempotency, and persisted desired/observed state.
- Queue typed create/delete work through the Sprint 2 job service; return accepted/pending status until the agent reports completion.
- Map agent completion and safe error summaries back to site state. Failed apply must remain visible and retryable.
- Add API and store tests for auth, validation boundaries, idempotency, state transitions, persistence, and conflict handling.

**Local verification**: `cargo test --locked -p fusionpanel-api` and focused control-store/jobs tests. Root lock changes belong to the sprint integration step.

### Task 3.2 — `feat/sprint-3-proxy-config-apply`

**Ownership**: typed agent operations and the Nginx apply/remove adapter.

- Add typed `site.apply` and `site.delete` job payloads to the shared protocol and worker.
- Reuse the existing `modules/hosting` domain/config generation only after verifying its current validation and rollback behavior against this contract.
- Confine writes and removals to a dedicated FusionPanel-managed Nginx directory. Reject symlinks and paths outside that directory.
- Stage new config to a unique temporary file, validate the complete candidate config with `nginx -t`, atomically promote it, and reload Nginx only after validation succeeds.
- On write, validation, or reload failure, restore the exact prior managed file and enabled link before returning failure. Never delete or overwrite unrelated Nginx files.
- Keep PHP-FPM, TLS, HTTP/3 tuning, and advanced WAF rules out of the new operations.

**Local verification**: focused hosting renderer/adapter tests and agent protocol/worker unit tests. Add a disposable-container test harness for the Sprint 3 close gate; a mock alone cannot qualify Nginx behavior.

### Task 3.3 — `feat/sprint-3-websites-ui`

**Ownership**: the existing Websites view and typed frontend API client.

- Replace placeholder behavior with the shared API contract for listing, creating, viewing status, and deleting sites.
- Support the static and reverse-proxy request forms, show validation/API errors, and refresh pending jobs until terminal site state is reported.
- Keep any existing PHP controls visible only as explicitly unavailable/deferred; do not submit PHP work or display fabricated success.
- Keep the existing cookie/session authentication flow and accessible labels/keyboard behavior.

**Local verification**: `cd frontend && npm test -- --runInBand && npm run build`.

## 4. Existing implementation to inspect before reuse

The repository already contains `modules/hosting/src/domain/site.rs`, Nginx generation under `modules/hosting/src/config/nginx.rs`, and `VhostManager` in `modules/hosting/src/vhost.rs`, plus an existing React Websites view. These are useful starting points, not proof of acceptance. In particular, verify that failed updates restore any pre-existing config and that file operations cannot escape the managed directory before wiring them into the agent.

## 5. Out of scope

- PHP-FPM provisioning and WordPress installation.
- TLS issuance/renewal (Sprint 4), databases (Sprint 5), backups (Sprint 6), DNS (Sprint 7).
- Arbitrary shell execution, user-supplied Nginx snippets, multi-node clustering, advanced ModSecurity rules, and custom HTTP/3 tuning.
- Host-machine Nginx changes during development or tests; use an ephemeral container.

## 6. Sprint close gate

Run only after all three atomic PRs are merged into `sprint-3`:

1. `cargo test --locked --workspace`.
2. Agent crate unit tests and `cd frontend && npm test -- --runInBand && npm run build`.
3. Start an ephemeral Nginx container with a temporary document root and a local upstream service.
4. Through the UI/API, create a static site and a reverse-proxy site. Observe persisted `active` states, fetch both through Nginx with `curl`, and verify the expected content/status from the static root and upstream.
5. Submit an invalid candidate config and prove `nginx -t` rejects it while the prior live config continues serving traffic.
6. Delete both sites, verify their routes return `404`, verify unrelated test vhosts remain available, and tear down all containers/files.
7. Record exact commands, image versions, actual outcomes, and limitations in `docs/execution/SPRINTS.md`; update root lock/version and package in the sprint integration phase; then open one `sprint-3` -> `master` PR.

No local test, generated config, or mock response counts as proof that live Nginx applied or served a site. If the disposable Nginx/upstream prerequisites are unavailable, record that case as `BLOCKED` with the missing prerequisite.
