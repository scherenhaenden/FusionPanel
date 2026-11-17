#!/usr/bin/env bash
set -Eeuo pipefail

: "${FUSIONPANEL_NGINX_CONFIG:?isolated Nginx config path is required}"
: "${FUSIONPANEL_NGINX_MANAGED_ROOT:?isolated managed root is required}"
: "${FUSIONPANEL_AGENT_MODE:?agent mode is required}"
: "${FUSIONPANEL_NGINX_HELPER_SOCKET:?helper socket path is required}"
: "${FUSIONPANEL_S8_AGENT_UID:?unprivileged fixture agent UID is required}"
: "${FUSIONPANEL_S8_AGENT_GID:?unprivileged fixture agent GID is required}"

nginx_pid=
helper_pid=
agent_pid=

cleanup() {
  local result=$?
  trap - EXIT INT TERM
  for pid in "$agent_pid" "$helper_pid" "$nginx_pid"; do
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
      kill -TERM "$pid" 2>/dev/null || true
    fi
  done
  for pid in "$agent_pid" "$helper_pid" "$nginx_pid"; do
    if [ -n "$pid" ]; then wait "$pid" 2>/dev/null || true; fi
  done
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
  "$FUSIONPANEL_NGINX_MANAGED_ROOT/staging"
chmod 0755 -- /run/fusionpanel-nginx

setpriv --reuid=33 --regid=33 --groups=33,2000 --no-new-privs -- /bin/sh -c '
  for temp_path in body fastcgi proxy scgi uwsgi; do
    temp_dir="/var/lib/nginx/$temp_path"
    mkdir -p -- "$temp_dir"
    chmod 2770 -- "$temp_dir"
  done
'

nginx -t -q -c "$FUSIONPANEL_NGINX_CONFIG"
nginx -g 'daemon off;' -c "$FUSIONPANEL_NGINX_CONFIG" &
nginx_pid=$!

/usr/local/bin/fusionpanel-nginx-helper &
helper_pid=$!
printf '%s\n' "$helper_pid" >/run/fusionpanel-nginx-helper.pid

helper_ready=0
for _ in $(seq 1 100); do
  if [ -S "$FUSIONPANEL_NGINX_HELPER_SOCKET" ]; then
    helper_ready=1
    break
  fi
  sleep 0.1
done
[ "$helper_ready" -eq 1 ]

api_ready=0
for _ in $(seq 1 120); do
  if (exec 3<>/dev/tcp/127.0.0.1/8080) 2>/dev/null; then
    api_ready=1
    break
  fi
  sleep 0.5
done
[ "$api_ready" -eq 1 ]

setpriv \
  --reuid="$FUSIONPANEL_S8_AGENT_UID" \
  --regid="$FUSIONPANEL_S8_AGENT_GID" \
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
