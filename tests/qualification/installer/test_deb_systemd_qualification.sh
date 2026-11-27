#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd -- "$SCRIPT_DIR/../../.." && pwd)

MODE=""
DEB_PATH=""
SSH_TARGET=""
SSH_PORT="22"
SSH_KEY=""
EVIDENCE_DIR=""
STATE_FILE="/var/tmp/fusionpanel-deb-e2e-state.env"
TIMEOUT_SEC="60"

usage() {
    cat <<'EOF'
Usage: test_deb_systemd_qualification.sh [OPTIONS]

Modes (choose one):
  --check, --dry-run, --syntax-check
      Pre-merge fixture wiring & syntax validation. Does not install packages
      or touch host systemd.
  --deb PATH --ssh USER@HOST [--ssh-port PORT] [--ssh-key PATH]
      Run the full clean-install, health/UI/heartbeat, reboot, and persistence
      qualification against a clean Debian 12 amd64 systemd VM over SSH.
  --deb PATH --local-guest
      Run Phase 1 (install + pre-reboot checks) locally inside a disposable
      Debian 12 amd64 systemd guest. Requires FUSIONPANEL_DEB_E2E_ALLOW_LOCAL_INSTALL=1.
  --post-reboot --local-guest
      Run Phase 2 (post-reboot persistence checks) locally inside the guest
      after rebooting from --local-guest Phase 1.

Options:
  --evidence-dir DIR   Save sanitized package/journal/API evidence to DIR.
  --state-file PATH    State file for pre/post-reboot comparison
                       (default: /var/tmp/fusionpanel-deb-e2e-state.env).
  --timeout SEC        Max seconds to wait for health/heartbeat/reboot (default: 60).
  -h, --help           Show this help.
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check|--dry-run|--syntax-check)
            MODE="check"
            shift
            ;;
        --local-guest)
            if [[ -z "$MODE" ]]; then
                MODE="local-pre"
            elif [[ "$MODE" == "local-post" ]]; then
                :
            fi
            shift
            ;;
        --post-reboot)
            MODE="local-post"
            shift
            ;;
        --deb)
            DEB_PATH=${2:-}
            shift 2
            ;;
        --ssh)
            MODE="ssh"
            SSH_TARGET=${2:-}
            shift 2
            ;;
        --ssh-port)
            SSH_PORT=${2:-}
            shift 2
            ;;
        --ssh-key)
            SSH_KEY=${2:-}
            shift 2
            ;;
        --evidence-dir)
            EVIDENCE_DIR=${2:-}
            shift 2
            ;;
        --state-file)
            STATE_FILE=${2:-}
            shift 2
            ;;
        --timeout)
            TIMEOUT_SEC=${2:-}
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "ERROR: Unknown option: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [[ -z "$MODE" ]]; then
    echo "ERROR: Specify --check, --ssh USER@HOST --deb PATH, or --local-guest --deb PATH." >&2
    usage >&2
    exit 1
fi

run_check_mode() {
    CHECK_TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-deb-e2e-check.XXXXXX")
    trap 'rm -rf -- "${CHECK_TMP_DIR:-}"' EXIT HUP INT TERM

    echo "[check] Validating shell syntax in tests/qualification/installer/..."
    local script
    for script in "$SCRIPT_DIR"/*.sh; do
        bash -n "$script"
    done

    echo "[check] Verifying packaging and systemd fixture prerequisites..."
    local required_files=(
        "$ROOT_DIR/packaging/build-deb.sh"
        "$ROOT_DIR/packaging/layout.sh"
        "$ROOT_DIR/packaging/deb/control.in"
        "$ROOT_DIR/packaging/deb/postinst"
        "$ROOT_DIR/packaging/deb/prerm"
        "$ROOT_DIR/packaging/deb/postrm"
        "$ROOT_DIR/packaging/systemd/fusionpanel.service"
        "$ROOT_DIR/packaging/systemd/fusionpanel-agent.service"
        "$ROOT_DIR/packaging/systemd/fusionpanel-nginx-helper.service"
    )
    local req
    for req in "${required_files[@]}"; do
        if [[ ! -f "$req" ]]; then
            echo "FAIL: Missing required packaging file: $req" >&2
            exit 1
        fi
    done

    echo "[check] Verifying systemd unit and postinst contract..."
    grep -Fxq 'User=fusionpanel' "$ROOT_DIR/packaging/systemd/fusionpanel.service"
    grep -Fxq 'User=fusionpanel' "$ROOT_DIR/packaging/systemd/fusionpanel-agent.service"
    grep -Fxq 'Group=fusionpanel-nginx' "$ROOT_DIR/packaging/systemd/fusionpanel-nginx-helper.service"
    grep -q 'fusionpanel.service' "$ROOT_DIR/packaging/deb/postinst"
    grep -q 'fusionpanel-agent.service' "$ROOT_DIR/packaging/deb/postinst"
    grep -q 'initial-credentials.txt' "$ROOT_DIR/packaging/deb/postinst"
    grep -q 'FUSIONPANEL_LISTEN=127.0.0.1:8080' "$ROOT_DIR/packaging/deb/postinst"
    grep -q 'FUSIONPANEL_NODE_NAME=' "$ROOT_DIR/packaging/deb/postinst"

    echo "[check] Verifying API health and node heartbeat routes..."
    grep -q '"/v1/health"' "$ROOT_DIR/crates/api/src/main.rs"
    grep -q '"/v1/nodes"' "$ROOT_DIR/crates/api/src/node_jobs.rs"
    grep -q '"/v1/nodes/{id}/heartbeat"' "$ROOT_DIR/crates/api/src/node_jobs.rs"

    echo "[check] Verifying safety guard refuses --local-guest without explicit opt-in..."
    if env -u FUSIONPANEL_DEB_E2E_ALLOW_LOCAL_INSTALL \
        bash "$SCRIPT_DIR/test_deb_systemd_qualification.sh" --local-guest --deb "$CHECK_TMP_DIR/nonexistent.deb" >/dev/null 2>&1; then
        echo "FAIL: --local-guest must refuse to run without FUSIONPANEL_DEB_E2E_ALLOW_LOCAL_INSTALL=1" >&2
        exit 1
    fi

    echo "PASS: Debian systemd qualification fixture wiring & syntax check passed."
}

# Emit the self-contained guest verification script executed either locally in a
# disposable guest or remotely on a clean Debian 12 amd64 VM over SSH.
emit_guest_helper() {
    cat <<'GUEST_EOF'
#!/usr/bin/env bash
set -euo pipefail

PHASE="$1"
DEB_FILE="${2:-}"
STATE_FILE="${3:-/var/tmp/fusionpanel-deb-e2e-state.env}"
TIMEOUT_SEC="${4:-60}"
EVIDENCE_DIR="${5:-}"

require_systemd_guest() {
    if [[ ! -d /run/systemd/system ]]; then
        echo "BLOCKED: /run/systemd/system not present; target is not running systemd as PID 1." >&2
        exit 2
    fi
    if [[ "$(id -u)" -ne 0 ]]; then
        echo "BLOCKED: Guest qualification phase must run as root." >&2
        exit 2
    fi
}

verify_unit_active_and_user() {
    local unit="$1"
    local expected_user="$2"
    local active_state
    active_state=$(systemctl is-active "$unit" || true)
    if [[ "$active_state" != "active" ]]; then
        echo "FAIL: Unit $unit is '$active_state', expected 'active'." >&2
        systemctl status "$unit" --no-pager || true
        exit 1
    fi
    local unit_user main_pid proc_user
    unit_user=$(systemctl show -p User --value "$unit")
    main_pid=$(systemctl show -p MainPID --value "$unit")
    if [[ -z "$main_pid" || "$main_pid" == "0" ]]; then
        echo "FAIL: Unit $unit has invalid MainPID '$main_pid'." >&2
        exit 1
    fi
    proc_user=$(ps -o user= -p "$main_pid" | tr -d '[:space:]')
    if [[ "$unit_user" != "$expected_user" || "$proc_user" != "$expected_user" ]]; then
        echo "FAIL: Unit $unit running as unit_user='$unit_user' proc_user='$proc_user', expected '$expected_user'." >&2
        exit 1
    fi
}

wait_for_loopback_health_and_ui() {
    local deadline=$((SECONDS + TIMEOUT_SEC))
    while (( SECONDS < deadline )); do
        if curl -fsS "http://127.0.0.1:8080/v1/health" >/dev/null 2>&1; then
            break
        fi
        sleep 1
    done
    curl -fsS "http://127.0.0.1:8080/v1/health" >/dev/null

    local ui_html
    ui_html=$(curl -fsS "http://127.0.0.1:8080/")
    if ! printf '%s' "$ui_html" | grep -qiE '<!doctype html|<html'; then
        echo "FAIL: Bundled UI at http://127.0.0.1:8080/ did not return HTML." >&2
        exit 1
    fi
}

read_admin_token() {
    local creds_file="/etc/fusionpanel/initial-credentials.txt"
    if [[ ! -f "$creds_file" ]]; then
        echo "FAIL: Missing $creds_file" >&2
        exit 1
    fi
    local perms
    perms=$(stat -c '%a' "$creds_file")
    if [[ "$perms" != "600" ]]; then
        echo "FAIL: $creds_file permissions are $perms, expected 600." >&2
        exit 1
    fi
    awk -F= '/^ADMIN_TOKEN=/{print $2; exit}' "$creds_file"
}

wait_for_agent_heartbeat() {
    local token="$1"
    local expected_node="$2"
    local deadline=$((SECONDS + TIMEOUT_SEC))
    local nodes_json=""
    local parsed=""

    while (( SECONDS < deadline )); do
        nodes_json=$(curl -fsS -H "Authorization: Bearer $token" "http://127.0.0.1:8080/v1/nodes" 2>/dev/null || true)
        if [[ -n "$nodes_json" ]]; then
            parsed=$(python3 - "$expected_node" "$nodes_json" <<'PY'
import json, sys
expected_name = sys.argv[1]
data = json.loads(sys.argv[2])
nodes = data if isinstance(data, list) else data.get("nodes", data.get("items", []))
for n in nodes:
    name = n.get("name") or n.get("node_name") or ""
    hb = n.get("last_heartbeat") or n.get("last_seen_at") or n.get("heartbeat_at") or ""
    nid = n.get("id") or n.get("node_id") or ""
    if (not expected_name or name == expected_name) and nid and hb:
        print(f"{nid}|{name}|{hb}")
        sys.exit(0)
sys.exit(1)
PY
            ) || true
            if [[ -n "$parsed" ]]; then
                printf '%s\n' "$parsed"
                return 0
            fi
        fi
        sleep 2
    done
    echo "FAIL: Timed out waiting for agent heartbeat for node '$expected_node'." >&2
    return 1
}

collect_sanitized_evidence() {
    local label="$1"
    [[ -n "$EVIDENCE_DIR" ]] || return 0
    mkdir -p "$EVIDENCE_DIR"
    dpkg-query -W -f='${Package}\t${Version}\t${Architecture}\t${Status}\n' fusionpanel \
        > "$EVIDENCE_DIR/${label}-dpkg.txt" 2>&1 || true
    systemctl status fusionpanel.service fusionpanel-agent.service fusionpanel-nginx-helper.service --no-pager \
        > "$EVIDENCE_DIR/${label}-systemd-status.txt" 2>&1 || true
    journalctl -u fusionpanel.service -u fusionpanel-agent.service --no-pager -n 100 \
        | sed -E 's/(ADMIN_TOKEN|FUSIONPANEL_ADMIN_TOKEN|Bearer )[A-Za-z0-9._-]+/\1[REDACTED]/g' \
        > "$EVIDENCE_DIR/${label}-journal.txt" 2>&1 || true
}

require_systemd_guest

if [[ "$PHASE" == "pre" ]]; then
    [[ -f "$DEB_FILE" ]] || { echo "FAIL: .deb not found: $DEB_FILE" >&2; exit 1; }
    echo "[guest:pre] Installing $DEB_FILE via apt-get..."
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y "$DEB_FILE"

    echo "[guest:pre] Verifying systemd units and service users..."
    verify_unit_active_and_user "fusionpanel.service" "fusionpanel"
    verify_unit_active_and_user "fusionpanel-agent.service" "fusionpanel"
    verify_unit_active_and_user "fusionpanel-nginx-helper.service" "root"

    echo "[guest:pre] Verifying loopback health and bundled UI..."
    wait_for_loopback_health_and_ui

    ADMIN_TOKEN=$(read_admin_token)
    [[ -n "$ADMIN_TOKEN" ]] || { echo "FAIL: Empty ADMIN_TOKEN." >&2; exit 1; }
    NODE_NAME=$(awk -F= '/^FUSIONPANEL_NODE_NAME=/{print $2; exit}' /etc/fusionpanel/fusionpanel-agent.env)

    echo "[guest:pre] Verifying authenticated API capabilities and agent heartbeat..."
    curl -fsS -H "Authorization: Bearer $ADMIN_TOKEN" "http://127.0.0.1:8080/v1/capabilities" >/dev/null
    HB_INFO=$(wait_for_agent_heartbeat "$ADMIN_TOKEN" "$NODE_NAME")
    IFS='|' read -r NODE_ID REGISTERED_NAME HB_TS <<<"$HB_INFO"

    TOKEN_SHA=$(printf '%s' "$ADMIN_TOKEN" | sha256sum | awk '{print $1}')
    DB_PATH="/var/lib/fusionpanel/fusionpanel.db"
    [[ -f "$DB_PATH" ]] || { echo "FAIL: Expected database $DB_PATH not found." >&2; exit 1; }
    DB_INODE=$(stat -c '%i' "$DB_PATH")

    cat > "$STATE_FILE" <<EOF
PRE_NODE_ID=$NODE_ID
PRE_NODE_NAME=$REGISTERED_NAME
PRE_HB_TS=$HB_TS
PRE_TOKEN_SHA=$TOKEN_SHA
PRE_DB_INODE=$DB_INODE
EOF
    chmod 0600 "$STATE_FILE"
    collect_sanitized_evidence "pre-reboot"
    echo "[guest:pre] PASS: node_id=$NODE_ID name=$REGISTERED_NAME hb=$HB_TS"

elif [[ "$PHASE" == "post" ]]; then
    [[ -f "$STATE_FILE" ]] || { echo "FAIL: Missing pre-reboot state file: $STATE_FILE" >&2; exit 1; }
    # shellcheck disable=SC1090
    . "$STATE_FILE"

    echo "[guest:post] Verifying systemd units after reboot..."
    verify_unit_active_and_user "fusionpanel.service" "fusionpanel"
    verify_unit_active_and_user "fusionpanel-agent.service" "fusionpanel"
    verify_unit_active_and_user "fusionpanel-nginx-helper.service" "root"

    echo "[guest:post] Verifying loopback health and bundled UI after reboot..."
    wait_for_loopback_health_and_ui

    ADMIN_TOKEN=$(read_admin_token)
    TOKEN_SHA=$(printf '%s' "$ADMIN_TOKEN" | sha256sum | awk '{print $1}')
    if [[ "$TOKEN_SHA" != "$PRE_TOKEN_SHA" ]]; then
        echo "FAIL: ADMIN_TOKEN changed across reboot." >&2
        exit 1
    fi

    DB_INODE=$(stat -c '%i' /var/lib/fusionpanel/fusionpanel.db)
    if [[ "$DB_INODE" != "$PRE_DB_INODE" ]]; then
        echo "FAIL: Database inode changed across reboot ($PRE_DB_INODE -> $DB_INODE)." >&2
        exit 1
    fi

    HB_INFO=$(wait_for_agent_heartbeat "$ADMIN_TOKEN" "$PRE_NODE_NAME")
    IFS='|' read -r POST_NODE_ID POST_NODE_NAME POST_HB_TS <<<"$HB_INFO"
    if [[ "$POST_NODE_ID" != "$PRE_NODE_ID" || "$POST_NODE_NAME" != "$PRE_NODE_NAME" ]]; then
        echo "FAIL: Node identity changed across reboot ($PRE_NODE_ID/$PRE_NODE_NAME -> $POST_NODE_ID/$POST_NODE_NAME)." >&2
        exit 1
    fi

    collect_sanitized_evidence "post-reboot"
    echo "[guest:post] PASS: Persistent node_id=$POST_NODE_ID name=$POST_NODE_NAME hb=$POST_HB_TS"
else
    echo "ERROR: Unknown guest phase: $PHASE" >&2
    exit 1
fi
GUEST_EOF
}

if [[ "$MODE" == "check" ]]; then
    run_check_mode
    exit 0
fi

if [[ "$MODE" == "local-pre" || "$MODE" == "local-post" ]]; then
    if [[ "${FUSIONPANEL_DEB_E2E_ALLOW_LOCAL_INSTALL:-0}" != "1" ]]; then
        echo "REFUSED: --local-guest requires FUSIONPANEL_DEB_E2E_ALLOW_LOCAL_INSTALL=1 (must only run inside a disposable Debian 12 systemd guest)." >&2
        exit 2
    fi
    helper=$(mktemp "${TMPDIR:-/tmp}/fusionpanel-guest-helper.XXXXXX.sh")
    trap 'rm -f -- "$helper"' EXIT HUP INT TERM
    emit_guest_helper > "$helper"
    chmod +x "$helper"
    if [[ "$MODE" == "local-pre" ]]; then
        [[ -n "$DEB_PATH" ]] || { echo "ERROR: --deb PATH is required." >&2; exit 1; }
        "$helper" pre "$DEB_PATH" "$STATE_FILE" "$TIMEOUT_SEC" "$EVIDENCE_DIR"
    else
        "$helper" post "" "$STATE_FILE" "$TIMEOUT_SEC" "$EVIDENCE_DIR"
    fi
    exit 0
fi

if [[ "$MODE" == "ssh" ]]; then
    [[ -n "$DEB_PATH" && -f "$DEB_PATH" ]] || { echo "ERROR: Valid --deb PATH is required." >&2; exit 1; }
    [[ -n "$SSH_TARGET" ]] || { echo "ERROR: --ssh USER@HOST is required." >&2; exit 1; }

    SSH_OPTS=(-p "$SSH_PORT" -o BatchMode=yes -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new)
    SCP_OPTS=(-P "$SSH_PORT" -o BatchMode=yes -o ConnectTimeout=5 -o StrictHostKeyChecking=accept-new)
    if [[ -n "$SSH_KEY" ]]; then
        SSH_OPTS+=(-i "$SSH_KEY")
        SCP_OPTS+=(-i "$SSH_KEY")
    fi

    tmp_local=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-ssh-e2e.XXXXXX")
    trap 'rm -rf -- "$tmp_local"' EXIT HUP INT TERM
    emit_guest_helper > "$tmp_local/guest-helper.sh"
    chmod +x "$tmp_local/guest-helper.sh"

    remote_dir="/tmp/fusionpanel-deb-e2e"
    remote_deb="$remote_dir/$(basename -- "$DEB_PATH")"
    ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "mkdir -p '$remote_dir'"
    scp "${SCP_OPTS[@]}" "$DEB_PATH" "$tmp_local/guest-helper.sh" "$SSH_TARGET:$remote_dir/"

    remote_ev=""
    if [[ -n "$EVIDENCE_DIR" ]]; then
        remote_ev="$remote_dir/evidence"
    fi

    echo "[ssh] Running Phase 1 (clean install & pre-reboot verification) on $SSH_TARGET..."
    ssh "${SSH_OPTS[@]}" "$SSH_TARGET" \
        "sudo bash '$remote_dir/guest-helper.sh' pre '$remote_deb' '$STATE_FILE' '$TIMEOUT_SEC' '$remote_ev'"

    echo "[ssh] Rebooting guest $SSH_TARGET..."
    ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "sudo systemctl reboot" || true
    sleep 5

    deadline=$((SECONDS + TIMEOUT_SEC + 60))
    while (( SECONDS < deadline )); do
        if ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "true" >/dev/null 2>&1; then
            break
        fi
        sleep 2
    done
    ssh "${SSH_OPTS[@]}" "$SSH_TARGET" "true"

    echo "[ssh] Running Phase 2 (post-reboot persistence verification) on $SSH_TARGET..."
    ssh "${SSH_OPTS[@]}" "$SSH_TARGET" \
        "sudo bash '$remote_dir/guest-helper.sh' post '' '$STATE_FILE' '$TIMEOUT_SEC' '$remote_ev'"

    if [[ -n "$EVIDENCE_DIR" ]]; then
        mkdir -p "$EVIDENCE_DIR"
        scp -r "${SCP_OPTS[@]}" "$SSH_TARGET:$remote_ev/." "$EVIDENCE_DIR/" || true
    fi

    echo "PASS: Full Debian 12 systemd clean-install & reboot qualification succeeded on $SSH_TARGET."
fi
