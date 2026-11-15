# Changelog

Release identities follow [the versioning policy](docs/engineering/VERSIONING.md): the visible version and tag consist solely of the requested date stamp and source hash. Historical release entries retain their original names.

## Unreleased

- Adopt the date-and-hash release identity; preserve existing annotated SemVer tags as historical records. Cargo package versions remain internal.

### Sprint 2 — Node Agent and Jobs Runtime

- Added authenticated node registration, heartbeats, job queueing, and status/log retrieval through the API and Nodes/Jobs UI.
- Added the local HTTP agent worker for typed `system.info` and `site.validate` jobs, with persisted execution history and logs.
- Verified a local single-node flow and persistence across API/agent restarts. Remote-node transport, clean-host installation, and multi-node behavior remain unqualified.
- Set the internal Cargo workspace package version to `0.3.0-dev.1`. Release PR [#590](https://github.com/scherenhaenden/FusionPanel/pull/590) is open to `master`; no date/hash release tag or published release artifact is recorded.
- Upgrade compatibility: the agent now defaults to HTTP mode. Existing deployments that use the legacy WebSocket transport must set `FUSIONPANEL_AGENT_MODE=legacy-ws` and retain `FUSIONPANEL_AGENT_URL` and `FUSIONPANEL_AGENT_SECRET` (the heartbeat URL remains optional). If legacy WebSocket environment variables are present without an explicit mode, the agent exits with a configuration error.

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
