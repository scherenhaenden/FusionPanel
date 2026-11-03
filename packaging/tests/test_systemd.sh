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

mkdir -p /tmp/fusionpanel-test-bin
touch /tmp/fusionpanel-test-bin/fusionpanel-api
touch /tmp/fusionpanel-test-bin/fusionpanel-agent
chmod +x /tmp/fusionpanel-test-bin/fusionpanel-api
chmod +x /tmp/fusionpanel-test-bin/fusionpanel-agent

# Create temporary modified unit files pointing to our fake binaries for testing
TMP_UNIT_DIR=$(mktemp -d)
trap 'rm -rf "${TMP_UNIT_DIR}"' EXIT

sed 's|/opt/fusionpanel/bin/fusionpanel-api|/tmp/fusionpanel-test-bin/fusionpanel-api|g' "${SYSTEMD_DIR}/fusionpanel.service" > "${TMP_UNIT_DIR}/fusionpanel.service"
sed 's|/opt/fusionpanel/bin/fusionpanel-agent|/tmp/fusionpanel-test-bin/fusionpanel-agent|g' "${SYSTEMD_DIR}/fusionpanel-agent.service" > "${TMP_UNIT_DIR}/fusionpanel-agent.service"

# Update env file paths
sed -i 's|/etc/fusionpanel/fusionpanel.env|/tmp/fusionpanel.env|g' "${TMP_UNIT_DIR}/fusionpanel.service"
sed -i 's|/etc/fusionpanel/fusionpanel-agent.env|/tmp/fusionpanel-agent.env|g' "${TMP_UNIT_DIR}/fusionpanel-agent.service"
touch /tmp/fusionpanel.env
touch /tmp/fusionpanel-agent.env

# Note: systemd-analyze will complain about missing users if fusionpanel user doesn't exist on the host
# We will temporarily mock the user line for the analyzer if it doesn't exist
if ! id fusionpanel &>/dev/null; then
    sed -i 's/User=fusionpanel/User=root/' "${TMP_UNIT_DIR}/fusionpanel.service" "${TMP_UNIT_DIR}/fusionpanel-agent.service"
    sed -i 's/Group=fusionpanel/Group=root/' "${TMP_UNIT_DIR}/fusionpanel.service" "${TMP_UNIT_DIR}/fusionpanel-agent.service"
fi

echo "Verifying fusionpanel.service..."
systemd-analyze verify "${TMP_UNIT_DIR}/fusionpanel.service"

echo "Verifying fusionpanel-agent.service..."
systemd-analyze verify "${TMP_UNIT_DIR}/fusionpanel-agent.service"

echo "Systemd unit verification passed."