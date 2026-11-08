# Release version and immutable tags

The user-visible FusionPanel version and its annotated Git tag are exactly:

```text
yyyy.MM.dd.HH.mm.sss.g<12-character-commit-prefix>
Example: 2026.11.01.15.30.123.gabcdef123456
```

There is no `v` prefix and no SemVer prefix in this identity. The timestamp uses `Europe/Berlin`, including daylight saving. `sss` is the three-digit millisecond component. The requested pattern does not contain seconds; the annotation records the complete timestamp in both local time and UTC. Do not use the visible version alone to order releases: the omitted seconds and daylight-saving changes make it ambiguous.

Cargo still requires its own SemVer-compatible package version. The Rust manifests retain that internal value; it is not the release identity. A build pipeline must embed the annotated release version into shipped binaries and artifacts instead of presenting the Cargo package version as the release version. A local build without such a manifest is unversioned for release purposes.

## Manifest and tag validation

New annotated tags contain a JSON manifest with `schema_version: 2`, `version`, `tag`, `commit`, `short_commit`, `branch`, `date_pattern`, `timezone`, `created_at_local`, `created_at_utc`, and `release_kind`. Both `version` and `tag` have the exact identity above. The release tool currently creates prereleases; a stable release requires a separate acceptance decision.

Previously created annotated `v<SemVer>+<date>.g<hash>` tags remain valid historical records. They are validated using their original schema and are never rewritten or moved. They do not dictate the format of new versions. An existing release tag is immutable, and an already tagged commit cannot be tagged again.

```sh
python3 tools/versioning/version.py check
python3 tools/versioning/version.py describe
python3 tools/versioning/version.py tag
python3 tools/versioning/version.py verify --tag '<date-hash-tag>'
python3 -m unittest discover -s tools/versioning/tests -v
```

`check` validates locally available release tags; fetch all remote tags before using it for a publication decision. `describe` and `tag` require clean `master`; `tag` creates an annotated tag locally without pushing. `verify` validates the given new-format tag and prints its annotated manifest. CI must independently confirm that its checked-out commit equals the tag's peeled commit and that all required checks succeeded. The tool itself does not publish artifacts, establish production readiness, or inspect GitHub state.

## Publication sequence

Review and integrate the intended source and checks on `master`, document scope and limitations, and verify the remote master commit. Generate an annotated tag from a clean checkout with the version tool; preserve its manifest, push the exact commit and tag without force, and verify the peeled remote SHA. Produce and publish artifacts and notes with their actual acceptance status. Existing tags must never be retargeted or overwritten.
