#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
TEST_DIR=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-deb-layout.XXXXXX")
trap 'rm -rf -- "$TEST_DIR"' EXIT HUP INT TERM

command -v dpkg-deb >/dev/null 2>&1 || {
    echo "BLOCKED: dpkg-deb is required for test_deb_layout.sh" >&2
    exit 2
}

# 1. Validate maintainer script and build script syntax without executing them.
for script in \
    "$ROOT_DIR/packaging/deb/postinst" \
    "$ROOT_DIR/packaging/deb/prerm" \
    "$ROOT_DIR/packaging/deb/postrm" \
    "$ROOT_DIR/packaging/layout.sh"; do
    sh -n "$script"
    bash -n "$script"
done
bash -n "$ROOT_DIR/packaging/build-deb.sh"
bash -n "$ROOT_DIR/packaging/build-package.sh"

# 2. Verify Cargo-to-Debian version normalization.
# shellcheck source=packaging/layout.sh
. "$ROOT_DIR/packaging/layout.sh"
[[ "$(fusionpanel_normalize_deb_version '0.12.0-dev.1')" == "0.12.0~dev.1-1" ]]
[[ "$(fusionpanel_normalize_deb_version '0.10.0-dev.1')" == "0.10.0~dev.1-1" ]]
[[ "$(fusionpanel_normalize_deb_version '1.2.3')" == "1.2.3-1" ]]

# 3. Stage fixture binaries, systemd units, nginx-include.sh, and frontend dist.
create_fixture_payload() {
    local dest="$1"
    mkdir -p "$dest/systemd" "$dest/packaging" "$dest/frontend/dist/assets"
    for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper; do
        printf '#!/bin/sh\nprintf "%%s\\n" "%s"\n' "$binary" > "$dest/$binary"
        chmod 0755 "$dest/$binary"
    done
    for unit in fusionpanel.service fusionpanel-agent.service fusionpanel-nginx-helper.service; do
        cp "$ROOT_DIR/packaging/systemd/$unit" "$dest/systemd/$unit"
        chmod 0644 "$dest/systemd/$unit"
    done
    cp "$ROOT_DIR/packaging/nginx-include.sh" "$dest/packaging/nginx-include.sh"
    chmod 0755 "$dest/packaging/nginx-include.sh"
    printf '<!doctype html><title>deb-fixture</title>\n' > "$dest/frontend/dist/index.html"
    printf 'console.log("fixture");\n' > "$dest/frontend/dist/assets/app.js"
}

PAYLOAD_A="$TEST_DIR/payload-a"
PAYLOAD_B="$TEST_DIR/payload-b"
OUT_A="$TEST_DIR/out-a"
OUT_B="$TEST_DIR/out-b"
create_fixture_payload "$PAYLOAD_A"
create_fixture_payload "$PAYLOAD_B"

FIXED_EPOCH="1730000000"
FIXED_SHA="0123456789abcdef0123456789abcdef01234567"
FIXED_VER="0.12.0~dev.1-1"

bash "$ROOT_DIR/packaging/build-deb.sh" \
    --payload-dir "$PAYLOAD_A" \
    --output-dir "$OUT_A" \
    --version "$FIXED_VER" \
    --source-sha "$FIXED_SHA" \
    --source-date-epoch "$FIXED_EPOCH"

# Build a second time from a separate clean directory to verify reproducibility.
bash "$ROOT_DIR/packaging/build-deb.sh" \
    --payload-dir "$PAYLOAD_B" \
    --output-dir "$OUT_B" \
    --version "$FIXED_VER" \
    --source-sha "$FIXED_SHA" \
    --source-date-epoch "$FIXED_EPOCH"

DEB_A="$OUT_A/fusionpanel_${FIXED_VER}_amd64.deb"
DEB_B="$OUT_B/fusionpanel_${FIXED_VER}_amd64.deb"
[[ -f "$DEB_A" && -f "$DEB_B" ]]

HASH_A=$(sha256sum "$DEB_A" | awk '{ print $1 }')
HASH_B=$(sha256sum "$DEB_B" | awk '{ print $1 }')
if [[ "$HASH_A" != "$HASH_B" ]]; then
    echo "FAIL: .deb builds are not reproducible ($HASH_A != $HASH_B)" >&2
    exit 1
fi

# 4. Validate package metadata via dpkg-deb --field / --info.
[[ "$(dpkg-deb --field "$DEB_A" Package)" == "fusionpanel" ]]
[[ "$(dpkg-deb --field "$DEB_A" Architecture)" == "amd64" ]]
[[ "$(dpkg-deb --field "$DEB_A" Version)" == "$FIXED_VER" ]]
[[ "$(dpkg-deb --field "$DEB_A" X-FusionPanel-Source-Commit)" == "$FIXED_SHA" ]]
[[ "$(dpkg-deb --field "$DEB_A" X-FusionPanel-Build-Timestamp)" == "$FIXED_EPOCH" ]]
DEPENDS=$(dpkg-deb --field "$DEB_A" Depends)
for dep in ca-certificates libc6 openssl passwd systemd; do
    if [[ "$DEPENDS" != *"$dep"* ]]; then
        echo "FAIL: Depends missing '$dep': $DEPENDS" >&2
        exit 1
    fi
done

# 5. Validate package contents, ownership, permissions, and normalized timestamps.
CONTENTS=$(dpkg-deb --contents "$DEB_A")
printf '%s\n' "$CONTENTS" > "$TEST_DIR/contents.txt"

# Every entry must be owned by root/root.
if awk '$2 != "root/root" { exit 1 }' "$TEST_DIR/contents.txt" >/dev/null 2>&1; then
    :
else
    echo "FAIL: non-root ownership found in .deb contents" >&2
    cat "$TEST_DIR/contents.txt" >&2
    exit 1
fi

# Expected paths and modes.
assert_deb_entry() {
    local mode_prefix="$1"
    local path="$2"
    if ! grep -Eq "^${mode_prefix}[[:space:]]+root/root[[:space:]]+.*[[:space:]]+${path}\$" "$TEST_DIR/contents.txt"; then
        echo "FAIL: missing or wrong mode for '$path' (expected $mode_prefix)" >&2
        cat "$TEST_DIR/contents.txt" >&2
        exit 1
    fi
}

assert_deb_entry '-rwxr-xr-x' '\./usr/bin/fusionpanel-api'
assert_deb_entry '-rwxr-xr-x' '\./usr/bin/fusionpanel-agent'
assert_deb_entry '-rwxr-xr-x' '\./usr/bin/fusionpanel-cli'
assert_deb_entry '-rwxr-xr-x' '\./usr/bin/fusionpanel-nginx-helper'
assert_deb_entry '-rw-r--r--' '\./lib/systemd/system/fusionpanel\.service'
assert_deb_entry '-rw-r--r--' '\./lib/systemd/system/fusionpanel-agent\.service'
assert_deb_entry '-rw-r--r--' '\./lib/systemd/system/fusionpanel-nginx-helper\.service'
assert_deb_entry '-rwxr-xr-x' '\./usr/share/fusionpanel/packaging/nginx-include\.sh'
assert_deb_entry '-rw-r--r--' '\./usr/share/fusionpanel/frontend/dist/index\.html'
assert_deb_entry '-rw-r--r--' '\./usr/share/fusionpanel/frontend/dist/assets/app\.js'
assert_deb_entry '-rw-r--r--' '\./usr/share/fusionpanel/BUILD_INFO\.txt'
assert_deb_entry 'drwx------' '\./etc/fusionpanel/'

# Secrets must never be baked into the .deb archive.
if grep -Eq '\./etc/fusionpanel/(fusionpanel\.env|fusionpanel-agent\.env|initial-credentials\.txt)' "$TEST_DIR/contents.txt"; then
    echo "FAIL: secret/runtime env files must not be shipped inside the .deb" >&2
    exit 1
fi

# 6. Extract .deb into a temp directory and verify unit ExecStart paths & BUILD_INFO.
EXTRACT_DIR="$TEST_DIR/extracted"
dpkg-deb -x "$DEB_A" "$EXTRACT_DIR"
dpkg-deb -e "$DEB_A" "$EXTRACT_DIR/DEBIAN"

grep -Fxq 'ExecStart=/usr/bin/fusionpanel-api' "$EXTRACT_DIR/lib/systemd/system/fusionpanel.service"
grep -Fxq 'ExecStart=/usr/bin/fusionpanel-agent' "$EXTRACT_DIR/lib/systemd/system/fusionpanel-agent.service"
grep -Fxq 'ExecStart=/usr/bin/fusionpanel-nginx-helper' "$EXTRACT_DIR/lib/systemd/system/fusionpanel-nginx-helper.service"
grep -Fxq 'SupplementaryGroups=fusionpanel-nginx' "$EXTRACT_DIR/lib/systemd/system/fusionpanel-agent.service"
! grep -q '^SupplementaryGroups=fusionpanel-nginx$' "$EXTRACT_DIR/lib/systemd/system/fusionpanel.service"
grep -Fxq 'Group=fusionpanel-nginx' "$EXTRACT_DIR/lib/systemd/system/fusionpanel-nginx-helper.service"

grep -Fxq "Version: $FIXED_VER" "$EXTRACT_DIR/usr/share/fusionpanel/BUILD_INFO.txt"
grep -Fxq "Source commit: $FIXED_SHA" "$EXTRACT_DIR/usr/share/fusionpanel/BUILD_INFO.txt"
grep -Fxq "SOURCE_DATE_EPOCH: $FIXED_EPOCH" "$EXTRACT_DIR/usr/share/fusionpanel/BUILD_INFO.txt"

for maint in postinst prerm postrm; do
    [[ -x "$EXTRACT_DIR/DEBIAN/$maint" ]] || {
        echo "FAIL: DEBIAN/$maint is not executable" >&2
        exit 1
    }
done

# Verify postinst preserves existing credentials/env and postrm only purges on purge.
grep -q 'if \[ -f "\$api_env" \]; then' "$EXTRACT_DIR/DEBIAN/postinst"
grep -q 'if \[ -f "\$agent_env" \]; then' "$EXTRACT_DIR/DEBIAN/postinst"
grep -q 'if \[ ! -e "\$creds_file" \]' "$EXTRACT_DIR/DEBIAN/postinst"
grep -q 'purge)' "$EXTRACT_DIR/DEBIAN/postrm"

# 7. Verify build-package.sh produces both .tar.gz and .deb from mocked cargo/npm.
MOCK_BIN="$TEST_DIR/mock-bin"
MOCK_TARGET="$TEST_DIR/mock-target"
MOCK_DIST="$TEST_DIR/mock-dist"
MOCK_OUT="$TEST_DIR/mock-out"
mkdir -p "$MOCK_BIN" "$MOCK_TARGET" "$MOCK_DIST" "$MOCK_OUT"

cat > "$MOCK_BIN/cargo" <<'SH'
#!/bin/sh
mkdir -p "$CARGO_TARGET_DIR/release"
for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper; do
    printf '#!/bin/sh\nprintf "%%s\\n" "%s"\n' "$binary" > "$CARGO_TARGET_DIR/release/$binary"
    chmod 0755 "$CARGO_TARGET_DIR/release/$binary"
done
SH
cat > "$MOCK_BIN/npm" <<'SH'
#!/bin/sh
case " $* " in
    *" run build "*)
        out_dir=""
        previous=""
        for arg in "$@"; do
            if [ "$previous" = "--outDir" ]; then out_dir="$arg"; fi
            previous="$arg"
        done
        mkdir -p "$out_dir/assets"
        printf '<!doctype html><title>fixture</title>\n' > "$out_dir/index.html"
        printf 'fixture asset\n' > "$out_dir/assets/app.js"
        ;;
    *" ci "*) ;;
esac
SH
chmod +x "$MOCK_BIN/cargo" "$MOCK_BIN/npm"

PATH="$MOCK_BIN:$PATH" \
CARGO_TARGET_DIR="$MOCK_TARGET" \
FUSIONPANEL_CARGO="$MOCK_BIN/cargo" \
FUSIONPANEL_NPM="$MOCK_BIN/npm" \
FUSIONPANEL_FRONTEND_DIST="$MOCK_DIST" \
FUSIONPANEL_DEB_VERSION="$FIXED_VER" \
SOURCE_DATE_EPOCH="$FIXED_EPOCH" \
    bash "$ROOT_DIR/packaging/build-package.sh" ci-deb-test "$MOCK_OUT"

[[ -f "$MOCK_OUT/fusionpanel-ci-deb-test-debian-x86_64.tar.gz" ]]
[[ -f "$MOCK_OUT/fusionpanel_${FIXED_VER}_amd64.deb" ]]
grep -q "fusionpanel_${FIXED_VER}_amd64.deb" "$MOCK_OUT/SHA256SUMS"
(cd "$MOCK_OUT" && sha256sum -c SHA256SUMS >/dev/null)

echo "Debian package layout and reproducibility tests passed."
