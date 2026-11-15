#!/usr/bin/env bash
set -Eeuo pipefail

fixture_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$fixture_dir/../../../../.." && pwd)
browser_dir="$repo_root/tests/e2e/browser"
compose_file="$fixture_dir/docker-compose.yml"
project_name="fp-s2-e2e-${GITHUB_RUN_ID:-local}-$$"
state_dir=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-sprint2-e2e.XXXXXX")
log_file="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/fusionpanel-sprint2-compose-${GITHUB_RUN_ID:-local}-$$.log"

export FUSIONPANEL_E2E_REPO_ROOT="$repo_root"
export FUSIONPANEL_E2E_STATE_DIR="$state_dir"
export FUSIONPANEL_E2E_IMAGE="fusionpanel-sprint2-e2e:${project_name}"
export FUSIONPANEL_E2E_PORT="${FUSIONPANEL_E2E_PORT:-18080}"
export FUSIONPANEL_E2E_UID="$(id -u)"
export FUSIONPANEL_E2E_GID="$(id -g)"
export FUSIONPANEL_E2E_USERNAME="e2e-admin"
export FUSIONPANEL_E2E_PASSWORD="sprint2-fixture-only-password"
FUSIONPANEL_E2E_ADMIN_TOKEN=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
export FUSIONPANEL_E2E_ADMIN_TOKEN
export FUSIONPANEL_FRONTEND_URL="http://127.0.0.1:${FUSIONPANEL_E2E_PORT}"

compose() {
  docker compose --project-name "$project_name" --file "$compose_file" "$@"
}

source "$fixture_dir/preflight.sh"

cleanup() {
  result=$?
  trap - EXIT INT TERM
  compose logs --no-color >"$log_file" 2>&1 || true
  sed -i \
    -e "s/${FUSIONPANEL_E2E_ADMIN_TOKEN}/[REDACTED_ADMIN_TOKEN]/g" \
    -e "s/${FUSIONPANEL_E2E_PASSWORD}/[REDACTED_TEST_PASSWORD]/g" \
    "$log_file" 2>/dev/null || true
  if [ "$result" -ne 0 ]; then
    cat "$log_file" >&2
  fi
  compose down --volumes --remove-orphans --rmi all || result=1
  rm -rf -- "$state_dir"
  exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

if sprint2_fixture_artifacts_missing "$repo_root"; then
  echo "Build the API, CLI, agent, and frontend before running this disposable fixture." >&2
  exit 2
fi

compose config --quiet
compose up --detach --build --wait --wait-timeout 90
cd "$browser_dir"
npm run test:sprint2
