#!/bin/bash
set -euo pipefail

echo "Running full CI test suite locally..."

./scripts/ci/audit_unsafe.sh
./scripts/ci/check_mock_bypasses.sh

echo "Discovering manifests..."
OUTPUT=$(./scripts/ci/discover.py)

CARGO_MANIFESTS=$(echo "$OUTPUT" | grep 'cargo_manifests=' | sed 's/cargo_manifests=//')

# jq is needed to parse the JSON array
MANIFESTS_ARRAY=$(echo "$CARGO_MANIFESTS" | jq -r '.[]')

for manifest in $MANIFESTS_ARRAY; do
    echo "Running checks for $manifest"

    cargo check --manifest-path "$manifest"
    cargo fmt --manifest-path "$manifest" -- --check
    cargo clippy --manifest-path "$manifest" -- -D warnings
    cargo test --manifest-path "$manifest"
done

echo "CI matrix check passed."
