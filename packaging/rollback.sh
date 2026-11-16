#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
. "$SCRIPT_DIR/layout.sh"

BACKUP_PARENT="/var/lib/fusionpanel-update"
BACKUP_DIR="$BACKUP_PARENT/backup-pre-update"
CONFIG_DIR="/etc/fusionpanel"
BIN_DIR="/usr/local/bin"
VERSION_LIB_DIR="/usr/local/lib/fusionpanel/versions"
STATE_DIR="/var/lib/fusionpanel"
SYSTEMD_DIR="/etc/systemd/system"
TEST_MODE=0
if fusionpanel_is_source_checkout "$SCRIPT_DIR"; then
    TEST_MODE=1
fi
if [ "$TEST_MODE" -eq 1 ]; then
    [ -z "${FUSIONPANEL_TEST_BACKUP_DIR:-}" ] || BACKUP_DIR="$FUSIONPANEL_TEST_BACKUP_DIR"
    [ -z "${FUSIONPANEL_TEST_CONFIG_DIR:-}" ] || CONFIG_DIR="$FUSIONPANEL_TEST_CONFIG_DIR"
    [ -z "${FUSIONPANEL_TEST_BIN_DIR:-}" ] || BIN_DIR="$FUSIONPANEL_TEST_BIN_DIR"
    [ -z "${FUSIONPANEL_TEST_VERSION_LIB_DIR:-}" ] || VERSION_LIB_DIR="$FUSIONPANEL_TEST_VERSION_LIB_DIR"
    [ -z "${FUSIONPANEL_TEST_STATE_DIR:-}" ] || STATE_DIR="$FUSIONPANEL_TEST_STATE_DIR"
    [ -z "${FUSIONPANEL_TEST_SYSTEMD_DIR:-}" ] || SYSTEMD_DIR="$FUSIONPANEL_TEST_SYSTEMD_DIR"
fi

echo "Starting rollback procedure..."
if [ "$BACKUP_DIR" = "$BACKUP_PARENT/backup-pre-update" ]; then
    if [ "$(id -u)" -ne 0 ]; then
        echo "Error: rollback must run as root." >&2
        exit 1
    fi
    if ! fusionpanel_ensure_private_directory "$BACKUP_PARENT" root root; then
        echo "Error: the root-owned update backup directory is unsafe." >&2
        exit 1
    fi
fi
if [ -L "$BACKUP_DIR" ] || [ ! -d "$BACKUP_DIR" ]; then
    echo "Error: Backup directory $BACKUP_DIR does not exist. Cannot rollback." >&2
    exit 1
fi
if [ "$BACKUP_DIR" = "$BACKUP_PARENT/backup-pre-update" ]; then
    backup_metadata=$(stat -c '%u %a' "$BACKUP_DIR" 2>/dev/null) || {
        echo "Error: Could not verify the update backup directory." >&2
        exit 1
    }
    if [ "$backup_metadata" != "0 700" ]; then
        echo "Error: update backup directory must be root-owned with mode 0700; refusing to restore it." >&2
        exit 1
    fi
fi

echo "Stopping services..."
for unit in fusionpanel-agent.service fusionpanel.service fusionpanel-nginx-helper.service; do
    unit_path="$SYSTEMD_DIR/$unit"
    if [ -f "$unit_path" ] && [ ! -L "$unit_path" ] \
        && ! fusionpanel_stop_unit_if_active "$unit"; then
        echo "Error: Could not stop $unit safely; rollback cannot continue." >&2
        exit 1
    fi
done

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
    for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper; do
        if [ -f "$BACKUP_DIR/bin/$binary" ] && {
            [ ! -f "$BACKUP_DIR/bin/present" ] || grep -Fxq "$binary" "$BACKUP_DIR/bin/present";
        }; then
            rm -f "$BIN_DIR/$binary"
            cp "$BACKUP_DIR/bin/$binary" "$BIN_DIR/$binary"
            chmod 0755 "$BIN_DIR/$binary"
        elif [ -f "$BACKUP_DIR/bin/installed-checksums" ] && [ -e "$BIN_DIR/$binary" ]; then
            expected=$(awk -v name="$binary" '$1 == name { print $2 }' "$BACKUP_DIR/bin/installed-checksums")
            actual=$(cksum "$BIN_DIR/$binary" | awk '{ print $1 ":" $2 }')
            if [ -n "$expected" ] && [ "$actual" = "$expected" ]; then
                rm -f "$BIN_DIR/$binary"
            fi
        fi
    done
fi

if [ -f "$BACKUP_DIR/systemd/present" ]; then
    mkdir -p "$SYSTEMD_DIR"
    for unit in fusionpanel.service fusionpanel-agent.service fusionpanel-nginx-helper.service; do
        target="$SYSTEMD_DIR/$unit"
        if grep -Fxq "$unit" "$BACKUP_DIR/systemd/present"; then
            if [ -L "$target" ] || { [ -e "$target" ] && [ ! -f "$target" ]; }; then
                echo "Error: Refusing to restore over a non-regular systemd unit: $target" >&2
                exit 1
            fi
            cp -a "$BACKUP_DIR/systemd/$unit" "$target"
        elif [ -f "$BACKUP_DIR/systemd/installed-checksums" ] && [ -f "$target" ] && [ ! -L "$target" ]; then
            expected=$(awk -v name="$unit" '$1 == name { print $2 }' "$BACKUP_DIR/systemd/installed-checksums")
            actual=$(cksum "$target" | awk '{ print $1 ":" $2 }')
            if [ -n "$expected" ] && [ "$actual" = "$expected" ]; then
                rm -f "$target"
            fi
        fi
    done
    if ! systemctl daemon-reload; then
        echo "Error: systemd daemon-reload failed while restoring units." >&2
        exit 1
    fi
fi

if [ -f "$BACKUP_DIR/previous-version" ]; then
    mkdir -p "$STATE_DIR"
    cp -a "$BACKUP_DIR/previous-version" "$STATE_DIR/.version"
fi

echo "Restarting services..."
for unit in fusionpanel-nginx-helper.service fusionpanel.service fusionpanel-agent.service; do
    unit_path="$SYSTEMD_DIR/$unit"
    if [ -f "$unit_path" ] && [ ! -L "$unit_path" ]; then
        if ! systemctl start "$unit"; then
            echo "Error: rollback restored files but could not start $unit; manual recovery is required." >&2
            exit 1
        fi
        if ! systemctl is-active --quiet "$unit"; then
            echo "Error: rollback restored files but $unit is not active; manual recovery is required." >&2
            exit 1
        fi
    fi
done

echo "Rollback complete."
