#!/usr/bin/env bash

sprint3_fixture_artifacts_missing() {
  local repo_root="$1"
  local browser_dir="$2"
  local missing=0

  for artifact in \
    "$repo_root/target/debug/fusionpanel-api" \
    "$repo_root/target/debug/fusionpanel-cli" \
    "$repo_root/target/debug/fusionpanel-agent" \
    "$repo_root/target/debug/fusionpanel-nginx-helper"; do
    if [ ! -x "$artifact" ]; then
      printf 'BLOCKED: required Sprint 3 E2E artifact is missing: %s\n' "$artifact" >&2
      missing=1
    fi
  done

  if [ ! -f "$repo_root/frontend/dist/index.html" ]; then
    printf 'BLOCKED: required Sprint 3 frontend build is missing: %s\n' \
      "$repo_root/frontend/dist/index.html" >&2
    missing=1
  fi
  if [ ! -x "$browser_dir/node_modules/.bin/playwright" ]; then
    printf 'BLOCKED: Playwright is unavailable; use the existing browser test dependency install.\n' >&2
    missing=1
  fi

  if [ "$missing" -eq 1 ]; then return 0; fi
  return 1
}

sprint3_fixture_build_prerequisites_missing() {
  local repo_root="$1"
  local browser_dir="$2"
  local missing=0

  for tool in cargo npm; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      printf 'BLOCKED: %s is required to build fixture artifacts from this checkout.\n' \
        "$tool" >&2
      missing=1
    fi
  done

  if [ ! -f "$repo_root/crates/agent/src/nginx_helper_rpc.rs" ] \
    || [ ! -f "$repo_root/crates/agent/src/bin/fusionpanel-nginx-helper.rs" ]; then
    printf 'BLOCKED: the current checkout does not contain the Nginx helper RPC implementation; integrate the proxy helper branch before running this fixture.\n' >&2
    missing=1
  fi
  if [ ! -x "$repo_root/frontend/node_modules/.bin/tsc" ] \
    || [ ! -x "$repo_root/frontend/node_modules/.bin/vite" ]; then
    printf 'BLOCKED: frontend build dependencies are missing; install them before running this fixture.\n' >&2
    missing=1
  fi
  if [ ! -x "$browser_dir/node_modules/.bin/playwright" ]; then
    printf 'BLOCKED: Playwright is unavailable; use the existing browser test dependency install.\n' >&2
    missing=1
  fi

  if [ "$missing" -eq 1 ]; then return 0; fi
  return 1
}

sprint3_check_free_space() {
  local path="$1"
  local minimum_kib="$2"
  local available_kib

  if ! available_kib=$(df -Pk -- "$path" | awk 'END { print $4 }') \
    || ! [[ "$available_kib" =~ ^[0-9]+$ ]]; then
    printf 'BLOCKED: could not determine free space for %s.\n' "$path" >&2
    return 1
  fi

  if [ "$available_kib" -lt "$minimum_kib" ]; then
    printf 'BLOCKED: %s has %s KiB free; this fixture requires at least %s KiB.\n' \
      "$path" "$available_kib" "$minimum_kib" >&2
    return 1
  fi
}

sprint3_check_available_memory() {
  local minimum_mib="$1"
  local meminfo_path="${2:-/proc/meminfo}"
  local available_kib
  local minimum_kib

  if ! [[ "$minimum_mib" =~ ^[0-9]+$ ]] || [ ! -r "$meminfo_path" ]; then
    printf 'BLOCKED: could not verify available host memory from %s.\n' "$meminfo_path" >&2
    return 1
  fi
  if ! available_kib=$(awk '$1 == "MemAvailable:" { print $2; exit }' "$meminfo_path") \
    || ! [[ "$available_kib" =~ ^[0-9]+$ ]]; then
    printf 'BLOCKED: MemAvailable is missing or invalid in %s.\n' "$meminfo_path" >&2
    return 1
  fi

  minimum_kib=$((minimum_mib * 1024))
  if [ "$available_kib" -lt "$minimum_kib" ]; then
    printf 'BLOCKED: host has %s MiB available; this fixture requires at least %s MiB before building.\n' \
      "$((available_kib / 1024))" "$minimum_mib" >&2
    return 1
  fi
}

sprint3_acquire_runner_lock() {
  local lock_path="$1"

  if ! command -v flock >/dev/null 2>&1; then
    printf 'BLOCKED: flock is required to prevent concurrent Sprint 3 fixture builds.\n' >&2
    return 1
  fi
  if ! exec 9>"$lock_path"; then
    printf 'BLOCKED: could not open the fixture lock file at %s.\n' "$lock_path" >&2
    return 1
  fi
  if ! flock --nonblock 9; then
    exec 9>&-
    printf 'BLOCKED: another Sprint 3 fixture run is active (lock: %s).\n' "$lock_path" >&2
    return 1
  fi
}

sprint3_prepare_runner_lock_dir() {
  local lock_dir="$1"
  local expected_uid="$2"
  local parent_dir="${lock_dir%/*}"
  local owner mode parent_owner parent_mode

  if [ -L "$parent_dir" ] || [ ! -d "$parent_dir" ] || [ -L "$lock_dir" ]; then
    printf 'BLOCKED: fixture lock path must use real directories: %s\n' "$lock_dir" >&2
    return 1
  fi
  parent_owner=$(stat -c '%u' -- "$parent_dir")
  parent_mode=$(stat -c '%a' -- "$parent_dir")
  if [ "$parent_owner" != "$expected_uid" ] \
    || (( (8#$parent_mode) & 0022 )); then
    printf 'BLOCKED: fixture lock parent must be user-owned and not group/world writable: %s\n' \
      "$parent_dir" >&2
    return 1
  fi

  if [ ! -e "$lock_dir" ] && ! mkdir -m 700 -- "$lock_dir"; then
    printf 'BLOCKED: could not create the private fixture lock directory: %s\n' "$lock_dir" >&2
    return 1
  fi
  if [ -L "$lock_dir" ] || [ ! -d "$lock_dir" ]; then
    printf 'BLOCKED: fixture lock path is not a real directory: %s\n' "$lock_dir" >&2
    return 1
  fi
  owner=$(stat -c '%u' -- "$lock_dir")
  mode=$(stat -c '%a' -- "$lock_dir")
  if [ "$owner" != "$expected_uid" ] || [ "$mode" != 700 ]; then
    printf 'BLOCKED: fixture lock directory must be owned by UID %s with mode 0700: %s\n' \
      "$expected_uid" "$lock_dir" >&2
    return 1
  fi
}

sprint3_require_non_root_uid() {
  local uid="$1"

  if ! [[ "$uid" =~ ^[0-9]+$ ]]; then
    printf 'BLOCKED: the invoking UID could not be verified.\n' >&2
    return 1
  fi
  if [ "$uid" -eq 0 ]; then
    printf 'BLOCKED: run the Sprint 3 fixture as a non-root host user.\n' >&2
    return 1
  fi
}

sprint3_validate_docker_target() {
  local context_name="$1"
  local context_endpoint="$2"
  local docker_host="$3"
  local endpoint_scheme="${context_endpoint%%:*}"

  if [[ "$context_endpoint" != unix:///* ]]; then
    printf 'BLOCKED: Docker context "%s" is not local (endpoint scheme: %s); use a Unix-socket context on this host.\n' \
      "$context_name" "${endpoint_scheme:-unknown}" >&2
    return 1
  fi
  if [ -n "$docker_host" ] && [[ "$docker_host" != unix:///* ]]; then
    printf 'BLOCKED: DOCKER_HOST is not a local Unix socket; remote Docker endpoints are not allowed for this fixture.\n' >&2
    return 1
  fi
  return 0
}

sprint3_validate_fixture_log_rotation() {
  local compose_file="$1"
  local max_files

  if [ ! -f "$compose_file" ] || ! max_files=$(awk '
    $1 == "x-sprint3-logging:" { in_logging = 1; next }
    in_logging && /^[^[:space:]]/ { in_logging = 0 }
    in_logging && $1 == "max-file:" {
      gsub(/\"/, "", $2)
      print $2
      found++
    }
    END { if (found != 1) exit 1 }
  ' "$compose_file"); then
    printf 'BLOCKED: could not read one Sprint 3 Docker log rotation limit from %s.\n' \
      "$compose_file" >&2
    return 1
  fi
  if ! [[ "$max_files" =~ ^[0-9]+$ ]] || [ "$max_files" -lt 2 ]; then
    printf 'BLOCKED: Sprint 3 Docker local log rotation must retain at least two files because compression is enabled by default.\n' >&2
    return 1
  fi
}

sprint3_fixture_digest() {
  local fixture_dir="$1"

  (
    cd -- "$fixture_dir" || exit 1
    find . -type f -print0 | sort -z | xargs -0 sha256sum | sha256sum | awk '{ print $1 }'
  )
}

sprint3_stage_build_context() {
  local context_dir="$1"
  local dockerfile_source="$2"
  local entrypoint_source="$3"
  local entrypoint_name="${entrypoint_source##*/}"
  local existing_entry

  mkdir -p -- "$context_dir"
  existing_entry=$(find "$context_dir" -mindepth 1 -maxdepth 1 -print -quit)
  if [ -n "$existing_entry" ]; then
    printf 'BLOCKED: refusing to stage into a non-empty build context: %s\n' "$context_dir" >&2
    return 1
  fi
  if ! cp -- "$dockerfile_source" "$context_dir/Dockerfile" \
    || ! cp -- "$entrypoint_source" "$context_dir/$entrypoint_name"; then
    printf 'BLOCKED: could not stage the minimal Docker build context at %s.\n' "$context_dir" >&2
    return 1
  fi
}

sprint3_write_bounded_buildkit_config() {
  local config_path="$1"

  cat >"$config_path" <<'EOF'
[worker.oci]
  max-parallelism = 1
  gc = true
  reservedSpace = "64MB"
  maxUsedSpace = "512MB"
  minFreeSpace = "1GB"
EOF
}

sprint3_docker_preflight() {
  local repo_root="$1"
  local caller_uid docker_context context_endpoint docker_root active_e2e_projects

  if ! caller_uid=$(id -u); then
    printf 'BLOCKED: could not determine the host caller UID.\n' >&2
    return 1
  fi
  sprint3_require_non_root_uid "$caller_uid" || return 1

  if ! command -v docker >/dev/null 2>&1; then
    printf 'BLOCKED: Docker CLI is not installed.\n' >&2
    return 1
  fi
  if [ -n "${DOCKER_CONTEXT:-}" ]; then
    docker_context="$DOCKER_CONTEXT"
  elif ! docker_context=$(docker context show); then
    printf 'BLOCKED: could not determine the selected Docker context.\n' >&2
    return 1
  fi
  if ! context_endpoint=$(docker context inspect "$docker_context" \
    --format '{{ (index .Endpoints "docker").Host }}'); then
    printf 'BLOCKED: could not inspect Docker context "%s".\n' "$docker_context" >&2
    return 1
  fi
  sprint3_validate_docker_target "$docker_context" "$context_endpoint" "${DOCKER_HOST:-}" \
    || return 1

  if ! docker compose version >/dev/null 2>&1; then
    printf 'BLOCKED: Docker Compose v2 is unavailable.\n' >&2
    return 1
  fi
  if ! docker_root=$(docker info --format '{{.DockerRootDir}}'); then
    printf 'BLOCKED: Docker daemon is unavailable.\n' >&2
    return 1
  fi
  if [ -z "$docker_root" ]; then
    printf 'BLOCKED: Docker reported an empty data-root path.\n' >&2
    return 1
  fi

  sprint3_check_free_space / 1048576 \
    && sprint3_check_free_space "$repo_root" 10485760 \
    && sprint3_check_free_space /tmp 1048576 \
    && sprint3_check_free_space "${TMPDIR:-/tmp}" 1048576 \
    && sprint3_check_free_space "$docker_root" 2097152 || return 1

  printf '%s\n' 'Docker usage before Sprint 3 E2E:'
  docker system df
  docker ps --format 'table {{.Names}}\t{{.Image}}\t{{.Status}}'

  active_e2e_projects=$(docker ps \
    --format '{{.Label "com.docker.compose.project"}}' \
    | grep -E '^fp-s[0-9]+-e2e-' || true)
  if [ -n "$active_e2e_projects" ]; then
    printf 'BLOCKED: another FusionPanel E2E Compose project is active:\n%s\n' \
      "$active_e2e_projects" >&2
    return 1
  fi

}
