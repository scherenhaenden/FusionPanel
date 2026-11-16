#!/bin/sh
set -eu

# Non-interactive atomic updater script for FusionPanel.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
. "$SCRIPT_DIR/layout.sh"

VERSION="${1:-latest}"
BACKUP_DIR="/var/lib/fusionpanel/backup-pre-update"
CONFIG_DIR="/etc/fusionpanel"
BIN_DIR="/usr/local/bin"
VERSION_LIB_DIR="/usr/local/lib/fusionpanel/versions"
STATE_DIR="/var/lib/fusionpanel"
HEALTH_URL="http://127.0.0.1:8080/v1/health"
HEALTH_TIMEOUT=30

# Allow isolated layout tests without touching host paths or services.
[ -z "${FUSIONPANEL_TEST_BACKUP_DIR:-}" ] || BACKUP_DIR="$FUSIONPANEL_TEST_BACKUP_DIR"
[ -z "${FUSIONPANEL_TEST_CONFIG_DIR:-}" ] || CONFIG_DIR="$FUSIONPANEL_TEST_CONFIG_DIR"
[ -z "${FUSIONPANEL_TEST_BIN_DIR:-}" ] || BIN_DIR="$FUSIONPANEL_TEST_BIN_DIR"
[ -z "${FUSIONPANEL_TEST_VERSION_LIB_DIR:-}" ] || VERSION_LIB_DIR="$FUSIONPANEL_TEST_VERSION_LIB_DIR"
[ -z "${FUSIONPANEL_TEST_STATE_DIR:-}" ] || STATE_DIR="$FUSIONPANEL_TEST_STATE_DIR"
[ -z "${FUSIONPANEL_HEALTH_URL:-}" ] || HEALTH_URL="$FUSIONPANEL_HEALTH_URL"

detect_os() {
    if [ -n "${FUSIONPANEL_TEST_OS:-}" ]; then
        OS="$FUSIONPANEL_TEST_OS"
        return
    fi
    if [ -f /etc/os-release ]; then
        OS_ID=$(grep -E '^ID=' /etc/os-release | cut -d= -f2 | tr -d '"')
        case "$OS_ID" in
            debian|ubuntu) OS="debian" ;;
            rhel|almalinux|centos|rocky) OS="rhel" ;;
            *) echo "Unsupported OS: $OS_ID" >&2; exit 1 ;;
        esac
    else
        echo "Could not detect OS. /etc/os-release not found." >&2
        exit 1
    fi
}

detect_arch() {
    if [ -n "${FUSIONPANEL_TEST_ARCH:-}" ]; then
        ARCH="$FUSIONPANEL_TEST_ARCH"
        return
    fi
    ARCH=$(uname -m)
    case "$ARCH" in
        x86_64|aarch64) ;;
        *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
    esac
}

record_previous_release() {
    previous_target=""
    if [ -L "$VERSION_LIB_DIR/current" ]; then
        previous_target=$(readlink "$VERSION_LIB_DIR/current")
    elif [ -L "$BIN_DIR/fusionpanel-api" ]; then
        previous_api=$(readlink "$BIN_DIR/fusionpanel-api")
        case "$previous_api" in
            "$VERSION_LIB_DIR"/*/bin/fusionpanel-api)
                previous_target=${previous_api#"$VERSION_LIB_DIR"/}
                previous_target=${previous_target%/bin/fusionpanel-api}
                ;;
        esac
    fi

    case "$previous_target" in
        "" ) ;;
        */*|.|..) echo "Error: Unrecognized current version target: $previous_target" >&2; return 1 ;;
        * )
            printf '%s\n' "$previous_target" > "$BACKUP_DIR/previous-current"
            if [ -d "$VERSION_LIB_DIR/$previous_target" ]; then
                cp -a "$VERSION_LIB_DIR/$previous_target" "$BACKUP_DIR/previous-release"
            fi
            ;;
    esac
}

echo "Updating FusionPanel to version $VERSION..."
case "$VERSION" in *[!a-zA-Z0-9.+_-]*|'') echo "Invalid version" >&2; exit 1;; esac
detect_os
detect_arch

if [ -L "$CONFIG_DIR" ] || { [ -e "$CONFIG_DIR" ] && [ ! -d "$CONFIG_DIR" ]; }; then
    echo "Error: FusionPanel config path is not a real directory: $CONFIG_DIR" >&2
    exit 1
fi
for env_file in "$CONFIG_DIR/fusionpanel.env" "$CONFIG_DIR/fusionpanel-agent.env"; do
    if [ -L "$env_file" ] || { [ -e "$env_file" ] && [ ! -f "$env_file" ]; }; then
        echo "Error: Refusing to update with a non-regular environment file: $env_file" >&2
        exit 1
    fi
done

# Preserve the active state before any service or symlink is changed.
rm -rf "$BACKUP_DIR"
mkdir -p "$BACKUP_DIR/bin" "$BACKUP_DIR/config"
if [ -e "$BIN_DIR/fusionpanel-api" ]; then cp -L "$BIN_DIR/fusionpanel-api" "$BACKUP_DIR/bin/fusionpanel-api"; fi
if [ -e "$BIN_DIR/fusionpanel-agent" ]; then cp -L "$BIN_DIR/fusionpanel-agent" "$BACKUP_DIR/bin/fusionpanel-agent"; fi
if [ -e "$BIN_DIR/fusionpanel-cli" ]; then cp -L "$BIN_DIR/fusionpanel-cli" "$BACKUP_DIR/bin/fusionpanel-cli"; fi
if [ -d "$CONFIG_DIR" ] && [ ! -L "$CONFIG_DIR" ]; then cp -a "$CONFIG_DIR/." "$BACKUP_DIR/config/"; fi
if [ -f "$STATE_DIR/.version" ] && [ ! -L "$STATE_DIR/.version" ]; then
    cp -a "$STATE_DIR/.version" "$BACKUP_DIR/previous-version"
fi
mkdir -p "$VERSION_LIB_DIR"
record_previous_release

TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT HUP INT TERM

LEGACY_DOWNLOAD_URL=""
if [ "$VERSION" = "latest" ]; then
    DOWNLOAD_URL="https://github.com/scherenhaenden/FusionPanel/releases/latest/download/fusionpanel-${OS}-${ARCH}.tar.gz"
else
    URL_VERSION=$(printf '%s' "$VERSION" | sed 's/+/%2B/g')
    DOWNLOAD_URL="https://github.com/scherenhaenden/FusionPanel/releases/download/${URL_VERSION}/fusionpanel-${OS}-${ARCH}.tar.gz"
    LEGACY_DOWNLOAD_URL="https://github.com/scherenhaenden/FusionPanel/releases/download/${URL_VERSION}/fusionpanel-${URL_VERSION}-${OS}-${ARCH}.tar.gz"
fi

if [ -n "${FUSIONPANEL_MOCK_DOWNLOAD_URL:-}" ]; then
    DOWNLOAD_URL="$FUSIONPANEL_MOCK_DOWNLOAD_URL"
    LEGACY_DOWNLOAD_URL=""
fi

echo "Downloading $DOWNLOAD_URL..."
if ! curl -sSLf "$DOWNLOAD_URL" -o "$TMP_DIR/fusionpanel.tar.gz"; then
    if [ -z "$LEGACY_DOWNLOAD_URL" ] || ! curl -sSLf "$LEGACY_DOWNLOAD_URL" -o "$TMP_DIR/fusionpanel.tar.gz"; then
        echo "Error downloading update." >&2
        exit 1
    fi
fi

mkdir -p "$TMP_DIR/payload"
if ! tar -xzf "$TMP_DIR/fusionpanel.tar.gz" -C "$TMP_DIR/payload"; then
    echo "Error extracting archive." >&2
    exit 1
fi
if ! fusionpanel_validate_payload "$TMP_DIR/payload"; then
    exit 1
fi

# Prepare the exact same version-directory shape as install.sh before stopping services.
mkdir -p "$TMP_DIR/release/bin" "$TMP_DIR/release/config" "$TMP_DIR/release/data"
cp -a "$TMP_DIR/payload/." "$TMP_DIR/release/bin/"
if [ -d "$CONFIG_DIR" ] && [ ! -L "$CONFIG_DIR" ]; then
    cp -a "$CONFIG_DIR/." "$TMP_DIR/release/config/"
fi
fusionpanel_set_frontend_env \
    "$TMP_DIR/release/config/fusionpanel.env" \
    "$VERSION_LIB_DIR/current/bin/frontend/dist"
printf '%s\n' "$VERSION" > "$TMP_DIR/release/.version"
chmod 0700 "$TMP_DIR/release/config"
chown root:root "$TMP_DIR/release/config"
for env_file in "$TMP_DIR/release/config/fusionpanel.env" "$TMP_DIR/release/config/fusionpanel-agent.env"; do
    if [ -f "$env_file" ]; then
        chown root:root "$env_file"
        chmod 0600 "$env_file"
    fi
done

echo "Stopping services..."
systemctl stop fusionpanel.service fusionpanel-agent.service || true

install_dir="$VERSION_LIB_DIR/$VERSION"
fusionpanel_publish_version_dir "$TMP_DIR/release" "$install_dir"
fusionpanel_switch_current "$VERSION_LIB_DIR" "$VERSION"
fusionpanel_link_runtime_binaries "$BIN_DIR" "$VERSION_LIB_DIR"

mkdir -p "$CONFIG_DIR" "$STATE_DIR"
cp -a "$install_dir/config/." "$CONFIG_DIR/"
cp "$install_dir/.version" "$STATE_DIR/.version"
chmod 0700 "$CONFIG_DIR"
chown root:root "$CONFIG_DIR"
for env_file in "$CONFIG_DIR/fusionpanel.env" "$CONFIG_DIR/fusionpanel-agent.env"; do
    if [ -f "$env_file" ]; then
        chown root:root "$env_file"
        chmod 0600 "$env_file"
    fi
done

echo "Restarting services..."
systemctl start fusionpanel.service fusionpanel-agent.service || true

echo "Performing health check..."
START_TIME=$(date +%s)
HEALTH_PASS=0
while true; do
    if curl -sSLf "$HEALTH_URL" > /dev/null 2>&1; then
        HEALTH_PASS=1
        break
    fi
    CURRENT_TIME=$(date +%s)
    ELAPSED=$((CURRENT_TIME - START_TIME))
    if [ "$ELAPSED" -ge "$HEALTH_TIMEOUT" ]; then break; fi
    sleep 1
done

if [ "$HEALTH_PASS" -eq 1 ]; then
    echo "Update successful."
else
    echo "Health check failed after $HEALTH_TIMEOUT seconds. Rolling back." >&2
    "$SCRIPT_DIR/rollback.sh"
    exit 1
fi
