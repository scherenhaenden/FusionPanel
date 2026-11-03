#!/bin/sh
set -e

# Default values
PREFIX="/opt/fusionpanel"
PORT="8080"
VERSION="latest"
OS=""
ARCH=""

usage() {
    echo "Usage: $0 [OPTIONS]"
    echo "Options:"
    echo "  --prefix <path>   Installation prefix (default: /opt/fusionpanel)"
    echo "  --port <port>     Port for FusionPanel (default: 8080)"
    echo "  --version <ver>   Version to install (default: latest)"
    echo "  --help            Show this message"
}

# Parse arguments
while [ $# -gt 0 ]; do
    case "$1" in
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        --port)
            PORT="$2"
            shift 2
            ;;
        --version)
            VERSION="$2"
            shift 2
            ;;
        --help)
            usage
            exit 0
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
        groupadd --system fusionpanel
        echo "Creating fusionpanel system user..."
        useradd --system -g fusionpanel -s /sbin/nologin -d "$PREFIX" -c "FusionPanel Service User" fusionpanel
    fi
}

install() {
    echo "Installing FusionPanel $VERSION to $PREFIX (Port: $PORT) on $OS ($ARCH)"

    local version_file="$PREFIX/.version"

    if [ -d "$PREFIX" ] && [ -f "$PREFIX/bin/fusionpanel-api" ]; then
        if [ -f "$version_file" ]; then
            INSTALLED_VERSION=$(cat "$version_file")
            if [ "$INSTALLED_VERSION" = "$VERSION" ]; then
                echo "FusionPanel is already installed at $PREFIX with version $VERSION."
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

    # Download real versioned artifacts
    if [ "$VERSION" = "latest" ]; then
        # For simplicity, if latest is specified, we construct a generic URL pattern. In a real system, you'd query GitHub API.
        DOWNLOAD_URL="https://github.com/scherenhaenden/FusionPanel/releases/latest/download/fusionpanel-${OS}-${ARCH}.tar.gz"
    else
        DOWNLOAD_URL="https://github.com/scherenhaenden/FusionPanel/releases/download/${VERSION}/fusionpanel-${VERSION}-${OS}-${ARCH}.tar.gz"
    fi

    # Check if download URL is mocked for tests
    if [ -n "$FUSIONPANEL_MOCK_DOWNLOAD_URL" ]; then
        DOWNLOAD_URL="$FUSIONPANEL_MOCK_DOWNLOAD_URL"
    fi

    if ! curl -sSLf "$DOWNLOAD_URL" -o "$TMP_DIR/fusionpanel.tar.gz"; then
         echo "Error downloading $DOWNLOAD_URL" >&2
         exit 1
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

    local admin_token=""
    if [ -f "$sysconfdir/fusionpanel.env" ]; then
         admin_token=$(grep '^FUSIONPANEL_ADMIN_TOKEN=' "$sysconfdir/fusionpanel.env" | cut -d= -f2- || true)
    fi

    if [ -z "$admin_token" ]; then
         admin_token=$(openssl rand -hex 32)
    fi

    echo "FUSIONPANEL_LISTEN=127.0.0.1:$PORT" > "$TMP_DIR/config/fusionpanel.env"
    echo "FUSIONPANEL_ADMIN_TOKEN=$admin_token" >> "$TMP_DIR/config/fusionpanel.env"

    echo "FUSIONPANEL_AGENT_ENDPOINT=http://127.0.0.1:$PORT" > "$TMP_DIR/config/fusionpanel-agent.env"

    # Atomic move logic: copy to temporary path in final destination, then rename to ensure no incomplete installs.
    mkdir -p "$PREFIX"

    # To support update/recovery, we will install to a versioned directory and symlink
    local install_dir="${PREFIX}/versions/${VERSION}"
    mkdir -p "${PREFIX}/versions"

    # If the version directory already exists and was partial, we remove it
    if [ -d "$install_dir" ]; then
        rm -rf "$install_dir"
    fi

    # Copy from TMP_DIR to final version dir
    chmod 755 "$TMP_DIR"
    cp -a "$TMP_DIR" "${install_dir}.tmp"
    mv "${install_dir}.tmp" "$install_dir"

    # Now set up symlinks and permissions
    ln -sfn "$install_dir/bin" "$PREFIX/bin"

    mkdir -p "$sysconfdir"
    cp "$install_dir/config/fusionpanel.env" "$sysconfdir/"
    cp "$install_dir/config/fusionpanel-agent.env" "$sysconfdir/"

    # Setup data directory
    if [ ! -d "$PREFIX/data" ]; then
        cp -a "$install_dir/data" "$PREFIX/data"
    fi

    cp "$install_dir/.version" "$PREFIX/.version"

    # Set permissions
    chown -R fusionpanel:fusionpanel "$PREFIX/data" || echo "Warning: chown failed, running as non-root?"
    chmod 700 "$PREFIX/data"

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
        sed -e "s|/opt/fusionpanel/bin/fusionpanel-api|$PREFIX/bin/fusionpanel-api|g" \
            -e "s|/etc/fusionpanel/fusionpanel.env|$sysconfdir/fusionpanel.env|g" \
            "$source_dir/systemd/fusionpanel.service" > "$systemd_dir/fusionpanel.service"

        sed -e "s|/opt/fusionpanel/bin/fusionpanel-agent|$PREFIX/bin/fusionpanel-agent|g" \
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
