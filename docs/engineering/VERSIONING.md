# Strict Versioning and Immutable Tags

## Version identity

FusionPanel uses **Semantic Versioning 2.0.0**, a user-specified date stamp and an exact source commit identity. The initial development base is `0.1.0-dev.1`. This is a prerelease, not a stable or production-ready release.

The full release version and annotated Git tag have these forms:

```text
Version: MAJOR.MINOR.PATCH[-PRERELEASE]+yyyy.MM.dd.HH.mm.sss.g<12-character-commit-prefix>
Tag:     vMAJOR.MINOR.PATCH[-PRERELEASE]+yyyy.MM.dd.HH.mm.sss.g<12-character-commit-prefix>
Example: v0.1.0-dev.1+2026.11.01.15.30.123.gabcdef123456
```

The example is illustrative, not an existing tag. The actual timestamp and hash are generated from the tagged commit and current release time. The date zone is **Europe/Berlin**, including its daylight-saving offset. `yyyy`, `MM`, `dd`, `HH` and `mm` represent year, month, day, 24-hour hour and minute. `sss` is the millisecond component, exactly three digits, `000` through `999`.

The exact requested pattern `yyyy.MM.dd.HH.mm.sss` has no seconds field. Preserve that pattern in version metadata; retain the complete instant, including seconds and milliseconds, in the annotated tag's local and UTC ISO 8601 fields. Do not interpret the shorter stamp alone as an unambiguous chronological timestamp. The prerelease sequence controls release ordering; the hash identifies the source.

## Authoritative values

- Root `VERSION` contains one strict SemVer core/prerelease value, such as `0.1.0-dev.1`, with no manually written date/hash metadata.
- `workspace.package.version` in root `Cargo.toml` must match `VERSION` exactly. Packages in the shared workspace inherit it. Standalone modules are normalized on integration; they must not silently define the overall product's version.
- Annotated tag metadata contains the full generated version, full Git commit SHA, 12-character prefix, canonical branch, timestamp pattern/time zone, complete local/UTC timestamps and release kind.
- A build/release pipeline should expose the manifest's full version when embedded. Package versions remain the core/prerelease value. Until embedding is integrated, executables must expose their actual package version, not an unrelated hard-coded version or a fabricated build hash.

Build metadata after `+` does **not** affect SemVer precedence. Changing only date/hash metadata is not a new SemVer release. Every newly published snapshot must increase core/prerelease, even when the code change is small. Do not republish `0.1.0-dev.1` against another commit under another date/hash tag.

## Choosing the next version

| Change | Required version decision |
|---|---|
| Another development snapshot before initial release | Increment prerelease numerically: `0.1.0-dev.1` → `0.1.0-dev.2`. |
| Compatible bug fix after a release | Increment PATCH, optionally progressing through prereleases first. |
| Compatible new public capability after a release | Increment MINOR and reset PATCH. |
| Breaking stable public contract | Increment MAJOR and reset MINOR/PATCH. |
| Breaking change while MAJOR is zero | Increment MINOR, document the incompatible contract and migration; do not disguise it as PATCH. |
| Documentation/build-only published snapshot | Increment the applicable prerelease sequence; changing only metadata does not establish precedence. |
| Stable release from an accepted prerelease | Remove prerelease only when release acceptance is complete; stable has higher precedence than its prereleases. |

A larger version is necessary but does not prove the semantic impact was classified correctly. The reviewer must inspect API/data/protocol/user-workflow changes and record the bump rationale in the changelog/PR. Dates and branch names do not replace this review.

## Local commands

The Python 3.11+ standard-library tool uses only local Git and performs no network, push or GitHub operations:

```sh
python3 tools/versioning/version.py check
python3 tools/versioning/version.py describe
python3 tools/versioning/version.py tag
python3 -m unittest discover -s tools/versioning/tests -v
```

`check` validates configuration and existing version-tag history; it does not certify release readiness or require a clean working tree. `describe` computes a release manifest only on clean `master` with a publishable version. `tag` creates an annotated tag from that manifest, targeting the exact checked commit. Both reject dirty tracked/untracked content, non-`master` or detached HEAD, reused/downgraded versions and invalid existing version tags. The tool never creates lightweight release tags, overwrites tags or forces Git operations.

Historical `v*` tags must use the exact strict format, be annotated, contain a matching JSON manifest and target a commit whose prefix matches their metadata. Malformed/tampered annotations or multiple tags for one core/prerelease version block publication. Non-release tags without the `v` prefix are outside the release history.

## Publication sequence

1. Review and integrate the code and required tests on `master`.
2. Set and review the next `VERSION` and matching Cargo version. Update `CHANGELOG.md` with behavior, compatibility, validation and limitations.
3. Commit everything intended for the snapshot. Complete required checks and verify a clean tree.
4. Fetch/verify the remote default branch and ensure the intended `master` commit is the reviewed publication target. The offline version tool cannot verify remote state.
5. Run `check`, then `tag`; save the emitted JSON manifest as the release/build artifact. Do not put an untracked generated manifest inside the repository before tagging.
6. Push the exact commit and exact annotated tag through the authorized external Git workflow. Never use force or move an existing tag.
7. Verify the remote tag's peeled commit against the recorded full SHA. A failed push is not permission to move/delete/recreate a published tag; diagnose and preserve its immutable identity.
8. Publish release artifacts and notes only with truthful acceptance/known-gap status. A development tag does not establish production readiness.

GitHub tag protection/rulesets should prohibit updates and deletion of `v*` tags. The local tool enforces policy for its own operations; repository administrators can still bypass an unprotected local workflow using raw Git. Server-side policy must be configured separately by the repository coordinator.

## Verification performed

The versioning suite uses real temporary local Git repositories, commits and annotated tags. It checks exact target SHA and annotation identity, refusal of dirty/untracked trees and wrong/detached branches, immutable publication, monotonic prereleases, invalid lightweight/hash-mismatched/tampered tags and portable CLI checks. Unit cases cover strict SemVer, numeric prerelease ordering, ignored build metadata, exact millisecond formatting, summer/winter offsets and invalid input. No production repository tag is created by the test suite.
