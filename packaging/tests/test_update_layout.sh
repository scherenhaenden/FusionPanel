#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
TEST_DIR=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-update-layout.XXXXXX")
trap 'rm -rf -- "$TEST_DIR"' EXIT HUP INT TERM

mkdir -p \
    "$TEST_DIR/bin" \
    "$TEST_DIR/config" \
    "$TEST_DIR/state" \
    "$TEST_DIR/versions/0.1.0/bin/frontend/dist" \
    "$TEST_DIR/versions/0.1.0/config" \
    "$TEST_DIR/payload/frontend/dist" \
    "$TEST_DIR/payload/systemd" \
    "$TEST_DIR/payload/packaging" \
    "$TEST_DIR/systemd" \
    "$TEST_DIR/logs"

for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli; do
    cat > "$TEST_DIR/versions/0.1.0/bin/$binary" <<EOF_OLD
#!/bin/sh
echo old-$binary
EOF_OLD
    chmod 0755 "$TEST_DIR/versions/0.1.0/bin/$binary"
    cat > "$TEST_DIR/payload/$binary" <<EOF_NEW
#!/bin/sh
echo new-$binary
EOF_NEW
    chmod 0755 "$TEST_DIR/payload/$binary"
done
cat > "$TEST_DIR/payload/fusionpanel-nginx-helper" <<'EOF_NEW_HELPER'
#!/bin/sh
echo new-fusionpanel-nginx-helper
EOF_NEW_HELPER
chmod 0755 "$TEST_DIR/payload/fusionpanel-nginx-helper"
printf '<html>old UI</html>\n' > "$TEST_DIR/versions/0.1.0/bin/frontend/dist/index.html"
printf '<html>new UI</html>\n' > "$TEST_DIR/payload/frontend/dist/index.html"
printf 'FUSIONPANEL_LISTEN=127.0.0.1:9090\nFUSIONPANEL_ADMIN_TOKEN=old-admin-secret\nFUSIONPANEL_FRONTEND_DIR=/old/ui\n' > "$TEST_DIR/config/fusionpanel.env"
printf 'FUSIONPANEL_AGENT_MODE=http\nFUSIONPANEL_API_URL=https://control.example.test\nFUSIONPANEL_NODE_ID=00000000-0000-4000-8000-000000000001\nFUSIONPANEL_NODE_TOKEN=remote-node-secret\n' > "$TEST_DIR/config/fusionpanel-agent.env"
cp -a "$TEST_DIR/config/." "$TEST_DIR/versions/0.1.0/config/"
printf '0.1.0\n' > "$TEST_DIR/versions/0.1.0/.version"
ln -s 0.1.0 "$TEST_DIR/versions/current"
for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli; do
    ln -s "$TEST_DIR/versions/current/bin/$binary" "$TEST_DIR/bin/$binary"
done
printf '0.1.0\n' > "$TEST_DIR/state/.version"
printf 'old api unit\n' > "$TEST_DIR/systemd/fusionpanel.service"
printf 'old agent unit\n' > "$TEST_DIR/systemd/fusionpanel-agent.service"
cat > "$TEST_DIR/payload/systemd/fusionpanel.service" <<'UNIT'
[Service]
User=fusionpanel
Group=fusionpanel
ReadWritePaths=/var/lib/fusionpanel/ /var/log/fusionpanel/ /etc/fusionpanel/
UNIT
cat > "$TEST_DIR/payload/systemd/fusionpanel-agent.service" <<'UNIT'
[Service]
User=fusionpanel
Group=fusionpanel
SupplementaryGroups=fusionpanel-nginx
EnvironmentFile=-/etc/fusionpanel/fusionpanel-agent.env
UNIT
cat > "$TEST_DIR/payload/systemd/fusionpanel-nginx-helper.service" <<'UNIT'
[Service]
User=root
Group=fusionpanel-nginx
RuntimeDirectory=fusionpanel-nginx
UNIT
cat > "$TEST_DIR/payload/packaging/nginx-include.sh" <<'SH'
#!/bin/sh
printf 'keep nginx state across rollback\n' > "$FUSIONPANEL_TEST_NGINX_STATE"
SH
chmod 0755 "$TEST_DIR/payload/packaging/nginx-include.sh"
for unit in fusionpanel.service fusionpanel-agent.service fusionpanel-nginx-helper.service; do
    chmod 0644 "$TEST_DIR/payload/systemd/$unit"
done
tar -C "$TEST_DIR/payload" -czf "$TEST_DIR/package.tar.gz" .

mkdir -p "$TEST_DIR/mock-bin"
cat > "$TEST_DIR/mock-bin/curl" <<'SH'
#!/bin/sh
output=""
previous=""
for arg in "$@"; do
    if [ "$previous" = "-o" ]; then output="$arg"; fi
    previous="$arg"
done
if [ -n "$output" ]; then
    cp "$FUSIONPANEL_TEST_ARCHIVE" "$output"
    exit 0
fi
printf '%s\n' "$*" >> "$FUSIONPANEL_TEST_CURL_LOG"
if [ "${FUSIONPANEL_TEST_FAIL_HEALTH:-0}" = 1 ]; then exit 22; fi
if [ -n "${FUSIONPANEL_TEST_FAIL_URL:-}" ]; then
    for arg in "$@"; do
        if [ "$arg" = "$FUSIONPANEL_TEST_FAIL_URL" ]; then exit 22; fi
    done
fi
exit 0
SH
cat > "$TEST_DIR/mock-bin/systemctl" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$FUSIONPANEL_TEST_SYSTEMCTL_LOG"
[ "${FUSIONPANEL_TEST_SYSTEMCTL_FAIL:-}" != "$*" ] || exit 1
SH
cat > "$TEST_DIR/mock-bin/chown" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$FUSIONPANEL_TEST_CHOWN_LOG"
SH
cat > "$TEST_DIR/mock-bin/getent" <<'SH'
#!/bin/sh
if [ "$1" = passwd ]; then exit 0; fi
if [ "$1" = group ] && [ "$2" = fusionpanel-nginx ]; then
    printf 'fusionpanel-nginx:x:999:\n'
    exit 0
fi
exit 2
SH
cat > "$TEST_DIR/mock-bin/groupadd" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$FUSIONPANEL_TEST_GROUPADD_LOG"
SH
chmod +x "$TEST_DIR/mock-bin/curl" "$TEST_DIR/mock-bin/systemctl" \
    "$TEST_DIR/mock-bin/chown" "$TEST_DIR/mock-bin/getent" "$TEST_DIR/mock-bin/groupadd"

export PATH="$TEST_DIR/mock-bin:$PATH"
export FUSIONPANEL_TEST_ARCHIVE="$TEST_DIR/package.tar.gz"
export FUSIONPANEL_TEST_SYSTEMCTL_LOG="$TEST_DIR/logs/systemctl.log"
export FUSIONPANEL_TEST_CHOWN_LOG="$TEST_DIR/logs/chown.log"
export FUSIONPANEL_TEST_CURL_LOG="$TEST_DIR/logs/curl.log"
export FUSIONPANEL_TEST_BACKUP_DIR="$TEST_DIR/backup"
export FUSIONPANEL_TEST_CONFIG_DIR="$TEST_DIR/config"
export FUSIONPANEL_TEST_BIN_DIR="$TEST_DIR/bin"
export FUSIONPANEL_TEST_VERSION_LIB_DIR="$TEST_DIR/versions"
export FUSIONPANEL_TEST_STATE_DIR="$TEST_DIR/state"
export FUSIONPANEL_TEST_SYSTEMD_DIR="$TEST_DIR/systemd"
export FUSIONPANEL_TEST_NGINX_STATE="$TEST_DIR/nginx-state"
export FUSIONPANEL_TEST_GROUPADD_LOG="$TEST_DIR/logs/groupadd.log"
export FUSIONPANEL_TEST_OS=debian
export FUSIONPANEL_TEST_ARCH=x86_64
export FUSIONPANEL_MOCK_DOWNLOAD_URL=synthetic-package

sh "$ROOT_DIR/packaging/update.sh" 0.2.0-test

[[ "$(readlink "$TEST_DIR/versions/current")" == '0.2.0-test' ]]
[[ -d "$TEST_DIR/versions/current/data" ]]
for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper; do
    [[ -L "$TEST_DIR/bin/$binary" ]]
    [[ "$("$TEST_DIR/bin/$binary")" == "new-$binary" ]]
    [[ -x "$TEST_DIR/versions/current/bin/$binary" ]]
done
grep -Fxq 'SupplementaryGroups=fusionpanel-nginx' "$TEST_DIR/systemd/fusionpanel-agent.service"
! grep -q '^SupplementaryGroups=fusionpanel-nginx$' "$TEST_DIR/systemd/fusionpanel.service"
[[ -f "$TEST_DIR/systemd/fusionpanel-nginx-helper.service" ]]
[[ "$(cat "$TEST_DIR/nginx-state")" == 'keep nginx state across rollback' ]]
[[ -f "$TEST_DIR/backup/systemd/fusionpanel-agent.service" ]]
[[ ! -e "$TEST_DIR/backup/systemd/fusionpanel-nginx-helper.service" ]]
grep -Fxq 'start fusionpanel-nginx-helper.service' "$TEST_DIR/logs/systemctl.log"
grep -Fxq 'start fusionpanel.service' "$TEST_DIR/logs/systemctl.log"
grep -Fxq 'start fusionpanel-agent.service' "$TEST_DIR/logs/systemctl.log"
grep -Fxq 'is-active --quiet fusionpanel-nginx-helper.service' "$TEST_DIR/logs/systemctl.log"
[[ "$(cat "$TEST_DIR/versions/current/bin/frontend/dist/index.html")" == '<html>new UI</html>' ]]
grep -Fxq 'FUSIONPANEL_ADMIN_TOKEN=old-admin-secret' "$TEST_DIR/config/fusionpanel.env"
grep -Fxq "FUSIONPANEL_FRONTEND_DIR=$TEST_DIR/versions/current/bin/frontend/dist" "$TEST_DIR/config/fusionpanel.env"
[[ "$(grep -c '^FUSIONPANEL_FRONTEND_DIR=' "$TEST_DIR/config/fusionpanel.env")" -eq 1 ]]
grep -Fxq 'FUSIONPANEL_API_URL=https://control.example.test' "$TEST_DIR/config/fusionpanel-agent.env"
grep -Fxq 'FUSIONPANEL_NODE_TOKEN=remote-node-secret' "$TEST_DIR/config/fusionpanel-agent.env"
! grep -q '^FUSIONPANEL_ADMIN_TOKEN=' "$TEST_DIR/config/fusionpanel-agent.env"
[[ "$(cat "$TEST_DIR/state/.version")" == '0.2.0-test' ]]
grep -Fxq 'previous_version=0.1.0' "$TEST_DIR/backup/state-compatibility"
grep -Fxq 'target_version=0.2.0-test' "$TEST_DIR/backup/state-compatibility"
grep -Fxq 'incompatible_schema_migration=0' "$TEST_DIR/backup/state-compatibility"
[[ "$(stat -c '%a' "$TEST_DIR/config/fusionpanel.env")" == '600' ]]
grep -Fxq "root:root $TEST_DIR/config/fusionpanel.env" "$TEST_DIR/logs/chown.log"
grep -Fxq "root:root $TEST_DIR/config/fusionpanel-agent.env" "$TEST_DIR/logs/chown.log"
grep -Fxq -- '-sSLf http://127.0.0.1:9090/v1/health' "$TEST_DIR/logs/curl.log"

sh "$ROOT_DIR/packaging/rollback.sh"
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.1.0' ]]
[[ "$("$TEST_DIR/bin/fusionpanel-api")" == 'old-fusionpanel-api' ]]
[[ "$(cat "$TEST_DIR/versions/current/bin/frontend/dist/index.html")" == '<html>old UI</html>' ]]
[[ "$(cat "$TEST_DIR/state/.version")" == '0.1.0' ]]
grep -Fxq 'FUSIONPANEL_FRONTEND_DIR=/old/ui' "$TEST_DIR/config/fusionpanel.env"
grep -Fxq 'FUSIONPANEL_NODE_TOKEN=remote-node-secret' "$TEST_DIR/config/fusionpanel-agent.env"
[[ "$(cat "$TEST_DIR/systemd/fusionpanel.service")" == 'old api unit' ]]
[[ "$(cat "$TEST_DIR/systemd/fusionpanel-agent.service")" == 'old agent unit' ]]
[[ ! -e "$TEST_DIR/systemd/fusionpanel-nginx-helper.service" ]]
[[ ! -e "$TEST_DIR/bin/fusionpanel-nginx-helper" && ! -L "$TEST_DIR/bin/fusionpanel-nginx-helper" ]]
[[ "$(cat "$TEST_DIR/nginx-state")" == 'keep nginx state across rollback' ]]

# Reinstalling the same version key must retain enough state for rollback too.
: > "$TEST_DIR/logs/curl.log"
FUSIONPANEL_HEALTH_URL=http://127.0.0.1:9191/v1/health \
    sh "$ROOT_DIR/packaging/update.sh" 0.1.0
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.1.0' ]]
[[ "$("$TEST_DIR/bin/fusionpanel-api")" == 'new-fusionpanel-api' ]]
[[ "$(cat "$TEST_DIR/versions/current/bin/frontend/dist/index.html")" == '<html>new UI</html>' ]]
grep -Fxq -- '-sSLf http://127.0.0.1:9191/v1/health' "$TEST_DIR/logs/curl.log"
sh "$ROOT_DIR/packaging/rollback.sh"
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.1.0' ]]
[[ "$("$TEST_DIR/bin/fusionpanel-api")" == 'old-fusionpanel-api' ]]
[[ "$(cat "$TEST_DIR/versions/current/bin/frontend/dist/index.html")" == '<html>old UI</html>' ]]

# Any deterministic error after stopping services must trigger one automatic rollback.
: > "$TEST_DIR/logs/curl.log"
: > "$TEST_DIR/logs/systemctl.log"
set +e
FUSIONPANEL_TEST_FAIL_AFTER_STOP=1 sh "$ROOT_DIR/packaging/update.sh" 0.2.0-test
update_status=$?
set -e
if [[ "$update_status" -ne 73 ]]; then
    echo "FAIL: expected synthetic post-stop failure status 73, got $update_status" >&2
    exit 1
fi
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.1.0' ]]
[[ "$("$TEST_DIR/bin/fusionpanel-api")" == 'old-fusionpanel-api' ]]
[[ "$(cat "$TEST_DIR/versions/current/bin/frontend/dist/index.html")" == '<html>old UI</html>' ]]
[[ "$(cat "$TEST_DIR/state/.version")" == '0.1.0' ]]
grep -Fxq 'FUSIONPANEL_LISTEN=127.0.0.1:9090' "$TEST_DIR/config/fusionpanel.env"
grep -Fxq 'start fusionpanel.service' "$TEST_DIR/logs/systemctl.log"
grep -Fxq 'start fusionpanel-agent.service' "$TEST_DIR/logs/systemctl.log"

# A failed helper start must fail the update and restore the old release.
: > "$TEST_DIR/logs/systemctl.log"
set +e
helper_start_output=$(FUSIONPANEL_TEST_SYSTEMCTL_FAIL='start fusionpanel-nginx-helper.service' \
    sh "$ROOT_DIR/packaging/update.sh" 0.2.0-test 2>&1)
helper_start_status=$?
set -e
if [[ "$helper_start_status" -eq 0 ]]; then
    echo 'FAIL: updater reported success after the privileged helper failed to start.' >&2
    exit 1
fi
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.1.0' ]]
case "$helper_start_output" in
    *"systemd could not start fusionpanel-nginx-helper.service"*) ;;
    *)
        echo 'FAIL: updater did not report the helper startup failure.' >&2
        printf '%s\n' "$helper_start_output" >&2
        exit 1
        ;;
esac
if [[ "$(grep -c '^start fusionpanel-nginx-helper.service$' "$TEST_DIR/logs/systemctl.log")" -ne 1 ]]; then
    echo 'FAIL: helper service startup was not attempted exactly once.' >&2
    exit 1
fi

# Upgrade the exact legacy local-agent file emitted by the prior installer.
printf 'FUSIONPANEL_AGENT_ENDPOINT=http://127.0.0.1:9090\n' > "$TEST_DIR/config/fusionpanel-agent.env"
sh "$ROOT_DIR/packaging/update.sh" 0.2.0-legacy-test
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.2.0-legacy-test' ]]
grep -Fxq 'FUSIONPANEL_AGENT_MODE=http' "$TEST_DIR/config/fusionpanel-agent.env"
grep -Fxq 'FUSIONPANEL_API_URL=http://127.0.0.1:9090' "$TEST_DIR/config/fusionpanel-agent.env"
grep -Fxq 'FUSIONPANEL_ADMIN_TOKEN=old-admin-secret' "$TEST_DIR/config/fusionpanel-agent.env"
[[ "$(grep -c '^FUSIONPANEL_NODE_NAME=' "$TEST_DIR/config/fusionpanel-agent.env")" -eq 1 ]]
! grep -q '^FUSIONPANEL_AGENT_ENDPOINT=' "$TEST_DIR/config/fusionpanel-agent.env"
[[ "$(stat -c '%a' "$TEST_DIR/config/fusionpanel-agent.env")" == '600' ]]

# A legacy endpoint that disagrees with the local API listener fails before
# stopping services or changing the active release, without exposing tokens.
: > "$TEST_DIR/logs/systemctl.log"
printf 'FUSIONPANEL_AGENT_ENDPOINT=http://127.0.0.1:9191\n' > "$TEST_DIR/config/fusionpanel-agent.env"
set +e
migration_output=$(sh "$ROOT_DIR/packaging/update.sh" 0.3.0-legacy-mismatch 2>&1)
migration_status=$?
set -e
if [[ "$migration_status" -eq 0 ]]; then
    echo 'FAIL: inconsistent legacy API endpoint was accepted' >&2
    exit 1
fi
case "$migration_output" in
    *old-admin-secret*)
        echo 'FAIL: legacy migration exposed the API admin token' >&2
        exit 1
        ;;
esac
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.2.0-legacy-test' ]]
grep -Fxq 'FUSIONPANEL_AGENT_ENDPOINT=http://127.0.0.1:9191' "$TEST_DIR/config/fusionpanel-agent.env"
! grep -q '^stop fusionpanel.service fusionpanel-agent.service$' "$TEST_DIR/logs/systemctl.log"

# Restore valid agent config for remaining tests.
printf 'FUSIONPANEL_AGENT_MODE=http\nFUSIONPANEL_API_URL=http://127.0.0.1:9090\nFUSIONPANEL_ADMIN_TOKEN=old-admin-secret\nFUSIONPANEL_NODE_NAME=test-node\n' > "$TEST_DIR/config/fusionpanel-agent.env"

# Preflight: non-adjacent release transition must fail before stopping services.
: > "$TEST_DIR/logs/systemctl.log"
set +e
nonadj_output=$(sh "$ROOT_DIR/packaging/update.sh" 0.4.0-nonadj 2>&1)
nonadj_status=$?
set -e
if [[ "$nonadj_status" -eq 0 ]]; then
    echo 'FAIL: non-adjacent release transition was accepted' >&2
    exit 1
fi
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.2.0-legacy-test' ]]
! grep -q '^stop ' "$TEST_DIR/logs/systemctl.log"

# Preflight: FUSIONPANEL_MIN_FREE_KB disk space guard must fail before stopping services.
: > "$TEST_DIR/logs/systemctl.log"
set +e
disk_output=$(FUSIONPANEL_MIN_FREE_KB=999999999999 sh "$ROOT_DIR/packaging/update.sh" 0.3.0-diskcheck 2>&1)
disk_status=$?
set -e
if [[ "$disk_status" -eq 0 ]]; then
    echo 'FAIL: update proceeded despite insufficient free disk space' >&2
    exit 1
fi
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.2.0-legacy-test' ]]
! grep -q '^stop ' "$TEST_DIR/logs/systemctl.log"

# Simulated agent health failure must trigger automatic rollback.
: > "$TEST_DIR/logs/curl.log"
set +e
agent_fail_output=$(FUSIONPANEL_HEALTH_TIMEOUT=1 \
    FUSIONPANEL_AGENT_HEALTH_URL=http://127.0.0.1:9192/health \
    FUSIONPANEL_TEST_FAIL_URL=http://127.0.0.1:9192/health \
    sh "$ROOT_DIR/packaging/update.sh" 0.3.0-agent-fail 2>&1)
agent_fail_status=$?
set -e
if [[ "$agent_fail_status" -eq 0 ]]; then
    echo 'FAIL: updater succeeded despite agent health failure' >&2
    exit 1
fi
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.2.0-legacy-test' ]]
[[ "$(cat "$TEST_DIR/state/.version")" == '0.2.0-legacy-test' ]]
grep -Fxq -- '-sSLf http://127.0.0.1:9192/health' "$TEST_DIR/logs/curl.log"

# Repeat rollback must be idempotent and preserve credentials, config, and durable state.
printf 'important-db-state\n' > "$TEST_DIR/state/durable.db"
sh "$ROOT_DIR/packaging/update.sh" 0.3.0-repeat-test
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.3.0-repeat-test' ]]
sh "$ROOT_DIR/packaging/rollback.sh"
sh "$ROOT_DIR/packaging/rollback.sh"
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.2.0-legacy-test' ]]
[[ "$(cat "$TEST_DIR/state/.version")" == '0.2.0-legacy-test' ]]
grep -Fxq 'FUSIONPANEL_ADMIN_TOKEN=old-admin-secret' "$TEST_DIR/config/fusionpanel.env"
grep -Fxq 'FUSIONPANEL_ADMIN_TOKEN=old-admin-secret' "$TEST_DIR/config/fusionpanel-agent.env"
[[ "$(cat "$TEST_DIR/state/durable.db")" == 'important-db-state' ]]

# Incompatible schema migration must block automatic rewind unless explicitly overridden.
set +e
incompat_output=$(FUSIONPANEL_INCOMPATIBLE_SCHEMA_MIGRATION=1 \
    FUSIONPANEL_HEALTH_TIMEOUT=1 \
    FUSIONPANEL_TEST_FAIL_HEALTH=1 \
    sh "$ROOT_DIR/packaging/update.sh" 0.3.0-incompat 2>&1)
incompat_status=$?
set -e
if [[ "$incompat_status" -eq 0 ]]; then
    echo 'FAIL: updater succeeded despite health failure on incompatible migration' >&2
    exit 1
fi
[[ -f "$TEST_DIR/backup/incompatible-migration" ]]
grep -Fxq 'incompatible_schema_migration=1' "$TEST_DIR/backup/state-compatibility"
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.3.0-incompat' ]]
set +e
manual_rewind_output=$(sh "$ROOT_DIR/packaging/rollback.sh" 2>&1)
manual_rewind_status=$?
set -e
if [[ "$manual_rewind_status" -eq 0 ]]; then
    echo 'FAIL: rollback.sh allowed rewind across incompatible migration without override' >&2
    exit 1
fi
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.3.0-incompat' ]]
sh "$ROOT_DIR/packaging/rollback.sh" --force-rewind
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.2.0-legacy-test' ]]
[[ "$(cat "$TEST_DIR/state/.version")" == '0.2.0-legacy-test' ]]

echo 'Updater package-layout and rollback tests passed.'
