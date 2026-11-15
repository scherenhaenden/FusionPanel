# Changelog

Release identities follow [the versioning policy](docs/engineering/VERSIONING.md): the visible version and tag consist solely of the requested date stamp and source hash. Historical release entries retain their original names.

## Unreleased

- Adopt the date-and-hash release identity; preserve existing annotated SemVer tags as historical records. Cargo package versions remain internal.

### Sprint 2 — Node Agent and Jobs Runtime

- Added authenticated node registration, heartbeats, job queueing, and status/log retrieval through the API and Nodes/Jobs UI.
- Added the local HTTP agent worker for typed `system.info` and `site.validate` jobs, with persisted execution history and logs.
- Verified the local single-node browser flow through a successful `system.info` job; store reopen tests cover persisted records. A manual API/agent restart check was recorded on the earlier gate head `55e1a143`, but was not repeated on the final versioned head. Remote-node transport, clean-host installation, final-head process restarts, and multi-node behavior remain unqualified.
- Remote HTTP agents now require pre-provisioned node credentials and never bootstrap with the global admin token. Register each node from a trusted operator host through `POST /v1/nodes/register`, then configure `FUSIONPANEL_NODE_ID` and `FUSIONPANEL_NODE_TOKEN` on that agent; see [packaging/README.md](packaging/README.md).
- Set the internal Cargo workspace package version to `0.3.0-dev.1`. Release PR [#590](https://github.com/scherenhaenden/FusionPanel/pull/590) is open to `master`; no date/hash release tag or published release artifact is recorded.
- Upgrade compatibility: the agent now defaults to HTTP mode. Existing loopback deployments can keep `FUSIONPANEL_ADMIN_TOKEN` and `FUSIONPANEL_NODE_NAME` for automatic self-registration. Remote HTTP agents must remove the admin token and configure a pre-provisioned node ID/token; otherwise startup fails. The current installer regenerates a default local agent environment during install/upgrade, so remote agent settings must be reapplied after each package upgrade. Existing deployments that use the legacy WebSocket transport must set `FUSIONPANEL_AGENT_MODE=legacy-ws` and retain `FUSIONPANEL_AGENT_URL` and `FUSIONPANEL_AGENT_SECRET` (the heartbeat URL remains optional). Remote legacy endpoints must use `wss://`; `ws://` is accepted only for loopback/localhost, including the default local heartbeat endpoint. If legacy WebSocket environment variables are present without an explicit mode, the agent exits with a configuration error.

## 0.1.0-dev.1 — Initial development snapshot

### Added

- Product capability catalog and preservation policy, including selectable workflow modes.
- A dependency-aware delivery plan through 11 November 2026, detailed session specifications and real-system test expectations.
- Initial Rust workspace and independently assigned foundation packages.
- Actual-process API qualification tooling and recorded isolated foundation test observations, where available in this snapshot.
- Strict SemVer/date/hash validation, immutable annotated-tag generation and real local Git integration tests.

### Compatibility and status

- Early development prerelease. Hosting, migration and operational readiness require their individual acceptance evidence.
- Existing foundation PRs and parallel sessions remain separately reviewable until integrated. A tag must describe only code present at its exact commit.
- Initial package version is `0.1.0-dev.1`; executables and build metadata must be reconciled during integration instead of retaining unrelated hard-coded version strings.
- No production deployment or zero-downtime guarantee is implied.
