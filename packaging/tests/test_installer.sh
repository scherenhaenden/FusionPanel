#!/bin/bash
set -e

# Test installer functionality

TEST_DIR=$(mktemp -d)
INSTALL_SCRIPT="$(dirname "$(dirname "$0")")/install.sh"

cleanup() {
    rm -rf "$TEST_DIR"
}
trap cleanup EXIT

echo "Running tests for $INSTALL_SCRIPT"

# Ensure script is executable
chmod +x "$INSTALL_SCRIPT"

# Mocking commands for the test environment
export PATH="$TEST_DIR/bin:$PATH"
mkdir -p "$TEST_DIR/bin"
for cmd in curl systemctl tar useradd groupadd id chown openssl; do
    echo "#!/bin/sh" > "$TEST_DIR/bin/$cmd"

    if [ "$cmd" = "openssl" ]; then
        echo "echo 'mocked-secure-token-1234567890abcdef'" >> "$TEST_DIR/bin/$cmd"
    elif [ "$cmd" = "id" ]; then
        echo "if [ \"\$1\" = 'fusionpanel' ] && [ -f \"$TEST_DIR/fusionpanel_user_created\" ]; then exit 0; elif [ \"\$1\" = 'fusionpanel' ]; then exit 1; else exit 0; fi" >> "$TEST_DIR/bin/$cmd"
    elif [ "$cmd" = "useradd" ]; then
        echo "touch \"$TEST_DIR/fusionpanel_user_created\"" >> "$TEST_DIR/bin/$cmd"
    elif [ "$cmd" = "systemctl" ]; then
        echo "echo \"\$@\" >> \"$TEST_DIR/systemctl_calls.log\"" >> "$TEST_DIR/bin/$cmd"
    elif [ "$cmd" = "curl" ]; then
        echo "touch \"\$4\"" >> "$TEST_DIR/bin/$cmd"
    elif [ "$cmd" = "tar" ]; then
        echo "mkdir -p \"\$4\"" >> "$TEST_DIR/bin/$cmd"
        echo "touch \"\$4/fusionpanel-api\"" >> "$TEST_DIR/bin/$cmd"
        echo "touch \"\$4/fusionpanel-agent\"" >> "$TEST_DIR/bin/$cmd"
    else
        echo "echo 'Mocked $cmd called' >&2" >> "$TEST_DIR/bin/$cmd"
    fi

    chmod +x "$TEST_DIR/bin/$cmd"
done

# Clear systemctl log between tests
clear_systemctl_log() {
    rm -f "$TEST_DIR/systemctl_calls.log"
    touch "$TEST_DIR/systemctl_calls.log"
}

# Env vars to configure install script locations to be within our test dir
export FUSIONPANEL_SYSCONFDIR="$TEST_DIR/etc/fusionpanel"
export FUSIONPANEL_SYSTEMD_DIR="$TEST_DIR/etc/systemd/system"
export FUSIONPANEL_INSTALLER_NO_SYSTEMD_RELOAD=""
export FUSIONPANEL_MOCK_DOWNLOAD_URL="http://mock-url"

test_clean_install() {
    echo "--- Test: Clean Install ---"
    clear_systemctl_log

    local test_prefix="$TEST_DIR/opt/fusionpanel_clean"

    "$INSTALL_SCRIPT" --prefix "$test_prefix"

    # Verify installation
    if [ ! -f "$test_prefix/bin/fusionpanel-api" ]; then
        echo "FAIL: Expected $test_prefix/bin/fusionpanel-api to exist"
        exit 1
    fi
    if [ ! -x "$test_prefix/bin/fusionpanel-api" ]; then
        echo "FAIL: Expected $test_prefix/bin/fusionpanel-api to be executable"
        exit 1
    fi
    if [ ! -f "$test_prefix/.version" ]; then
        echo "FAIL: Expected $test_prefix/.version to exist"
        exit 1
    fi

    # Verify versioned install
    if [ ! -d "$test_prefix/versions/latest" ]; then
        echo "FAIL: Expected versioned install path"
        exit 1
    fi

    # Verify config
    if [ ! -f "$FUSIONPANEL_SYSCONFDIR/fusionpanel.env" ]; then
        echo "FAIL: Expected config file at $FUSIONPANEL_SYSCONFDIR/fusionpanel.env"
        exit 1
    fi
    if ! grep -q "FUSIONPANEL_LISTEN=127.0.0.1:8080" "$FUSIONPANEL_SYSCONFDIR/fusionpanel.env"; then
        echo "FAIL: Expected 8080 port in config"
        exit 1
    fi
    if ! grep -q "FUSIONPANEL_ADMIN_TOKEN=mocked-secure-token-1234567890abcdef" "$FUSIONPANEL_SYSCONFDIR/fusionpanel.env"; then
        echo "FAIL: Expected generated admin token in config"
        exit 1
    fi

    # Check systemd services
    if [ ! -f "$FUSIONPANEL_SYSTEMD_DIR/fusionpanel.service" ]; then
        echo "FAIL: Expected systemd service at $FUSIONPANEL_SYSTEMD_DIR/fusionpanel.service"
        exit 1
    fi

    # Check if systemctl was called properly
    if ! grep -q "daemon-reload" "$TEST_DIR/systemctl_calls.log"; then
        echo "FAIL: systemctl daemon-reload was not called"
        exit 1
    fi
    if ! grep -q "enable fusionpanel.service" "$TEST_DIR/systemctl_calls.log"; then
        echo "FAIL: systemctl enable was not called"
        exit 1
    fi
    echo "PASS: Clean Install"
}

test_idempotency_and_reinstall() {
    echo "--- Test: Idempotency & Reinstallation ---"
    clear_systemctl_log

    local test_prefix="$TEST_DIR/opt/fusionpanel_idempotent"

    # Run first time
    "$INSTALL_SCRIPT" --prefix "$test_prefix" --version "1.0.0" > /dev/null

    # Second run with same version should be idempotent
    local output
    output=$("$INSTALL_SCRIPT" --prefix "$test_prefix" --version "1.0.0")

    if ! echo "$output" | grep -q "Idempotency guard triggered"; then
        echo "FAIL: Expected idempotency guard to trigger on second run"
        exit 1
    fi

    # Run with different version should perform an update (reinstallation)
    output=$("$INSTALL_SCRIPT" --prefix "$test_prefix" --version "2.0.0")
    if echo "$output" | grep -q "Idempotency guard triggered"; then
        echo "FAIL: Expected install to proceed for a new version"
        exit 1
    fi

    # Check if symlinks point to new version
    if [ ! -d "$test_prefix/versions/2.0.0" ]; then
        echo "FAIL: Expected new version directory"
        exit 1
    fi

    echo "PASS: Idempotency & Reinstallation"
}

test_flags() {
    echo "--- Test: Flag Parsing ---"

    export FUSIONPANEL_INSTALLER_NO_RUN=1
    # shellcheck disable=SC1090
    . "$INSTALL_SCRIPT" --prefix "/my/custom/path" --port "9090" --version "1.2.3"

    if [ "${PREFIX:-}" != "/my/custom/path" ]; then
        echo "FAIL: PREFIX not parsed correctly. Got: ${PREFIX:-}"
        exit 1
    fi
    if [ "${PORT:-}" != "9090" ]; then
        echo "FAIL: PORT not parsed correctly. Got: ${PORT:-}"
        exit 1
    fi
    if [ "${VERSION:-}" != "1.2.3" ]; then
        echo "FAIL: VERSION not parsed correctly. Got: ${VERSION:-}"
        exit 1
    fi
    unset FUSIONPANEL_INSTALLER_NO_RUN
    echo "PASS: Flag Parsing"
}

test_start_restart_mock() {
    echo "--- Test: Start and Restart (via systemctl logging) ---"
    clear_systemctl_log
    systemctl start fusionpanel.service
    systemctl restart fusionpanel.service

    if ! grep -q "start fusionpanel.service" "$TEST_DIR/systemctl_calls.log"; then
        echo "FAIL: Mock start failed"
        exit 1
    fi
    if ! grep -q "restart fusionpanel.service" "$TEST_DIR/systemctl_calls.log"; then
        echo "FAIL: Mock restart failed"
        exit 1
    fi

    echo "PASS: Start and Restart mock"
}

test_valid_auth() {
    echo "--- Test: Valid Auth Generation ---"

    # Run an install and verify the token was generated in config
    local test_prefix="$TEST_DIR/opt/fusionpanel_auth"
    "$INSTALL_SCRIPT" --prefix "$test_prefix" > /dev/null

    local config_file="$FUSIONPANEL_SYSCONFDIR/fusionpanel.env"
    if ! grep -q "FUSIONPANEL_ADMIN_TOKEN" "$config_file"; then
        echo "FAIL: Admin token missing from config"
        exit 1
    fi

    echo "PASS: Valid Auth"
}

test_partial_install_recovery() {
    echo "--- Test: Partial Install Recovery ---"

    local test_prefix="$TEST_DIR/opt/fusionpanel_partial"
    local version="v0.9.9"

    # Manually create a partial/corrupted install directory
    mkdir -p "$test_prefix/versions/$version/bin"
    touch "$test_prefix/versions/$version/corrupted_file"

    # Run the installer, which should clear the partial install before proceeding
    "$INSTALL_SCRIPT" --prefix "$test_prefix" --version "$version" > /dev/null

    # Check that the real API binary is there and not just the corrupted file
    if [ ! -f "$test_prefix/versions/$version/bin/fusionpanel-api" ]; then
        echo "FAIL: Installer did not recover from partial install properly"
        exit 1
    fi

    if [ -f "$test_prefix/versions/$version/corrupted_file" ]; then
        echo "FAIL: Corrupted file was not cleared"
        exit 1
    fi

    echo "PASS: Partial Install Recovery"
}

test_missing_binaries_fail_closed() {
    echo "--- Test: Missing Binaries Fail-Closed ---"
    local test_prefix="$TEST_DIR/opt/fusionpanel_missing"

    # Create mock tar that does NOT extract binaries
    local old_path="$PATH"
    local bad_bin_dir="$TEST_DIR/bad_bin"
    mkdir -p "$bad_bin_dir"
    cat << 'EOF' > "$bad_bin_dir/tar"
#!/bin/sh
mkdir -p "$4"
# do not extract fusionpanel-api
touch "$4/some_other_file"
EOF
    chmod +x "$bad_bin_dir/tar"

    set +e
    PATH="$bad_bin_dir:$PATH" "$INSTALL_SCRIPT" --prefix "$test_prefix" > /dev/null 2>&1
    local exit_code=$?
    set -e

    if [ "$exit_code" -eq 0 ]; then
        echo "FAIL: Expected installer to fail when binaries are missing from archive"
        exit 1
    fi

    if [ -d "$test_prefix/bin" ]; then
        echo "FAIL: Missing binaries should not produce installed bin dir"
        exit 1
    fi

    echo "PASS: Missing Binaries Fail-Closed"
}

test_dry_run() {
    echo "--- Test: Dry Run ---"
    local test_prefix="$TEST_DIR/opt/fusionpanel_dryrun"

    "$INSTALL_SCRIPT" --prefix "$test_prefix" --dry-run > /dev/null

    if [ -d "$test_prefix" ]; then
        echo "FAIL: Dry run created the prefix directory"
        exit 1
    fi

    echo "PASS: Dry Run"
}

# Run tests
test_clean_install
test_idempotency_and_reinstall
test_flags
test_start_restart_mock
test_valid_auth
test_partial_install_recovery
test_missing_binaries_fail_closed
test_dry_run

echo "All tests passed successfully."
