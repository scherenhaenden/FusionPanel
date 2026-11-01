# Changelog

Version changes follow [the strict versioning policy](docs/engineering/VERSIONING.md). Full annotated tags add the exact requested date stamp and source hash; this file records the SemVer core/prerelease.

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
