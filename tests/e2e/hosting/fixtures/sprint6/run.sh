#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../../../../.." && pwd)
hosting_dir="$repo_root/tests/e2e/hosting"
browser_dir="$repo_root/tests/e2e/browser"

suffix=$(tr -dc 'a-f0-9' </dev/urandom | head -c 12 || true)
[ -n "$suffix" ] || suffix=$(date +%s)
project_name="fp-s6-e2e-${suffix}"
state_dir=$(mktemp -d /tmp/fusionpanel-sprint6-e2e.XXXXXX)
api_ctx="$state_dir/api-ctx"
node_ctx="$state_dir/node-ctx"
sites_dir="$state_dir/sites"
log_file="/tmp/fusionpanel-sprint6-e2e-${suffix}.log"
compose_invoked=0

export FUSIONPANEL_S6_REPO_ROOT="$repo_root"
export FUSIONPANEL_S6_UID=$(id -u)
export FUSIONPANEL_S6_GID=$(id -g)
export FUSIONPANEL_S6_SUFFIX="$suffix"
export FUSIONPANEL_S6_SITES_DIR="$sites_dir"
export FUSIONPANEL_S6_API_IMAGE="fusionpanel-s6-api:${suffix}"
export FUSIONPANEL_S6_NODE_IMAGE="fusionpanel-s6-node:${suffix}"
export FUSIONPANEL_S6_API_BUILD_CONTEXT="$api_ctx"
export FUSIONPANEL_S6_NODE_BUILD_CONTEXT="$node_ctx"
export FUSIONPANEL_S6_MINIO_USER="minioadmin"
export FUSIONPANEL_S6_MINIO_PASS="minioadmin123"
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
  docker image rm -f "$FUSIONPANEL_S6_API_IMAGE" "$FUSIONPANEL_S6_NODE_IMAGE" >/dev/null 2>&1 || true
  rm -rf -- "$state_dir"
  docker system df || true
  printf 'Sprint 6 E2E log: %s\n' "$log_file"
  exit "$rc"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

printf 'Docker usage before Sprint 6 E2E:\n'
docker system df

mkdir -p "$api_ctx" "$node_ctx" "$sites_dir"
chmod 0777 "$sites_dir"
cp "$repo_root/tests/e2e/browser/fixtures/sprint2/Dockerfile" "$api_ctx/Dockerfile"
cp "$repo_root/tests/e2e/browser/fixtures/sprint2/seed-admin.sh" "$api_ctx/seed-admin.sh"
cp "$script_dir/Dockerfile" "$node_ctx/Dockerfile"
cp "$script_dir/start-node.sh" "$node_ctx/start-node.sh"

RUST_MIN_STACK=33554432 cargo build --locked --offline --manifest-path "$repo_root/Cargo.toml" -p fusionpanel-api -p fusionpanel-cli
RUST_MIN_STACK=33554432 cargo build --locked --offline --manifest-path "$repo_root/crates/agent/Cargo.toml" --bin fusionpanel-agent --bin fusionpanel-nginx-helper
npm --prefix "$repo_root/frontend" run build

DOCKER_BUILDKIT=0 docker build --file "$api_ctx/Dockerfile" --tag "$FUSIONPANEL_S6_API_IMAGE" "$api_ctx"
DOCKER_BUILDKIT=0 docker build --file "$node_ctx/Dockerfile" --tag "$FUSIONPANEL_S6_NODE_IMAGE" "$node_ctx"

compose config --quiet
compose_invoked=1
compose up --detach --wait --wait-timeout 120

minio_container="${project_name}-minio-1"
docker exec "$minio_container" mc alias set local http://127.0.0.1:9000 "$FUSIONPANEL_S6_MINIO_USER" "$FUSIONPANEL_S6_MINIO_PASS"
docker exec "$minio_container" mc mb --ignore-existing local/fusionpanel-backups

api_addr=$(compose port node 8080)
api_port=${api_addr##*:}
export FUSIONPANEL_FRONTEND_URL="http://127.0.0.1:${api_port}"
export FUSIONPANEL_S6_PG_CONTAINER="${project_name}-postgres-1"
export FUSIONPANEL_S6_MINIO_CONTAINER="$minio_container"

"$browser_dir/node_modules/.bin/playwright" test \
  --config "$hosting_dir/playwright.sprint6.config.mjs"
