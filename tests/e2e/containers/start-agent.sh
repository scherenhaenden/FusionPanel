#!/usr/bin/env bash
set -Eeuo pipefail

: "${FUSIONPANEL_API_URL:?FUSIONPANEL_API_URL is required}"
: "${FUSIONPANEL_ADMIN_TOKEN:?FUSIONPANEL_ADMIN_TOKEN is required}"
: "${FUSIONPANEL_NODE_NAME:?FUSIONPANEL_NODE_NAME is required}"

mkdir -p /var/lib/fusionpanel-agent

api_host_port="${FUSIONPANEL_API_URL#http://}"
api_host_port="${api_host_port#https://}"
api_host_port="${api_host_port%%/*}"
api_host="${api_host_port%%:*}"
api_port="${api_host_port##*:}"
[ -n "$api_port" ] && [ "$api_port" != "$api_host" ] || api_port=8080

api_ready=0
for _ in $(seq 1 120); do
  if (exec 3<>"/dev/tcp/${api_host}/${api_port}") 2>/dev/null; then
    exec 3>&-
    api_ready=1
    break
  fi
  sleep 0.5
done
[ "$api_ready" -eq 1 ] || {
  echo "Timed out waiting for API at ${api_host}:${api_port}" >&2
  exit 1
}

exec /usr/local/bin/fusionpanel-agent
