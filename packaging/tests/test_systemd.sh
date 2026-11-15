#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
SYSTEMD_DIR="${SCRIPT_DIR}/../systemd"

echo "Testing systemd unit files..."

# Ensure systemd-analyze is installed
if ! command -v systemd-analyze &> /dev/null; then
    echo "BLOCKED: systemd-analyze is not installed." >&2
    exit 2
fi

# Create temporary modified unit files pointing to our fake binaries for testing
TMP_UNIT_DIR=$(mktemp -d)
trap 'rm -rf "${TMP_UNIT_DIR}"' EXIT
mkdir -p "${TMP_UNIT_DIR}/bin"
touch "${TMP_UNIT_DIR}/bin/fusionpanel-api"
touch "${TMP_UNIT_DIR}/bin/fusionpanel-agent"
chmod +x "${TMP_UNIT_DIR}/bin/fusionpanel-api" "${TMP_UNIT_DIR}/bin/fusionpanel-agent"

sed "s|/usr/local/bin/fusionpanel-api|${TMP_UNIT_DIR}/bin/fusionpanel-api|g" "${SYSTEMD_DIR}/fusionpanel.service" > "${TMP_UNIT_DIR}/fusionpanel.service"
sed "s|/usr/local/bin/fusionpanel-agent|${TMP_UNIT_DIR}/bin/fusionpanel-agent|g" "${SYSTEMD_DIR}/fusionpanel-agent.service" > "${TMP_UNIT_DIR}/fusionpanel-agent.service"

# Update env file paths
sed -i "s|/etc/fusionpanel/fusionpanel.env|${TMP_UNIT_DIR}/fusionpanel.env|g" "${TMP_UNIT_DIR}/fusionpanel.service"
sed -i "s|/etc/fusionpanel/fusionpanel-agent.env|${TMP_UNIT_DIR}/fusionpanel-agent.env|g" "${TMP_UNIT_DIR}/fusionpanel-agent.service"
touch "${TMP_UNIT_DIR}/fusionpanel.env" "${TMP_UNIT_DIR}/fusionpanel-agent.env"

if ! grep -q '^After=network.target fusionpanel.service$' "${SYSTEMD_DIR}/fusionpanel-agent.service"; then
    echo "FAIL: agent must start after the API service."
    exit 1
fi
if ! grep -q '^Requires=fusionpanel.service$' "${SYSTEMD_DIR}/fusionpanel-agent.service"; then
    echo "FAIL: agent must require the API service."
    exit 1
fi

# Note: systemd-analyze will complain about missing users if fusionpanel user doesn't exist on the host
# We will temporarily mock the user line for the analyzer if it doesn't exist
if ! id fusionpanel &>/dev/null; then
    sed -i 's/User=fusionpanel/User=root/' "${TMP_UNIT_DIR}/fusionpanel.service" "${TMP_UNIT_DIR}/fusionpanel-agent.service"
    sed -i 's/Group=fusionpanel/Group=root/' "${TMP_UNIT_DIR}/fusionpanel.service" "${TMP_UNIT_DIR}/fusionpanel-agent.service"
fi

echo "Verifying fusionpanel.service..."
systemd-analyze verify "${TMP_UNIT_DIR}/fusionpanel.service" "${TMP_UNIT_DIR}/fusionpanel-agent.service"

echo "Systemd unit verification passed."
