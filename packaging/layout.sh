#!/bin/sh

# Shared package and version-layout helpers for the installer and updater.

fusionpanel_validate_payload() {
    payload_dir=$1
    for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli; do
        if [ ! -x "$payload_dir/$binary" ]; then
            echo "Error: Required executable not found in package: $binary" >&2
            return 1
        fi
    done
    if [ ! -f "$payload_dir/frontend/dist/index.html" ]; then
        echo "Error: Required frontend file not found in package: frontend/dist/index.html" >&2
        return 1
    fi
}

fusionpanel_normalize_node_name() {
    fusionpanel_node_name=$(printf '%s' "$1" | LC_ALL=C tr -c 'A-Za-z0-9._-' '-')
    fusionpanel_node_name=$(printf '%s' "$fusionpanel_node_name" | sed 's/^[.-]*//; s/[.-]*$//')
    if [ -z "$fusionpanel_node_name" ]; then
        fusionpanel_node_name="fusionpanel-node"
    fi
    printf '%s' "$fusionpanel_node_name"
}

fusionpanel_unique_env_value() {
    awk -v key="$2" '
        index($0, key "=") == 1 {
            count++
            value = substr($0, length(key) + 2)
        }
        END {
            if (count != 1 || value == "") exit 1
            print value
        }
    ' "$1"
}

fusionpanel_is_legacy_local_agent_env() {
    [ -f "$1" ] || return 1
    awk '
        /^[[:space:]]*$/ || /^[[:space:]]*#/ { next }
        /^FUSIONPANEL_AGENT_ENDPOINT=/ { endpoint_count++; next }
        { other_assignment = 1 }
        END { exit !(endpoint_count == 1 && !other_assignment) }
    ' "$1"
}

fusionpanel_normalize_port() {
    awk -v port="$1" 'BEGIN {
        if (port !~ /^[0-9]+$/ || port + 0 < 1 || port + 0 > 65535) exit 1
        printf "%d\n", port + 0
    }'
}

fusionpanel_local_http_endpoint_port() {
    fusionpanel_endpoint=$1
    case "$fusionpanel_endpoint" in
        http://127.0.0.1:*) fusionpanel_endpoint_port=${fusionpanel_endpoint#http://127.0.0.1:} ;;
        http://localhost:*) fusionpanel_endpoint_port=${fusionpanel_endpoint#http://localhost:} ;;
        http://\[::1\]:*) fusionpanel_endpoint_port=${fusionpanel_endpoint#http://\[::1\]:} ;;
        *) return 1 ;;
    esac
    case "$fusionpanel_endpoint_port" in
        */) fusionpanel_endpoint_port=${fusionpanel_endpoint_port%/} ;;
    esac
    fusionpanel_normalize_port "$fusionpanel_endpoint_port"
}

fusionpanel_local_listen_port() {
    fusionpanel_listen_addr=$1
    case "$fusionpanel_listen_addr" in
        \[*\]:*)
            fusionpanel_listen_host=${fusionpanel_listen_addr%%]*}
            fusionpanel_listen_host=${fusionpanel_listen_host#\[}
            fusionpanel_listen_port=${fusionpanel_listen_addr##*:}
            ;;
        *:*)
            fusionpanel_listen_host=${fusionpanel_listen_addr%:*}
            fusionpanel_listen_port=${fusionpanel_listen_addr##*:}
            ;;
        *) return 1 ;;
    esac
    case "$fusionpanel_listen_host" in
        127.0.0.1|localhost|0.0.0.0|\*|::1|::) ;;
        *) return 1 ;;
    esac
    fusionpanel_normalize_port "$fusionpanel_listen_port"
}

fusionpanel_valid_node_name() {
    case "$1" in
        ''|*[!A-Za-z0-9._-]*) return 1 ;;
    esac
    [ "${#1}" -le 128 ]
}

fusionpanel_migrate_legacy_local_agent_env() {
    fusionpanel_legacy_agent_env=$1
    fusionpanel_existing_api_env=$2
    fusionpanel_staged_api_env=$3
    fusionpanel_staged_agent_env=$4

    if ! fusionpanel_is_legacy_local_agent_env "$fusionpanel_legacy_agent_env"; then
        return 0
    fi

    fusionpanel_endpoint=$(fusionpanel_unique_env_value \
        "$fusionpanel_legacy_agent_env" FUSIONPANEL_AGENT_ENDPOINT) || {
        echo "Error: Legacy agent configuration is ambiguous; refusing automatic migration." >&2
        return 1
    }
    fusionpanel_endpoint_port=$(fusionpanel_local_http_endpoint_port "$fusionpanel_endpoint") || {
        echo "Error: Legacy agent endpoint is not a supported local HTTP endpoint; refusing automatic migration." >&2
        return 1
    }

    fusionpanel_existing_token=$(fusionpanel_unique_env_value \
        "$fusionpanel_existing_api_env" FUSIONPANEL_ADMIN_TOKEN) || {
        echo "Error: Cannot migrate legacy agent configuration without one existing API admin token." >&2
        return 1
    }
    fusionpanel_staged_token=$(fusionpanel_unique_env_value \
        "$fusionpanel_staged_api_env" FUSIONPANEL_ADMIN_TOKEN) || {
        echo "Error: Cannot verify the staged API admin token for legacy agent migration." >&2
        return 1
    }
    case "$fusionpanel_existing_token" in
        *[!A-Za-z0-9._~+/=-]*|'')
            echo "Error: Existing API admin token cannot be migrated safely." >&2
            return 1
            ;;
    esac
    if [ "$fusionpanel_existing_token" != "$fusionpanel_staged_token" ]; then
        echo "Error: Existing and staged API credentials do not match; refusing agent migration." >&2
        return 1
    fi

    fusionpanel_existing_listen=$(fusionpanel_unique_env_value \
        "$fusionpanel_existing_api_env" FUSIONPANEL_LISTEN) || {
        echo "Error: Cannot verify the existing local API listener for legacy agent migration." >&2
        return 1
    }
    fusionpanel_staged_listen=$(fusionpanel_unique_env_value \
        "$fusionpanel_staged_api_env" FUSIONPANEL_LISTEN) || {
        echo "Error: Cannot verify the staged local API listener for legacy agent migration." >&2
        return 1
    }
    fusionpanel_existing_listen_port=$(fusionpanel_local_listen_port "$fusionpanel_existing_listen") || {
        echo "Error: Existing API listener is not local; refusing legacy agent migration." >&2
        return 1
    }
    fusionpanel_staged_listen_port=$(fusionpanel_local_listen_port "$fusionpanel_staged_listen") || {
        echo "Error: Staged API listener is not local; refusing legacy agent migration." >&2
        return 1
    }
    if [ "$fusionpanel_existing_listen_port" != "$fusionpanel_endpoint_port" ] \
        || [ "$fusionpanel_staged_listen_port" != "$fusionpanel_endpoint_port" ]; then
        echo "Error: Legacy agent endpoint does not match the existing and staged local API listeners." >&2
        return 1
    fi

    fusionpanel_node_name=$(fusionpanel_unique_env_value \
        "$fusionpanel_staged_agent_env" FUSIONPANEL_NODE_NAME 2>/dev/null || true)
    if [ -z "$fusionpanel_node_name" ]; then
        fusionpanel_node_name=$(fusionpanel_normalize_node_name "$(uname -n 2>/dev/null || true)")
    fi
    if ! fusionpanel_valid_node_name "$fusionpanel_node_name"; then
        echo "Error: Cannot safely select a node name for legacy agent migration." >&2
        return 1
    fi

    fusionpanel_migration_tmp="${fusionpanel_staged_agent_env}.migration.$$"
    if [ -e "$fusionpanel_migration_tmp" ] || [ -L "$fusionpanel_migration_tmp" ]; then
        echo "Error: Temporary agent configuration path already exists; refusing migration." >&2
        return 1
    fi
    (
        umask 077
        printf 'FUSIONPANEL_AGENT_MODE=http\n'
        printf 'FUSIONPANEL_API_URL=%s\n' "$fusionpanel_endpoint"
        printf 'FUSIONPANEL_ADMIN_TOKEN=%s\n' "$fusionpanel_existing_token"
        printf 'FUSIONPANEL_NODE_NAME=%s\n' "$fusionpanel_node_name"
    ) > "$fusionpanel_migration_tmp" || {
        rm -f "$fusionpanel_migration_tmp"
        echo "Error: Could not stage migrated agent configuration." >&2
        return 1
    }
    chmod 0600 "$fusionpanel_migration_tmp" || {
        rm -f "$fusionpanel_migration_tmp"
        echo "Error: Could not secure migrated agent configuration." >&2
        return 1
    }
    mv -f "$fusionpanel_migration_tmp" "$fusionpanel_staged_agent_env" || {
        rm -f "$fusionpanel_migration_tmp"
        echo "Error: Could not activate migrated agent configuration." >&2
        return 1
    }
    echo "Migrated the legacy local agent configuration using the existing API credentials."
}

fusionpanel_publish_version_dir() {
    staged_dir=$1
    install_dir=$2
    parent_dir=${install_dir%/*}
    base_name=${install_dir##*/}
    temp_dir="$parent_dir/.${base_name}.stage.$$"
    old_dir="$parent_dir/.${base_name}.previous.$$"

    mkdir -p "$parent_dir"
    rm -rf "$temp_dir" "$old_dir"
    if ! cp -a "$staged_dir" "$temp_dir"; then
        rm -rf "$temp_dir"
        return 1
    fi

    if [ -e "$install_dir" ] || [ -L "$install_dir" ]; then
        if ! mv -Tf "$install_dir" "$old_dir"; then
            rm -rf "$temp_dir"
            return 1
        fi
    fi
    if ! mv -Tf "$temp_dir" "$install_dir"; then
        if [ -e "$old_dir" ] || [ -L "$old_dir" ]; then
            mv -Tf "$old_dir" "$install_dir" || true
        fi
        rm -rf "$temp_dir"
        return 1
    fi
    rm -rf "$old_dir"
}

fusionpanel_switch_current() {
    versions_dir=$1
    version=$2
    current_tmp="$versions_dir/.current.$$"
    if [ ! -d "$versions_dir/$version" ]; then
        echo "Error: Version directory does not exist: $versions_dir/$version" >&2
        return 1
    fi
    rm -f "$current_tmp"
    ln -s "$version" "$current_tmp"
    mv -Tf "$current_tmp" "$versions_dir/current"
}

fusionpanel_link_runtime_binaries() {
    bin_dir=$1
    versions_dir=$2
    mkdir -p "$bin_dir"
    for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli; do
        ln -sfn "$versions_dir/current/bin/$binary" "$bin_dir/$binary"
    done
}

fusionpanel_set_frontend_env() {
    env_file=$1
    frontend_dir=$2
    temp_file="${env_file}.tmp.$$"
    env_dir=${env_file%/*}
    if [ -L "$env_file" ] || { [ -e "$env_file" ] && [ ! -f "$env_file" ]; }; then
        echo "Error: Refusing to replace a non-regular API environment file: $env_file" >&2
        return 1
    fi
    mkdir -p "$env_dir"
    umask 077
    if [ -f "$env_file" ] && [ ! -L "$env_file" ]; then
        sed '/^FUSIONPANEL_FRONTEND_DIR=/d' "$env_file" > "$temp_file"
    else
        : > "$temp_file"
    fi
    printf 'FUSIONPANEL_FRONTEND_DIR=%s\n' "$frontend_dir" >> "$temp_file"
    chmod 0600 "$temp_file"
    mv -f "$temp_file" "$env_file"
}
