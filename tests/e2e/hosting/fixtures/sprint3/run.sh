#!/usr/bin/env bash
set -Eeuo pipefail

fixture_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
hosting_dir=$(cd -- "$fixture_dir/../.." && pwd)
repo_root=$(cd -- "$fixture_dir/../../../../.." && pwd)
browser_dir="$repo_root/tests/e2e/browser"
compose_file="$fixture_dir/docker-compose.yml"
tmp_root=$(cd -- "${TMPDIR:-/tmp}" && pwd -P)

source "$fixture_dir/preflight.sh"
sprint3_docker_preflight
if sprint3_fixture_artifacts_missing "$repo_root" "$browser_dir"; then
  printf 'Build the API, CLI, agent, and frontend before running this fixture.\n' >&2
  exit 2
fi

project_suffix="$(id -u)-$(od -An -N6 -tx1 /dev/urandom | tr -d ' \n')"
project_name="fp-s3-e2e-${project_suffix}"
log_dir=${RUNNER_TEMP:-$tmp_root}
mkdir -p "$log_dir"
log_file="$log_dir/fusionpanel-sprint3-compose-${project_suffix}.log"
umask 077

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

base_config_hash=$(sha256sum "$fixture_dir/nginx.conf" | awk '{print $1}')
state_dir=$(mktemp -d "$tmp_root/fusionpanel-sprint3-e2e.XXXXXX")
validation_log="$state_dir/nginx-invalid-upstream.log"
export FUSIONPANEL_S3_STATE_DIR="$state_dir"

compose() {
  docker compose --project-name "$project_name" --project-directory "$fixture_dir" \
    --file "$compose_file" "$@"
}

compose_invoked=0

cleanup() {
  local result=$?
  local leftover
  local docker_root
  trap - EXIT INT TERM

  if [ "$compose_invoked" -eq 1 ]; then
    {
      if [ -f "$validation_log" ]; then
        printf '%s\n' 'Direct Nginx .invalid candidate check:'
        cat "$validation_log"
      fi
      printf '%s\n' 'Disposable Compose service logs:'
      compose logs --no-color 2>&1 || true
    } >"$log_file"
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
    printf 'Compose logs: %s\n' "$log_file"
  fi

  if [ -n "$state_dir" ] && [ -d "$state_dir" ] && [ ! -L "$state_dir" ] \
    && [[ "$state_dir" == "$tmp_root"/fusionpanel-sprint3-e2e.* ]]; then
    rm -rf -- "$state_dir"
  elif [ -n "$state_dir" ]; then
    printf 'Refusing to remove an unexpected fixture state path: %s\n' "$state_dir" >&2
    result=1
  fi

  current_hash=$(sha256sum "$fixture_dir/nginx.conf" | awk '{print $1}')
  if [ "$current_hash" != "$base_config_hash" ]; then
    printf 'The unrelated-vhost base config changed during the fixture.\n' >&2
    result=1
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
  exit "$result"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

mkdir -p "$state_dir/sites/static.sprint3.test"
chmod 0700 "$state_dir"
chmod 0755 "$state_dir/sites" "$state_dir/sites/static.sprint3.test"
printf '%s\n' 'Sprint 3 static site marker' >"$state_dir/sites/static.sprint3.test/index.html"
chmod 0644 "$state_dir/sites/static.sprint3.test/index.html"

compose config --quiet
compose_invoked=1
compose up --detach --build --wait --wait-timeout 120

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
