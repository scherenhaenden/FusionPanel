#!/bin/bash
set -euo pipefail

echo "Setting up mock environment..."
MOCK_DIR=$(mktemp -d)
touch "$MOCK_DIR/fusionpanel-api"
touch "$MOCK_DIR/fusionpanel-agent"
chmod +x "$MOCK_DIR/fusionpanel-api" "$MOCK_DIR/fusionpanel-agent"

tar -czf "$MOCK_DIR/mock.tar.gz" -C "$MOCK_DIR" fusionpanel-api fusionpanel-agent
export FUSIONPANEL_MOCK_DOWNLOAD_URL="file://$MOCK_DIR/mock.tar.gz"
export FUSIONPANEL_INSTALLER_NO_SYSTEMD_RELOAD=1
unset FUSIONPANEL_SYSCONFDIR
unset FUSIONPANEL_SYSTEMD_DIR

echo "Running install.sh (First pass)..."
./packaging/install.sh

echo "Verifying installation directories..."
if [ ! -d /etc/fusionpanel ]; then
    echo "FAILED: /etc/fusionpanel directory not created"
    exit 1
fi
if [ ! -d /var/lib/fusionpanel ]; then
    echo "FAILED: /var/lib/fusionpanel directory not created"
    exit 1
fi
if [ ! -d /var/log/fusionpanel ]; then
    echo "FAILED: /var/log/fusionpanel directory not created"
    exit 1
fi
if [ ! -f /usr/local/bin/fusionpanel-api ]; then
    echo "FAILED: /usr/local/bin/fusionpanel-api not found"
    exit 1
fi

echo "Verifying initial credentials file..."
CRED_FILE="/etc/fusionpanel/initial-credentials.txt"
if [ ! -f "$CRED_FILE" ]; then
    echo "FAILED: Credentials file $CRED_FILE not found"
    exit 1
fi

PERMS=$(stat -c "%a" "$CRED_FILE")
if [ "$PERMS" != "600" ]; then
    echo "FAILED: Credentials file permissions are $PERMS, expected 600"
    exit 1
fi

echo "Running install.sh (Second pass for idempotency)..."
OUTPUT=$(./packaging/install.sh)
if ! echo "$OUTPUT" | grep -q "Idempotency guard triggered"; then
    echo "FAILED: Idempotency guard not triggered on second run."
    echo "Installer output: $OUTPUT"
    exit 1
fi

echo "Verifying systemd syntax..."
# Quick check of systemd directives
for svc in packaging/systemd/fusionpanel.service packaging/systemd/fusionpanel-agent.service; do
    if ! grep -q "ReadWritePaths=/var/lib/fusionpanel/ /var/log/fusionpanel/ /etc/fusionpanel/" "$svc"; then
        echo "FAILED: ReadWritePaths missing from $svc"
        exit 1
    fi
    if ! grep -q "ProtectSystem=strict" "$svc"; then
        echo "FAILED: ProtectSystem=strict missing from $svc"
        exit 1
    fi
    if ! grep -q "ProtectHome=read-only" "$svc"; then
        echo "FAILED: ProtectHome=read-only missing from $svc"
        exit 1
    fi
    if ! grep -q "Restart=on-failure" "$svc"; then
        echo "FAILED: Restart=on-failure missing from $svc"
        exit 1
    fi
done

echo "SUCCESS: Installer qualification test passed!"
rm -rf "$MOCK_DIR"
