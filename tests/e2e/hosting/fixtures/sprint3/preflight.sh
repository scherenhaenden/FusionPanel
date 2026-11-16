#!/usr/bin/env bash

sprint3_fixture_artifacts_missing() {
  local repo_root="$1"
  local browser_dir="$2"
  local missing=0

  for artifact in \
    "$repo_root/target/debug/fusionpanel-api" \
    "$repo_root/target/debug/fusionpanel-cli" \
    "$repo_root/target/debug/fusionpanel-agent"; do
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

  return "$missing"
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

sprint3_docker_preflight() {
  local docker_root active_e2e_projects

  if ! command -v docker >/dev/null 2>&1; then
    printf 'BLOCKED: Docker CLI is not installed.\n' >&2
    return 1
  fi
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

  sprint3_check_free_space / 1048576 \
    && sprint3_check_free_space "${TMPDIR:-/tmp}" 1048576 \
    && sprint3_check_free_space "$docker_root" 2097152
}
