#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
TEST_DIR=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-package-layout.XXXXXX")
trap 'rm -rf -- "$TEST_DIR"' EXIT HUP INT TERM

mkdir -p "$TEST_DIR/bin" "$TEST_DIR/target" "$TEST_DIR/frontend-dist" "$TEST_DIR/output"
cat > "$TEST_DIR/bin/cargo" <<'SH'
#!/bin/sh
mkdir -p "$CARGO_TARGET_DIR/release"
for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli; do
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
for member in fusionpanel-api fusionpanel-agent fusionpanel-cli frontend/dist/index.html frontend/dist/assets/app.js BUILD_INFO.txt; do
    if ! grep -Fxq "./$member" "$TEST_DIR/archive-members.txt"; then
        echo "FAIL: package archive is missing $member" >&2
        cat "$TEST_DIR/archive-members.txt" >&2
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
write_runtime_env_files "$TEST_DIR/installer-config" 8181 synthetic-installer-token fixture-node \
    "$VERSIONS/current/bin/frontend/dist"
grep -Fxq "FUSIONPANEL_FRONTEND_DIR=$VERSIONS/current/bin/frontend/dist" \
    "$TEST_DIR/installer-config/fusionpanel.env"
mkdir -p "$TEST_DIR/release-stage/bin/frontend/dist" "$TEST_DIR/release-stage/config"
for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli; do
    cp "$TEST_DIR/target/release/$binary" "$TEST_DIR/release-stage/bin/$binary"
done
cp "$TEST_DIR/frontend-dist/index.html" "$TEST_DIR/release-stage/bin/frontend/dist/index.html"
: > "$TEST_DIR/release-stage/.version"
fusionpanel_publish_version_dir "$TEST_DIR/release-stage" "$VERSIONS/0.2.0-test"
fusionpanel_switch_current "$VERSIONS" 0.2.0-test
fusionpanel_link_runtime_binaries "$BIN_DIR" "$VERSIONS"

for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli; do
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
