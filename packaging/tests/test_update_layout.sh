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
printf '<html>old UI</html>\n' > "$TEST_DIR/versions/0.1.0/bin/frontend/dist/index.html"
printf '<html>new UI</html>\n' > "$TEST_DIR/payload/frontend/dist/index.html"
printf 'FUSIONPANEL_ADMIN_TOKEN=old-admin-secret\nFUSIONPANEL_FRONTEND_DIR=/old/ui\n' > "$TEST_DIR/config/fusionpanel.env"
printf 'FUSIONPANEL_AGENT_MODE=http\nFUSIONPANEL_API_URL=https://control.example.test\nFUSIONPANEL_NODE_ID=00000000-0000-4000-8000-000000000001\nFUSIONPANEL_NODE_TOKEN=remote-node-secret\n' > "$TEST_DIR/config/fusionpanel-agent.env"
cp -a "$TEST_DIR/config/." "$TEST_DIR/versions/0.1.0/config/"
printf '0.1.0\n' > "$TEST_DIR/versions/0.1.0/.version"
ln -s 0.1.0 "$TEST_DIR/versions/current"
for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli; do
    ln -s "$TEST_DIR/versions/current/bin/$binary" "$TEST_DIR/bin/$binary"
done
printf '0.1.0\n' > "$TEST_DIR/state/.version"
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
exit 0
SH
cat > "$TEST_DIR/mock-bin/systemctl" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$FUSIONPANEL_TEST_SYSTEMCTL_LOG"
SH
cat > "$TEST_DIR/mock-bin/chown" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$FUSIONPANEL_TEST_CHOWN_LOG"
SH
chmod +x "$TEST_DIR/mock-bin/curl" "$TEST_DIR/mock-bin/systemctl" "$TEST_DIR/mock-bin/chown"

export PATH="$TEST_DIR/mock-bin:$PATH"
export FUSIONPANEL_TEST_ARCHIVE="$TEST_DIR/package.tar.gz"
export FUSIONPANEL_TEST_SYSTEMCTL_LOG="$TEST_DIR/logs/systemctl.log"
export FUSIONPANEL_TEST_CHOWN_LOG="$TEST_DIR/logs/chown.log"
export FUSIONPANEL_TEST_BACKUP_DIR="$TEST_DIR/backup"
export FUSIONPANEL_TEST_CONFIG_DIR="$TEST_DIR/config"
export FUSIONPANEL_TEST_BIN_DIR="$TEST_DIR/bin"
export FUSIONPANEL_TEST_VERSION_LIB_DIR="$TEST_DIR/versions"
export FUSIONPANEL_TEST_STATE_DIR="$TEST_DIR/state"
export FUSIONPANEL_TEST_OS=debian
export FUSIONPANEL_TEST_ARCH=x86_64
export FUSIONPANEL_MOCK_DOWNLOAD_URL=synthetic-package

sh "$ROOT_DIR/packaging/update.sh" 0.2.0-test

[[ "$(readlink "$TEST_DIR/versions/current")" == '0.2.0-test' ]]
[[ -d "$TEST_DIR/versions/current/data" ]]
for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli; do
    [[ -L "$TEST_DIR/bin/$binary" ]]
    [[ "$("$TEST_DIR/bin/$binary")" == "new-$binary" ]]
    [[ -x "$TEST_DIR/versions/current/bin/$binary" ]]
done
[[ "$(cat "$TEST_DIR/versions/current/bin/frontend/dist/index.html")" == '<html>new UI</html>' ]]
grep -Fxq 'FUSIONPANEL_ADMIN_TOKEN=old-admin-secret' "$TEST_DIR/config/fusionpanel.env"
grep -Fxq "FUSIONPANEL_FRONTEND_DIR=$TEST_DIR/versions/current/bin/frontend/dist" "$TEST_DIR/config/fusionpanel.env"
[[ "$(grep -c '^FUSIONPANEL_FRONTEND_DIR=' "$TEST_DIR/config/fusionpanel.env")" -eq 1 ]]
grep -Fxq 'FUSIONPANEL_API_URL=https://control.example.test' "$TEST_DIR/config/fusionpanel-agent.env"
grep -Fxq 'FUSIONPANEL_NODE_TOKEN=remote-node-secret' "$TEST_DIR/config/fusionpanel-agent.env"
! grep -q '^FUSIONPANEL_ADMIN_TOKEN=' "$TEST_DIR/config/fusionpanel-agent.env"
[[ "$(cat "$TEST_DIR/state/.version")" == '0.2.0-test' ]]
[[ "$(stat -c '%a' "$TEST_DIR/config/fusionpanel.env")" == '600' ]]
grep -Fxq "root:root $TEST_DIR/config/fusionpanel.env" "$TEST_DIR/logs/chown.log"
grep -Fxq "root:root $TEST_DIR/config/fusionpanel-agent.env" "$TEST_DIR/logs/chown.log"

sh "$ROOT_DIR/packaging/rollback.sh"
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.1.0' ]]
[[ "$("$TEST_DIR/bin/fusionpanel-api")" == 'old-fusionpanel-api' ]]
[[ "$(cat "$TEST_DIR/versions/current/bin/frontend/dist/index.html")" == '<html>old UI</html>' ]]
[[ "$(cat "$TEST_DIR/state/.version")" == '0.1.0' ]]
grep -Fxq 'FUSIONPANEL_FRONTEND_DIR=/old/ui' "$TEST_DIR/config/fusionpanel.env"
grep -Fxq 'FUSIONPANEL_NODE_TOKEN=remote-node-secret' "$TEST_DIR/config/fusionpanel-agent.env"

# Reinstalling the same version key must retain enough state for rollback too.
sh "$ROOT_DIR/packaging/update.sh" 0.1.0
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.1.0' ]]
[[ "$("$TEST_DIR/bin/fusionpanel-api")" == 'new-fusionpanel-api' ]]
[[ "$(cat "$TEST_DIR/versions/current/bin/frontend/dist/index.html")" == '<html>new UI</html>' ]]
sh "$ROOT_DIR/packaging/rollback.sh"
[[ "$(readlink "$TEST_DIR/versions/current")" == '0.1.0' ]]
[[ "$("$TEST_DIR/bin/fusionpanel-api")" == 'old-fusionpanel-api' ]]
[[ "$(cat "$TEST_DIR/versions/current/bin/frontend/dist/index.html")" == '<html>old UI</html>' ]]

echo 'Updater package-layout and rollback tests passed.'
