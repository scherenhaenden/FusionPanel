#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
# shellcheck source=packaging/layout.sh
. "$root/packaging/layout.sh"

usage() {
    cat >&2 <<'EOF'
Usage: build-deb.sh --payload-dir DIR --output-dir DIR [OPTIONS]
Options:
  --payload-dir DIR         Staged package payload directory (binaries, systemd, frontend)
  --output-dir DIR          Output directory for the .deb package
  --version VER             Debian package version (default: derived from Cargo.toml)
  --source-sha SHA          Source commit SHA (default: git rev-parse HEAD)
  --source-date-epoch SEC   Reproducible build timestamp (default: git commit timestamp)
EOF
}

payload_dir=""
output_dir=""
deb_version=${FUSIONPANEL_DEB_VERSION:-}
source_sha=${FUSIONPANEL_SOURCE_SHA:-}
source_date_epoch=${SOURCE_DATE_EPOCH:-}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --payload-dir)
            payload_dir=${2:-}
            shift 2
            ;;
        --output-dir)
            output_dir=${2:-}
            shift 2
            ;;
        --version)
            deb_version=${2:-}
            shift 2
            ;;
        --source-sha)
            source_sha=${2:-}
            shift 2
            ;;
        --source-date-epoch)
            source_date_epoch=${2:-}
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1" >&2
            usage
            exit 1
            ;;
    esac
done

[[ -n "$payload_dir" && -n "$output_dir" ]] || { usage; exit 1; }
command -v dpkg-deb >/dev/null 2>&1 || {
    echo "BLOCKED: dpkg-deb is required to build the Debian package" >&2
    exit 1
}

if [[ -z "$source_sha" ]]; then
    if git -C "$root" rev-parse HEAD >/dev/null 2>&1; then
        source_sha=$(git -C "$root" rev-parse HEAD)
    else
        source_sha="unknown"
    fi
fi

if [[ -z "$source_date_epoch" ]]; then
    if git -C "$root" log -1 --format=%ct >/dev/null 2>&1; then
        source_date_epoch=$(git -C "$root" log -1 --format=%ct)
    else
        source_date_epoch=$(date -u +%s)
    fi
fi
[[ "$source_date_epoch" =~ ^[0-9]+$ ]] || {
    echo "Invalid SOURCE_DATE_EPOCH: $source_date_epoch" >&2
    exit 1
}
export SOURCE_DATE_EPOCH="$source_date_epoch"

if [[ -z "$deb_version" ]]; then
    cargo_version=$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")
    deb_version=$(fusionpanel_normalize_deb_version "$cargo_version")
fi
[[ "$deb_version" =~ ^[0-9][A-Za-z0-9.+~:-]*$ ]] || {
    echo "Invalid Debian package version: $deb_version" >&2
    exit 1
}

mkdir -p "$output_dir"
output_dir=$(cd -- "$output_dir" && pwd)

deb_stage=$(mktemp -d)
trap 'rm -rf -- "$deb_stage"' EXIT
deb_root="$deb_stage/pkg"

fusionpanel_stage_deb_tree "$payload_dir" "$deb_root" "$source_sha" "$deb_version" "$source_date_epoch"

for script in postinst prerm postrm; do
    install -m 0755 "$root/packaging/deb/$script" "$deb_root/DEBIAN/$script"
done

installed_size=$(du -sk "$deb_root/usr" "$deb_root/lib" "$deb_root/etc" | awk '{ sum += $1 } END { print sum + 0 }')

sed \
    -e "s|@VERSION@|$deb_version|g" \
    -e "s|@INSTALLED_SIZE@|$installed_size|g" \
    -e "s|@SOURCE_SHA@|$source_sha|g" \
    -e "s|@SOURCE_DATE_EPOCH@|$source_date_epoch|g" \
    "$root/packaging/deb/control.in" > "$deb_root/DEBIAN/control"
chmod 0644 "$deb_root/DEBIAN/control"

find "$deb_root" -depth -exec touch -h -d "@${source_date_epoch}" {} +

deb_filename="fusionpanel_${deb_version}_amd64.deb"
deb_path="$output_dir/$deb_filename"
dpkg-deb --root-owner-group -Zgzip --build "$deb_root" "$deb_path" >/dev/null
touch -h -d "@${source_date_epoch}" "$deb_path"
printf 'Created %s\n' "$deb_path"
