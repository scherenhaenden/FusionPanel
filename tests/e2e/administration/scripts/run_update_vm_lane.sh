#!/usr/bin/env bash
set -euo pipefail

# Entrypoint for the Sprint 12 two-release Debian VM E2E lane.
# Always tears down temporary staging directories on exit.

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
ADMIN_E2E_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
REPO_ROOT=$(CDPATH= cd -- "$ADMIN_E2E_DIR/../../.." && pwd)

WORK_DIR=$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-s12-vm-lane.XXXXXX")
EVIDENCE_OUT="${FUSIONPANEL_VM_EVIDENCE_PATH:-$WORK_DIR/update-vm-evidence.json}"

cleanup() {
    local exit_code=$?
    trap - EXIT HUP INT TERM
    if [[ -n "${VM_TEARDOWN_CMD:-}" ]]; then
        eval "$VM_TEARDOWN_CMD" || true
    fi
    rm -rf -- "$WORK_DIR"
    exit "$exit_code"
}
trap cleanup EXIT HUP INT TERM

export FUSIONPANEL_HEAD_SHA="${FUSIONPANEL_HEAD_SHA:-$(git -C "$REPO_ROOT" rev-parse HEAD)}"
export FUSIONPANEL_BASE_SHA="${FUSIONPANEL_BASE_SHA:-$(git -C "$REPO_ROOT" merge-base HEAD origin/master 2>/dev/null || git -C "$REPO_ROOT" rev-parse HEAD~1)}"
export FUSIONPANEL_VM_EVIDENCE_PATH="$EVIDENCE_OUT"

echo "Running Sprint 12 local update VM lane checks..."
cargo test --manifest-path "$ADMIN_E2E_DIR/Cargo.toml" --lib
cargo test --manifest-path "$ADMIN_E2E_DIR/Cargo.toml" --test update_vm_lane

if [[ "${FUSIONPANEL_VM_E2E_DISPOSABLE:-0}" == "1" ]]; then
    echo "Running ignored Debian VM fixture test..."
    cargo test --manifest-path "$ADMIN_E2E_DIR/Cargo.toml" --test update_vm_lane -- --ignored --nocapture
    if [[ -f "$EVIDENCE_OUT" ]]; then
        echo "Evidence bundle recorded at: $EVIDENCE_OUT"
    fi
else
    echo "Skipping Debian VM fixture test (set FUSIONPANEL_VM_EVIDENCE_PATH and FUSIONPANEL_VM_E2E_DISPOSABLE=1 inside a disposable Debian VM)."
fi
