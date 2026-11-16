#!/usr/bin/env bash
set -Eeuo pipefail

fixture_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
source "$fixture_dir/preflight.sh"
temporary_dir=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-sprint3-preflight.XXXXXX")
trap 'rm -rf -- "$temporary_dir"' EXIT

mkdir -p "$temporary_dir/target/debug" "$temporary_dir/frontend/dist" \
  "$temporary_dir/browser/node_modules/.bin"
touch "$temporary_dir/target/debug/fusionpanel-api" \
  "$temporary_dir/target/debug/fusionpanel-cli" \
  "$temporary_dir/target/debug/fusionpanel-agent" \
  "$temporary_dir/frontend/dist/index.html" \
  "$temporary_dir/browser/node_modules/.bin/playwright"
chmod +x "$temporary_dir/target/debug/fusionpanel-api" \
  "$temporary_dir/target/debug/fusionpanel-cli" \
  "$temporary_dir/target/debug/fusionpanel-agent" \
  "$temporary_dir/browser/node_modules/.bin/playwright"

if sprint3_fixture_artifacts_missing "$temporary_dir" "$temporary_dir/browser"; then
  printf 'PASS: complete fixture artifacts are accepted.\n'
else
  printf 'FAIL: complete fixture artifacts were rejected.\n' >&2
  exit 1
fi

if sprint3_check_free_space "$temporary_dir" 1; then
  printf 'PASS: the free-space check accepts a one-KiB minimum.\n'
else
  printf 'FAIL: the free-space check rejected a one-KiB minimum.\n' >&2
  exit 1
fi

if sprint3_check_free_space "$temporary_dir" 999999999 >/dev/null 2>&1; then
  printf 'FAIL: the free-space check accepted an impossible minimum.\n' >&2
  exit 1
else
  printf 'PASS: the free-space check blocks an unavailable minimum.\n'
fi

rm "$temporary_dir/target/debug/fusionpanel-agent"
if sprint3_fixture_artifacts_missing "$temporary_dir" "$temporary_dir/browser"; then
  printf 'FAIL: a missing agent binary was accepted.\n' >&2
  exit 1
else
  printf 'PASS: a missing agent binary blocks the fixture.\n'
fi

printf 'PASS: Sprint 3 fixture preflight unit checks completed.\n'
