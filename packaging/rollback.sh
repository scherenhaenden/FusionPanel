#!/bin/sh
set -e

# Rollback script for FusionPanel updater

BACKUP_DIR="/var/lib/fusionpanel/backup-pre-update"
CONFIG_DIR="/etc/fusionpanel"
BIN_DIR="/usr/local/bin"

# Allow overriding for tests
if [ -n "$FUSIONPANEL_TEST_BACKUP_DIR" ]; then BACKUP_DIR="$FUSIONPANEL_TEST_BACKUP_DIR"; fi
if [ -n "$FUSIONPANEL_TEST_CONFIG_DIR" ]; then CONFIG_DIR="$FUSIONPANEL_TEST_CONFIG_DIR"; fi
if [ -n "$FUSIONPANEL_TEST_BIN_DIR" ]; then BIN_DIR="$FUSIONPANEL_TEST_BIN_DIR"; fi

echo "Starting rollback procedure..."

if [ ! -d "$BACKUP_DIR" ]; then
    echo "Error: Backup directory $BACKUP_DIR does not exist. Cannot rollback." >&2
    exit 1
fi

echo "Stopping services..."
systemctl stop fusionpanel.service fusionpanel-agent.service || true

echo "Restoring config..."
if [ -d "$BACKUP_DIR/config" ]; then
    mkdir -p "$CONFIG_DIR"
    cp -a "$BACKUP_DIR/config/." "$CONFIG_DIR/" 2>/dev/null || true
fi

echo "Restoring binaries..."
if [ -d "$BACKUP_DIR/bin" ]; then
    mkdir -p "$BIN_DIR"
    if [ -e "$BACKUP_DIR/bin/fusionpanel-api" ]; then
        cp "$BACKUP_DIR/bin/fusionpanel-api" "$BIN_DIR/fusionpanel-api"
        chmod +x "$BIN_DIR/fusionpanel-api"
    fi
    if [ -e "$BACKUP_DIR/bin/fusionpanel-agent" ]; then
        cp "$BACKUP_DIR/bin/fusionpanel-agent" "$BIN_DIR/fusionpanel-agent"
        chmod +x "$BIN_DIR/fusionpanel-agent"
    fi
fi

echo "Restarting services..."
systemctl start fusionpanel.service fusionpanel-agent.service || true

echo "Rollback complete."
