#!/bin/bash
set -euo pipefail

if [[ "${1:-}" != "--changed-from" || $# -lt 2 || $# -gt 3 ]]; then
    echo "Usage: $0 --changed-from <base-sha-or-ref> [head-sha-or-ref]" >&2
    echo "This local helper runs only affected manifests. Run the full catalog audit with GitHub Actions workflow_dispatch." >&2
    exit 64
fi

BASE_REF=$2
HEAD_REF=${3:-HEAD}
AVAILABLE_KIB=$(df -Pk . | awk 'NR == 2 {print $4}')
MINIMUM_KIB=$((10 * 1024 * 1024))
printf 'Available filesystem space: %s KiB\n' "$AVAILABLE_KIB"
if [[ "$AVAILABLE_KIB" -lt "$MINIMUM_KIB" ]]; then
    echo "Refusing local Cargo matrix: at least 10 GiB free space is required." >&2
    exit 1
fi

echo "Running local CI checks for changes between $BASE_REF and $HEAD_REF..."

./scripts/ci/audit_unsafe.sh
./scripts/ci/check_mock_bypasses.sh

OUTPUT=$(./scripts/ci/discover.py --changed-from "$BASE_REF" --changed-to "$HEAD_REF")

CARGO_MANIFESTS=$(echo "$OUTPUT" | grep 'cargo_manifests=' | sed 's/cargo_manifests=//')
PACKAGE_JSONS=$(echo "$OUTPUT" | grep 'package_jsons=' | sed 's/package_jsons=//')

# jq is needed to parse the discovered JSON arrays.
MANIFESTS_ARRAY=$(echo "$CARGO_MANIFESTS" | jq -r '.[]')
PACKAGE_ARRAY=$(echo "$PACKAGE_JSONS" | jq -r '.[]')

for manifest in $MANIFESTS_ARRAY; do
    echo "Running checks for $manifest"

    cargo check --manifest-path "$manifest"
    cargo fmt --manifest-path "$manifest" -- --check
    cargo clippy --manifest-path "$manifest" -- -D warnings
    cargo test --manifest-path "$manifest"
done

for manifest in $PACKAGE_ARRAY; do
    directory=$(dirname "$manifest")
    echo "Running checks for $manifest"
    (
        cd "$directory"
        npm ci
        POLICY=$(node -p "require('./package.json').fusionpanel?.policy || 'unit'")
        if [[ "$POLICY" == "compile-only" ]]; then
            npm run compile
        else
            npm run build
            npm test
        fi
    )
done

echo "Changed-manifest checks passed."
