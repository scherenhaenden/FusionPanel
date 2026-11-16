#!/bin/sh
set -eu

# Add only a FusionPanel-owned conf.d fragment after the running Nginx config
# proves that the exact conf.d glob is active. This script never edits
# nginx.conf and never reloads or otherwise controls the Nginx service.

nginx_bin=${1:-/usr/sbin/nginx}
active_config=${2:-/etc/nginx/nginx.conf}
conf_d_dir=${3:-/etc/nginx/conf.d}
managed_root=${4:-/etc/nginx/fusionpanel}
expected_uid=${5:-0}
target="$conf_d_dir/fusionpanel.conf"
managed_include="include $managed_root/sites-enabled/*.conf;"
conf_d_include='include /etc/nginx/conf.d/*.conf;'
marker='# Managed by FusionPanel installer; do not edit.'

has_config_line() {
    expected=$1
    dump=$2
    printf '%s\n' "$dump" | awk -v expected="$expected" '
        {
            sub(/^[[:space:]]+/, "")
            sub(/[[:space:]]+$/, "")
            if ($0 == expected) found = 1
        }
        END { exit !found }
    '
}

secure_directory() {
    path=$1
    metadata=$(stat -c '%u %a' "$path" 2>/dev/null) || return 1
    owner=${metadata%% *}
    mode=${metadata#* }
    [ "$owner" = "$expected_uid" ] || return 1
    mode_value=$((0$mode))
    [ $((mode_value & 0022)) -eq 0 ]
}

secure_file() {
    path=$1
    [ -f "$path" ] && [ ! -L "$path" ] || return 1
    metadata=$(stat -c '%u %a' "$path" 2>/dev/null) || return 1
    owner=${metadata%% *}
    mode=${metadata#* }
    [ "$owner" = "$expected_uid" ] || return 1
    mode_value=$((0$mode))
    [ $((mode_value & 0022)) -eq 0 ]
}

secure_managed_tree() {
    available="$managed_root/sites-available"
    for directory in "$available" "$managed_root/sites-enabled" "$managed_root/staging"; do
        if [ -L "$directory" ] || { [ -e "$directory" ] && [ ! -d "$directory" ]; }; then
            return 1
        fi
        [ -e "$directory" ] || continue
        secure_directory "$directory" || return 1

        for entry in "$directory"/* "$directory"/.[!.]* "$directory"/..?*; do
            [ -e "$entry" ] || [ -L "$entry" ] || continue
            if [ "$directory" = "$managed_root/sites-enabled" ] && [ -L "$entry" ]; then
                link_target=$(readlink "$entry" 2>/dev/null) || return 1
                case "$link_target" in
                    "$available"/*) ;;
                    *) return 1 ;;
                esac
                case "/$link_target/" in
                    *"/../"*) return 1 ;;
                esac
                target_name=${link_target#"$available"/}
                case "$target_name" in
                    ""|*/*) return 1 ;;
                esac
                secure_file "$link_target" || return 1
            else
                secure_file "$entry" || return 1
            fi
        done
    done
}

if [ ! -x "$nginx_bin" ]; then
    echo "Nginx include skipped: the fixed Nginx executable is unavailable." >&2
    exit 1
fi
if [ -L "$conf_d_dir" ] || [ ! -d "$conf_d_dir" ] || ! secure_directory "$conf_d_dir"; then
    echo "Nginx include skipped: /etc/nginx/conf.d is not a real directory." >&2
    exit 1
fi
managed_parent=${managed_root%/*}
if [ -L "$managed_parent" ] || [ ! -d "$managed_parent" ] || ! secure_directory "$managed_parent"; then
    echo "Nginx include skipped: the parent of the FusionPanel managed root is not a trusted directory." >&2
    exit 1
fi

if ! initial_dump=$("$nginx_bin" -T -c "$active_config" 2>/dev/null); then
    echo "Nginx include skipped: the active Nginx configuration could not be inspected." >&2
    exit 1
fi

# An already-active managed include is itself compatible. Otherwise, the exact
# conf.d glob must be active before this script may create its fragment.
managed_include_active=0
if has_config_line "$managed_include" "$initial_dump"; then
    managed_include_active=1
elif ! has_config_line "$conf_d_include" "$initial_dump"; then
    echo "Nginx include skipped: nginx -T did not prove the compatible conf.d include is active." >&2
    exit 1
fi

if [ -L "$managed_root" ] || { [ -e "$managed_root" ] && [ ! -d "$managed_root" ]; }; then
    echo "Nginx include skipped: the FusionPanel managed root is not a real directory." >&2
    exit 1
fi
if [ ! -e "$managed_root" ]; then
    current_uid=$(id -u 2>/dev/null) || {
        echo "Nginx include skipped: could not verify the installer identity." >&2
        exit 1
    }
    if [ "$current_uid" != "$expected_uid" ]; then
        echo "Nginx include skipped: only the expected privileged installer may create the managed root." >&2
        exit 1
    fi
    # Create only this missing, directly managed directory. Do not create its
    # parents or change metadata/content in an existing Nginx tree.
    if ! mkdir -m 0700 "$managed_root"; then
        echo "Nginx include skipped: the FusionPanel managed root could not be created." >&2
        exit 1
    fi
fi
if ! secure_directory "$managed_root"; then
    echo "Nginx include skipped: the FusionPanel managed root must be root-owned and not group/world-writable." >&2
    exit 1
fi
if ! secure_managed_tree; then
    echo "Nginx include skipped: an existing FusionPanel managed site directory or entry is unsafe." >&2
    exit 1
fi
if [ "$managed_include_active" -eq 1 ]; then
    if { [ -e "$target" ] || [ -L "$target" ]; } && ! secure_file "$target"; then
        echo "Nginx include skipped: the existing FusionPanel fragment is not a root-owned, non-writable regular file." >&2
        exit 1
    fi
    exit 0
fi

if [ -L "$target" ] || { [ -e "$target" ] && [ ! -f "$target" ]; }; then
    echo "Nginx include skipped: refusing to replace a non-regular FusionPanel fragment." >&2
    exit 1
fi
if [ -e "$target" ]; then
    if [ "$(cat "$target")" = "$(printf '%s\n%s' "$marker" "$managed_include")" ]; then
        if ! secure_file "$target"; then
            echo "Nginx include skipped: the existing FusionPanel fragment has unsafe ownership or permissions." >&2
            exit 1
        fi
        echo "Nginx include skipped: the existing fragment is not active in nginx -T." >&2
    else
        echo "Nginx include skipped: refusing to replace an existing FusionPanel fragment." >&2
    fi
    exit 1
fi

temporary=$(mktemp "$conf_d_dir/.fusionpanel.conf.XXXXXX") || {
    echo "Nginx include skipped: could not stage a configuration fragment." >&2
    exit 1
}
created=0
cleanup_staged_fragment() {
    if [ "$created" -eq 1 ] && [ -e "$target" ] && [ "$target" -ef "$temporary" ]; then
        rm -f "$target"
    fi
    rm -f "$temporary"
}
trap cleanup_staged_fragment EXIT HUP INT TERM
(umask 022; printf '%s\n%s\n' "$marker" "$managed_include" > "$temporary") || exit 1
chmod 0644 "$temporary" || exit 1
if ! ln "$temporary" "$target" 2>/dev/null; then
    echo "Nginx include skipped: the target fragment appeared during setup." >&2
    exit 1
fi
created=1

if ! "$nginx_bin" -t -c "$active_config" >/dev/null 2>&1 \
    || ! updated_dump=$("$nginx_bin" -T -c "$active_config" 2>/dev/null) \
    || ! has_config_line "$managed_include" "$updated_dump"; then
    cleanup_staged_fragment
    created=0
    trap - EXIT HUP INT TERM
    echo "Nginx include rolled back: the candidate did not pass Nginx validation." >&2
    exit 1
fi

rm -f "$temporary"
created=0
trap - EXIT HUP INT TERM
exit 0
