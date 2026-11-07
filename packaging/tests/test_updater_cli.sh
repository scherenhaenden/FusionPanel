#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd)"
UPDATER_DIR="${SCRIPT_DIR}/../updater"
UPDATER_TARGET_DIR="${CARGO_TARGET_DIR:-${UPDATER_DIR}/target}"
CARGO_TARGET_DIR="${UPDATER_TARGET_DIR}" cargo build --release --manifest-path "${UPDATER_DIR}/Cargo.toml"

UPDATER_BIN="${UPDATER_TARGET_DIR}/release/updater"
if [[ ! -x "${UPDATER_BIN}" ]]; then
    echo "FAIL: Expected updater binary at ${UPDATER_BIN}" >&2
    exit 1
fi

TEST_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fusionpanel-updater-cli.XXXXXX")"
SERVER_PID=""
CANDIDATE_PID=""

stop_process() {
    local pid="$1"
    [[ "${pid}" =~ ^[0-9]+$ ]] || return 0
    kill -TERM "${pid}" 2>/dev/null || return 0

    for ((attempt = 0; attempt < 20; attempt++)); do
        if ! kill -0 "${pid}" 2>/dev/null; then
            return 0
        fi
        sleep 0.05
    done

    kill -KILL "${pid}" 2>/dev/null || true
}

cleanup() {
    local status=$?
    trap - EXIT INT TERM HUP
    set +e

    if [[ -n "${CANDIDATE_PID}" ]]; then
        stop_process "${CANDIDATE_PID}"
    fi
    if [[ -n "${SERVER_PID}" ]]; then
        kill -TERM "${SERVER_PID}" 2>/dev/null || true
        wait "${SERVER_PID}" 2>/dev/null || true
    fi
    rm -rf -- "${TEST_DIR}"
    exit "${status}"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP

cd -- "${TEST_DIR}"

cat > candidate.sh <<'SHELL'
#!/bin/sh
printf '%s\n' "$$" > "${FP_TEST_CANDIDATE_PID_FILE}"
exec sleep 300
SHELL
chmod +x candidate.sh

cat > server.py <<'PY'
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import os
import pathlib
import sys
import time


class MockHandler(BaseHTTPRequestHandler):
    def send_body(self, status, body):
        self.send_response(status)
        self.end_headers()
        try:
            self.wfile.write(body)
        except BrokenPipeError:
            # The updater intentionally closes timed-out health requests.
            pass

    def candidate_is_running(self):
        try:
            pid = int(pathlib.Path(os.environ["FP_TEST_CANDIDATE_PID_FILE"]).read_text())
            os.kill(pid, 0)
            return True
        except (OSError, ValueError):
            return False

    def do_GET(self):
        if self.path == "/bin":
            self.send_body(200, pathlib.Path("candidate.sh").read_bytes())
        elif self.path == "/sig":
            self.send_body(200, pathlib.Path("signature.bin").read_bytes())
        elif self.path == "/health":
            if self.candidate_is_running():
                self.send_body(200, b"OK")
            else:
                self.send_body(503, b"Candidate is not running")
        elif self.path == "/health_fail":
            self.send_body(500, b"FAIL")
        elif self.path == "/health_slow":
            time.sleep(3)
            self.send_body(200, b"Too late")
        else:
            self.send_body(404, b"Not found")

    def log_message(self, _format, *_args):
        pass


if __name__ == "__main__":
    port_file = pathlib.Path(sys.argv[1])
    server = ThreadingHTTPServer(("127.0.0.1", 0), MockHandler)
    server.daemon_threads = True
    port_file.write_text(str(server.server_address[1]))
    server.serve_forever(poll_interval=0.1)
PY

# Build an ephemeral signing key and signature for this synthetic candidate.
mkdir -p signer_crate/src
cat > signer_crate/src/main.rs <<'RUST'
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use std::fs;

fn main() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let public_key = signing_key.verifying_key().to_bytes();
    let candidate = fs::read("../candidate.sh").expect("read candidate fixture");
    let signature = signing_key.sign(&candidate);

    fs::write("../public_key.hex", hex::encode(public_key)).expect("write public key");
    fs::write("../signature.bin", signature.to_bytes()).expect("write signature");
}
RUST

cat > signer_crate/Cargo.toml <<'CARGO'
[package]
name = "signer"
version = "0.1.0"
edition = "2021"

[dependencies]
ed25519-dalek = { version = "2.0", features = ["rand_core"] }
rand = "0.8"
hex = "0.4"

[workspace]
CARGO

(cd -- "${TEST_DIR}/signer_crate" && cargo run --release)
PUBLIC_KEY="$(<public_key.hex)"
PID_FILE="${TEST_DIR}/candidate.pid"
export FP_TEST_CANDIDATE_PID_FILE="${PID_FILE}"

PORT_FILE="${TEST_DIR}/server.port"
python3 server.py "${PORT_FILE}" &
SERVER_PID=$!

PORT=""
for ((attempt = 0; attempt < 100; attempt++)); do
    if [[ -s "${PORT_FILE}" ]]; then
        PORT="$(<"${PORT_FILE}")"
        break
    fi
    if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
        echo "FAIL: Mock server exited before binding" >&2
        exit 1
    fi
    sleep 0.05
done
if [[ ! "${PORT}" =~ ^[0-9]+$ ]]; then
    echo "FAIL: Mock server did not publish its ephemeral port" >&2
    exit 1
fi

BINARY_URL="http://127.0.0.1:${PORT}/bin"
SIGNATURE_URL="http://127.0.0.1:${PORT}/sig"
HEALTH_URL="http://127.0.0.1:${PORT}/health"
TARGET_PATH="${TEST_DIR}/target_binary"
printf 'Original Content\n' > "${TARGET_PATH}"
chmod +x "${TARGET_PATH}"

run_updater() {
    local health_path="$1"
    local timeout_seconds="$2"
    "${UPDATER_BIN}" \
        "${BINARY_URL}" \
        "${SIGNATURE_URL}" \
        "${PUBLIC_KEY}" \
        "${TARGET_PATH}" \
        "http://127.0.0.1:${PORT}/${health_path}" \
        "${timeout_seconds}"
}

echo "--- Test: Updater CLI Success ---"
SUCCESS_OUTPUT_FILE="${TEST_DIR}/updater-success.log"
if run_updater health 2 >"${SUCCESS_OUTPUT_FILE}" 2>&1; then
    SUCCESS_OUTPUT="$(<"${SUCCESS_OUTPUT_FILE}")"
else
    SUCCESS_OUTPUT="$(<"${SUCCESS_OUTPUT_FILE}")"
    echo "FAIL: updater rejected the healthy candidate" >&2
    echo "${SUCCESS_OUTPUT}" >&2
    exit 1
fi
echo "${SUCCESS_OUTPUT}"

OUTPUT_PID="$(sed -n 's/.*Candidate started with PID \([0-9][0-9]*\).*/\1/p' <<< "${SUCCESS_OUTPUT}")"
if [[ ! -s "${PID_FILE}" ]]; then
    echo "FAIL: candidate did not publish its test PID" >&2
    exit 1
fi
CANDIDATE_PID="$(<"${PID_FILE}")"
if [[ ! "${CANDIDATE_PID}" =~ ^[0-9]+$ || "${OUTPUT_PID}" != "${CANDIDATE_PID}" ]]; then
    echo "FAIL: updater output did not match the spawned candidate PID" >&2
    exit 1
fi
if ! kill -0 "${CANDIDATE_PID}" 2>/dev/null; then
    echo "FAIL: candidate process was not running after a successful update" >&2
    exit 1
fi
if ! grep -Fq 'exec sleep 300' "${TARGET_PATH}"; then
    echo "FAIL: target_binary was not replaced with the candidate" >&2
    exit 1
fi
echo "PASS: Updater CLI Success"

stop_process "${CANDIDATE_PID}"
CANDIDATE_PID=""
rm -f -- "${PID_FILE}"
printf 'Original Content\n' > "${TARGET_PATH}"

echo "--- Test: Updater CLI Failure (Health Check) ---"
set +e
FAIL_OUTPUT_FILE="${TEST_DIR}/updater-health-fail.log"
run_updater health_fail 2 >"${FAIL_OUTPUT_FILE}" 2>&1
FAIL_STATUS=$?
set -e
FAIL_OUTPUT="$(<"${FAIL_OUTPUT_FILE}")"
if [[ "${FAIL_STATUS}" -eq 0 ]]; then
    echo "FAIL: updater succeeded despite a failing health check" >&2
    echo "${FAIL_OUTPUT}" >&2
    exit 1
fi
if ! grep -Fq 'Original Content' "${TARGET_PATH}"; then
    echo "FAIL: previous target was not restored after health-check failure" >&2
    exit 1
fi
if [[ ! -s "${PID_FILE}" ]]; then
    echo "FAIL: failing candidate did not publish its test PID" >&2
    exit 1
fi
FAILED_CANDIDATE_PID="$(<"${PID_FILE}")"
if kill -0 "${FAILED_CANDIDATE_PID}" 2>/dev/null; then
    echo "FAIL: candidate remained running after health-check rollback" >&2
    CANDIDATE_PID="${FAILED_CANDIDATE_PID}"
    exit 1
fi
echo "PASS: Updater CLI Failure (Rollback)"

rm -f -- "${PID_FILE}"
printf 'Original Content\n' > "${TARGET_PATH}"

echo "--- Test: Updater CLI Health Request Timeout ---"
set +e
TIMEOUT_OUTPUT_FILE="${TEST_DIR}/updater-health-timeout.log"
run_updater health_slow 1 >"${TIMEOUT_OUTPUT_FILE}" 2>&1
TIMEOUT_STATUS=$?
set -e
TIMEOUT_OUTPUT="$(<"${TIMEOUT_OUTPUT_FILE}")"
if [[ "${TIMEOUT_STATUS}" -eq 0 ]]; then
    echo "FAIL: updater accepted a health response that arrived after its deadline" >&2
    echo "${TIMEOUT_OUTPUT}" >&2
    exit 1
fi
if ! grep -Fq 'Original Content' "${TARGET_PATH}"; then
    echo "FAIL: previous target was not restored after health request timeout" >&2
    exit 1
fi
if [[ ! -s "${PID_FILE}" ]]; then
    echo "FAIL: timed-out candidate did not publish its test PID" >&2
    exit 1
fi
TIMED_OUT_CANDIDATE_PID="$(<"${PID_FILE}")"
if kill -0 "${TIMED_OUT_CANDIDATE_PID}" 2>/dev/null; then
    echo "FAIL: candidate remained running after health request timeout" >&2
    CANDIDATE_PID="${TIMED_OUT_CANDIDATE_PID}"
    exit 1
fi
echo "PASS: Updater CLI Health Request Timeout"

echo "All updater CLI tests passed."
