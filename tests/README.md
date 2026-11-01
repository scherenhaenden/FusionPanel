# FusionPanel Tests

This directory contains the Python standard-library runner for FusionPanel tests.

## Test Suites

- **Unit (`--suite unit`)**: Verification of domain invariants and negative paths.
- **Integration (`--suite integration`)**: Real HTTP processes, databases, and Docker service fixtures.
- **E2E (`--suite e2e`)**: API/CLI process journey. Later browser/VM suites will fail if prerequisites are missing.

## Running Tests

Execute the runner:
```bash
python3 tests/runner.py --suite all
```

The process end-to-end lane uses the real workspace binaries. Build them first with:

```bash
cargo build -p fusionpanel-api -p fusionpanel-cli
```

The runner finds `target/debug/fusionpanel-api` and `target/debug/fusionpanel-cli` automatically. Override them with `FUSIONPANEL_API_BIN` and `FUSIONPANEL_CLI_BIN` when using binaries elsewhere. This journey exercises the real `/v1/health` and `/v1/capabilities` routes with a generated token.

Output is written in JSON format to `test_evidence.json` (configurable with `--evidence-out`). It records a run ID, completion state, Git commit, exact runner command, Python version, per-case status, expected/observed details, sanitized errors, durations, and artifact location. Integration runs also capture Docker Engine/Compose versions and resolved fixture image references pinned by digest; only image names are retained from resolved Compose configuration, never its environment or credentials. The report is written atomically under a file lock and first marks the new run `RUNNING`, so interrupted runs cannot leave an older report looking current. Any `BLOCKED`, `NOT_RUN`, or `SKIPPED` case makes qualification exit nonzero.

## Disposable VM Lane

For host mutation tests (like systemd, firewall, OS quotas), a separate disposable Debian VM lane will be used. These tests should *never* be run on the operator workstation or production hosts. VM teardown must be verified, and logs must be captured with credentials sanitized.

## Fixtures

Bounded Docker Compose fixtures for actual PostgreSQL, MariaDB, Nginx, and object-store (MinIO) dependencies are provided in `tests/fixtures/docker-compose.yml`. The MinIO fixture uses the CoollabsIO GHCR build from upstream MinIO releases because upstream no longer publishes pullable images from Docker Hub or Quay. This image is for disposable integration testing only; it is not a production image recommendation. `compose.py` generates random database and object-store passwords in a mode-0600 environment file under the system temporary directory. It never prints the credentials or puts them in command arguments, and the generated file is removed by the teardown command.

To bring them up for integration testing:
```bash
python3 tests/fixtures/compose.py up --project fusionpanel-test
FP_TEST_COMPOSE_PROJECT=fusionpanel-test python3 tests/runner.py --suite integration
python3 tests/fixtures/compose.py down --project fusionpanel-test
```

Service ports bind to loopback only and Compose assigns ephemeral host ports by default; the integration suite resolves those mappings from the isolated `fusionpanel-test` project. Set `FP_TEST_*_PORT` variables before setup and test commands only when fixed host ports are required. The suite verifies a fixture-specific Nginx response and runs identity queries against the actual PostgreSQL and MariaDB services, so an unrelated host service cannot satisfy those checks. Teardown removes only the named disposable Compose project and its volumes, then deletes its generated credential file.
