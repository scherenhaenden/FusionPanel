#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
TEST_DIR=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-package-layout.XXXXXX")
trap 'rm -rf -- "$TEST_DIR"' EXIT HUP INT TERM

mkdir -p "$TEST_DIR/bin" "$TEST_DIR/target" "$TEST_DIR/frontend-dist" "$TEST_DIR/output"
cat > "$TEST_DIR/bin/cargo" <<'SH'
#!/bin/sh
mkdir -p "$CARGO_TARGET_DIR/release"
for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper; do
    printf '#!/bin/sh\nprintf "%%s\\n" "%s"\n' "$binary" > "$CARGO_TARGET_DIR/release/$binary"
    chmod 0755 "$CARGO_TARGET_DIR/release/$binary"
done
SH
cat > "$TEST_DIR/bin/npm" <<'SH'
#!/bin/sh
case " $* " in
    *" run build "*)
        out_dir=""
        previous=""
        for arg in "$@"; do
            if [ "$previous" = "--outDir" ]; then out_dir="$arg"; fi
            previous="$arg"
        done
        [ -n "$out_dir" ] || { echo 'missing --outDir' >&2; exit 1; }
        mkdir -p "$out_dir/assets"
        printf '<!doctype html><title>fixture</title>\n' > "$out_dir/index.html"
        printf 'fixture asset\n' > "$out_dir/assets/app.js"
        ;;
    *" ci "*) ;;
    *) echo "unexpected npm invocation: $*" >&2; exit 1 ;;
esac
SH
chmod +x "$TEST_DIR/bin/cargo" "$TEST_DIR/bin/npm"

PATH="$TEST_DIR/bin:$PATH" \
CARGO_TARGET_DIR="$TEST_DIR/target" \
FUSIONPANEL_CARGO="$TEST_DIR/bin/cargo" \
FUSIONPANEL_NPM="$TEST_DIR/bin/npm" \
FUSIONPANEL_FRONTEND_DIST="$TEST_DIR/frontend-dist" \
    bash "$ROOT_DIR/packaging/build-package.sh" ci-package-layout "$TEST_DIR/output"

ARCHIVE="$TEST_DIR/output/fusionpanel-ci-package-layout-debian-x86_64.tar.gz"
[[ -f "$ARCHIVE" ]]
tar -tzf "$ARCHIVE" > "$TEST_DIR/archive-members.txt"
for member in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper \
    systemd/fusionpanel.service systemd/fusionpanel-agent.service systemd/fusionpanel-nginx-helper.service \
    packaging/nginx-include.sh frontend/dist/index.html frontend/dist/assets/app.js BUILD_INFO.txt; do
    if ! grep -Fxq "./$member" "$TEST_DIR/archive-members.txt"; then
        echo "FAIL: package archive is missing $member" >&2
        cat "$TEST_DIR/archive-members.txt" >&2
        exit 1
    fi
done

EXTRACTED_PAYLOAD="$TEST_DIR/extracted/bin"
mkdir -p "$EXTRACTED_PAYLOAD"
tar -xzf "$ARCHIVE" -C "$EXTRACTED_PAYLOAD"
root_mode=$(stat -c '%a' "$EXTRACTED_PAYLOAD")
if [[ "$root_mode" != 755 ]]; then
    echo "FAIL: extracted package root has mode $root_mode; expected 755" >&2
    exit 1
fi
for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper; do
    binary_mode=$(stat -c '%a' "$EXTRACTED_PAYLOAD/$binary")
    if [[ "$binary_mode" != 755 ]]; then
        echo "FAIL: extracted executable $binary has mode $binary_mode; expected 755" >&2
        exit 1
    fi
done

# Exercise the same functions used by install.sh and update.sh with isolated paths.
. "$ROOT_DIR/packaging/layout.sh"
VERSIONS="$TEST_DIR/versions"
BIN_DIR="$TEST_DIR/usr-local-bin"
FUSIONPANEL_INSTALLER_NO_RUN=1
. "$ROOT_DIR/packaging/install.sh"
unset FUSIONPANEL_INSTALLER_NO_RUN

mkdir -p "$TEST_DIR/group-bin"
cat > "$TEST_DIR/group-bin/getent" <<'SH'
#!/bin/sh
if [ "$1" = group ]; then
    if [ "$2" = fusionpanel-nginx ] \
        && { [ "${FUSIONPANEL_TEST_GROUP_PRESENT:-0}" = 1 ] || [ -f "$FUSIONPANEL_TEST_GROUP_STATE_FILE" ]; }; then
        printf 'fusionpanel-nginx:x:991:%s\n' "${FUSIONPANEL_TEST_GROUP_MEMBERS:-}"
        exit 0
    fi
    if [ "$2" = "${FUSIONPANEL_TEST_PRIVATE_GROUP:-}" ]; then
        printf '%s:x:%s:\n' "$2" "${FUSIONPANEL_TEST_PRIVATE_GID:-}"
        exit 0
    fi
fi
if [ "$1" = passwd ]; then
    if [ -n "${FUSIONPANEL_TEST_PRIMARY_GID:-}" ]; then
        printf 'primary-user:x:992:%s:Fixture primary group user:/nonexistent:/usr/sbin/nologin\n' \
            "$FUSIONPANEL_TEST_PRIMARY_GID"
    fi
    exit 0
fi
exit 2
SH
cat > "$TEST_DIR/group-bin/groupadd" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$FUSIONPANEL_TEST_GROUPADD_LOG"
: > "$FUSIONPANEL_TEST_GROUP_STATE_FILE"
SH
chmod +x "$TEST_DIR/group-bin/getent" "$TEST_DIR/group-bin/groupadd"
export PATH="$TEST_DIR/group-bin:$PATH"
export FUSIONPANEL_TEST_GROUPADD_LOG="$TEST_DIR/groupadd.log"
export FUSIONPANEL_TEST_GROUP_STATE_FILE="$TEST_DIR/fusionpanel-nginx.group"
export FUSIONPANEL_TEST_PRIVATE_GROUP="$(id -gn)"
export FUSIONPANEL_TEST_PRIVATE_GID="$(id -g)"
fusionpanel_ensure_nginx_helper_group
grep -Fxq -- '--system fusionpanel-nginx' "$FUSIONPANEL_TEST_GROUPADD_LOG"
if FUSIONPANEL_TEST_GROUP_PRESENT=1 FUSIONPANEL_TEST_GROUP_MEMBERS=fusionpanel \
    fusionpanel_ensure_nginx_helper_group 2>/dev/null; then
    echo 'FAIL: helper group with existing members was accepted.' >&2
    exit 1
fi
if FUSIONPANEL_TEST_GROUP_PRESENT=1 FUSIONPANEL_TEST_PRIMARY_GID=991 \
    fusionpanel_ensure_nginx_helper_group 2>/dev/null; then
    echo 'FAIL: helper group with an existing primary-GID account was accepted.' >&2
    exit 1
fi

# Runtime directories are created as private leaf directories. Existing paths
# and data are only inspected; an incompatible directory is left untouched.
PRIVATE_PARENT="$TEST_DIR/private-runtime"
PRIVATE_DIR="$PRIVATE_PARENT/service-data"
mkdir -m 0700 "$PRIVATE_PARENT"
printf 'preserve this file\n' > "$PRIVATE_PARENT/existing-data"
fusionpanel_ensure_private_directory "$PRIVATE_DIR" "$(id -un)" "$(id -gn)" "$(id -u)"
[[ "$(stat -c '%u:%g:%a' "$PRIVATE_DIR")" == "$(id -u):$(id -g):700" ]]
[[ "$(cat "$PRIVATE_PARENT/existing-data")" == 'preserve this file' ]]
printf 'existing runtime data\n' > "$PRIVATE_DIR/db.sqlite"
PRIVATE_FILE_HASH=$(sha256sum "$PRIVATE_DIR/db.sqlite" | awk '{ print $1 }')
fusionpanel_ensure_private_directory "$PRIVATE_DIR" "$(id -un)" "$(id -gn)" "$(id -u)"
[[ "$(sha256sum "$PRIVATE_DIR/db.sqlite" | awk '{ print $1 }')" == "$PRIVATE_FILE_HASH" ]]
[[ "$(stat -c '%a' "$PRIVATE_DIR")" == 700 ]]
chmod 0755 "$PRIVATE_DIR"
if fusionpanel_ensure_private_directory "$PRIVATE_DIR" "$(id -un)" "$(id -gn)" "$(id -u)" 2>/dev/null; then
    echo 'FAIL: insecure existing runtime directory was accepted.' >&2
    exit 1
fi
[[ "$(stat -c '%a' "$PRIVATE_DIR")" == 755 ]]
[[ "$(sha256sum "$PRIVATE_DIR/db.sqlite" | awk '{ print $1 }')" == "$PRIVATE_FILE_HASH" ]]
PRIVATE_SYMLINK="$PRIVATE_PARENT/service-link"
ln -s "$PRIVATE_DIR" "$PRIVATE_SYMLINK"
if fusionpanel_ensure_private_directory "$PRIVATE_SYMLINK" "$(id -un)" "$(id -gn)" "$(id -u)" 2>/dev/null; then
    echo 'FAIL: symlinked runtime directory was accepted.' >&2
    exit 1
fi
[[ -L "$PRIVATE_SYMLINK" ]]
[[ "$(sha256sum "$PRIVATE_DIR/db.sqlite" | awk '{ print $1 }')" == "$PRIVATE_FILE_HASH" ]]
MISSING_PARENT="$TEST_DIR/missing-parent"
if fusionpanel_ensure_private_directory "$MISSING_PARENT/child" "$(id -un)" "$(id -gn)" "$(id -u)" 2>/dev/null; then
    echo 'FAIL: missing parent path was recursively created.' >&2
    exit 1
fi
[[ ! -e "$MISSING_PARENT" ]]

SYSTEMD_DEST="$TEST_DIR/install-systemd"
fusionpanel_install_systemd_units "$EXTRACTED_PAYLOAD" "$SYSTEMD_DEST" "$TEST_DIR/install-config"
grep -Fxq 'SupplementaryGroups=fusionpanel-nginx' "$SYSTEMD_DEST/fusionpanel-agent.service"
! grep -q '^SupplementaryGroups=fusionpanel-nginx$' "$SYSTEMD_DEST/fusionpanel.service"
grep -Fxq 'Group=fusionpanel-nginx' "$SYSTEMD_DEST/fusionpanel-nginx-helper.service"
! grep -q '^EnvironmentFile=' "$SYSTEMD_DEST/fusionpanel-nginx-helper.service"
grep -Fxq 'ReadWritePaths=-/etc/nginx/fusionpanel' "$SYSTEMD_DEST/fusionpanel-nginx-helper.service"

write_runtime_env_files "$TEST_DIR/installer-config" 8181 synthetic-installer-token fixture-node \
    "$VERSIONS/current/bin/frontend/dist"
grep -Fxq "FUSIONPANEL_FRONTEND_DIR=$VERSIONS/current/bin/frontend/dist" \
    "$TEST_DIR/installer-config/fusionpanel.env"
mkdir -p "$TEST_DIR/release-stage/bin/frontend/dist" "$TEST_DIR/release-stage/config"
for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper; do
    cp "$TEST_DIR/target/release/$binary" "$TEST_DIR/release-stage/bin/$binary"
done
cp -a "$TEST_DIR/extracted/bin/systemd" "$TEST_DIR/release-stage/bin/systemd"
cp -a "$TEST_DIR/extracted/bin/packaging" "$TEST_DIR/release-stage/bin/packaging"
cp "$TEST_DIR/frontend-dist/index.html" "$TEST_DIR/release-stage/bin/frontend/dist/index.html"
: > "$TEST_DIR/release-stage/.version"
fusionpanel_publish_version_dir "$TEST_DIR/release-stage" "$VERSIONS/0.2.0-test"
fusionpanel_switch_current "$VERSIONS" 0.2.0-test
fusionpanel_link_runtime_binaries "$BIN_DIR" "$VERSIONS"

for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper; do
    [[ -L "$BIN_DIR/$binary" ]]
    [[ -x "$BIN_DIR/$binary" ]]
done
[[ -f "$VERSIONS/current/bin/frontend/dist/index.html" ]]

mkdir -p "$TEST_DIR/invalid-payload/frontend/dist"
for binary in fusionpanel-api fusionpanel-cli; do
    cp "$TEST_DIR/target/release/$binary" "$TEST_DIR/invalid-payload/$binary"
done
: > "$TEST_DIR/invalid-payload/frontend/dist/index.html"
if fusionpanel_validate_payload "$TEST_DIR/invalid-payload" 2>/dev/null; then
    echo 'FAIL: payload validator accepted a package without fusionpanel-agent' >&2
    exit 1
fi

echo 'Package artifact and version-layout tests passed.'
