#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
. "$SCRIPT_DIR/layout.sh"

BACKUP_DIR="/var/lib/fusionpanel/backup-pre-update"
CONFIG_DIR="/etc/fusionpanel"
BIN_DIR="/usr/local/bin"
VERSION_LIB_DIR="/usr/local/lib/fusionpanel/versions"
STATE_DIR="/var/lib/fusionpanel"
[ -z "${FUSIONPANEL_TEST_BACKUP_DIR:-}" ] || BACKUP_DIR="$FUSIONPANEL_TEST_BACKUP_DIR"
[ -z "${FUSIONPANEL_TEST_CONFIG_DIR:-}" ] || CONFIG_DIR="$FUSIONPANEL_TEST_CONFIG_DIR"
[ -z "${FUSIONPANEL_TEST_BIN_DIR:-}" ] || BIN_DIR="$FUSIONPANEL_TEST_BIN_DIR"
[ -z "${FUSIONPANEL_TEST_VERSION_LIB_DIR:-}" ] || VERSION_LIB_DIR="$FUSIONPANEL_TEST_VERSION_LIB_DIR"
[ -z "${FUSIONPANEL_TEST_STATE_DIR:-}" ] || STATE_DIR="$FUSIONPANEL_TEST_STATE_DIR"

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
    cp -a "$BACKUP_DIR/config/." "$CONFIG_DIR/"
fi

if [ -f "$BACKUP_DIR/previous-current" ]; then
    previous_target=$(cat "$BACKUP_DIR/previous-current")
    case "$previous_target" in
        ""|*/*|.|..) echo "Error: Invalid backed up version target." >&2; exit 1 ;;
    esac
    if [ -d "$BACKUP_DIR/previous-release" ]; then
        fusionpanel_publish_version_dir "$BACKUP_DIR/previous-release" "$VERSION_LIB_DIR/$previous_target"
    fi
    if [ ! -d "$VERSION_LIB_DIR/$previous_target" ]; then
        echo "Error: Previous release is missing: $VERSION_LIB_DIR/$previous_target" >&2
        exit 1
    fi
    fusionpanel_switch_current "$VERSION_LIB_DIR" "$previous_target"
    fusionpanel_link_runtime_binaries "$BIN_DIR" "$VERSION_LIB_DIR"
elif [ -d "$BACKUP_DIR/bin" ]; then
    # Pre-current installs used regular files or version-specific links in /usr/local/bin.
    mkdir -p "$BIN_DIR"
    for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli; do
        if [ -f "$BACKUP_DIR/bin/$binary" ]; then
            rm -f "$BIN_DIR/$binary"
            cp "$BACKUP_DIR/bin/$binary" "$BIN_DIR/$binary"
            chmod 0755 "$BIN_DIR/$binary"
        fi
    done
fi

if [ -f "$BACKUP_DIR/previous-version" ]; then
    mkdir -p "$STATE_DIR"
    cp -a "$BACKUP_DIR/previous-version" "$STATE_DIR/.version"
fi

echo "Restarting services..."
systemctl start fusionpanel.service fusionpanel-agent.service || true

echo "Rollback complete."
