#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../../.." && pwd)
browser_dir="$repo_root/tests/e2e/browser"

suffix=$(tr -dc 'a-f0-9' </dev/urandom | head -c 12 || true)
[ -n "$suffix" ] || suffix=$(date +%s)
project_name="fp-s13-e2e-${suffix}"
state_dir=$(mktemp -d /tmp/fusionpanel-sprint13-e2e.XXXXXX)
api_ctx="$state_dir/api-ctx"
agent_ctx="$state_dir/agent-ctx"
api_state_dir="$state_dir/api-state"
agent_state_dir="$state_dir/agent-state"
log_file="/tmp/fusionpanel-sprint13-e2e-${suffix}.log"
evidence_file="/tmp/fusionpanel-sprint13-e2e-${suffix}-evidence.json"
compose_invoked=0

export FUSIONPANEL_S13_REPO_ROOT="$repo_root"
export FUSIONPANEL_S13_UID=$(id -u)
export FUSIONPANEL_S13_GID=$(id -g)
export FUSIONPANEL_S13_SUFFIX="$suffix"
export FUSIONPANEL_S13_COMPOSE_PROJECT="$project_name"
export FUSIONPANEL_S13_COMPOSE_FILE="$script_dir/docker-compose.yml"
export FUSIONPANEL_S13_API_IMAGE="fusionpanel-s13-api:${suffix}"
export FUSIONPANEL_S13_AGENT_IMAGE="fusionpanel-s13-agent:${suffix}"
export FUSIONPANEL_S13_SENTINEL_IMAGE="fusionpanel-s13-sentinel:${suffix}"
export FUSIONPANEL_S13_TEST_IMAGE_LOCAL="fusionpanel-s13-test-app:${suffix}"
export FUSIONPANEL_S13_API_BUILD_CONTEXT="$api_ctx"
export FUSIONPANEL_S13_AGENT_BUILD_CONTEXT="$agent_ctx"
export FUSIONPANEL_S13_API_STATE_DIR="$api_state_dir"
export FUSIONPANEL_S13_AGENT_STATE_DIR="$agent_state_dir"
export FUSIONPANEL_S13_EVIDENCE_PATH="$evidence_file"
export FUSIONPANEL_E2E_ADMIN_TOKEN=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')
export FUSIONPANEL_E2E_USERNAME="e2e-admin"
export FUSIONPANEL_E2E_PASSWORD=$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')

compose() {
  docker compose -p "$project_name" -f "$script_dir/docker-compose.yml" "$@"
}

pick_ephemeral_port() {
  python3 -c '
import socket
s = socket.socket()
s.bind(("127.0.0.1", 0))
print(s.getsockname()[1])
s.close()
'
}

cleanup() {
  local rc=$?
  trap - EXIT INT TERM
  if [ "$compose_invoked" -eq 1 ]; then
    compose logs --no-color >"$log_file" 2>&1 || true
  fi

  # Remove only task-owned managed workload containers, never the sentinel or unrelated workloads.
  local managed_ids
  managed_ids=$(docker ps -aq --filter "label=fusionpanel.e2e.run=${suffix}" 2>/dev/null || true)
  if [ -n "$managed_ids" ]; then
    docker rm -f $managed_ids >/dev/null 2>&1 || true
  fi

  if [ "$compose_invoked" -eq 1 ]; then
    compose down -v --remove-orphans || rc=1
  fi

  docker image rm -f \
    "$FUSIONPANEL_S13_API_IMAGE" \
    "$FUSIONPANEL_S13_AGENT_IMAGE" \
    "$FUSIONPANEL_S13_SENTINEL_IMAGE" \
    "$FUSIONPANEL_S13_TEST_IMAGE_LOCAL" \
    "${FUSIONPANEL_S13_REGISTRY_IMAGE_TAG:-}" \
    "${FUSIONPANEL_S13_IMAGE_REF:-}" >/dev/null 2>&1 || true

  rm -rf -- "$state_dir"
  docker system df || true
  printf 'Sprint 13 E2E log: %s\n' "$log_file"
  printf 'Sprint 13 E2E evidence: %s\n' "$evidence_file"
  exit "$rc"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

printf 'Docker root and disk space before Sprint 13 E2E:\n'
docker_root=$(docker info --format '{{.DockerRootDir}}')
df -h / /tmp "$docker_root"
docker system df

mkdir -p "$api_ctx" "$agent_ctx" "$api_state_dir" "$agent_state_dir"
chmod 0700 "$api_state_dir" "$agent_state_dir"
cp "$repo_root/tests/e2e/browser/fixtures/sprint2/Dockerfile" "$api_ctx/Dockerfile"
cp "$repo_root/tests/e2e/browser/fixtures/sprint2/seed-admin.sh" "$api_ctx/seed-admin.sh"
cp "$script_dir/Dockerfile.agent" "$agent_ctx/Dockerfile"
cp "$script_dir/start-agent.sh" "$agent_ctx/start-agent.sh"

RUST_MIN_STACK=33554432 cargo build --locked --offline --manifest-path "$repo_root/Cargo.toml" -p fusionpanel-api -p fusionpanel-cli
RUST_MIN_STACK=33554432 cargo build --locked --offline --manifest-path "$repo_root/crates/agent/Cargo.toml" --bin fusionpanel-agent
npm --prefix "$repo_root/frontend" run build

DOCKER_BUILDKIT=0 docker build --file "$api_ctx/Dockerfile" --tag "$FUSIONPANEL_S13_API_IMAGE" "$api_ctx"
DOCKER_BUILDKIT=0 docker build --file "$agent_ctx/Dockerfile" --tag "$FUSIONPANEL_S13_AGENT_IMAGE" "$agent_ctx"
DOCKER_BUILDKIT=0 docker build --file "$script_dir/fixtures/sentinel/Dockerfile" --tag "$FUSIONPANEL_S13_SENTINEL_IMAGE" "$script_dir/fixtures/sentinel"
DOCKER_BUILDKIT=0 docker build --file "$script_dir/fixtures/test-app/Dockerfile" --tag "$FUSIONPANEL_S13_TEST_IMAGE_LOCAL" "$script_dir/fixtures/test-app"

compose config --quiet
compose_invoked=1
compose up --detach --wait --wait-timeout 120 registry sentinel

registry_addr=$(compose port registry 5000)
registry_port=${registry_addr##*:}
registry_host="127.0.0.1:${registry_port}"
registry_tag="${registry_host}/fusionpanel-e2e/test-app:${suffix}"
export FUSIONPANEL_S13_REGISTRY_URL="http://${registry_host}"
export FUSIONPANEL_S13_REGISTRY_IMAGE_TAG="$registry_tag"

docker tag "$FUSIONPANEL_S13_TEST_IMAGE_LOCAL" "$registry_tag"
docker push "$registry_tag"
image_ref=$(docker inspect --format='{{index .RepoDigests 0}}' "$registry_tag")
[ -n "$image_ref" ] || {
  echo "Failed to resolve digest-pinned image reference for $registry_tag" >&2
  exit 1
}
export FUSIONPANEL_S13_IMAGE_REF="$image_ref"
export FUSIONPANEL_S13_IMAGE_DIGEST="sha256:${image_ref##*@sha256:}"

sentinel_id=$(compose ps -q sentinel)
sentinel_addr=$(compose port sentinel 8080)
sentinel_port=${sentinel_addr##*:}
export FUSIONPANEL_S13_SENTINEL_ID="$sentinel_id"
export FUSIONPANEL_S13_SENTINEL_URL="http://127.0.0.1:${sentinel_port}"

compose up --detach --wait --wait-timeout 120 api agent

api_addr=$(compose port api 8080)
api_port=${api_addr##*:}
export FUSIONPANEL_FRONTEND_URL="http://127.0.0.1:${api_port}"
export FUSIONPANEL_S13_WORKLOAD_PORT=$(pick_ephemeral_port)
export FUSIONPANEL_S13_COLLISION_PORT=$(pick_ephemeral_port)

"$browser_dir/node_modules/.bin/playwright" test \
  --config "$script_dir/playwright.sprint13.config.mjs"
