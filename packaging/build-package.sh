#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
label=${1:?Usage: build-package.sh PACKAGE_LABEL OUTPUT_DIR [MANIFEST_PATH]}
output_dir=${2:?Usage: build-package.sh PACKAGE_LABEL OUTPUT_DIR [MANIFEST_PATH]}
manifest=${3:-}
cargo_bin=${FUSIONPANEL_CARGO:-cargo}
npm_bin=${FUSIONPANEL_NPM:-npm}
target_dir=${CARGO_TARGET_DIR:-"$root/target"}
frontend_dist=${FUSIONPANEL_FRONTEND_DIST:-"$root/frontend/dist"}

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

"$cargo_bin" build --locked --release --manifest-path "$root/Cargo.toml" \
    --target-dir "$target_dir" -p fusionpanel-api -p fusionpanel-cli
"$cargo_bin" build --locked --release --manifest-path "$root/crates/agent/Cargo.toml" \
    --target-dir "$target_dir" --bin fusionpanel-agent --bin fusionpanel-nginx-helper
"$npm_bin" ci --prefix "$root/frontend"
"$npm_bin" --prefix "$root/frontend" run build -- --outDir "$frontend_dist" --emptyOutDir

for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper; do
    [[ -x "$target_dir/release/$binary" ]] || {
        echo "BLOCKED: release binary missing or not executable: $target_dir/release/$binary" >&2
        exit 1
    }
    install -m 755 "$target_dir/release/$binary" "$staging/$binary"
done
mkdir -p "$staging/systemd" "$staging/packaging"
for unit in fusionpanel.service fusionpanel-agent.service fusionpanel-nginx-helper.service; do
    install -m 644 "$root/packaging/systemd/$unit" "$staging/systemd/$unit"
done
install -m 755 "$root/packaging/nginx-include.sh" "$staging/packaging/nginx-include.sh"
[[ -f "$frontend_dist/index.html" ]] || {
    echo "BLOCKED: frontend build did not produce $frontend_dist/index.html" >&2
    exit 1
}
mkdir -p "$staging/frontend/dist"
cp -a "$frontend_dist/." "$staging/frontend/dist/"
if [[ -n "$manifest" ]]; then
    install -m 644 "$manifest" "$staging/RELEASE_MANIFEST.json"
    install -m 644 "$manifest" "$output_dir/RELEASE_MANIFEST.json"
else
    printf 'Unpublished CI package\nSource commit: %s\n' "$(git -C "$root" rev-parse HEAD)" > "$staging/BUILD_INFO.txt"
fi

archive="fusionpanel-${label}-debian-x86_64.tar.gz"
# mktemp creates staging with mode 0700; this directory is archived as the
# package root and becomes the installer's bin/payload directory on extraction.
chmod 0755 "$staging"
tar -C "$staging" -czf "$output_dir/$archive" .
cp "$output_dir/$archive" "$output_dir/fusionpanel-debian-x86_64.tar.gz"
(cd "$output_dir" && sha256sum "$archive" fusionpanel-debian-x86_64.tar.gz > SHA256SUMS)
printf 'Created %s\n' "$output_dir/$archive"
