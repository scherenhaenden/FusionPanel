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

test_default_install() {
    echo "--- Test: Default Installation ---"

    # We set prefix to the test dir, but since we are executing the script as a whole
    # We should use the --prefix flag. Let's create a fake environment.
    local test_prefix="$TEST_DIR/opt/fusionpanel"

    # Run the script with custom prefix
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
    echo "PASS: Default Installation"
}

test_idempotency() {
    echo "--- Test: Idempotency Guard ---"

    local test_prefix="$TEST_DIR/opt/fusionpanel_idempotent"

    # Run first time
    "$INSTALL_SCRIPT" --prefix "$test_prefix" > /dev/null

    # Touch the file to see if it gets overwritten (if script simulates download)
    # Wait, our simulated install just touches the file. Let's record timestamp.
    # Actually, the script prints "Idempotency guard triggered: skipping installation."

    local output
    output=$("$INSTALL_SCRIPT" --prefix "$test_prefix")

    if ! echo "$output" | grep -q "Idempotency guard triggered"; then
        echo "FAIL: Expected idempotency guard to trigger on second run"
        exit 1
    fi
    echo "PASS: Idempotency Guard"
}

test_flags() {
    echo "--- Test: Flag Parsing ---"

    # We source the script to test internal variables, but we disabled running main
    # when FUSIONPANEL_INSTALLER_NO_RUN is set.
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
    echo "PASS: Flag Parsing"
}

# Ensure required commands are present for the test environment
if ! command -v curl >/dev/null 2>&1 || ! command -v systemctl >/dev/null 2>&1 || ! command -v tar >/dev/null 2>&1; then
    echo "Warning: Test environment missing required dependencies (curl, systemctl, tar) to fully run the installer script."
    # We could mock them, but for a bash test script, usually running on the CI, they might be there.
    # We will provide simple mock scripts in the PATH if needed.
fi

# Mocking commands for the test environment
export PATH="$TEST_DIR/bin:$PATH"
mkdir -p "$TEST_DIR/bin"
for cmd in curl systemctl tar; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "#!/bin/sh" > "$TEST_DIR/bin/$cmd"
        echo "echo 'Mocked $cmd called'" >> "$TEST_DIR/bin/$cmd"
        chmod +x "$TEST_DIR/bin/$cmd"
    fi
done

# Run tests
test_default_install
test_idempotency
test_flags

echo "All tests passed successfully."
