#!/bin/sh
set -eu

: "${FUSIONPANEL_E2E_PASSWORD:?synthetic fixture password is required}"
: "${FUSIONPANEL_DATABASE_URL:?disposable database path is required}"

database_dir=$(dirname "$FUSIONPANEL_DATABASE_URL")
mkdir -p "$database_dir"

/usr/local/bin/fusionpanel-cli --json bootstrap operator \
  --database-url "$FUSIONPANEL_DATABASE_URL" \
  --username "${FUSIONPANEL_E2E_USERNAME:-e2e-admin}" \
  --password "$FUSIONPANEL_E2E_PASSWORD" >/dev/null

exec /usr/local/bin/fusionpanel-api
