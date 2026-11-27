#!/usr/bin/env bash
set -euo pipefail

action="${1:-}"
shift || true

case "$action" in
  validate)
    staging="${1:-${FUSIONPANEL_MAIL_STAGING_DIR:-}}"
    exec curl -fsS -X POST "http://mail:9025/validate?staging=${staging}" >/dev/null
    ;;
  reload)
    exec curl -fsS -X POST "http://mail:9025/reload" >/dev/null
    ;;
  *)
    echo "usage: $0 {validate|reload} [staging_dir]" >&2
    exit 2
    ;;
esac
