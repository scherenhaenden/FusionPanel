#!/usr/bin/env bash
set -Eeuo pipefail

fixture_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
hosting_dir=$(cd -- "$fixture_dir/../.." && pwd)
repo_root=$(cd -- "$fixture_dir/../../../../.." && pwd)
browser_dir="$repo_root/tests/e2e/browser"
compose_file="$fixture_dir/docker-compose.yml"
tmp_root=$(cd -- "${TMPDIR:-/tmp}" && pwd -P)

source "$fixture_dir/preflight.sh"
sprint3_require_non_root_uid "$(id -u)"
git_common_dir=$(git -C "$repo_root" rev-parse --git-common-dir)
if [[ "$git_common_dir" != /* ]]; then
  git_common_dir=$(cd -- "$repo_root/$git_common_dir" && pwd -P)
fi
lock_dir="$git_common_dir/fusionpanel-sprint3-e2e-lock"
sprint3_prepare_runner_lock_dir "$lock_dir" "$(id -u)" || exit 2
sprint3_acquire_runner_lock "$lock_dir/fusionpanel-sprint3-e2e.lock" || exit 2
sprint3_check_available_memory 4096 || exit 2
if sprint3_fixture_build_prerequisites_missing "$repo_root" "$browser_dir"; then
  exit 2
fi
if ! git -C "$repo_root" diff --quiet HEAD -- . \
  ':(exclude)tests/e2e/hosting/fixtures/sprint3/**'; then
  printf 'BLOCKED: runtime source has tracked changes outside the fixture; commit or discard them so the printed commit identifies the binaries.\n' >&2
  exit 2
fi
untracked_source_files=$(git -C "$repo_root" ls-files --others --exclude-standard \
  | awk '$0 !~ /^tests\/e2e\/hosting\/fixtures\/sprint3\//')
if [ -n "$untracked_source_files" ]; then
  printf 'BLOCKED: untracked source files would make artifact provenance ambiguous:\n%s\n' \
    "$untracked_source_files" >&2
  exit 2
fi
sprint3_docker_preflight "$repo_root"

source_commit=$(git -C "$repo_root" rev-parse HEAD)
printf 'Building Sprint 3 E2E artifacts from checkout commit %s.\n' "$source_commit"
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
cargo build --offline --locked --jobs 2 \
  --manifest-path "$repo_root/Cargo.toml" \
  --target-dir "$repo_root/target" \
  -p fusionpanel-api -p fusionpanel-cli
cargo build --offline --locked --jobs 2 \
  --manifest-path "$repo_root/crates/agent/Cargo.toml" \
  --target-dir "$repo_root/target" \
  --bin fusionpanel-agent --bin fusionpanel-nginx-helper
npm --prefix "$repo_root/frontend" run build

if sprint3_fixture_artifacts_missing "$repo_root" "$browser_dir"; then
  exit 2
fi
sprint3_check_free_space / 1048576 \
  && sprint3_check_free_space "$repo_root" 2097152 \
  && sprint3_check_free_space /tmp 1048576 \
  && sprint3_check_free_space "$tmp_root" 1048576 || exit 1
docker_root=$(docker info --format '{{.DockerRootDir}}')
sprint3_check_free_space "$docker_root" 2097152 || exit 1

project_suffix="$(id -u)-$(od -An -N6 -tx1 /dev/urandom | tr -d ' \n')"
project_name="fp-s3-e2e-${project_suffix}"
builder_name="fp-s3-builder-${project_suffix}"
log_dir=${RUNNER_TEMP:-$tmp_root}
mkdir -p "$log_dir"
log_file="$log_dir/fusionpanel-sprint3-compose-${project_suffix}.log"
umask 077
: >"$log_file"

export FUSIONPANEL_S3_REPO_ROOT="$repo_root"
export FUSIONPANEL_S3_PROJECT_SUFFIX="$project_suffix"
export FUSIONPANEL_S3_API_IMAGE="fusionpanel-s3-e2e-api:${project_suffix}"
export FUSIONPANEL_S3_NODE_IMAGE="fusionpanel-s3-e2e-node:${project_suffix}"
export FUSIONPANEL_S3_UID="$(id -u)"
export FUSIONPANEL_S3_GID="$(id -g)"
export FUSIONPANEL_E2E_USERNAME=e2e-admin
FUSIONPANEL_E2E_ADMIN_TOKEN=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
FUSIONPANEL_E2E_PASSWORD=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
export FUSIONPANEL_E2E_ADMIN_TOKEN FUSIONPANEL_E2E_PASSWORD
export COMPOSE_PARALLEL_LIMIT=1

state_dir=$(mktemp -d "$tmp_root/fusionpanel-sprint3-e2e.XXXXXX")
validation_log="$state_dir/nginx-invalid-upstream.log"
export FUSIONPANEL_S3_STATE_DIR="$state_dir"
api_build_context="$state_dir/build-context/api"
node_build_context="$state_dir/build-context/node"
buildkitd_config="$state_dir/buildkitd.toml"
metadata_file="$state_dir/run-metadata.txt"

compose() {
  docker compose --project-name "$project_name" --project-directory "$fixture_dir" \
    --file "$compose_file" "$@"
}

compose_invoked=0
builder_created=0
fixture_config_digest=""

cleanup() {
  local result=$?
  local leftover
  local docker_root
  local current_fixture_digest
  trap - EXIT INT TERM

  if [ "$compose_invoked" -eq 1 ]; then
    {
      if [ -f "$validation_log" ]; then
        printf '%s\n' 'Direct Nginx .invalid candidate check:'
        cat "$validation_log"
      fi
      printf '%s\n' 'Disposable Compose service logs:'
      compose logs --no-color 2>&1 || true
    } >>"$log_file"
    sed -i \
      -e "s/${FUSIONPANEL_E2E_ADMIN_TOKEN}/[REDACTED_ADMIN_TOKEN]/g" \
      -e "s/${FUSIONPANEL_E2E_PASSWORD}/[REDACTED_TEST_PASSWORD]/g" \
      "$log_file" 2>/dev/null || true
    if [ "$result" -ne 0 ]; then
      cat "$log_file" >&2
    fi

    compose down --volumes --remove-orphans || result=1
    for image in "$FUSIONPANEL_S3_API_IMAGE" "$FUSIONPANEL_S3_NODE_IMAGE"; do
      if docker image inspect "$image" >/dev/null 2>&1; then
        docker image rm "$image" >/dev/null || result=1
      fi
    done

    if [ "$builder_created" -eq 1 ]; then
      if docker buildx inspect "$builder_name" >/dev/null 2>&1 \
        && docker buildx rm --force "$builder_name" >/dev/null; then
        builder_created=0
      else
        printf 'Could not verify and remove fixture-owned Buildx builder %s; inspect that exact builder before cleanup.\n' \
          "$builder_name" >&2
        result=1
      fi
    fi

    leftover=$(docker ps -a --filter "label=com.docker.compose.project=$project_name" -q)
    if [ -n "$leftover" ]; then
      printf 'Cleanup left project-owned containers; preserving them for inspection: %s\n' \
        "$leftover" >&2
      result=1
    fi
    leftover=$(docker network ls --filter "label=com.docker.compose.project=$project_name" -q)
    if [ -n "$leftover" ]; then
      printf 'Cleanup left project-owned networks; preserving them for inspection: %s\n' \
        "$leftover" >&2
      result=1
    fi
    leftover=$(docker volume ls --filter "label=com.docker.compose.project=$project_name" -q)
    if [ -n "$leftover" ]; then
      printf 'Cleanup left project-owned volumes; preserving them for inspection: %s\n' \
        "$leftover" >&2
      result=1
    fi
  fi

  if [ -n "$state_dir" ] && [ -d "$state_dir" ] && [ ! -L "$state_dir" ] \
    && [[ "$state_dir" == "$tmp_root"/fusionpanel-sprint3-e2e.* ]]; then
    rm -rf -- "$state_dir"
  elif [ -n "$state_dir" ]; then
    printf 'Refusing to remove an unexpected fixture state path: %s\n' "$state_dir" >&2
    result=1
  fi

  if [ -n "$fixture_config_digest" ]; then
    current_fixture_digest=$(sprint3_fixture_digest "$fixture_dir")
    if [ "$current_fixture_digest" != "$fixture_config_digest" ]; then
      printf 'Fixture inputs changed after their recorded digest was captured.\n' >&2
      result=1
    fi
  fi

  if [ "$compose_invoked" -eq 1 ]; then
    docker system df || result=1
    if docker_root=$(docker info --format '{{.DockerRootDir}}'); then
      sprint3_check_free_space / 1048576 || result=1
      sprint3_check_free_space "$tmp_root" 1048576 || result=1
      sprint3_check_free_space "$docker_root" 2097152 || result=1
    else
      printf 'Could not recheck Docker data-root free space after fixture cleanup.\n' >&2
      result=1
    fi
  fi
  printf 'Fixture run metadata and logs: %s\n' "$log_file"
  exit "$result"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

sprint3_stage_build_context \
  "$api_build_context" \
  "$repo_root/tests/e2e/browser/fixtures/sprint2/Dockerfile" \
  "$repo_root/tests/e2e/browser/fixtures/sprint2/seed-admin.sh"
sprint3_stage_build_context \
  "$node_build_context" \
  "$fixture_dir/Dockerfile" \
  "$fixture_dir/start-node.sh"
export FUSIONPANEL_S3_API_BUILD_CONTEXT="$api_build_context"
export FUSIONPANEL_S3_NODE_BUILD_CONTEXT="$node_build_context"

fixture_config_digest=$(sprint3_fixture_digest "$fixture_dir")
api_build_context_digest=$(sprint3_fixture_digest "$api_build_context")
node_build_context_digest=$(sprint3_fixture_digest "$node_build_context")
compose_file_digest=$(sha256sum "$compose_file" | awk '{print $1}')
if ! docker_engine_version=$(docker version --format '{{.Server.Version}}') \
  || [ -z "$docker_engine_version" ]; then
  printf 'BLOCKED: could not record the local Docker Engine version.\n' >&2
  exit 2
fi
if ! docker_compose_version=$(docker compose version) || [ -z "$docker_compose_version" ]; then
  printf 'BLOCKED: could not record the Docker Compose version.\n' >&2
  exit 2
fi
if ! docker_buildx_version=$(docker buildx version) || [ -z "$docker_buildx_version" ]; then
  printf 'BLOCKED: could not record the Docker Buildx version.\n' >&2
  exit 2
fi
{
  printf 'source_commit=%s\n' "$source_commit"
  printf 'fixture_config_sha256=%s\n' "$fixture_config_digest"
  printf 'api_build_context_sha256=%s\n' "$api_build_context_digest"
  printf 'node_build_context_sha256=%s\n' "$node_build_context_digest"
  printf 'compose_file_sha256=%s\n' "$compose_file_digest"
  printf 'docker_engine_version=%s\n' "$docker_engine_version"
  printf 'docker_compose_version=%s\n' "$docker_compose_version"
  printf 'docker_buildx_version=%s\n' "$docker_buildx_version"
  printf 'run_scoped_builder_cache_max_used_space=512MB\n'
} >"$metadata_file"
{
  printf '%s\n' 'Sprint 3 E2E run metadata:'
  cat "$metadata_file"
} | tee "$log_file"

mkdir -p "$state_dir/sites/static.sprint3.test"
chmod 0700 "$state_dir"
chmod 0755 "$state_dir/sites" "$state_dir/sites/static.sprint3.test"
printf '%s\n' 'Sprint 3 static site marker' >"$state_dir/sites/static.sprint3.test/index.html"
chmod 0644 "$state_dir/sites/static.sprint3.test/index.html"

compose config --quiet
compose_invoked=1
sprint3_write_bounded_buildkit_config "$buildkitd_config"
if docker buildx inspect "$builder_name" >/dev/null 2>&1; then
  printf 'BLOCKED: refusing to reuse an existing Buildx builder named %s.\n' "$builder_name" >&2
  exit 2
fi
builder_created=1
docker buildx create \
  --name "$builder_name" \
  --driver docker-container \
  --buildkitd-config "$buildkitd_config" \
  --bootstrap >/dev/null
docker buildx build \
  --builder "$builder_name" \
  --file "$api_build_context/Dockerfile" \
  --tag "$FUSIONPANEL_S3_API_IMAGE" \
  --load "$api_build_context"
docker buildx build \
  --builder "$builder_name" \
  --file "$node_build_context/Dockerfile" \
  --tag "$FUSIONPANEL_S3_NODE_IMAGE" \
  --load "$node_build_context"
compose up --detach --wait --wait-timeout 120

if compose exec -T node nginx -t -q -c /etc/nginx/fixture-invalid-upstream.conf \
  >"$validation_log" 2>&1; then
  printf 'The pinned Nginx accepted the syntactically valid .invalid upstream candidate.\n' >&2
  cat "$validation_log" >&2
  exit 1
fi
if ! grep -E 'host not found in upstream .*unresolvable\.invalid' "$validation_log"; then
  printf 'Nginx rejected the .invalid probe for an unexpected reason.\n' >&2
  cat "$validation_log" >&2
  exit 1
fi

api_address=$(compose port node 8080)
nginx_address=$(compose port node 80)
api_port=${api_address##*:}
nginx_port=${nginx_address##*:}
export FUSIONPANEL_FRONTEND_URL="http://127.0.0.1:${api_port}"
export FUSIONPANEL_S3_NGINX_URL="http://127.0.0.1:${nginx_port}"
export FUSIONPANEL_S3_NGINX_CONFIG="$fixture_dir/nginx.conf"

"$browser_dir/node_modules/.bin/playwright" test \
  --config "$hosting_dir/playwright.sprint3.config.mjs"

if compose exec -T node sh -c \
  'test -d /etc/nginx/fusionpanel/sites-enabled \
   && test ! -e /etc/nginx/fusionpanel/sites-enabled/failed.sprint3.test.conf \
   && test ! -L /etc/nginx/fusionpanel/sites-enabled/failed.sprint3.test.conf \
   && ! grep -R -qF "domain=failed.sprint3.test" \
     /etc/nginx/fusionpanel/sites-available /etc/nginx/fusionpanel/staging'; then
  printf 'PASS: rejected .invalid site left no managed candidate or enabled config.\n'
else
  printf 'The rejected .invalid site left managed configuration behind.\n' >&2
  exit 1
fi
