#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
INSTALL_SCRIPT="$SCRIPT_DIR/../install.sh"
TEST_DIR=$(mktemp -d)
trap 'rm -rf "$TEST_DIR"' EXIT HUP INT TERM
export TEST_DIR
mkdir -p "$TEST_DIR/bin"
cat > "$TEST_DIR/bin/chown" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >> "$TEST_DIR/chown-calls"
EOF
chmod +x "$TEST_DIR/bin/chown"
PATH="$TEST_DIR/bin:$PATH"
export PATH

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

assert_mode() {
    expected="$1"
    file="$2"
    actual=$(stat -c '%a' "$file")
    if [ "$actual" != "$expected" ]; then
        echo "FAIL: expected mode $expected for $file, got $actual" >&2
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

secure_active_env_files "$TEST_DIR/config"
assert_mode 600 "$api_env"
assert_mode 600 "$agent_env"
if ! grep -Fqx "root:root $api_env" "$TEST_DIR/chown-calls"; then
    echo "FAIL: API environment file must be owned by root only" >&2
    exit 1
fi
if ! grep -Fqx "root:root $agent_env" "$TEST_DIR/chown-calls"; then
    echo "FAIL: agent environment file must be root-owned" >&2
    exit 1
fi

if grep -q '^FUSIONPANEL_AGENT_ENDPOINT=' "$agent_env"; then
    echo "FAIL: obsolete FUSIONPANEL_AGENT_ENDPOINT was generated" >&2
    exit 1
fi

# The installer later changes the staging directory to 0755 and copies it with cp -a.
# Explicit 0600 modes must keep both token-bearing files private through that path.
umask 000
write_runtime_env_files "$TEST_DIR/stage/config" "9091" "synthetic-token-copy-test" "stage-node"
assert_mode 700 "$TEST_DIR/stage/config"
assert_mode 600 "$TEST_DIR/stage/config/fusionpanel.env"
assert_mode 600 "$TEST_DIR/stage/config/fusionpanel-agent.env"

remote_agent_env="$TEST_DIR/existing-agent.env"
printf 'FUSIONPANEL_AGENT_MODE=http\nFUSIONPANEL_API_URL=https://control.example.test\nFUSIONPANEL_NODE_ID=00000000-0000-4000-8000-000000000001\nFUSIONPANEL_NODE_TOKEN=scoped-node-secret\n' > "$remote_agent_env"
chmod 0640 "$remote_agent_env"
preserve_agent_env_file "$remote_agent_env" "$TEST_DIR/stage/config/fusionpanel-agent.env"
assert_line 'FUSIONPANEL_API_URL=https://control.example.test' "$TEST_DIR/stage/config/fusionpanel-agent.env"
assert_line 'FUSIONPANEL_NODE_ID=00000000-0000-4000-8000-000000000001' "$TEST_DIR/stage/config/fusionpanel-agent.env"
assert_line 'FUSIONPANEL_NODE_TOKEN=scoped-node-secret' "$TEST_DIR/stage/config/fusionpanel-agent.env"
assert_mode 600 "$TEST_DIR/stage/config/fusionpanel-agent.env"
if grep -q '^FUSIONPANEL_ADMIN_TOKEN=' "$TEST_DIR/stage/config/fusionpanel-agent.env"; then
    echo "FAIL: remote agent environment unexpectedly contains the admin token" >&2
    exit 1
fi
if ! grep -Fqx "root:root $TEST_DIR/stage/config/fusionpanel-agent.env" "$TEST_DIR/chown-calls"; then
    echo "FAIL: preserved agent environment must be made root-owned" >&2
    exit 1
fi

ln -s "$remote_agent_env" "$TEST_DIR/symlink-agent.env"
if preserve_agent_env_file "$TEST_DIR/symlink-agent.env" "$TEST_DIR/stage/config/fusionpanel-agent.env" 2>/dev/null; then
    echo "FAIL: symlinked existing agent environment should be rejected" >&2
    exit 1
fi

versions_dir="$TEST_DIR/versions"
mkdir -p "$versions_dir"
chmod 0755 "$TEST_DIR/stage"
cp -a "$TEST_DIR/stage" "$versions_dir/current"
assert_mode 700 "$versions_dir/current/config"
assert_mode 600 "$versions_dir/current/config/fusionpanel.env"
assert_mode 600 "$versions_dir/current/config/fusionpanel-agent.env"

# A normal upgrade must harden token files persisted by older installer versions.
mkdir -p "$versions_dir/old-release/config"
printf 'FUSIONPANEL_ADMIN_TOKEN=old-synthetic-token\n' > "$versions_dir/old-release/config/fusionpanel.env"
printf 'FUSIONPANEL_ADMIN_TOKEN=old-synthetic-agent-token\n' > "$versions_dir/old-release/config/fusionpanel-agent.env"
chmod 0644 "$versions_dir/old-release/config/fusionpanel.env" "$versions_dir/old-release/config/fusionpanel-agent.env"

# Do not follow a replaced config directory into an unrelated path while sweeping versions.
mkdir -p "$versions_dir/symlink-release" "$TEST_DIR/external-config"
printf 'FUSIONPANEL_ADMIN_TOKEN=external-synthetic-token\n' > "$TEST_DIR/external-config/fusionpanel.env"
chmod 0644 "$TEST_DIR/external-config/fusionpanel.env"
ln -s "$TEST_DIR/external-config" "$versions_dir/symlink-release/config"

secure_versioned_env_files "$versions_dir"
assert_mode 700 "$versions_dir/old-release/config"
assert_mode 600 "$versions_dir/current/config/fusionpanel.env"
assert_mode 600 "$versions_dir/current/config/fusionpanel-agent.env"
assert_mode 600 "$versions_dir/old-release/config/fusionpanel.env"
assert_mode 600 "$versions_dir/old-release/config/fusionpanel-agent.env"
assert_mode 644 "$TEST_DIR/external-config/fusionpanel.env"
if ! grep -Fqx "root:root $versions_dir/old-release/config" "$TEST_DIR/chown-calls"; then
    echo "FAIL: versioned config directories must be root-owned" >&2
    exit 1
fi
if ! grep -Fqx "root:root $versions_dir/old-release/config/fusionpanel.env" "$TEST_DIR/chown-calls"; then
    echo "FAIL: versioned API environment file must be root-owned" >&2
    exit 1
fi

echo "Installer environment configuration tests passed."
