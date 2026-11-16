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
