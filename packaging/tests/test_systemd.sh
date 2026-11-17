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
touch "${TMP_UNIT_DIR}/bin/fusionpanel-nginx-helper"
chmod +x "${TMP_UNIT_DIR}/bin/fusionpanel-api" "${TMP_UNIT_DIR}/bin/fusionpanel-agent" "${TMP_UNIT_DIR}/bin/fusionpanel-nginx-helper"

sed "s|/usr/local/bin/fusionpanel-api|${TMP_UNIT_DIR}/bin/fusionpanel-api|g" "${SYSTEMD_DIR}/fusionpanel.service" > "${TMP_UNIT_DIR}/fusionpanel.service"
sed "s|/usr/local/bin/fusionpanel-agent|${TMP_UNIT_DIR}/bin/fusionpanel-agent|g" "${SYSTEMD_DIR}/fusionpanel-agent.service" > "${TMP_UNIT_DIR}/fusionpanel-agent.service"
sed "s|/usr/local/bin/fusionpanel-nginx-helper|${TMP_UNIT_DIR}/bin/fusionpanel-nginx-helper|g" \
    "${SYSTEMD_DIR}/fusionpanel-nginx-helper.service" > "${TMP_UNIT_DIR}/fusionpanel-nginx-helper.service"

# Update env file paths
sed -i "s|/etc/fusionpanel/fusionpanel.env|${TMP_UNIT_DIR}/fusionpanel.env|g" "${TMP_UNIT_DIR}/fusionpanel.service"
sed -i "s|/etc/fusionpanel/fusionpanel-agent.env|${TMP_UNIT_DIR}/fusionpanel-agent.env|g" "${TMP_UNIT_DIR}/fusionpanel-agent.service"
touch "${TMP_UNIT_DIR}/fusionpanel.env" "${TMP_UNIT_DIR}/fusionpanel-agent.env"

if ! grep -q '^After=network.target fusionpanel.service fusionpanel-nginx-helper.service$' "${SYSTEMD_DIR}/fusionpanel-agent.service"; then
    echo "FAIL: agent must start after the API and helper services."
    exit 1
fi
if ! grep -q '^Wants=fusionpanel.service fusionpanel-nginx-helper.service$' "${SYSTEMD_DIR}/fusionpanel-agent.service"; then
    echo "FAIL: agent should request startup of the API and helper without coupling restart lifecycles."
    exit 1
fi
if grep -q '^Requires=fusionpanel.service$' "${SYSTEMD_DIR}/fusionpanel-agent.service"; then
    echo "FAIL: agent must remain independently restartable when the API restarts."
    exit 1
fi
if ! grep -q '^SupplementaryGroups=fusionpanel-nginx$' "${SYSTEMD_DIR}/fusionpanel-agent.service"; then
    echo "FAIL: only the agent may receive the private Nginx helper group."
    exit 1
fi
if grep -q '^SupplementaryGroups=fusionpanel-nginx$' "${SYSTEMD_DIR}/fusionpanel.service"; then
    echo "FAIL: API must not receive the private Nginx helper group."
    exit 1
fi
if ! grep -q '^Group=fusionpanel-nginx$' "${SYSTEMD_DIR}/fusionpanel-nginx-helper.service" \
    || ! grep -q '^User=root$' "${SYSTEMD_DIR}/fusionpanel-nginx-helper.service"; then
    echo "FAIL: privileged helper must run as root in its exclusive group."
    exit 1
fi
if grep -q '^EnvironmentFile=' "${SYSTEMD_DIR}/fusionpanel-nginx-helper.service"; then
    echo "FAIL: helper must not load the agent environment."
    exit 1
fi
for hardening in 'NoNewPrivileges=yes' 'ProtectSystem=strict' 'RuntimeDirectory=fusionpanel-nginx' 'RuntimeDirectoryMode=0750' 'UMask=0007' 'ReadWritePaths=-/etc/nginx/fusionpanel'; do
    grep -Fxq "$hardening" "${SYSTEMD_DIR}/fusionpanel-nginx-helper.service" || {
        echo "FAIL: helper hardening directive is missing: $hardening" >&2
        exit 1
    }
done
grep -Fxq 'CapabilityBoundingSet=CAP_DAC_READ_SEARCH CAP_KILL' \
    "${SYSTEMD_DIR}/fusionpanel-nginx-helper.service" || {
        echo "FAIL: helper capabilities exceed the reviewed read/reload boundary." >&2
        exit 1
    }
if grep '^ReadWritePaths=' "${SYSTEMD_DIR}/fusionpanel-nginx-helper.service" \
    | grep -vFx 'ReadWritePaths=-/etc/nginx/fusionpanel' \
    | grep -qE '/etc/nginx|/srv/fusionpanel/sites'; then
    echo "FAIL: helper write access is broader than the managed Nginx directory."
    exit 1
fi

# Note: systemd-analyze will complain about missing users if fusionpanel user doesn't exist on the host
# We will temporarily mock the user line for the analyzer if it doesn't exist
if ! id fusionpanel &>/dev/null; then
    sed -i 's/User=fusionpanel/User=root/' "${TMP_UNIT_DIR}/fusionpanel.service" "${TMP_UNIT_DIR}/fusionpanel-agent.service"
    sed -i 's/Group=fusionpanel/Group=root/' "${TMP_UNIT_DIR}/fusionpanel.service" "${TMP_UNIT_DIR}/fusionpanel-agent.service"
fi
if ! getent group fusionpanel-nginx >/dev/null 2>&1; then
    sed -i 's/Group=fusionpanel-nginx/Group=root/' "${TMP_UNIT_DIR}/fusionpanel-nginx-helper.service"
    sed -i '/^SupplementaryGroups=fusionpanel-nginx$/d' "${TMP_UNIT_DIR}/fusionpanel-agent.service"
fi

echo "Verifying fusionpanel.service..."
systemd-analyze verify \
    "${TMP_UNIT_DIR}/fusionpanel.service" \
    "${TMP_UNIT_DIR}/fusionpanel-agent.service" \
    "${TMP_UNIT_DIR}/fusionpanel-nginx-helper.service"

echo "Systemd unit verification passed."
