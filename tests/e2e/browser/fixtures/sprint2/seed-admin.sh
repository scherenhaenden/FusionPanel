#!/bin/sh
set -eu

: "${FUSIONPANEL_E2E_PASSWORD:?synthetic fixture password is required}"
: "${FUSIONPANEL_DATABASE_URL:?disposable database path is required}"

database_dir=$(dirname "$FUSIONPANEL_DATABASE_URL")
mkdir -p "$database_dir"
username=${FUSIONPANEL_E2E_USERNAME:-e2e-admin}
seed_marker="$database_dir/.fusionpanel-e2e-admin-seeded"
cli=${FUSIONPANEL_E2E_CLI:-/usr/local/bin/fusionpanel-cli}
api=${FUSIONPANEL_E2E_API:-/usr/local/bin/fusionpanel-api}

if [ -L "$seed_marker" ] || { [ -e "$seed_marker" ] && [ ! -f "$seed_marker" ]; }; then
    echo "Error: refusing to use a non-regular Sprint 2 admin seed marker." >&2
    exit 1
fi

if [ -f "$seed_marker" ]; then
    seeded_username=$(cat "$seed_marker")
    if [ "$seeded_username" != "$username" ]; then
        echo "Error: Sprint 2 seed marker belongs to a different operator username." >&2
        exit 1
    fi
else
    previous_umask=$(umask)
    umask 077
    marker_tmp=$(mktemp "${seed_marker}.tmp.XXXXXX")
    trap 'rm -f -- "$marker_tmp"' EXIT
    trap 'exit 129' HUP
    trap 'exit 130' INT
    trap 'exit 143' TERM

    "$cli" --json bootstrap operator \
      --database-url "$FUSIONPANEL_DATABASE_URL" \
      --username "$username" \
      --password "$FUSIONPANEL_E2E_PASSWORD" >/dev/null

    printf '%s\n' "$username" > "$marker_tmp"
    mv -f -- "$marker_tmp" "$seed_marker"
    trap - EXIT HUP INT TERM
    umask "$previous_umask"
fi

exec "$api"
