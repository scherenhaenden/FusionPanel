#!/usr/bin/env bash
set -Eeuo pipefail

fixture_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
source "$fixture_dir/preflight.sh"
fixture_root=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-e2e-preflight.XXXXXX")
trap 'rm -rf -- "$fixture_root"' EXIT

expect_missing() {
  local reason="$1"
  if ! sprint2_fixture_artifacts_missing "$fixture_root"; then
    echo "FAIL: missing condition was not detected: $reason" >&2
    exit 1
  fi
}

expect_ready() {
  if sprint2_fixture_artifacts_missing "$fixture_root"; then
    echo "FAIL: complete executable fixture artifacts were rejected" >&2
    exit 1
  fi
}

expect_missing "empty fixture root"

mkdir -p "$fixture_root/target/debug" "$fixture_root/frontend/dist"
touch \
  "$fixture_root/target/debug/fusionpanel-api" \
  "$fixture_root/target/debug/fusionpanel-cli" \
  "$fixture_root/target/debug/fusionpanel-agent" \
  "$fixture_root/frontend/dist/index.html"
chmod 0755 \
  "$fixture_root/target/debug/fusionpanel-api" \
  "$fixture_root/target/debug/fusionpanel-cli" \
  "$fixture_root/target/debug/fusionpanel-agent"
expect_ready

chmod 0644 "$fixture_root/target/debug/fusionpanel-api"
expect_missing "API binary is not executable"
chmod 0755 "$fixture_root/target/debug/fusionpanel-api"

rm "$fixture_root/target/debug/fusionpanel-cli"
expect_missing "CLI binary is missing"
touch "$fixture_root/target/debug/fusionpanel-cli"
chmod 0755 "$fixture_root/target/debug/fusionpanel-cli"

rm "$fixture_root/target/debug/fusionpanel-agent"
expect_missing "agent binary is missing"
touch "$fixture_root/target/debug/fusionpanel-agent"
chmod 0755 "$fixture_root/target/debug/fusionpanel-agent"

rm "$fixture_root/frontend/dist/index.html"
expect_missing "frontend entry point is missing"

echo "PASS: fixture precondition accepts complete executable artifacts and rejects missing or non-executable inputs."
