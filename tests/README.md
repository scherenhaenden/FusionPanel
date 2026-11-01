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

Output is written in JSON format to `test_evidence.json` (configurable with `--evidence-out`).

## Disposable VM Lane

For host mutation tests (like systemd, firewall, OS quotas), a separate disposable Debian VM lane will be used. These tests should *never* be run on the operator workstation or production hosts. VM teardown must be verified, and logs must be captured with credentials sanitized.

## Fixtures

Bounded Docker Compose fixtures for actual PostgreSQL, MariaDB, Nginx, and object-store (MinIO) dependencies are provided in `tests/fixtures/docker-compose.yml`.

To bring them up for integration testing:
```bash
docker-compose -f tests/fixtures/docker-compose.yml up -d
```
