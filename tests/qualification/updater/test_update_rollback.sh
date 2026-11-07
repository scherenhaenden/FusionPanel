#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGING_DIR="${SCRIPT_DIR}/../../../packaging"

UPDATE_SCRIPT="${PACKAGING_DIR}/update.sh"
ROLLBACK_SCRIPT="${PACKAGING_DIR}/rollback.sh"

echo "Running tests for updater scripts"

chmod +x "$UPDATE_SCRIPT" "$ROLLBACK_SCRIPT"

TEST_DIR=$(mktemp -d)
cleanup() {
    rm -rf "$TEST_DIR"
}
trap cleanup EXIT

# Mock dependencies
export PATH="$TEST_DIR/bin:$PATH"
mkdir -p "$TEST_DIR/bin"

for cmd in systemctl tar curl; do
    echo "#!/bin/sh" > "$TEST_DIR/bin/$cmd"

    if [ "$cmd" = "systemctl" ]; then
        echo "echo \"\$@\" >> \"$TEST_DIR/systemctl_calls.log\"" >> "$TEST_DIR/bin/$cmd"
    elif [ "$cmd" = "curl" ]; then
        echo "if [ \"\$1\" = \"-sSLf\" ]; then" >> "$TEST_DIR/bin/$cmd"
        echo "  if echo \"\$2\" | grep -q 'health'; then" >> "$TEST_DIR/bin/$cmd"
        echo "    if [ -f \"$TEST_DIR/health_fail\" ]; then exit 1; fi" >> "$TEST_DIR/bin/$cmd"
        echo "    exit 0" >> "$TEST_DIR/bin/$cmd"
        echo "  fi" >> "$TEST_DIR/bin/$cmd"
        echo "  touch \"\$4\"" >> "$TEST_DIR/bin/$cmd"
        echo "fi" >> "$TEST_DIR/bin/$cmd"
    elif [ "$cmd" = "tar" ]; then
        echo "mkdir -p \"\$4\"" >> "$TEST_DIR/bin/$cmd"
        echo "touch \"\$4/fusionpanel-api\"" >> "$TEST_DIR/bin/$cmd"
        echo "touch \"\$4/fusionpanel-agent\"" >> "$TEST_DIR/bin/$cmd"
    fi
    chmod +x "$TEST_DIR/bin/$cmd"
done

# Setup env vars to override system paths for tests
export FUSIONPANEL_TEST_BACKUP_DIR="$TEST_DIR/var/lib/fusionpanel/backup-pre-update"
export FUSIONPANEL_TEST_CONFIG_DIR="$TEST_DIR/etc/fusionpanel"
export FUSIONPANEL_TEST_BIN_DIR="$TEST_DIR/usr/local/bin"
export FUSIONPANEL_TEST_VERSION_LIB_DIR="$TEST_DIR/usr/local/lib/fusionpanel/versions"
export FUSIONPANEL_MOCK_DOWNLOAD_URL="http://mock-url"
export FUSIONPANEL_TEST_OS="debian"
export FUSIONPANEL_TEST_ARCH="x86_64"
export FUSIONPANEL_HEALTH_URL="http://127.0.0.1:8080/v1/health"

clear_systemctl_log() {
    rm -f "$TEST_DIR/systemctl_calls.log"
    touch "$TEST_DIR/systemctl_calls.log"
}

setup_initial_state() {
    rm -rf "$TEST_DIR/var" "$TEST_DIR/etc" "$TEST_DIR/usr" "$TEST_DIR/systemctl_calls.log" "$TEST_DIR/health_fail"
    mkdir -p "$FUSIONPANEL_TEST_CONFIG_DIR" "$FUSIONPANEL_TEST_BIN_DIR"

    echo "old_config" > "$FUSIONPANEL_TEST_CONFIG_DIR/config.env"

    # create dummy binaries
    mkdir -p "$FUSIONPANEL_TEST_VERSION_LIB_DIR/old_version"
    echo "old_api" > "$FUSIONPANEL_TEST_VERSION_LIB_DIR/old_version/fusionpanel-api"
    echo "old_agent" > "$FUSIONPANEL_TEST_VERSION_LIB_DIR/old_version/fusionpanel-agent"

    ln -sfn "$FUSIONPANEL_TEST_VERSION_LIB_DIR/old_version/fusionpanel-api" "$FUSIONPANEL_TEST_BIN_DIR/fusionpanel-api"
    ln -sfn "$FUSIONPANEL_TEST_VERSION_LIB_DIR/old_version/fusionpanel-agent" "$FUSIONPANEL_TEST_BIN_DIR/fusionpanel-agent"
}

test_successful_update() {
    echo "--- Test: Successful Update ---"
    setup_initial_state
    clear_systemctl_log

    "$UPDATE_SCRIPT" "new_version" > /dev/null

    # Check if backup was created
    if [ ! -f "$FUSIONPANEL_TEST_BACKUP_DIR/config/config.env" ]; then
        echo "FAIL: Config backup not found"
        exit 1
    fi
    if [ ! -f "$FUSIONPANEL_TEST_BACKUP_DIR/bin/fusionpanel-api" ]; then
         echo "FAIL: API binary backup not found"
         exit 1
    fi

    # Check if symlinks updated
    API_LINK=$(readlink "$FUSIONPANEL_TEST_BIN_DIR/fusionpanel-api")
    if [ "$API_LINK" != "$FUSIONPANEL_TEST_VERSION_LIB_DIR/new_version/fusionpanel-api" ]; then
        echo "FAIL: Symlink not updated correctly. Got: $API_LINK"
        exit 1
    fi

    # Check systemctl calls
    if ! grep -q "stop fusionpanel.service fusionpanel-agent.service" "$TEST_DIR/systemctl_calls.log"; then
        echo "FAIL: systemctl stop not called"
        exit 1
    fi
    if ! grep -q "start fusionpanel.service fusionpanel-agent.service" "$TEST_DIR/systemctl_calls.log"; then
        echo "FAIL: systemctl start not called"
        exit 1
    fi

    echo "PASS: Successful Update"
}

test_failed_update_rollback() {
    echo "--- Test: Failed Update Rollback ---"
    setup_initial_state
    clear_systemctl_log

    # mock health check failure
    touch "$TEST_DIR/health_fail"
    # mock health check timeout by overriding script
    # We will just patch the timeout in script dynamically or it will take 30s which is annoying for tests,
    # but we can let it run or patch it. Wait, the timeout is hardcoded to 30.
    # To avoid sleeping 30s in test, let's patch the update.sh health_timeout just for this run,
    # or export an env variable if we add one.

    sed -i 's/HEALTH_TIMEOUT=30/HEALTH_TIMEOUT=2/' "$UPDATE_SCRIPT"

    set +e
    "$UPDATE_SCRIPT" "new_version" > "$TEST_DIR/update_fail.log" 2>&1
    EXIT_CODE=$?
    set -e

    # Restore the timeout
    sed -i 's/HEALTH_TIMEOUT=2/HEALTH_TIMEOUT=30/' "$UPDATE_SCRIPT"

    if [ "$EXIT_CODE" -eq 0 ]; then
        echo "FAIL: Update script should have failed"
        exit 1
    fi

    if ! grep -q "Health check failed after" "$TEST_DIR/update_fail.log"; then
        echo "FAIL: Expected health check failure message"
        exit 1
    fi

    # Check if rolled back
    # With cp -L backup, it restores actual files instead of symlinks, so readlink will fail or return empty, and it will just be a normal file.
    if [ ! -f "$FUSIONPANEL_TEST_BIN_DIR/fusionpanel-api" ]; then
        echo "FAIL: API not rolled back (file missing)."
        exit 1
    fi
    API_CONTENT=$(cat "$FUSIONPANEL_TEST_BIN_DIR/fusionpanel-api")
    if [ "$API_CONTENT" != "old_api" ]; then
        echo "FAIL: API content not rolled back. Got: $API_CONTENT"
        exit 1
    fi

    # verify restart on rollback
    STOP_COUNT=$(grep -c "stop fusionpanel.service fusionpanel-agent.service" "$TEST_DIR/systemctl_calls.log")
    START_COUNT=$(grep -c "start fusionpanel.service fusionpanel-agent.service" "$TEST_DIR/systemctl_calls.log")

    # Should be 2 stops (update + rollback) and 2 starts (update + rollback)
    if [ "$STOP_COUNT" -ne 2 ]; then
        echo "FAIL: Expected 2 stops, got $STOP_COUNT"
        exit 1
    fi
    if [ "$START_COUNT" -ne 2 ]; then
         echo "FAIL: Expected 2 starts, got $START_COUNT"
         exit 1
    fi

    echo "PASS: Failed Update Rollback"
}

test_successful_update
test_failed_update_rollback

echo "All update/rollback tests passed successfully."
