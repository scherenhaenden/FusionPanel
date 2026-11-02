#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
SYSTEMD_DIR="${SCRIPT_DIR}/../systemd"

echo "Testing systemd unit files..."

# Ensure systemd-analyze is installed
if ! command -v systemd-analyze &> /dev/null; then
    echo "BLOCKED: systemd-analyze is not installed. Skipping tests."
    exit 0
fi

# We have to ignore "is not executable" errors since the files aren't actually installed on the test runner.
# systemd-analyze verify checks for missing executables and fails if they aren't there.
# To bypass, we can touch fake files or just grep the output.
# A better approach is to mock the binaries or filter the specific expected error.

mkdir -p /tmp/fusionpanel-test-bin
touch /tmp/fusionpanel-test-bin/fusionpanel-api
touch /tmp/fusionpanel-test-bin/fusionpanel-agent
chmod +x /tmp/fusionpanel-test-bin/fusionpanel-api
chmod +x /tmp/fusionpanel-test-bin/fusionpanel-agent

# Create temporary modified unit files pointing to our fake binaries for testing
TMP_UNIT_DIR=$(mktemp -d)
trap 'rm -rf "${TMP_UNIT_DIR}"' EXIT

sed 's|/usr/bin/fusionpanel-api|/tmp/fusionpanel-test-bin/fusionpanel-api|g' "${SYSTEMD_DIR}/fusionpanel.service" > "${TMP_UNIT_DIR}/fusionpanel.service"
sed 's|/usr/bin/fusionpanel-agent|/tmp/fusionpanel-test-bin/fusionpanel-agent|g' "${SYSTEMD_DIR}/fusionpanel-agent.service" > "${TMP_UNIT_DIR}/fusionpanel-agent.service"

echo "Verifying fusionpanel.service..."
systemd-analyze verify "${TMP_UNIT_DIR}/fusionpanel.service"

echo "Verifying fusionpanel-agent.service..."
systemd-analyze verify "${TMP_UNIT_DIR}/fusionpanel-agent.service"

echo "Systemd unit verification passed."
