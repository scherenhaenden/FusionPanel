#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
INSTALL_SCRIPT="$SCRIPT_DIR/../install.sh"
TEST_DIR=$(mktemp -d)
trap 'rm -rf "$TEST_DIR"' EXIT HUP INT TERM

FUSIONPANEL_INSTALLER_NO_RUN=1
export FUSIONPANEL_INSTALLER_NO_RUN
# shellcheck disable=SC1090
. "$INSTALL_SCRIPT"

node_name=$(normalize_node_name 'node name/example')
if [ "$node_name" != "node-name-example" ]; then
    echo "FAIL: node name was not normalized safely" >&2
    exit 1
fi

write_runtime_env_files "$TEST_DIR/config" "9090" "synthetic-token-for-test" "node name/example"

api_env="$TEST_DIR/config/fusionpanel.env"
agent_env="$TEST_DIR/config/fusionpanel-agent.env"

assert_line() {
    expected="$1"
    file="$2"
    if ! grep -Fqx "$expected" "$file"; then
        echo "FAIL: expected '$expected' in $file" >&2
        exit 1
    fi
}

assert_line 'FUSIONPANEL_LISTEN=127.0.0.1:9090' "$api_env"
assert_line 'FUSIONPANEL_DATABASE_URL=/var/lib/fusionpanel/fusionpanel.db' "$api_env"
assert_line 'FUSIONPANEL_ADMIN_TOKEN=synthetic-token-for-test' "$api_env"
assert_line 'FUSIONPANEL_AGENT_MODE=http' "$agent_env"
assert_line 'FUSIONPANEL_API_URL=http://127.0.0.1:9090' "$agent_env"
assert_line 'FUSIONPANEL_ADMIN_TOKEN=synthetic-token-for-test' "$agent_env"
assert_line 'FUSIONPANEL_NODE_NAME=node-name-example' "$agent_env"

if grep -q '^FUSIONPANEL_AGENT_ENDPOINT=' "$agent_env"; then
    echo "FAIL: obsolete FUSIONPANEL_AGENT_ENDPOINT was generated" >&2
    exit 1
fi

echo "Installer environment configuration tests passed."
