# Sprint 4 Plan & Handoff Guide — TLS / SSL Core

**Sprint Branch**: `sprint-4` (branched from `master` @ `678fab8`, `v0.4.0-dev.1`)  
**Target Release**: `sprint-4` -> `master` (`0.5.0-dev.1` upon sprint gate pass)  
**Docker Required**: **Yes for release gates** (disposable Pebble ACME + Nginx fixture; atomic feature unit tests run natively without Docker)  
**Estimated Effort**: M (5 story points)  

---

## 1. Sprint Goal (100% E2E Deliverable)

Enable operators to **issue/install TLS certificates (ACME HTTP-01 or custom PEM) for any managed website on `/v1/websites`**, promote HTTPS configuration safely via the privilege-separated Nginx helper, serve real HTTPS traffic (`443`), and renew/remove certificates with full rollback protection on failure.

### What Works at Sprint 4 Close
1. Control plane stores tenant-scoped TLS certificate metadata and state transitions (`pending`, `active`, `renewing`, `failed`) in `ControlStore` without exposing private key material in API responses or logs.
2. Authenticated operators can enable HTTPS via ACME or upload a custom cert/key pair from the Websites UI or REST API (`tls.apply` / `tls.delete` agent operations).
3. `fusionpanel-nginx-helper` validates staged HTTPS vhost configs (`nginx -t`), preserves HTTP-01 challenge isolation and non-managed Nginx configs, reloads safely, and rolls back automatically if validation or reload fails.
4. Disposable Sprint 4 Docker/Playwright fixture verifies real HTTPS traffic (`443` with Pebble CA) and certificate renewal/rollback without disk or Docker state leakage.

---

## 2. Atomic Branches & Handoff Task Breakdown

Each branch **must** be developed independently from `sprint-4`, run **only local unit tests** for touched crates/views (plus `cargo fmt --check` and `cargo clippy -- -D warnings`), and open an atomic **PR targeting `sprint-4`**.

### Task 4.1 — `feat/sprint-4-tls-api` (Control Plane TLS State, Protocol & Endpoints)
* **Primary Paths**:
  * `crates/control-store/` (TLS certificate/binding schema + state machine)
  * `crates/agent-protocol/` (typed `TlsApply` / `TlsDelete` operation payloads)
  * `crates/api/` (tenant-scoped `/v1/websites/:id/tls` endpoints + idempotency + redaction)
* **Local Unit Tests**:
  * `cargo fmt --all --manifest-path Cargo.toml -- --check`
  * `RUST_MIN_STACK=33554432 cargo clippy --manifest-path Cargo.toml -- -D warnings`
  * `RUST_MIN_STACK=33554432 cargo test --locked --offline -p fusionpanel-control-store -p fusionpanel-agent-protocol -p fusionpanel-api`
* **PR Target**: `feat/sprint-4-tls-api` -> `sprint-4`

### Task 4.2 — `feat/sprint-4-acme-runner` (Hosting TLS Rendering, Helper RPC & ACME Runner)
* **Primary Paths**:
  * `modules/hosting/` (HTTPS server block rendering, HTTP-01 challenge isolation, PEM/key path & permission validation)
  * `crates/agent/` (`fusionpanel-nginx-helper` root-owned cert/vhost promotion + rollback + ACME/Pebble HTTP-01 worker flow)
* **Local Unit Tests**:
  * `cargo fmt --manifest-path modules/hosting/Cargo.toml -- --check`
  * `RUST_MIN_STACK=33554432 cargo clippy --manifest-path modules/hosting/Cargo.toml -- -D warnings`
  * `RUST_MIN_STACK=33554432 cargo test --manifest-path modules/hosting/Cargo.toml --test unit_managed_site`
  * `RUST_MIN_STACK=33554432 cargo test --locked --offline --manifest-path crates/agent/Cargo.toml --lib`
* **PR Target**: `feat/sprint-4-acme-runner` -> `sprint-4`

### Task 4.3 — `feat/sprint-4-tls-ui` (Frontend TLS Enable/Upload/Status UI)
* **Primary Paths**:
  * `frontend/src/api/`, `frontend/src/types/`, `frontend/src/views/Websites/`, `frontend/src/__tests__/`
* **Local Unit Tests**:
  * `cd frontend && npm test -- --runInBand src/__tests__/Websites.test.tsx && npm run build`
* **PR Target**: `feat/sprint-4-tls-ui` -> `sprint-4`

---

## 3. Out of Scope (Do Not Implement in Sprint 4)

* Multi-provider DNS-01 wildcards and custom OCSP stapling.
* PHP-FPM, database provisioning, backups, or multi-node clustering (Sprints 5–12).

---

## 4. Resource Guard & Sprint 4 Close Gate

Before running any Docker/Playwright fixture:
- Check `df -h / /tmp`, `free -h`, `docker system df`, and never run global `docker system prune`.
- Reuse the Sprint 3 bounded BuildKit/Compose pattern (`512MB` cache cap, log rotation, container-only teardown).

After all 3 atomic PRs are merged into `sprint-4`:
1. Full workspace, `crates/agent`, `modules/hosting`, and frontend suites (`fmt`, `clippy -- -D warnings`, `test`, `build`).
2. Disposable Sprint 4 Pebble + Nginx + Playwright HTTPS E2E (`443` served, invalid cert/key rejected & rolled back, zero residual Docker state).
3. Bump version to `0.5.0-dev.1`, refresh `Cargo.lock` + `crates/agent/Cargo.lock`, build package via `packaging/build-package.sh`, update `docs/execution/SPRINTS.md`, and open **`sprint-4` -> `master`**.
