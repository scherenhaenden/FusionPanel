#!/usr/bin/env bash
set -Eeuo pipefail

: "${FUSIONPANEL_NGINX_CONFIG:?isolated Nginx config path is required}"
: "${FUSIONPANEL_NGINX_MANAGED_ROOT:?isolated managed root is required}"
: "${FUSIONPANEL_AGENT_MODE:?agent mode is required}"
: "${FUSIONPANEL_NGINX_HELPER_SOCKET:?helper socket path is required}"
: "${FUSIONPANEL_S3_AGENT_UID:?unprivileged fixture agent UID is required}"
: "${FUSIONPANEL_S3_AGENT_GID:?unprivileged fixture agent GID is required}"

if [ "$(id -u)" -ne 0 ]; then
  printf 'The isolated fixture node must start its root Nginx helper as UID 0.\n' >&2
  exit 1
fi

if ! [[ "$FUSIONPANEL_S3_AGENT_UID" =~ ^[0-9]+$ ]] \
  || ! [[ "$FUSIONPANEL_S3_AGENT_GID" =~ ^[0-9]+$ ]]; then
  printf 'The fixture agent UID and GID must be numeric.\n' >&2
  exit 1
fi

if [ "$FUSIONPANEL_S3_AGENT_UID" -eq 0 ]; then
  printf 'The fixture agent must run as an unprivileged UID.\n' >&2
  exit 1
fi

nginx_pid=
helper_pid=
agent_pid=

cleanup() {
  local result=$?
  trap - EXIT INT TERM
  if [ -n "$agent_pid" ] && kill -0 "$agent_pid" 2>/dev/null; then
    kill -TERM "$agent_pid" 2>/dev/null || true
  fi
  if [ -n "$helper_pid" ] && kill -0 "$helper_pid" 2>/dev/null; then
    kill -TERM "$helper_pid" 2>/dev/null || true
  fi
  if [ -n "$nginx_pid" ] && kill -0 "$nginx_pid" 2>/dev/null; then
    kill -TERM "$nginx_pid" 2>/dev/null || true
  fi
  if [ -n "$agent_pid" ]; then wait "$agent_pid" 2>/dev/null || true; fi
  if [ -n "$helper_pid" ]; then wait "$helper_pid" 2>/dev/null || true; fi
  if [ -n "$nginx_pid" ]; then wait "$nginx_pid" 2>/dev/null || true; fi
  exit "$result"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

mkdir -p -- \
  "$FUSIONPANEL_NGINX_MANAGED_ROOT" \
  "$FUSIONPANEL_NGINX_MANAGED_ROOT/sites-available" \
  "$FUSIONPANEL_NGINX_MANAGED_ROOT/sites-enabled" \
  "$FUSIONPANEL_NGINX_MANAGED_ROOT/staging" \
  /run/fusionpanel-nginx
chmod 0750 -- \
  "$FUSIONPANEL_NGINX_MANAGED_ROOT" \
  "$FUSIONPANEL_NGINX_MANAGED_ROOT/sites-available" \
  "$FUSIONPANEL_NGINX_MANAGED_ROOT/sites-enabled" \
  "$FUSIONPANEL_NGINX_MANAGED_ROOT/staging" \
  /run/fusionpanel-nginx
nginx -v
nginx -t -q -c "$FUSIONPANEL_NGINX_CONFIG"
nginx -g 'daemon off;' -c "$FUSIONPANEL_NGINX_CONFIG" &
nginx_pid=$!

/usr/local/bin/fusionpanel-nginx-helper &
helper_pid=$!
printf '%s\n' "$helper_pid" >/run/fusionpanel-nginx-helper.pid

helper_ready=0
for attempt in $(seq 1 100); do
  if [ -S "$FUSIONPANEL_NGINX_HELPER_SOCKET" ]; then
    helper_ready=1
    break
  fi
  if ! kill -0 "$helper_pid" 2>/dev/null; then
    printf 'The privileged Nginx helper exited before creating its socket.\n' >&2
    exit 1
  fi
  sleep 0.1
done
if [ "$helper_ready" -ne 1 ]; then
  printf 'The privileged Nginx helper did not create its socket before the startup deadline.\n' >&2
  exit 1
fi

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

setpriv \
  --reuid="$FUSIONPANEL_S3_AGENT_UID" \
  --regid="$FUSIONPANEL_S3_AGENT_GID" \
  --groups=2000 \
  --inh-caps=-all \
  --ambient-caps=-all \
  --no-new-privs \
  -- /usr/local/bin/fusionpanel-agent &
agent_pid=$!
printf '%s\n' "$agent_pid" >/run/fusionpanel-agent.pid

set +e
wait -n "$nginx_pid" "$helper_pid" "$agent_pid"
result=$?
set -e
exit "$result"
