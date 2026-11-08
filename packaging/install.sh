#!/bin/sh
set -e

# Default values
PORT="8080"
VERSION="latest"
OS=""
ARCH=""
DRY_RUN=0
export DEBIAN_FRONTEND=noninteractive

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --port <port>     Port for FusionPanel (default: 8080)"
    echo "  --version <ver>   Version to install (default: latest)"
    echo "  --dry-run         Perform a trial run with no changes made"
    echo "  --help            Show this message"
}

# Parse arguments
while [ $# -gt 0 ]; do
    case "$1" in
        --port)
            PORT="$2"
            shift 2
            ;;
        --version)
            VERSION="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        --help)
            usage
            exit 0
            ;;
        --prefix)
            # Ignore prefix argument for compatibility
            shift 2
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

detect_os() {
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
    ARCH=$(uname -m)
    case "$ARCH" in
        x86_64|aarch64) ;;
        *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
    esac
}

check_dependencies() {
    if [ "$OS" = "debian" ] && [ "$DRY_RUN" -eq 0 ]; then
        apt-get update -qq || true
        apt-get install -yqq curl tar openssl || true
    fi

    for cmd in curl systemctl tar useradd groupadd id mktemp openssl; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            echo "Missing required dependency: $cmd" >&2
            exit 1
        fi
    done
}

setup_users() {
    if ! id "fusionpanel" >/dev/null 2>&1; then
        echo "Creating fusionpanel system group..."
        if [ "$DRY_RUN" -eq 0 ]; then
            groupadd --system fusionpanel
        else
            echo "[Dry-run] Would run: groupadd --system fusionpanel"
        fi
        echo "Creating fusionpanel system user..."
        if [ "$DRY_RUN" -eq 0 ]; then
            useradd --system -g fusionpanel -s /sbin/nologin -d "/var/lib/fusionpanel" -c "FusionPanel Service User" fusionpanel
        else
            echo "[Dry-run] Would run: useradd --system -g fusionpanel -s /sbin/nologin -d \"/var/lib/fusionpanel\" -c \"FusionPanel Service User\" fusionpanel"
        fi
    fi
}

install() {
    echo "Installing FusionPanel $VERSION (Port: $PORT) on $OS ($ARCH)"

    if [ "$DRY_RUN" -eq 1 ]; then
        echo "[Dry-run] Would download and install FusionPanel $VERSION"
        echo "[Dry-run] Would create symlinks and systemd units"
        echo "[Dry-run] Installation skipped."
        return 0
    fi

    local version_file="/var/lib/fusionpanel/.version"

    if [ -f "/usr/local/bin/fusionpanel-api" ]; then
        if [ -f "$version_file" ]; then
            INSTALLED_VERSION=$(cat "$version_file")
            if [ "$INSTALLED_VERSION" = "$VERSION" ]; then
                echo "FusionPanel is already installed with version $VERSION."
                echo "Idempotency guard triggered: skipping installation."
                return 0
            fi
        fi
    fi

    # Create temporary directory for extraction
    TMP_DIR=$(mktemp -d -t fusionpanel-install-XXXXXX)
    trap 'rm -rf "$TMP_DIR"' EXIT

    echo "Downloading and extracting to staging directory..."
    mkdir -p "$TMP_DIR/bin"
    mkdir -p "$TMP_DIR/data"
    mkdir -p "$TMP_DIR/config"

    # Release assets use a stable filename; the tag identifies the version.
    LEGACY_DOWNLOAD_URL=""
    if [ "$VERSION" = "latest" ]; then
        DOWNLOAD_URL="https://github.com/scherenhaenden/FusionPanel/releases/latest/download/fusionpanel-${OS}-${ARCH}.tar.gz"
    else
        case "$VERSION" in *[!a-zA-Z0-9.+_-]*|'') echo "Invalid version" >&2; exit 1;; esac
        URL_VERSION=$(printf '%s' "$VERSION" | sed 's/+/%2B/g')
        DOWNLOAD_URL="https://github.com/scherenhaenden/FusionPanel/releases/download/${URL_VERSION}/fusionpanel-${OS}-${ARCH}.tar.gz"
        LEGACY_DOWNLOAD_URL="https://github.com/scherenhaenden/FusionPanel/releases/download/${URL_VERSION}/fusionpanel-${URL_VERSION}-${OS}-${ARCH}.tar.gz"
    fi

    # Check if download URL is mocked for tests
    if [ -n "$FUSIONPANEL_MOCK_DOWNLOAD_URL" ]; then
        DOWNLOAD_URL="$FUSIONPANEL_MOCK_DOWNLOAD_URL"
        LEGACY_DOWNLOAD_URL=""
    fi

    if ! curl -sSLf "$DOWNLOAD_URL" -o "$TMP_DIR/fusionpanel.tar.gz"; then
         if [ -z "$LEGACY_DOWNLOAD_URL" ] || ! curl -sSLf "$LEGACY_DOWNLOAD_URL" -o "$TMP_DIR/fusionpanel.tar.gz"; then
             echo "Error downloading $DOWNLOAD_URL" >&2
             exit 1
         fi
    fi

    if ! tar -xzf "$TMP_DIR/fusionpanel.tar.gz" -C "$TMP_DIR/bin/"; then
        echo "Error extracting archive" >&2
        exit 1
    fi

    rm -f "$TMP_DIR/fusionpanel.tar.gz"

    if [ ! -f "$TMP_DIR/bin/fusionpanel-api" ] || [ ! -f "$TMP_DIR/bin/fusionpanel-agent" ]; then
        echo "Error: Required binaries (fusionpanel-api, fusionpanel-agent) not found in archive" >&2
        exit 1
    fi

    chmod 755 "$TMP_DIR/bin/fusionpanel-api" "$TMP_DIR/bin/fusionpanel-agent"
    echo "$VERSION" > "$TMP_DIR/.version"

    # Create configuration environment file with securely generated admin token if not exists
    local sysconfdir="/etc/fusionpanel"
    if [ -n "$FUSIONPANEL_SYSCONFDIR" ]; then
        sysconfdir="$FUSIONPANEL_SYSCONFDIR"
    fi

    mkdir -p "$sysconfdir"

    local admin_token=""
    if [ -f "$sysconfdir/fusionpanel.env" ]; then
         admin_token=$(grep '^FUSIONPANEL_ADMIN_TOKEN=' "$sysconfdir/fusionpanel.env" | cut -d= -f2- || true)
    fi

    if [ -z "$admin_token" ]; then
         admin_token=$(openssl rand -hex 32)
    fi

    if [ ! -f "$sysconfdir/initial-credentials.txt" ]; then
         echo "ADMIN_TOKEN=$admin_token" > "$sysconfdir/initial-credentials.txt"
         chmod 0600 "$sysconfdir/initial-credentials.txt"
    fi

    echo "FUSIONPANEL_LISTEN=127.0.0.1:$PORT" > "$TMP_DIR/config/fusionpanel.env"
    echo "FUSIONPANEL_ADMIN_TOKEN=$admin_token" >> "$TMP_DIR/config/fusionpanel.env"

    echo "FUSIONPANEL_AGENT_ENDPOINT=http://127.0.0.1:$PORT" > "$TMP_DIR/config/fusionpanel-agent.env"

    # Atomic move logic: copy to temporary path in final destination, then rename to ensure no incomplete installs.
    local versions_dir="/usr/local/lib/fusionpanel/versions"
    local install_dir="${versions_dir}/${VERSION}"
    mkdir -p "$versions_dir"

    if [ -d "$install_dir" ]; then
        rm -rf "$install_dir"
    fi

    chmod 755 "$TMP_DIR"
    cp -a "$TMP_DIR" "${install_dir}.tmp"
    mv "${install_dir}.tmp" "$install_dir"

    # Now set up symlinks for binaries
    mkdir -p /usr/local/bin
    ln -sfn "$install_dir/bin/fusionpanel-api" "/usr/local/bin/fusionpanel-api"
    ln -sfn "$install_dir/bin/fusionpanel-agent" "/usr/local/bin/fusionpanel-agent"

    cp "$install_dir/config/fusionpanel.env" "$sysconfdir/"
    cp "$install_dir/config/fusionpanel-agent.env" "$sysconfdir/"

    # Setup data and log directory
    mkdir -p /var/lib/fusionpanel
    mkdir -p /var/log/fusionpanel

    cp "$install_dir/.version" "/var/lib/fusionpanel/.version"

    # Set permissions
    chown -R fusionpanel:fusionpanel /var/lib/fusionpanel /var/log/fusionpanel || echo "Warning: chown failed, running as non-root?"
    chmod 700 /var/lib/fusionpanel /var/log/fusionpanel

    chown root:fusionpanel "$sysconfdir/fusionpanel.env" "$sysconfdir/fusionpanel-agent.env" || true
    chmod 640 "$sysconfdir/fusionpanel.env" "$sysconfdir/fusionpanel-agent.env"

    # Install systemd services
    local systemd_dir="/etc/systemd/system"
    if [ -n "$FUSIONPANEL_SYSTEMD_DIR" ]; then
        systemd_dir="$FUSIONPANEL_SYSTEMD_DIR"
    fi

    local source_dir="$(cd "$(dirname "$0")" && pwd)"
    mkdir -p "$systemd_dir"

    if [ -f "$source_dir/systemd/fusionpanel.service" ]; then
        sed -e "s|/usr/local/bin/fusionpanel-api|/usr/local/bin/fusionpanel-api|g" \
            -e "s|/etc/fusionpanel/fusionpanel.env|$sysconfdir/fusionpanel.env|g" \
            "$source_dir/systemd/fusionpanel.service" > "$systemd_dir/fusionpanel.service"

        sed -e "s|/usr/local/bin/fusionpanel-agent|/usr/local/bin/fusionpanel-agent|g" \
            -e "s|/etc/fusionpanel/fusionpanel-agent.env|$sysconfdir/fusionpanel-agent.env|g" \
            "$source_dir/systemd/fusionpanel-agent.service" > "$systemd_dir/fusionpanel-agent.service"

        if [ -z "$FUSIONPANEL_INSTALLER_NO_SYSTEMD_RELOAD" ]; then
            if ! systemctl daemon-reload; then
                echo "Error: systemd daemon-reload failed; service enablement was not attempted. Staged files and existing unit state were not rolled back." >&2
                exit 1
            fi

            if ! systemctl enable fusionpanel.service fusionpanel-agent.service; then
                echo "Error: systemd could not enable both FusionPanel services; either service may now be enabled. Staged files and existing unit state were not rolled back." >&2
                exit 1
            fi
        fi
    fi

    echo "Installation complete."
}

main() {
    detect_os
    detect_arch
    check_dependencies
    setup_users
    install
}

if [ -z "$FUSIONPANEL_INSTALLER_NO_RUN" ]; then
    main "$@"
fi
