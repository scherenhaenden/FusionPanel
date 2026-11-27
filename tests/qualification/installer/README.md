# Installer & Debian `.deb` Systemd Qualification

This directory contains the qualification fixtures for FusionPanel installation:

- `test_install_qualification.sh`: Mocked archive/installer (`packaging/install.sh`) idempotency and unit hardening check.
- `test_deb_systemd_qualification.sh`: Sprint 11 clean Debian 12 (`bookworm`) `amd64` `.deb` install, systemd activation, loopback health, bundled UI, agent heartbeat, and reboot persistence fixture (issue #688).

## Pre-merge validation (safe on workstation / CI)

Validates bash syntax, packaging/systemd/API wiring, and safety guards without installing packages or touching host systemd:

```bash
./tests/qualification/installer/test_deb_systemd_qualification.sh --check
```

## Full OS release gate (clean Debian 12 amd64 guest with systemd as PID 1)

Run against a clean Debian 12 `amd64` VM over SSH (automates `apt-get install -y ./<candidate>.deb`, unit user/active checks, `http://127.0.0.1:8080/v1/health`, bundled UI check, `/v1/nodes` agent heartbeat, `systemctl reboot`, and post-reboot identity/database persistence):

```bash
./tests/qualification/installer/test_deb_systemd_qualification.sh \
  --deb /path/to/fusionpanel_0.12.0~dev.1-1_amd64.deb \
  --ssh root@debian12-guest \
  --evidence-dir /tmp/fusionpanel-deb-evidence
```

Or run inside a disposable Debian 12 `amd64` systemd guest directly (requires `FUSIONPANEL_DEB_E2E_ALLOW_LOCAL_INSTALL=1`):

```bash
# Phase 1 (before reboot)
FUSIONPANEL_DEB_E2E_ALLOW_LOCAL_INSTALL=1 \
  ./tests/qualification/installer/test_deb_systemd_qualification.sh \
  --local-guest --deb ./fusionpanel_0.12.0~dev.1-1_amd64.deb

# Reboot guest, then Phase 2 (after reboot)
FUSIONPANEL_DEB_E2E_ALLOW_LOCAL_INSTALL=1 \
  ./tests/qualification/installer/test_deb_systemd_qualification.sh \
  --local-guest --post-reboot
```
