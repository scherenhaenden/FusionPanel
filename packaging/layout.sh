#!/bin/sh

# Shared package and version-layout helpers for the installer and updater.

fusionpanel_validate_payload() {
    payload_dir=$1
    for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper; do
        if [ ! -x "$payload_dir/$binary" ]; then
            echo "Error: Required executable not found in package: $binary" >&2
            return 1
        fi
    done
    for unit in fusionpanel.service fusionpanel-agent.service fusionpanel-nginx-helper.service; do
        if [ ! -f "$payload_dir/systemd/$unit" ] || [ -L "$payload_dir/systemd/$unit" ]; then
            echo "Error: Required systemd unit not found in package: $unit" >&2
            return 1
        fi
    done
    if [ ! -x "$payload_dir/packaging/nginx-include.sh" ]; then
        echo "Error: Required Nginx integration setup script not found in package." >&2
        return 1
    fi
    if [ ! -f "$payload_dir/frontend/dist/index.html" ]; then
        echo "Error: Required frontend file not found in package: frontend/dist/index.html" >&2
        return 1
    fi
}

fusionpanel_is_source_checkout() {
    fusionpanel_script_dir=$1
    fusionpanel_repo_root=$(CDPATH= cd -- "$fusionpanel_script_dir/.." && pwd) || return 1
    [ -d "$fusionpanel_repo_root/.git" ] || [ -f "$fusionpanel_repo_root/.git" ]
}

fusionpanel_validate_nginx_helper_group() {
    if ! fusionpanel_group_entry=$(getent group fusionpanel-nginx 2>/dev/null); then
        echo "Error: Could not verify the private Nginx helper group." >&2
        return 1
    fi
    fusionpanel_group_members=$(printf '%s\n' "$fusionpanel_group_entry" | awk -F: 'NF >= 4 { print $4 }')
    fusionpanel_group_gid=$(printf '%s\n' "$fusionpanel_group_entry" | awk -F: 'NF >= 3 { print $3 }')
    case "$fusionpanel_group_gid" in
        ""|*[!0-9]*)
            echo "Error: Could not resolve the private Nginx helper group GID." >&2
            return 1
            ;;
    esac
    if [ -n "$fusionpanel_group_members" ]; then
        echo "Error: fusionpanel-nginx already has supplementary members; refusing to grant helper access." >&2
        return 1
    fi

    if ! fusionpanel_passwd_entries=$(getent passwd 2>/dev/null); then
        echo "Error: Could not verify primary-group users for fusionpanel-nginx." >&2
        return 1
    fi
    fusionpanel_primary_group_users=$(printf '%s\n' "$fusionpanel_passwd_entries" \
        | awk -F: -v gid="$fusionpanel_group_gid" '$4 == gid { print $1 }')
    if [ -n "$fusionpanel_primary_group_users" ]; then
        echo "Error: fusionpanel-nginx is a primary group for existing accounts; refusing to grant helper access." >&2
        return 1
    fi
}

fusionpanel_ensure_nginx_helper_group() {
    if ! command -v getent >/dev/null 2>&1 || ! command -v groupadd >/dev/null 2>&1; then
        echo "Error: getent and groupadd are required to provision the private Nginx helper group." >&2
        return 1
    fi

    if getent group fusionpanel-nginx >/dev/null 2>&1; then
        fusionpanel_validate_nginx_helper_group
        return $?
    fi

    if ! groupadd --system fusionpanel-nginx; then
        echo "Error: Could not create the private Nginx helper group." >&2
        return 1
    fi
    fusionpanel_validate_nginx_helper_group
}

fusionpanel_stop_unit_if_active() {
    fusionpanel_unit=$1
    if systemctl is-active --quiet "$fusionpanel_unit"; then
        systemctl stop "$fusionpanel_unit"
        return $?
    else
        fusionpanel_active_status=$?
        case "$fusionpanel_active_status" in
            3|4) return 0 ;;
            *) return 1 ;;
        esac
    fi
}

# Ensure a service-owned private directory without changing any existing
# directory or traversing its contents. The optional installer UID exists so
# package tests can exercise this helper with temporary, non-system paths.
fusionpanel_ensure_private_directory() {
    fusionpanel_private_dir=$1
    fusionpanel_private_user=$2
    fusionpanel_private_group=$3
    fusionpanel_required_installer_uid=${4:-0}

    case "$fusionpanel_private_dir" in
        /*/*) ;;
        *) echo "Error: Refusing an invalid private directory path." >&2; return 1 ;;
    esac
    if [ -L "$fusionpanel_private_dir" ] \
        || { [ -e "$fusionpanel_private_dir" ] && [ ! -d "$fusionpanel_private_dir" ]; }; then
        echo "Error: Refusing a symlink or non-directory at $fusionpanel_private_dir." >&2
        return 1
    fi

    fusionpanel_private_uid=$(id -u "$fusionpanel_private_user" 2>/dev/null) || {
        echo "Error: Could not resolve service user $fusionpanel_private_user." >&2
        return 1
    }
    fusionpanel_private_group_entry=$(getent group "$fusionpanel_private_group" 2>/dev/null) || {
        echo "Error: Could not resolve service group $fusionpanel_private_group." >&2
        return 1
    }
    fusionpanel_private_gid=$(printf '%s\n' "$fusionpanel_private_group_entry" | awk -F: 'NF >= 3 { print $3 }')
    case "$fusionpanel_private_uid:$fusionpanel_private_gid" in
        *[!0-9:]*|:|*:|:*)
            echo "Error: Could not resolve numeric service ownership." >&2
            return 1
            ;;
    esac

    if [ ! -e "$fusionpanel_private_dir" ]; then
        fusionpanel_private_parent=${fusionpanel_private_dir%/*}
        if [ -L "$fusionpanel_private_parent" ] || [ ! -d "$fusionpanel_private_parent" ]; then
            echo "Error: The parent of $fusionpanel_private_dir must already be a real directory." >&2
            return 1
        fi
        fusionpanel_current_uid=$(id -u 2>/dev/null) || {
            echo "Error: Could not verify installer identity." >&2
            return 1
        }
        if [ "$fusionpanel_current_uid" != "$fusionpanel_required_installer_uid" ]; then
            echo "Error: Only the expected privileged installer may create $fusionpanel_private_dir." >&2
            return 1
        fi
        if ! mkdir -m 0700 -- "$fusionpanel_private_dir"; then
            echo "Error: Could not create $fusionpanel_private_dir." >&2
            return 1
        fi
        fusionpanel_created_metadata=$(stat -c '%u %g' -- "$fusionpanel_private_dir" 2>/dev/null) || {
            rmdir -- "$fusionpanel_private_dir" 2>/dev/null || true
            echo "Error: Could not verify newly created directory $fusionpanel_private_dir." >&2
            return 1
        }
        set -- $fusionpanel_created_metadata
        if { [ "$1" != "$fusionpanel_private_uid" ] || [ "$2" != "$fusionpanel_private_gid" ]; } \
            && ! chown -- "$fusionpanel_private_user:$fusionpanel_private_group" "$fusionpanel_private_dir"; then
            rmdir -- "$fusionpanel_private_dir" 2>/dev/null || true
            echo "Error: Could not assign service ownership to newly created directory $fusionpanel_private_dir." >&2
            return 1
        fi
        if ! chmod 0700 -- "$fusionpanel_private_dir"; then
            rmdir -- "$fusionpanel_private_dir" 2>/dev/null || true
            echo "Error: Could not secure newly created directory $fusionpanel_private_dir." >&2
            return 1
        fi
    fi

    fusionpanel_private_metadata=$(stat -c '%u %g %a' -- "$fusionpanel_private_dir" 2>/dev/null) || {
        echo "Error: Could not verify $fusionpanel_private_dir." >&2
        return 1
    }
    set -- $fusionpanel_private_metadata
    if [ "$1" != "$fusionpanel_private_uid" ] \
        || [ "$2" != "$fusionpanel_private_gid" ] \
        || [ "$3" != 700 ]; then
        echo "Error: Existing directory $fusionpanel_private_dir must be owned by $fusionpanel_private_user:$fusionpanel_private_group with mode 0700; it was left unchanged." >&2
        return 1
    fi
}

fusionpanel_install_systemd_units() {
    payload_dir=$1
    systemd_dir=$2
    sysconfdir=$3
    mkdir -p "$systemd_dir"
    unit_temp_dir="$systemd_dir/.fusionpanel-unit-stage.$$"
    if [ -e "$unit_temp_dir" ] || [ -L "$unit_temp_dir" ]; then
        echo "Error: Systemd staging path already exists." >&2
        return 1
    fi
    mkdir -m 0700 "$unit_temp_dir" || return 1

    for unit in fusionpanel.service fusionpanel-agent.service fusionpanel-nginx-helper.service; do
        source_unit="$payload_dir/systemd/$unit"
        target_unit="$unit_temp_dir/$unit"
        if [ ! -f "$source_unit" ] || [ -L "$source_unit" ]; then
            rm -rf "$unit_temp_dir"
            echo "Error: Required systemd unit is missing or unsafe: $unit" >&2
            return 1
        fi
        case "$unit" in
            fusionpanel.service)
                sed "s|/etc/fusionpanel/fusionpanel.env|$sysconfdir/fusionpanel.env|g" "$source_unit" > "$target_unit"
                ;;
            fusionpanel-agent.service)
                sed "s|/etc/fusionpanel/fusionpanel-agent.env|$sysconfdir/fusionpanel-agent.env|g" "$source_unit" > "$target_unit"
                ;;
            *) cp "$source_unit" "$target_unit" ;;
        esac
        chmod 0644 "$target_unit" || { rm -rf "$unit_temp_dir"; return 1; }
    done

    for unit in fusionpanel.service fusionpanel-agent.service fusionpanel-nginx-helper.service; do
        target="$systemd_dir/$unit"
        if [ -L "$target" ] || { [ -e "$target" ] && [ ! -f "$target" ]; }; then
            rm -rf "$unit_temp_dir"
            echo "Error: Refusing to replace a non-regular systemd unit: $target" >&2
            return 1
        fi
    done
    for unit in fusionpanel.service fusionpanel-agent.service fusionpanel-nginx-helper.service; do
        mv -f "$unit_temp_dir/$unit" "$systemd_dir/$unit" || { rm -rf "$unit_temp_dir"; return 1; }
    done
    rmdir "$unit_temp_dir"
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
    for binary in fusionpanel-api fusionpanel-agent fusionpanel-cli fusionpanel-nginx-helper; do
        target="$versions_dir/current/bin/$binary"
        if [ -x "$target" ]; then
            ln -sfn "$target" "$bin_dir/$binary"
        elif [ -L "$bin_dir/$binary" ] && [ "$(readlink "$bin_dir/$binary" 2>/dev/null || true)" = "$target" ]; then
            rm -f "$bin_dir/$binary"
        fi
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

fusionpanel_parse_semver_major_minor() {
    printf '%s\n' "$1" | awk '
        {
            v = $0
            sub(/^[vV]/, "", v)
            if (v ~ /^[0-9]+\.[0-9]+(\.[0-9]+)?([+-].*)?$/) {
                split(v, parts, /[.+-]/)
                printf "%d %d\n", parts[1] + 0, parts[2] + 0
                exit 0
            }
            exit 1
        }
    '
}

fusionpanel_check_adjacent_release() {
    fusionpanel_cur_ver=$1
    fusionpanel_tgt_ver=$2
    fusionpanel_allow_non_adj=${3:-0}

    if [ "$fusionpanel_tgt_ver" = "latest" ]; then
        return 0
    fi
    if [ -z "$fusionpanel_cur_ver" ]; then
        return 0
    fi
    case "$fusionpanel_cur_ver" in
        *[!a-zA-Z0-9.+_-]*|'')
            echo "Error: Invalid version in state file: $fusionpanel_cur_ver" >&2
            return 1
            ;;
    esac

    if ! fusionpanel_cur_mm=$(fusionpanel_parse_semver_major_minor "$fusionpanel_cur_ver"); then
        return 0
    fi
    if ! fusionpanel_tgt_mm=$(fusionpanel_parse_semver_major_minor "$fusionpanel_tgt_ver"); then
        return 0
    fi

    set -- $fusionpanel_cur_mm
    fusionpanel_cur_maj=$1
    fusionpanel_cur_min=$2
    set -- $fusionpanel_tgt_mm
    fusionpanel_tgt_maj=$1
    fusionpanel_tgt_min=$2

    if [ "$fusionpanel_tgt_maj" -eq "$fusionpanel_cur_maj" ] \
        && { [ "$fusionpanel_tgt_min" -eq "$fusionpanel_cur_min" ] \
            || [ "$fusionpanel_tgt_min" -eq $((fusionpanel_cur_min + 1)) ]; }; then
        return 0
    fi
    if [ "$fusionpanel_tgt_maj" -eq $((fusionpanel_cur_maj + 1)) ] \
        && [ "$fusionpanel_tgt_min" -eq 0 ]; then
        return 0
    fi

    if [ "$fusionpanel_allow_non_adj" = "1" ]; then
        echo "Warning: Allowing non-adjacent release transition from $fusionpanel_cur_ver to $fusionpanel_tgt_ver." >&2
        return 0
    fi

    echo "Error: Non-adjacent or downgrade release transition from $fusionpanel_cur_ver to $fusionpanel_tgt_ver is blocked (set FUSIONPANEL_ALLOW_NON_ADJACENT_UPGRADE=1 to override)." >&2
    return 1
}

fusionpanel_check_min_free_kb() {
    fusionpanel_disk_target=$1
    fusionpanel_min_kb=${2:-}
    if [ -z "$fusionpanel_min_kb" ]; then
        return 0
    fi
    case "$fusionpanel_min_kb" in
        *[!0-9]*)
            echo "Error: Invalid FUSIONPANEL_MIN_FREE_KB value: $fusionpanel_min_kb" >&2
            return 1
            ;;
    esac
    fusionpanel_check_dir="$fusionpanel_disk_target"
    while [ ! -d "$fusionpanel_check_dir" ] && [ "$fusionpanel_check_dir" != "/" ]; do
        fusionpanel_parent_dir=${fusionpanel_check_dir%/*}
        fusionpanel_check_dir=${fusionpanel_parent_dir:-/}
    done
    fusionpanel_avail_kb=$(df -Pk "$fusionpanel_check_dir" 2>/dev/null | awk 'NR == 2 { print $4 }') || {
        echo "Error: Could not determine free disk space for $fusionpanel_check_dir." >&2
        return 1
    }
    case "$fusionpanel_avail_kb" in
        ''|*[!0-9]*)
            echo "Error: Unexpected df output for $fusionpanel_check_dir: $fusionpanel_avail_kb" >&2
            return 1
            ;;
    esac
    if [ "$fusionpanel_avail_kb" -lt "$fusionpanel_min_kb" ]; then
        echo "Error: Insufficient free disk space on $fusionpanel_check_dir (${fusionpanel_avail_kb} KB available, ${fusionpanel_min_kb} KB required)." >&2
        return 1
    fi
}
