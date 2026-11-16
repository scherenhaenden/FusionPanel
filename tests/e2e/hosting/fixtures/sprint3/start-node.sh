#!/usr/bin/env bash
set -Eeuo pipefail

: "${FUSIONPANEL_NGINX_CONFIG:?isolated Nginx config path is required}"
: "${FUSIONPANEL_NGINX_MANAGED_ROOT:?isolated managed root is required}"
: "${FUSIONPANEL_AGENT_MODE:?agent mode is required}"

mkdir -p "$FUSIONPANEL_NGINX_MANAGED_ROOT/sites-enabled"
nginx -v
nginx -t -q -c "$FUSIONPANEL_NGINX_CONFIG"
nginx -g 'daemon off;' -c "$FUSIONPANEL_NGINX_CONFIG" &
nginx_pid=$!
agent_pid=

cleanup() {
  local result=$?
  trap - EXIT INT TERM
  if [ -n "$agent_pid" ] && kill -0 "$agent_pid" 2>/dev/null; then
    kill -TERM "$agent_pid" 2>/dev/null || true
  fi
  if [ -n "$nginx_pid" ] && kill -0 "$nginx_pid" 2>/dev/null; then
    kill -TERM "$nginx_pid" 2>/dev/null || true
  fi
  if [ -n "$agent_pid" ]; then wait "$agent_pid" 2>/dev/null || true; fi
  if [ -n "$nginx_pid" ]; then wait "$nginx_pid" 2>/dev/null || true; fi
  exit "$result"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

api_ready=0
for attempt in $(seq 1 120); do
  if (exec 3<>/dev/tcp/127.0.0.1/8080) 2>/dev/null; then
    api_ready=1
    break
  fi
  if ! kill -0 "$nginx_pid" 2>/dev/null; then
    printf 'Nginx exited while waiting for the API on loopback.\n' >&2
    exit 1
  fi
  sleep 0.5
done
if [ "$api_ready" -ne 1 ]; then
  printf 'The API did not listen on 127.0.0.1:8080 before the agent startup deadline.\n' >&2
  exit 1
fi

fusionpanel-agent &
agent_pid=$!
printf '%s\n' "$agent_pid" >/run/fusionpanel-agent.pid

set +e
wait -n "$nginx_pid" "$agent_pid"
result=$?
set -e
exit "$result"
