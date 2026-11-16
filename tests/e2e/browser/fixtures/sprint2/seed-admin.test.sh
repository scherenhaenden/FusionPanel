#!/usr/bin/env bash
set -Eeuo pipefail

fixture_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
seed_script="$fixture_dir/seed-admin.sh"
test_dir=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-e2e-seed.XXXXXX")
trap 'rm -rf -- "$test_dir"' EXIT
mkdir -p "$test_dir/bin" "$test_dir/logs"

cat > "$test_dir/bin/cli" <<'SH'
#!/bin/sh
printf 'call\n' >> "$FUSIONPANEL_TEST_CLI_CALLS"
printf '<%s>\n' "$@" >> "$FUSIONPANEL_TEST_CLI_ARGS"
exit "${FUSIONPANEL_TEST_CLI_EXIT:-0}"
SH
cat > "$test_dir/bin/api" <<'SH'
#!/bin/sh
printf 'call\n' >> "$FUSIONPANEL_TEST_API_CALLS"
exit "${FUSIONPANEL_TEST_API_EXIT:-0}"
SH
chmod 0755 "$test_dir/bin/cli" "$test_dir/bin/api"

export FUSIONPANEL_E2E_CLI="$test_dir/bin/cli"
export FUSIONPANEL_E2E_API="$test_dir/bin/api"
export FUSIONPANEL_E2E_USERNAME=e2e-admin
export FUSIONPANEL_E2E_PASSWORD=synthetic-fixture-password
export FUSIONPANEL_DATABASE_URL="$test_dir/state/fusionpanel.db"
export FUSIONPANEL_TEST_CLI_CALLS="$test_dir/logs/cli-calls"
export FUSIONPANEL_TEST_CLI_ARGS="$test_dir/logs/cli-args"
export FUSIONPANEL_TEST_API_CALLS="$test_dir/logs/api-calls"

assert_call_count() {
  local file=$1 expected=$2 label=$3 actual=0
  if [[ -f "$file" ]]; then actual=$(wc -l < "$file"); fi
  if [[ "$actual" -ne "$expected" ]]; then
    echo "FAIL: expected $expected $label calls, got $actual" >&2
    exit 1
  fi
}

expect_failure() {
  local expected_status=$1 label=$2 output_file=$3
  shift 3
  local actual_status=0
  if "$@" >"$output_file" 2>&1; then
    echo "FAIL: expected $label to fail" >&2
    exit 1
  else
    actual_status=$?
  fi
  if [[ "$actual_status" -ne "$expected_status" ]]; then
    echo "FAIL: expected $label status $expected_status, got $actual_status" >&2
    cat "$output_file" >&2
    exit 1
  fi
}

# A failed bootstrap must not leave a marker or start the API.
expect_failure 43 'CLI bootstrap' "$test_dir/logs/cli-failure" \
  env FUSIONPANEL_TEST_CLI_EXIT=43 "$seed_script"
marker="$test_dir/state/.fusionpanel-e2e-admin-seeded"
[[ ! -e "$marker" ]]
[[ -d "$test_dir/state" ]]
[[ ! -e "$FUSIONPANEL_TEST_API_CALLS" ]]
assert_call_count "$FUSIONPANEL_TEST_CLI_CALLS" 1 'CLI bootstrap'
assert_call_count "$FUSIONPANEL_TEST_API_CALLS" 0 'API startup'

# A successful bootstrap records the username and starts the API.
"$seed_script"
[[ -f "$marker" ]]
[[ "$(cat "$marker")" == "$FUSIONPANEL_E2E_USERNAME" ]]
[[ "$(stat -c '%a' "$marker")" == 600 ]]
! grep -Fq "$FUSIONPANEL_E2E_PASSWORD" "$marker"
assert_call_count "$FUSIONPANEL_TEST_CLI_CALLS" 2 'CLI bootstrap'
assert_call_count "$FUSIONPANEL_TEST_API_CALLS" 1 'API startup'
for argument in \
  '<--json>' '<bootstrap>' '<operator>' \
  '<--database-url>' "<$FUSIONPANEL_DATABASE_URL>" \
  '<--username>' "<$FUSIONPANEL_E2E_USERNAME>" \
  '<--password>' "<$FUSIONPANEL_E2E_PASSWORD>"; do
  grep -Fxq "$argument" "$FUSIONPANEL_TEST_CLI_ARGS"
done

# Restart reuses the persistent marker, skips CLI bootstrap, and launches the API again.
"$seed_script"
assert_call_count "$FUSIONPANEL_TEST_CLI_CALLS" 2 'CLI bootstrap after restart'
assert_call_count "$FUSIONPANEL_TEST_API_CALLS" 2 'API startup after restart'

# A marker from a different username must not silently skip bootstrap.
expect_failure 1 'username mismatch' "$test_dir/logs/username-mismatch" \
  env FUSIONPANEL_E2E_USERNAME=other-operator "$seed_script"
grep -Fq 'marker belongs to a different operator username' "$test_dir/logs/username-mismatch"
assert_call_count "$FUSIONPANEL_TEST_CLI_CALLS" 2 'CLI bootstrap after username mismatch'
assert_call_count "$FUSIONPANEL_TEST_API_CALLS" 2 'API startup after username mismatch'

# exec must preserve the API process exit status on a marked restart.
expect_failure 37 'API process' "$test_dir/logs/api-failure" \
  env FUSIONPANEL_TEST_API_EXIT=37 "$seed_script"
assert_call_count "$FUSIONPANEL_TEST_CLI_CALLS" 2 'CLI bootstrap after marked restart'
assert_call_count "$FUSIONPANEL_TEST_API_CALLS" 3 'API startup after marked restart'

echo 'PASS: Sprint 2 admin seed marker is atomic, persistent across restarts, and username-scoped.'
