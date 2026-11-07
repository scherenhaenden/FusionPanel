#!/bin/sh
set -e

# Non-interactive atomic updater script for FusionPanel

VERSION="${1:-latest}"
BACKUP_DIR="/var/lib/fusionpanel/backup-pre-update"
CONFIG_DIR="/etc/fusionpanel"
BIN_DIR="/usr/local/bin"
VERSION_LIB_DIR="/usr/local/lib/fusionpanel/versions"
HEALTH_URL="http://127.0.0.1:8080/v1/health"
HEALTH_TIMEOUT=30

# Allow overriding for tests
if [ -n "$FUSIONPANEL_TEST_BACKUP_DIR" ]; then BACKUP_DIR="$FUSIONPANEL_TEST_BACKUP_DIR"; fi
if [ -n "$FUSIONPANEL_TEST_CONFIG_DIR" ]; then CONFIG_DIR="$FUSIONPANEL_TEST_CONFIG_DIR"; fi
if [ -n "$FUSIONPANEL_TEST_BIN_DIR" ]; then BIN_DIR="$FUSIONPANEL_TEST_BIN_DIR"; fi
if [ -n "$FUSIONPANEL_TEST_VERSION_LIB_DIR" ]; then VERSION_LIB_DIR="$FUSIONPANEL_TEST_VERSION_LIB_DIR"; fi
if [ -n "$FUSIONPANEL_HEALTH_URL" ]; then HEALTH_URL="$FUSIONPANEL_HEALTH_URL"; fi

OS=""
ARCH=""
detect_os() {
    if [ -n "$FUSIONPANEL_TEST_OS" ]; then
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
    if [ -n "$FUSIONPANEL_TEST_ARCH" ]; then
        ARCH="$FUSIONPANEL_TEST_ARCH"
        return
    fi
    ARCH=$(uname -m)
    case "$ARCH" in
        x86_64|aarch64) ;;
        *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
    esac
}

echo "Updating FusionPanel to version $VERSION..."
detect_os
detect_arch

# Create backup of current state
echo "Creating backup in $BACKUP_DIR..."
rm -rf "$BACKUP_DIR"
mkdir -p "$BACKUP_DIR/bin" "$BACKUP_DIR/config"

if [ -e "$BIN_DIR/fusionpanel-api" ]; then
    cp -L "$BIN_DIR/fusionpanel-api" "$BACKUP_DIR/bin/fusionpanel-api"
fi
if [ -e "$BIN_DIR/fusionpanel-agent" ]; then
    cp -L "$BIN_DIR/fusionpanel-agent" "$BACKUP_DIR/bin/fusionpanel-agent"
fi
if [ -d "$CONFIG_DIR" ]; then
    cp -a "$CONFIG_DIR/." "$BACKUP_DIR/config/" 2>/dev/null || true
fi

# Download new version
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

if [ "$VERSION" = "latest" ]; then
    DOWNLOAD_URL="https://github.com/scherenhaenden/FusionPanel/releases/latest/download/fusionpanel-${OS}-${ARCH}.tar.gz"
else
    DOWNLOAD_URL="https://github.com/scherenhaenden/FusionPanel/releases/download/${VERSION}/fusionpanel-${VERSION}-${OS}-${ARCH}.tar.gz"
fi

if [ -n "$FUSIONPANEL_MOCK_DOWNLOAD_URL" ]; then
    DOWNLOAD_URL="$FUSIONPANEL_MOCK_DOWNLOAD_URL"
fi

echo "Downloading $DOWNLOAD_URL..."
if ! curl -sSLf "$DOWNLOAD_URL" -o "$TMP_DIR/fusionpanel.tar.gz"; then
    echo "Error downloading update." >&2
    exit 1
fi

mkdir -p "$TMP_DIR/bin"
if ! tar -xzf "$TMP_DIR/fusionpanel.tar.gz" -C "$TMP_DIR/bin/"; then
    echo "Error extracting archive." >&2
    exit 1
fi

if [ ! -f "$TMP_DIR/bin/fusionpanel-api" ] || [ ! -f "$TMP_DIR/bin/fusionpanel-agent" ]; then
    echo "Error: Required binaries not found in archive." >&2
    exit 1
fi

# We use atomic installation logic
install_dir="${VERSION_LIB_DIR}/${VERSION}"
mkdir -p "${VERSION_LIB_DIR}"
if [ -d "$install_dir" ]; then
    rm -rf "$install_dir"
fi

cp -a "$TMP_DIR/bin" "${install_dir}.tmp"
mv "${install_dir}.tmp" "$install_dir"
chmod 755 "$install_dir/fusionpanel-api" "$install_dir/fusionpanel-agent"

# Stop services
echo "Stopping services..."
systemctl stop fusionpanel.service fusionpanel-agent.service || true

# Replace binaries
echo "Replacing binaries..."
mkdir -p "$BIN_DIR"
ln -sfn "$install_dir/fusionpanel-api" "$BIN_DIR/fusionpanel-api"
ln -sfn "$install_dir/fusionpanel-agent" "$BIN_DIR/fusionpanel-agent"

# Restart services
echo "Restarting services..."
systemctl start fusionpanel.service fusionpanel-agent.service || true

# Health check
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
    if [ "$ELAPSED" -ge "$HEALTH_TIMEOUT" ]; then
        break
    fi
    sleep 1
done

if [ "$HEALTH_PASS" -eq 1 ]; then
    echo "Update successful."
else
    echo "Health check failed after $HEALTH_TIMEOUT seconds. Rolling back." >&2
    "$(dirname "$0")/rollback.sh"
    exit 1
fi
