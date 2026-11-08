#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/bin"
cat > "$tmp/bin/curl" <<'EOF'
#!/bin/sh
printf '%s\n' "$2" >> "$URL_LOG"
exit 22
EOF
chmod +x "$tmp/bin/curl"
export PATH="$tmp/bin:$PATH" URL_LOG="$tmp/urls"
export FUSIONPANEL_MOCK_DOWNLOAD_URL=""

check_urls() {
    local version=$1 encoded=$2 caller=$3
    : > "$URL_LOG"
    if [[ "$caller" == install ]]; then
        (
            export FUSIONPANEL_INSTALLER_NO_RUN=1
            # shellcheck source=../install.sh
            . "$root/packaging/install.sh" --port 8080
            VERSION=$version OS=debian ARCH=x86_64 DRY_RUN=0
            install
        ) > "$tmp/output" 2>&1 && { echo 'Expected the download to fail' >&2; exit 1; }
    else
        FUSIONPANEL_TEST_OS=debian FUSIONPANEL_TEST_ARCH=x86_64 \
        FUSIONPANEL_TEST_BACKUP_DIR="$tmp/backup" \
        FUSIONPANEL_TEST_BIN_DIR="$tmp/missing-bin" \
        FUSIONPANEL_TEST_CONFIG_DIR="$tmp/missing-config" \
        FUSIONPANEL_TEST_VERSION_LIB_DIR="$tmp/versions" \
        sh "$root/packaging/update.sh" "$version" > "$tmp/output" 2>&1 \
            && { echo 'Expected the download to fail' >&2; exit 1; }
    fi
    if [[ "$version" == latest ]]; then
        printf '%s\n' 'https://github.com/scherenhaenden/FusionPanel/releases/latest/download/fusionpanel-debian-x86_64.tar.gz' > "$tmp/expected"
    else
        local base="https://github.com/scherenhaenden/FusionPanel/releases/download/$encoded"
        printf '%s\n' "$base/fusionpanel-debian-x86_64.tar.gz" \
            "$base/fusionpanel-$encoded-debian-x86_64.tar.gz" > "$tmp/expected"
    fi
    diff -u "$tmp/expected" "$URL_LOG"
}

for caller in install update; do
    check_urls latest latest "$caller"
    check_urls '2026.11.08.16.30.123.gabcdef123456' '2026.11.08.16.30.123.gabcdef123456' "$caller"
    check_urls 'v0.1.0-dev.1+2026.11.01.15.37.917.g61303a7df7d3' 'v0.1.0-dev.1%2B2026.11.01.15.37.917.g61303a7df7d3' "$caller"
done
echo 'Release URL checks passed.'
