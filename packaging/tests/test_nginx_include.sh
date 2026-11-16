#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
TEST_DIR=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-nginx-include.XXXXXX")
trap 'rm -rf -- "$TEST_DIR"' EXIT HUP INT TERM

mkdir -p "$TEST_DIR/bin" "$TEST_DIR/nginx/conf.d"
chmod 0755 "$TEST_DIR/nginx" "$TEST_DIR/nginx/conf.d"
cat > "$TEST_DIR/bin/nginx" <<'SH'
#!/bin/sh
case "$1" in
    -T)
        [ "${FUSIONPANEL_TEST_NGINX_T_FAIL:-0}" != 1 ] || exit 1
        if [ "${FUSIONPANEL_TEST_NGINX_NO_CONFD_INCLUDE:-0}" != 1 ]; then
            printf 'include /etc/nginx/conf.d/*.conf;\n'
        fi
        if [ -f "${FUSIONPANEL_TEST_FRAGMENT:-}" ]; then
            cat "$FUSIONPANEL_TEST_FRAGMENT"
        fi
        ;;
    -t)
        [ "${FUSIONPANEL_TEST_NGINX_TEST_FAIL:-0}" != 1 ]
        ;;
    *) exit 2 ;;
esac
SH
chmod 0755 "$TEST_DIR/bin/nginx"

CONF="$TEST_DIR/nginx/nginx.conf"
CONF_D="$TEST_DIR/nginx/conf.d"
MANAGED_ROOT="$TEST_DIR/nginx/fusionpanel"
TARGET="$CONF_D/fusionpanel.conf"
printf 'worker_processes 1;\n' > "$CONF"
CONF_HASH=$(sha256sum "$CONF" | awk '{ print $1 }')
export FUSIONPANEL_TEST_FRAGMENT="$TARGET"
EXPECTED_UID=$(id -u)

if FUSIONPANEL_TEST_NGINX_NO_CONFD_INCLUDE=1 \
    sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"; then
    echo 'FAIL: setup accepted an unproven conf.d include.' >&2
    exit 1
fi
[[ ! -e "$TARGET" && ! -e "$MANAGED_ROOT" ]]

if sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"; then
    :
else
    echo 'FAIL: setup rejected the proven compatible conf.d include.' >&2
    exit 1
fi
grep -Fxq '# Managed by FusionPanel installer; do not edit.' "$TARGET"
grep -Fxq "include $MANAGED_ROOT/sites-enabled/*.conf;" "$TARGET"
[[ "$(stat -c '%a' "$TARGET")" == 644 ]]
[[ "$(stat -c '%u:%a' "$MANAGED_ROOT")" == "$EXPECTED_UID:700" ]]
[[ ! -e "$MANAGED_ROOT/sites-enabled" && ! -e "$MANAGED_ROOT/sites-available" ]]
TARGET_HASH=$(sha256sum "$TARGET" | awk '{ print $1 }')
printf 'preserve existing managed state\n' > "$MANAGED_ROOT/state.marker"
chmod 0750 "$MANAGED_ROOT"
MANAGED_MARKER_HASH=$(sha256sum "$MANAGED_ROOT/state.marker" | awk '{ print $1 }')
sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"
[[ "$(sha256sum "$TARGET" | awk '{ print $1 }')" == "$TARGET_HASH" ]]
[[ "$(stat -c '%a' "$MANAGED_ROOT")" == 750 ]]
[[ "$(sha256sum "$MANAGED_ROOT/state.marker" | awk '{ print $1 }')" == "$MANAGED_MARKER_HASH" ]]

chmod 0666 "$TARGET"
if sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"; then
    echo 'FAIL: setup accepted an active group/world-writable FusionPanel fragment.' >&2
    exit 1
fi
[[ "$(stat -c '%a' "$TARGET")" == 666 ]]
grep -Fxq "include $MANAGED_ROOT/sites-enabled/*.conf;" "$TARGET"
chmod 0644 "$TARGET"

if sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$((EXPECTED_UID + 1))"; then
    echo 'FAIL: setup accepted a managed root with a different expected owner.' >&2
    exit 1
fi
[[ "$(stat -c '%a' "$MANAGED_ROOT")" == 750 ]]
[[ "$(sha256sum "$MANAGED_ROOT/state.marker" | awk '{ print $1 }')" == "$MANAGED_MARKER_HASH" ]]

MISSING_PARENT_ROOT="$TEST_DIR/nginx/missing-parent/fusionpanel"
if sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MISSING_PARENT_ROOT" "$EXPECTED_UID"; then
    echo 'FAIL: setup recursively created a missing managed parent.' >&2
    exit 1
fi
[[ ! -e "$TEST_DIR/nginx/missing-parent" ]]

rm -f "$TARGET"
mv "$MANAGED_ROOT" "$TEST_DIR/nginx/fusionpanel-real"
ln -s "$TEST_DIR/nginx/fusionpanel-real" "$MANAGED_ROOT"
if sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"; then
    echo 'FAIL: setup accepted a symlinked managed root.' >&2
    exit 1
fi
[[ -L "$MANAGED_ROOT" && ! -e "$TARGET" ]]
rm "$MANAGED_ROOT"
mv "$TEST_DIR/nginx/fusionpanel-real" "$MANAGED_ROOT"
chmod 0777 "$MANAGED_ROOT"
if sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"; then
    echo 'FAIL: setup accepted a group/world-writable managed root.' >&2
    exit 1
fi
chmod 0755 "$MANAGED_ROOT"

# Never activate the wildcard include over an unsafe pre-existing subtree.
mkdir -p "$TEST_DIR/nginx/outside"
ln -s "$TEST_DIR/nginx/outside" "$MANAGED_ROOT/sites-enabled"
if sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"; then
    echo 'FAIL: setup accepted a symlinked sites-enabled directory.' >&2
    exit 1
fi
[[ ! -e "$TARGET" ]]
rm "$MANAGED_ROOT/sites-enabled"

mkdir -m 0777 "$MANAGED_ROOT/sites-enabled"
if sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"; then
    echo 'FAIL: setup accepted a group/world-writable sites-enabled directory.' >&2
    exit 1
fi
[[ ! -e "$TARGET" ]]
chmod 0755 "$MANAGED_ROOT/sites-enabled"
printf 'unsafe writable Nginx config\n' > "$MANAGED_ROOT/sites-enabled/untrusted.conf"
chmod 0666 "$MANAGED_ROOT/sites-enabled/untrusted.conf"
if sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"; then
    echo 'FAIL: setup accepted a group/world-writable site config.' >&2
    exit 1
fi
[[ ! -e "$TARGET" ]]
rm -rf "$MANAGED_ROOT/sites-enabled"

mkdir -m 0755 "$MANAGED_ROOT/sites-enabled"
printf 'foreign-owned Nginx config\n' > "$MANAGED_ROOT/sites-enabled/untrusted.conf"
mkdir -p "$TEST_DIR/fake-stat"
cat > "$TEST_DIR/fake-stat/stat" <<'SH'
#!/bin/sh
if [ "$#" -eq 3 ] && [ "$1" = '-c' ] && [ "$2" = '%u %a' ] \
    && [ "$3" = "$FUSIONPANEL_TEST_FOREIGN_FILE" ]; then
    printf '65534 644\n'
else
    exec /usr/bin/stat "$@"
fi
SH
chmod 0755 "$TEST_DIR/fake-stat/stat"
if PATH="$TEST_DIR/fake-stat:$PATH" FUSIONPANEL_TEST_FOREIGN_FILE="$MANAGED_ROOT/sites-enabled/untrusted.conf" \
    sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"; then
    echo 'FAIL: setup accepted a foreign-owned site config.' >&2
    exit 1
fi
[[ ! -e "$TARGET" ]]
rm -rf "$MANAGED_ROOT/sites-enabled"

printf 'user-owned config\n' > "$TARGET"
if sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"; then
    echo 'FAIL: setup overwrote an existing regular config fragment.' >&2
    exit 1
fi
[[ "$(cat "$TARGET")" == 'user-owned config' ]]

rm -f "$TARGET"
printf 'symlink target\n' > "$TEST_DIR/nginx/user.conf"
ln -s "$TEST_DIR/nginx/user.conf" "$TARGET"
if sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID"; then
    echo 'FAIL: setup accepted a symlink at the managed fragment path.' >&2
    exit 1
fi
[[ -L "$TARGET" ]]
[[ "$(cat "$TEST_DIR/nginx/user.conf")" == 'symlink target' ]]

rm -f "$TARGET"
FUSIONPANEL_TEST_NGINX_TEST_FAIL=1 \
    sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID" && {
        echo 'FAIL: setup accepted an Nginx test failure.' >&2
        exit 1
    }
[[ ! -e "$TARGET" ]]

FUSIONPANEL_TEST_NGINX_T_FAIL=1 \
    sh "$ROOT_DIR/packaging/nginx-include.sh" "$TEST_DIR/bin/nginx" "$CONF" "$CONF_D" "$MANAGED_ROOT" "$EXPECTED_UID" && {
        echo 'FAIL: setup accepted an unreadable active Nginx configuration.' >&2
        exit 1
    }
[[ ! -e "$TARGET" ]]
[[ "$(sha256sum "$CONF" | awk '{ print $1 }')" == "$CONF_HASH" ]]

echo 'Nginx include safety tests passed.'
