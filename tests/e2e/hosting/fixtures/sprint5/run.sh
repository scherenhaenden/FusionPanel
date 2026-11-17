#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../../../../.." && pwd)
hosting_dir="$repo_root/tests/e2e/hosting"
browser_dir="$repo_root/tests/e2e/browser"

suffix=$(tr -dc 'a-f0-9' </dev/urandom | head -c 12 || true)
[ -n "$suffix" ] || suffix=$(date +%s)
project_name="fp-s5-e2e-${suffix}"
state_dir=$(mktemp -d /tmp/fusionpanel-sprint5-e2e.XXXXXX)
api_ctx="$state_dir/api-ctx"
node_ctx="$state_dir/node-ctx"
log_file="/tmp/fusionpanel-sprint5-e2e-${suffix}.log"
compose_invoked=0

export FUSIONPANEL_S5_REPO_ROOT="$repo_root"
export FUSIONPANEL_S5_UID=$(id -u)
export FUSIONPANEL_S5_GID=$(id -g)
export FUSIONPANEL_S5_SUFFIX="$suffix"
export FUSIONPANEL_S5_API_IMAGE="fusionpanel-s5-api:${suffix}"
export FUSIONPANEL_S5_NODE_IMAGE="fusionpanel-s5-node:${suffix}"
export FUSIONPANEL_S5_API_BUILD_CONTEXT="$api_ctx"
export FUSIONPANEL_S5_NODE_BUILD_CONTEXT="$node_ctx"
export FUSIONPANEL_E2E_ADMIN_TOKEN=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')
export FUSIONPANEL_E2E_USERNAME="e2e-admin"
export FUSIONPANEL_E2E_PASSWORD=$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')

compose() {
  docker compose -p "$project_name" -f "$script_dir/docker-compose.yml" "$@"
}

cleanup() {
  local rc=$?
  trap - EXIT INT TERM
  if [ "$compose_invoked" -eq 1 ]; then
    compose logs --no-color >"$log_file" 2>&1 || true
    compose down -v --remove-orphans || rc=1
  fi
  docker image rm -f "$FUSIONPANEL_S5_API_IMAGE" "$FUSIONPANEL_S5_NODE_IMAGE" >/dev/null 2>&1 || true
  rm -rf -- "$state_dir"
  docker system df || true
  printf 'Sprint 5 E2E log: %s\n' "$log_file"
  exit "$rc"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

printf 'Docker usage before Sprint 5 E2E:\n'
docker system df

mkdir -p "$api_ctx" "$node_ctx"
cp "$repo_root/tests/e2e/browser/fixtures/sprint2/Dockerfile" "$api_ctx/Dockerfile"
cp "$repo_root/tests/e2e/browser/fixtures/sprint2/seed-admin.sh" "$api_ctx/seed-admin.sh"
cp "$script_dir/Dockerfile" "$node_ctx/Dockerfile"

RUST_MIN_STACK=33554432 cargo build --locked --offline --manifest-path "$repo_root/Cargo.toml" -p fusionpanel-api -p fusionpanel-cli
RUST_MIN_STACK=33554432 cargo build --locked --offline --manifest-path "$repo_root/crates/agent/Cargo.toml" --bin fusionpanel-agent
npm --prefix "$repo_root/frontend" run build

DOCKER_BUILDKIT=0 docker build --file "$api_ctx/Dockerfile" --tag "$FUSIONPANEL_S5_API_IMAGE" "$api_ctx"
DOCKER_BUILDKIT=0 docker build --file "$node_ctx/Dockerfile" --tag "$FUSIONPANEL_S5_NODE_IMAGE" "$node_ctx"

compose config --quiet
compose_invoked=1
compose up --detach --wait --wait-timeout 120

api_addr=$(compose port api 8080)
api_port=${api_addr##*:}
export FUSIONPANEL_FRONTEND_URL="http://127.0.0.1:${api_port}"
export FUSIONPANEL_S5_PG_CONTAINER="${project_name}-postgres-1"
export FUSIONPANEL_S5_MARIADB_CONTAINER="${project_name}-mariadb-1"

"$browser_dir/node_modules/.bin/playwright" test \
  --config "$hosting_dir/playwright.sprint5.config.mjs"
