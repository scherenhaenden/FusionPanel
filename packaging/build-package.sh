#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
label=${1:?Usage: build-package.sh PACKAGE_LABEL OUTPUT_DIR [MANIFEST_PATH]}
output_dir=${2:?Usage: build-package.sh PACKAGE_LABEL OUTPUT_DIR [MANIFEST_PATH]}
manifest=${3:-}

if [[ ! "$label" =~ ^[a-zA-Z0-9][a-zA-Z0-9.+_-]*$ ]]; then
    echo "Invalid package label" >&2
    exit 1
fi
[[ "$(uname -m)" == x86_64 ]] || { echo "Only x86_64 builds are configured" >&2; exit 1; }
[[ -f "$root/crates/agent/Cargo.lock" ]] || {
    echo "BLOCKED: crates/agent/Cargo.lock is required for a locked agent build" >&2
    exit 1
}

mkdir -p "$output_dir"
output_dir=$(cd -- "$output_dir" && pwd)
staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT

if [[ -n "$manifest" ]]; then
    manifest_version=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$manifest")
    [[ "$manifest_version" == "$label" ]] || { echo "Release manifest version does not match package label" >&2; exit 1; }
    export FUSIONPANEL_BUILD_VERSION="$manifest_version"
else
    unset FUSIONPANEL_BUILD_VERSION
fi

cargo build --locked --release --manifest-path "$root/Cargo.toml" -p fusionpanel-api
cargo build --locked --release --manifest-path "$root/crates/agent/Cargo.toml" --bin fusionpanel-agent
install -m 755 "$root/target/release/fusionpanel-api" "$staging/fusionpanel-api"
install -m 755 "$root/crates/agent/target/release/fusionpanel-agent" "$staging/fusionpanel-agent"
if [[ -n "$manifest" ]]; then
    install -m 644 "$manifest" "$staging/RELEASE_MANIFEST.json"
    install -m 644 "$manifest" "$output_dir/RELEASE_MANIFEST.json"
else
    printf 'Unpublished CI package\nSource commit: %s\n' "$(git -C "$root" rev-parse HEAD)" > "$staging/BUILD_INFO.txt"
fi

archive="fusionpanel-${label}-debian-x86_64.tar.gz"
tar -C "$staging" -czf "$output_dir/$archive" .
cp "$output_dir/$archive" "$output_dir/fusionpanel-debian-x86_64.tar.gz"
(cd "$output_dir" && sha256sum "$archive" fusionpanel-debian-x86_64.tar.gz > SHA256SUMS)
printf 'Created %s\n' "$output_dir/$archive"
