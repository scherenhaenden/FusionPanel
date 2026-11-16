#!/usr/bin/env bash
set -Eeuo pipefail

fixture_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
source "$fixture_dir/preflight.sh"
temporary_dir=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-sprint3-preflight.XXXXXX")
trap 'rm -rf -- "$temporary_dir"' EXIT

mkdir -p "$temporary_dir/target/debug" "$temporary_dir/frontend/dist" \
  "$temporary_dir/frontend/node_modules/.bin" \
  "$temporary_dir/browser/node_modules/.bin" \
  "$temporary_dir/crates/agent/src/bin" "$temporary_dir/crates/agent/src" \
  "$temporary_dir/bin"
for tool in cargo npm; do
  touch "$temporary_dir/bin/$tool"
  chmod +x "$temporary_dir/bin/$tool"
done
touch "$temporary_dir/frontend/node_modules/.bin/tsc" \
  "$temporary_dir/frontend/node_modules/.bin/vite"
chmod +x "$temporary_dir/frontend/node_modules/.bin/tsc" \
  "$temporary_dir/frontend/node_modules/.bin/vite"
touch "$temporary_dir/target/debug/fusionpanel-api" \
  "$temporary_dir/target/debug/fusionpanel-cli" \
  "$temporary_dir/target/debug/fusionpanel-agent" \
  "$temporary_dir/target/debug/fusionpanel-nginx-helper" \
  "$temporary_dir/frontend/dist/index.html" \
  "$temporary_dir/browser/node_modules/.bin/playwright"
chmod +x "$temporary_dir/target/debug/fusionpanel-api" \
  "$temporary_dir/target/debug/fusionpanel-cli" \
  "$temporary_dir/target/debug/fusionpanel-agent" \
  "$temporary_dir/target/debug/fusionpanel-nginx-helper" \
  "$temporary_dir/browser/node_modules/.bin/playwright"

if ! sprint3_fixture_artifacts_missing "$temporary_dir" "$temporary_dir/browser"; then
  printf 'PASS: complete fixture artifacts are accepted.\n'
else
  printf 'FAIL: complete fixture artifacts were rejected.\n' >&2
  exit 1
fi

if PATH="$temporary_dir/bin:$PATH" \
  sprint3_fixture_build_prerequisites_missing "$temporary_dir" "$temporary_dir/browser" \
  >"$temporary_dir/build-prerequisites.log" 2>&1; then
  if ! grep -q 'does not contain the Nginx helper RPC implementation' \
    "$temporary_dir/build-prerequisites.log"; then
    printf 'FAIL: the checkout was blocked for an unexpected build prerequisite.\n' >&2
    cat "$temporary_dir/build-prerequisites.log" >&2
    exit 1
  fi
  printf 'PASS: fixture build blocks until the checkout includes the helper RPC source.\n'
else
  printf 'FAIL: a checkout without the Nginx helper source was accepted.\n' >&2
  exit 1
fi

touch "$temporary_dir/crates/agent/src/nginx_helper_rpc.rs" \
  "$temporary_dir/crates/agent/src/bin/fusionpanel-nginx-helper.rs"
if PATH="$temporary_dir/bin:$PATH" \
  sprint3_fixture_build_prerequisites_missing "$temporary_dir" "$temporary_dir/browser" \
  >"$temporary_dir/complete-build-prerequisites.log" 2>&1; then
  printf 'FAIL: complete local build prerequisites were rejected.\n' >&2
  cat "$temporary_dir/complete-build-prerequisites.log" >&2
  exit 1
else
  printf 'PASS: checkout helper source and installed local dependencies satisfy build preflight.\n'
fi

if sprint3_require_non_root_uid 0 >/dev/null 2>&1; then
  printf 'FAIL: root invocation was accepted.\n' >&2
  exit 1
else
  printf 'PASS: root invocation is blocked before Docker starts.\n'
fi

mkdir -p "$temporary_dir/root-preflight-bin"
cat >"$temporary_dir/root-preflight-bin/id" <<'EOF'
#!/bin/sh
printf '0\n'
EOF
cat >"$temporary_dir/root-preflight-bin/docker" <<EOF
#!/bin/sh
touch "$temporary_dir/docker-was-called"
exit 97
EOF
chmod +x "$temporary_dir/root-preflight-bin/id" "$temporary_dir/root-preflight-bin/docker"
if PATH="$temporary_dir/root-preflight-bin:$PATH" \
  sprint3_docker_preflight "$temporary_dir" \
  >"$temporary_dir/root-preflight.log" 2>&1; then
  printf 'FAIL: the Docker preflight accepted a simulated root caller.\n' >&2
  exit 1
elif ! grep -q 'run the Sprint 3 fixture as a non-root host user' \
  "$temporary_dir/root-preflight.log" || [ -e "$temporary_dir/docker-was-called" ]; then
  printf 'FAIL: root was not rejected before Docker was called.\n' >&2
  cat "$temporary_dir/root-preflight.log" >&2
  exit 1
else
  printf 'PASS: the Docker preflight rejects UID 0 before calling Docker.\n'
fi

if sprint3_require_non_root_uid 1000; then
  printf 'PASS: non-root fixture caller is accepted.\n'
else
  printf 'FAIL: a non-root fixture caller was rejected.\n' >&2
  exit 1
fi

if sprint3_validate_docker_target local "unix:///var/run/docker.sock" ""; then
  printf 'PASS: a local Unix-socket Docker context is accepted.\n'
else
  printf 'FAIL: a local Unix-socket Docker context was rejected.\n' >&2
  exit 1
fi

if sprint3_validate_docker_target remote "ssh://docker@example.test" "" \
  >"$temporary_dir/remote-context.log" 2>&1; then
  printf 'FAIL: a remote Docker context was accepted.\n' >&2
  exit 1
elif ! grep -q 'is not local' "$temporary_dir/remote-context.log"; then
  printf 'FAIL: remote Docker context rejection was not explained.\n' >&2
  cat "$temporary_dir/remote-context.log" >&2
  exit 1
else
  printf 'PASS: a remote Docker context is rejected.\n'
fi

if sprint3_validate_docker_target local "unix:///var/run/docker.sock" "tcp://docker.example.test:2376" \
  >"$temporary_dir/remote-docker-host.log" 2>&1; then
  printf 'FAIL: a remote DOCKER_HOST was accepted.\n' >&2
  exit 1
elif ! grep -q 'DOCKER_HOST is not a local Unix socket' "$temporary_dir/remote-docker-host.log"; then
  printf 'FAIL: remote DOCKER_HOST rejection was not explained.\n' >&2
  cat "$temporary_dir/remote-docker-host.log" >&2
  exit 1
else
  printf 'PASS: a remote DOCKER_HOST is rejected.\n'
fi

if ! sprint3_validate_docker_target local "unix:///var/run/docker.sock" "unix:///run/user/1000/docker.sock"; then
  printf 'FAIL: local Unix-socket DOCKER_HOST was rejected.\n' >&2
  exit 1
fi

mkdir -p "$temporary_dir/digest-input"
printf 'config one\n' >"$temporary_dir/digest-input/compose.yml"
digest_one=$(sprint3_fixture_digest "$temporary_dir/digest-input")
digest_repeat=$(sprint3_fixture_digest "$temporary_dir/digest-input")
if [ "$digest_one" != "$digest_repeat" ]; then
  printf 'FAIL: fixture digest changed without input changes.\n' >&2
  exit 1
fi
printf 'config two\n' >"$temporary_dir/digest-input/compose.yml"
digest_two=$(sprint3_fixture_digest "$temporary_dir/digest-input")
if [ "$digest_one" = "$digest_two" ]; then
  printf 'FAIL: fixture digest did not change when input changed.\n' >&2
  exit 1
fi
printf 'PASS: fixture digest is stable and changes with the input.\n'

mkdir -p "$temporary_dir/stage-source" "$temporary_dir/stage-context"
printf 'FROM scratch\n' >"$temporary_dir/stage-source/Dockerfile"
printf '#!/bin/sh\n' >"$temporary_dir/stage-source/start-node.sh"
printf 'unused\n' >"$temporary_dir/stage-source/extra.conf"
if ! sprint3_stage_build_context \
  "$temporary_dir/stage-context" \
  "$temporary_dir/stage-source/Dockerfile" \
  "$temporary_dir/stage-source/start-node.sh"; then
  printf 'FAIL: minimal build context staging failed.\n' >&2
  exit 1
fi
staged_files=$(cd "$temporary_dir/stage-context" && find . -type f -printf '%P\n' | sort)
if [ "$staged_files" != $'Dockerfile\nstart-node.sh' ]; then
  printf 'FAIL: build context included unexpected files:\n%s\n' "$staged_files" >&2
  exit 1
fi
printf 'PASS: staged Docker build context contains only the Dockerfile and entrypoint.\n'

if sprint3_stage_build_context \
  "$temporary_dir/stage-context" \
  "$temporary_dir/stage-source/Dockerfile" \
  "$temporary_dir/stage-source/start-node.sh" >/dev/null 2>&1; then
  printf 'FAIL: staging accepted a non-empty build context.\n' >&2
  exit 1
fi

sprint3_write_bounded_buildkit_config "$temporary_dir/buildkitd.toml"
if ! grep -q '^  gc = true$' "$temporary_dir/buildkitd.toml" \
  || ! grep -q '^  maxUsedSpace = "512MB"$' "$temporary_dir/buildkitd.toml" \
  || ! grep -q '^  max-parallelism = 1$' "$temporary_dir/buildkitd.toml"; then
  printf 'FAIL: run-scoped BuildKit cache configuration is not bounded.\n' >&2
  cat "$temporary_dir/buildkitd.toml" >&2
  exit 1
fi
if grep -Eq 'docker (system|builder|volume) prune' "$fixture_dir/run.sh"; then
  printf 'FAIL: fixture runner contains a global Docker prune.\n' >&2
  exit 1
fi
if ! grep -Fq 'maxUsedSpace = "512MB"' "$fixture_dir/preflight.sh" \
  || ! grep -Fq 'docker buildx rm --force' "$fixture_dir/run.sh" \
  || ! grep -Fq 'fixture_config_sha256=%s' "$fixture_dir/run.sh" \
  || ! grep -Fq 'docker_engine_version=%s' "$fixture_dir/run.sh" \
  || ! grep -Fq 'docker_compose_version=%s' "$fixture_dir/run.sh"; then
  printf 'FAIL: bounded builder cleanup or fixture/version evidence is missing.\n' >&2
  exit 1
fi
if ! grep -Fq 'FUSIONPANEL_S3_API_BUILD_CONTEXT' "$fixture_dir/docker-compose.yml" \
  || ! grep -Fq 'FUSIONPANEL_S3_NODE_BUILD_CONTEXT' "$fixture_dir/docker-compose.yml"; then
  printf 'FAIL: Compose does not use staged narrow image build contexts.\n' >&2
  exit 1
fi
if ! grep -Eq '^[[:space:]]+max-file: "([2-9]|[1-9][0-9]+)"$' \
  "$fixture_dir/docker-compose.yml"; then
  printf 'FAIL: Compose log rotation must retain at least two files for compressed Docker logs.\n' >&2
  exit 1
fi
printf 'PASS: isolated BuildKit cache has a 512 MiB GC cap and no global prune.\n'

if grep -Eq '^[[:space:]]*install -d.*[[:space:]](-o|--owner|-g|--group)([[:space:]=]|$)' \
  "$fixture_dir/start-node.sh" \
  || grep -Eq '(^|[[:space:]])(chown|chgrp)([[:space:]]|$)' "$fixture_dir/start-node.sh"; then
  printf 'FAIL: node startup requests ownership changes that need CAP_CHOWN.\n' >&2
  exit 1
fi
printf 'PASS: node startup creates its directories without ownership changes.\n'

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

cat >"$temporary_dir/meminfo" <<'EOF'
MemTotal:       16384 kB
MemAvailable:   8192 kB
EOF
if sprint3_check_available_memory 8 "$temporary_dir/meminfo"; then
  printf 'PASS: the available-memory check accepts its minimum.\n'
else
  printf 'FAIL: the available-memory check rejected its minimum.\n' >&2
  exit 1
fi
if sprint3_check_available_memory 9 "$temporary_dir/meminfo" \
  >"$temporary_dir/low-memory.log" 2>&1; then
  printf 'FAIL: the available-memory check accepted an unavailable minimum.\n' >&2
  exit 1
elif ! grep -q 'requires at least 9 MiB' "$temporary_dir/low-memory.log"; then
  printf 'FAIL: low available memory was not explained.\n' >&2
  cat "$temporary_dir/low-memory.log" >&2
  exit 1
else
  printf 'PASS: the available-memory check blocks a low-memory host.\n'
fi

runner_lock_dir="$temporary_dir/private-runner-lock"
sprint3_prepare_runner_lock_dir "$runner_lock_dir" "$(id -u)"
runner_lock="$runner_lock_dir/sprint3-run.lock"
sprint3_acquire_runner_lock "$runner_lock"
if bash -c 'source "$1"; sprint3_acquire_runner_lock "$2"' \
  _ "$fixture_dir/preflight.sh" "$runner_lock" >"$temporary_dir/lock-contention.log" 2>&1; then
  printf 'FAIL: a concurrent fixture runner acquired the active lock.\n' >&2
  exec 9>&-
  exit 1
elif ! grep -q 'another Sprint 3 fixture run is active' "$temporary_dir/lock-contention.log"; then
  printf 'FAIL: the runner lock contention was not explained.\n' >&2
  cat "$temporary_dir/lock-contention.log" >&2
  exec 9>&-
  exit 1
else
  printf 'PASS: the runner lock rejects a concurrent fixture invocation.\n'
fi
exec 9>&-

ln -s "$temporary_dir/private-runner-lock" "$temporary_dir/unsafe-runner-lock"
if sprint3_prepare_runner_lock_dir "$temporary_dir/unsafe-runner-lock" "$(id -u)" \
  >"$temporary_dir/unsafe-lock.log" 2>&1; then
  printf 'FAIL: a symlinked fixture lock directory was accepted.\n' >&2
  exit 1
elif ! grep -q 'must use real directories' "$temporary_dir/unsafe-lock.log"; then
  printf 'FAIL: symlinked fixture lock directory rejection was not explained.\n' >&2
  cat "$temporary_dir/unsafe-lock.log" >&2
  exit 1
else
  printf 'PASS: a symlinked fixture lock directory is rejected.\n'
fi

mkdir -m 0770 "$temporary_dir/group-writable-parent"
if sprint3_prepare_runner_lock_dir \
  "$temporary_dir/group-writable-parent/private-lock" "$(id -u)" \
  >"$temporary_dir/group-writable-parent.log" 2>&1; then
  printf 'FAIL: a group-writable fixture lock parent was accepted.\n' >&2
  exit 1
elif ! grep -q 'not group/world writable' "$temporary_dir/group-writable-parent.log"; then
  printf 'FAIL: unsafe fixture lock parent rejection was not explained.\n' >&2
  cat "$temporary_dir/group-writable-parent.log" >&2
  exit 1
else
  printf 'PASS: a group-writable fixture lock parent is rejected.\n'
fi

rm "$temporary_dir/target/debug/fusionpanel-agent"
if sprint3_fixture_artifacts_missing "$temporary_dir" "$temporary_dir/browser" \
  >"$temporary_dir/missing-agent.log" 2>&1; then
  if ! grep -q 'target/debug/fusionpanel-agent' "$temporary_dir/missing-agent.log"; then
    printf 'FAIL: missing agent binary was not identified by the artifact check.\n' >&2
    cat "$temporary_dir/missing-agent.log" >&2
    exit 1
  fi
  printf 'PASS: a missing agent binary blocks the fixture.\n'
else
  printf 'FAIL: a missing agent binary was accepted.\n' >&2
  exit 1
fi

rm "$temporary_dir/target/debug/fusionpanel-nginx-helper"
if sprint3_fixture_artifacts_missing "$temporary_dir" "$temporary_dir/browser" \
  >"$temporary_dir/missing-helper.log" 2>&1; then
  if ! grep -q 'target/debug/fusionpanel-nginx-helper' "$temporary_dir/missing-helper.log"; then
    printf 'FAIL: missing helper binary was not identified by the artifact check.\n' >&2
    cat "$temporary_dir/missing-helper.log" >&2
    exit 1
  fi
  printf 'PASS: a missing helper binary blocks the fixture.\n'
else
  printf 'FAIL: a missing helper binary was accepted.\n' >&2
  exit 1
fi

printf 'PASS: Sprint 3 fixture preflight unit checks completed.\n'
