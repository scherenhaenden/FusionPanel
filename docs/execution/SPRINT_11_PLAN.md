# Sprint 11 Plan — Mail Core and Debian Packaging

- **Status** · Ready for owner review · planning only · 22 November 2026
- **Integration branch after kickoff** · `sprint-11` from verified `origin/master`
- **Proposed versions** · Cargo workspace `0.12.0-dev.1`, Debian package `0.12.0~dev.1-1`

Part of the [Sprints 11–20 delivery plan](SPRINTS_11_20_PLAN.md).

## Goal

An operator can use the Mail screen to provision a hosted domain, mailboxes and local aliases on one node, publish a DKIM selector to an owned DNS zone, and confirm real SMTP submission and IMAP retrieval. A second operator can install the same Sprint 11 candidate from a reproducible `.deb` on clean Debian, sign in to the bundled UI, and see a real agent reconnect after reboot.

## Starting point and kickoff

- GitHub `master` was `a8861e6110efaae7dbc589474335ca98c93a44c2` at planning. The Sprint 4 TLS and Sprint 7 DNS/DKIM merges are in its history. Recheck and record the actual base SHA at kickoff.
- On 22 November, GitHub `sprint-10` was `7516b972cfc27bd66f0d4c2a241aa5058375e2ff`. Its audit follow-up and full release acceptance are pending. Use the [Sprint 10 handoff](SPRINTS_11_20_PLAN.md#handoff-from-sprint-10) to track open decisions and gates, including [#51](https://github.com/scherenhaenden/FusionPanel/issues/51) and [#685](https://github.com/scherenhaenden/FusionPanel/issues/685).
- **Proposed order for review:** finish and merge Sprint 10 first to keep release qualification in version order. Mail's roadmap dependencies are Sprints 4 and 7. If the owner changes the release order, agree on the version assignment before creating the branch, and still branch from the verified `master`.
- Open tickets [#686](https://github.com/scherenhaenden/FusionPanel/issues/686), [#687](https://github.com/scherenhaenden/FusionPanel/issues/687) and [#688](https://github.com/scherenhaenden/FusionPanel/issues/688) already cover the sprint. Use them for branch PRs. Confirm availability of the disposable mail runner and full Debian guest, and record who owns the integration and evidence, before committing to a delivery date.
- Reuse the existing `MailManage` permission, DNS DKIM path, job and agent model, [mail primitives](../../modules/mail/), [network-services mail worker](../../integration/network-services/src/mail_worker.rs), and [package build](../../packaging/build-package.sh). Treat their source and existing smoke tests as prerequisites to inspect, with product acceptance supplied by the release gates below.

## Work items and branch ownership

All branches below start from `sprint-11` after their prerequisites are present and all PRs target `sprint-11`. Points are relative planning estimates, including the associated fixture work.

| Work item | Branch and issue | Prerequisite | Estimate |
| --- | --- | --- | --- |
| 11.1a · Mail API and job contract | `feat/sprint-11-mail-api` · [#686](https://github.com/scherenhaenden/FusionPanel/issues/686) | Kickoff | 3 |
| 11.1b · Mail screen | `feat/sprint-11-mail-ui` · [#686](https://github.com/scherenhaenden/FusionPanel/issues/686) | 11.1a DTOs and routes merged | 2 |
| 11.2 · Agent apply and signing | `feat/sprint-11-mail-agent-sync` · [#687](https://github.com/scherenhaenden/FusionPanel/issues/687) | 11.1a job contract merged | 5 |
| 11.3 · Debian package | `feat/sprint-11-deb-packaging` · [#688](https://github.com/scherenhaenden/FusionPanel/issues/688) | Kickoff · independent of Mail API and UI | 3 |
| Mail E2E fixture | `test/sprint-11-mail-e2e` · [#687](https://github.com/scherenhaenden/FusionPanel/issues/687), [#54](https://github.com/scherenhaenden/FusionPanel/issues/54) | 11.1b and 11.2 merged | Included in 11.2 |
| Debian systemd fixture | `test/sprint-11-deb-systemd-e2e` · [#688](https://github.com/scherenhaenden/FusionPanel/issues/688) | 11.3 merged | Included in 11.3 |

### 11.1a · Mail API and job contract

**Write paths:** `crates/api/`, `crates/control-store/` and `crates/agent-protocol/` for module, route, storage and job changes. The integration owner handles root lockfile changes if wiring a reviewed path dependency requires them. Links for scope are [S14 / #17](https://github.com/scherenhaenden/FusionPanel/issues/17) and [S29 / #32](https://github.com/scherenhaenden/FusionPanel/issues/32).

- Add `/v1/mail/domains` and the `/api/v1` mirror, with domain-scoped `mailboxes`, `aliases`, and `dkim` resources. Define list/create/update/delete semantics and the typed agent request/result in the same PR. Keep names and addresses stable as identifiers. Surface queued, applied and failed revisions, including the job ID for a pending change.
- Derive the tenant and authorization from the session or scoped token. Require `MailManage`, a node that the operator can manage, and ownership of any linked DNS zone. Enforce canonical unique domain/mailbox names. Aliases in this slice point to local mailboxes owned by the same tenant. Report conflicts for duplicate names, foreign references, and deleting a domain with active mailboxes or aliases.
- Persist desired state and job reservation consistently. Scope request idempotency and distinguish an exact replay from a conflicting replay. Expose the public DKIM TXT material only. Use scoped secret handling for mailbox credentials, hashes and private key material, and keep them out of public DTOs, job logs and audit details.
- **Unit acceptance:** bad addresses/selectors, local alias target validation, unauthenticated requests, denied foreign tenant and node access, conflicting replays, failed or obsolete completions, and secret redaction. Exercise the changed mail cases with `CARGO_BUILD_JOBS=2 cargo test --locked --offline -p fusionpanel-api -p fusionpanel-control-store -p fusionpanel-agent-protocol mail`. Name the tests so the filter covers the new cases.

### 11.1b · Mail screen

**Write paths:** `frontend/src/views/Mail/`, route and sidebar registration, typed frontend API and types, and their focused Jest suites.

- Expose `/mail` to operators with the appropriate permission. Use the production API for domain, mailbox, local alias and DKIM operations. Show the public selector/TXT and publication state for an owned DNS zone.
- Distinguish loading, empty and denied states, mutation errors, and queued, applied or failed agent results. Confirm removals, clear credentials from form state after submission succeeds, and keep secrets out of network responses and UI history. Use the agreed idempotency contract when retrying a submission.
- **Unit acceptance:** capability and permissions, required form validation, domain/mailbox/alias/DKIM interactions, failed apply and retry, and no credentials displayed after submission. With the work item creating `src/__tests__/Mail.test.tsx`, run `CI=true npm test -- --runInBand src/__tests__/Mail.test.tsx src/__tests__/api.test.ts src/__tests__/routes.test.tsx` from `frontend/`. Browser acceptance uses the actual service in the sprint gate.

### 11.2 · Agent apply and signing

**Write paths:** `crates/agent/`, `integration/network-services/`, and `modules/mail/` only for worker or supported primitive changes within this slice. The API task owns any initial protocol changes. Later changes to shared contracts are separate coordinated PRs.

- Wire the supported mail job through the authenticated node worker. Reuse the validated mailbox/alias config and DKIM primitives. Stage tenant-scoped Postfix maps, Dovecot accounts/Maildirs and signing configuration, validate using fixed service commands, and switch/reload as one managed apply. Confine writes to the configured managed root and use the least privileges needed by the adapter.
- Keep the last valid config active if validation or reload fails. Hold one revision at a time per managed domain, deduplicate repeats, reject stale revisions, and preserve unaffected domains and message data. Return sanitized results to the control plane so jobs can be retried safely.
- Generate/store private DKIM material with restricted ownership on the node, then pass the public selector/TXT through the Sprint 7 DNS integration. Require a usable signing configuration before reporting DKIM signing ready.
- **Unit acceptance:** fixtures for render order, permissions, replay and stale revisions, validation/reload failure, restoration of the previous config, and secret redaction. Run the agent's filtered `CARGO_BUILD_JOBS=2 cargo test --locked --offline --manifest-path crates/agent/Cargo.toml mail` and only the changed unit targets in `integration/network-services` or `modules/mail` when edited. Unit doubles cover failure paths. Real daemon behavior is checked at sprint close.

### 11.3 · Reproducible Debian package

**Write paths:** `packaging/` for build, Debian staging, units and focused packaging tests. The integration owner handles `.github/workflows/package.yml` if an artifact upload is needed. Related coverage remains tracked in [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) and [#53](https://github.com/scherenhaenden/FusionPanel/issues/53).

- Add an `amd64` `.deb` built in the Debian Bookworm build environment from the same locked binaries and frontend as the existing archive. Bundle API, agent, CLI, bundled UI and applicable service units/helper. Define package dependencies, maintainer scripts, service ownership, files under `/etc/fusionpanel` and `/var/lib/fusionpanel`, and unit paths that match the packaged binaries. Preserve the existing archive and installer path.
- Fix the source SHA, build image/toolchain, dependencies and `SOURCE_DATE_EPOCH`. Normalize file order, ownership and timestamps in the archive. Include the candidate SHA and package version as build metadata. Repeated installation must preserve generated credentials and state.
- **Unit acceptance:** add `packaging/tests/test_deb_layout.sh` to validate package metadata, paths/modes, maintainer script syntax, and fixture staging without installing a package or touching system services. Run the focused new test and the affected archive-layout unit check. Compare actual independently built `.deb` files and exercise service activation in the final sprint gate.

## Sprint integration and acceptance

1. After approval and kickoff, advance 11.1a and 11.3 independently. Once 11.1a lands, advance 11.1b and 11.2 against that integration state. Review the unit evidence and merge each atomic PR with a regular merge commit. Add the two fixture PRs once their dependencies land. Limit their pre-merge validation to fixture wiring and syntax.
2. Merge `chore/sprint-11-version-bump` into `sprint-11`. Follow the preceding sprint's workspace, agent protocol metadata and lockfile version changes. Wire the Debian version to the same semver source. Freeze and record the candidate SHA and its `master` base, then run the following gates against that SHA.

| Gate | Required observation |
| --- | --- |
| Workspace and package regression | Locked workspace unit and integration suites, standalone agent and changed standalone mail/network-service packages, frontend suite and production build, format/Clippy/builds and required CI. Include login, RBAC, jobs, DNS/DKIM and existing archive/installer checks. Run changed-manifest CI and sprint-required gates on an isolated runner. Reserve the full catalog for the concurrency-limited manual workflow. |
| Mail journey · disposable Docker/Compose runner | Add the product fixture under `tests/e2e/network-services/`. Start candidate API, agent and UI, pinned real Postfix/Dovecot, fixture signing service and authoritative DNS. In a real browser, create a domain, two mailboxes and one local alias and verify the job reaches applied. Confirm the public DKIM TXT via an authoritative query. With fixture TLS, submit a uniquely identified message using SMTP AUTH to the alias, retrieve the same message in the intended mailbox over IMAP, and verify the message's DKIM signature against the fixture DNS. Verify denied cross-tenant UI/API access, rejection of incorrect credentials and unauthenticated relay, and unchanged delivery after a staged config failure and retry. Reopen the API/agent and verify durable state and one copy of the message. |
| Debian package reproducibility · isolated build runners | Build twice from the candidate with pinned inputs and separate clean build directories. Check `dpkg-deb --info`, content and modes, embedded SHA/version, and matching SHA-256. Save both digests and retain the verified candidate package. |
| Debian clean install · full OS fixture | Run the `tests/qualification/installer/` lane on a clean Debian 12 amd64 guest running real systemd as PID 1. Install the verified artifact with `apt install ./<candidate>.deb` inside the guest. Confirm active API and agent units under the expected accounts, loopback health, browser login to the bundled UI and a real agent heartbeat. Reboot, confirm the units are active and the same data and node identity persist, and preserve sanitized package, journal and browser evidence. |

**Environment decision for review:** #688 names a systemd-enabled Debian container. This plan proposes a clean Debian VM for the systemd and reboot evidence, consistent with the [test strategy](../testing/TEST_STRATEGY.md). If the issue needs its container lane retained, run it inside a disposable runner with real systemd and record it separately. Missing services, Docker or the Debian guest make the affected required gate `BLOCKED`.

3. Keep feature checks local and focused. For the final gate use one heavy workload at a time, preferably on the disposable CI/test runner. Preflight RAM, swap and disk including at least 10 GiB at the Cargo target for a local matrix. Before any Docker pull, build or start, check `/`, `/tmp` and the Docker data filesystem and record storage/volume inventory. Use bounded timeouts, isolated directories, unique fixture names and ephemeral local ports. Remove only attributable fixture resources. On the workstation, avoid system installs, service/reload commands and global pruning.
4. Record exact commands, elapsed times, runner/fixture versions, sanitized logs, artifacts, teardown and limitations in the evidence PR. Update [SPRINTS.md](SPRINTS.md), [PR_ISSUE_TRACKER.md](PR_ISSUE_TRACKER.md) and this plan. Recheck the final sprint head after evidence and fixes. Repeat affected gates on any code, version, dependency, fixture or base change. Reuse evidence after a docs-only commit only where the repository gate supports it.
5. After required integration, live E2E and quality gates pass, open one PR `sprint-11` → `master`. Link the feature/test PRs, issue references, exact head/base SHAs and package evidence. Confirm required checks including the release-ready package run on the reviewed candidate before any merge. Issue closure requires verified acceptance on `master` and recorded merge SHA/results. Treat immutable date/hash tag and artifact publication as a separate release decision under [VERSIONING.md](../engineering/VERSIONING.md).

## Scope limits

The sprint delivers a single managed mail node, local aliases, one supported DKIM signing setup and a clean Debian amd64 package. Keep external deliverability, remote relay, public DNS propagation, DNSSEC, mailbox restore, bulk migration, webmail, antispam, HA, installer upgrade/rollback qualification and PHP in their existing follow-up scope. A passing roundtrip is scoped evidence for #54. DNSSEC and mailbox restore still have their own acceptance cases.

## Integrated candidate & qualification evidence — 28 November 2026

- **Synced `master` base:** `84d0c190422e5f5fae02b4b93adbb99b39e0bc8b` (Sprint 10 merge PR #728).
- **Candidate SHA before docs commit:** `733a87ada5a57aba99705b6f765d96e34c8ba9a6` (`0.12.0-dev.1`, Debian `0.12.0~dev.1-1`).
- **Release PR (`sprint-11` → `master`, Draft):** [#731](https://github.com/scherenhaenden/FusionPanel/pull/731).

| PR | Scope | Status |
| --- | --- | --- |
| [#708](https://github.com/scherenhaenden/FusionPanel/pull/708) | 11.3 Reproducible Debian Bookworm `amd64` `.deb` (#688) | Merged (`16ec095`) |
| [#709](https://github.com/scherenhaenden/FusionPanel/pull/709) | 11.1a Mail API, control-store v11, `agent-protocol` (#686) | Merged (`48e84d1`) |
| [#714](https://github.com/scherenhaenden/FusionPanel/pull/714) | Debian 12 systemd clean-install & reboot fixture (#688) | Merged (`5d89b95`) |
| [#715](https://github.com/scherenhaenden/FusionPanel/pull/715) | 11.1b `/mail` screen, typed API, Jest tests (#686) | Merged (`1cb8fb6`) |
| [#716](https://github.com/scherenhaenden/FusionPanel/pull/716) | 11.2 Agent `mail_driver` Postfix/Dovecot/DKIM + rollback (#687) | Merged (`92e331d`) |
| [#724](https://github.com/scherenhaenden/FusionPanel/pull/724) | Mail Docker/Compose/Playwright E2E fixture (#687, #54) | Merged (`993955f`) |
| [#725](https://github.com/scherenhaenden/FusionPanel/pull/725) | Version bump to `0.12.0-dev.1` | Merged (`b35a6f2`) |
| [#726](https://github.com/scherenhaenden/FusionPanel/pull/726) | Workspace & agent `rustfmt` alignment | Merged (`e074224`) |
| [#729](https://github.com/scherenhaenden/FusionPanel/pull/729) | `control-store` v11 `nodes.tenant_id` migration guard & test update | Merged (`04085cc`) |
| [#733](https://github.com/scherenhaenden/FusionPanel/pull/733) | Sync `sprint-11` with `master` (`84d0c19`, Sprint 10 #728) | Merged (`733a87a`) |

| Gate | Observed result | Status |
| --- | --- | --- |
| `cargo fmt --all -- --check` | Clean on `733a87a` (and `crates/agent`, `modules/mail` on `04085cc`) | PASS |
| Workspace unit/integration (`cargo test --locked --offline --workspace`) | Passed on `04085cc`; `fusionpanel-api` (43/43) & `fusionpanel-control-store` (23/23) re-verified on `733a87a` | PASS |
| Agent suite (`cargo test --locked --offline --manifest-path crates/agent/Cargo.toml`) | 58 passed, 0 failed (`04085cc`) | PASS |
| Mail module (`cargo test --offline --manifest-path modules/mail/Cargo.toml`) | Passed on `04085cc`; 1 live SMTP/IMAP daemon test ignored (`BLOCKED` in unit mode) | PASS for exercised cases |
| Clippy (`--all-targets -- -D warnings`) | Workspace and `crates/agent` passed with 0 warnings (`04085cc`) | PASS |
| Frontend Jest & build | 12 suites / 55 tests passed; `tsc -b && vite build` passed (`733a87a`) | PASS |
| Packaging & fixture syntax/wiring | `test_deb_layout.sh` (2x reproducible `.deb`), `test_package_layout.sh`, `test_systemd.sh`, `test_deb_systemd_qualification.sh --check`, Mail E2E `bash -n`/`node --check` passed (`04085cc`) | PASS |
| Mail journey · live Docker/Playwright (`tests/e2e/network-services/fixtures/sprint11/run.sh`) | Fixture wired and syntax-checked; live container stack not executed | NOT_RUN |
| Debian clean install & reboot · full OS guest (`test_deb_systemd_qualification.sh --ssh ...`) | Fixture wired and `--check` passed; live Debian 12 VM not booted | NOT_RUN |
