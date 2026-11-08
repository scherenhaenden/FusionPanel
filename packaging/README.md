# Development packages

The package workflow builds the control-plane API and node agent from locked Rust dependencies. Pull requests and manual runs upload a development artifact to GitHub Actions for review. These artifacts are not published releases and do not imply installation or production readiness. The package currently targets x86_64 Linux with a Debian-compatible runtime.

An annotated release tag has the exact form `yyyy.MM.dd.HH.mm.sss.g<12-character-commit-prefix>` and must pass `tools/versioning/version.py verify`. A tag-triggered build includes the annotation's JSON manifest in the archive, exposes its full version in the API build, and publishes the artifacts as a GitHub prerelease. The release also contains `RELEASE_MANIFEST.json` and `SHA256SUMS`. Follow the project's versioning policy and verify the reviewed commit before creating a tag.

To install a published prerelease, specify its **complete** tag:

```sh
./packaging/install.sh --version '2026.11.08.16.30.123.gabcdef123456'
```

That tag is an example; use an actual published tag. Each release uses a fixed asset name such as `fusionpanel-debian-x86_64.tar.gz`, and the installer falls back to the historical versioned filename. GitHub's `releases/latest` endpoint excludes prereleases, so `--version latest` does not select these development packages.

To build locally, provide a non-release label and an output directory. The standalone agent lockfile is required:

```sh
bash packaging/build-package.sh ci-local /tmp/fusionpanel-package
```
