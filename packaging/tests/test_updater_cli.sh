#!/bin/bash
set -e

# Wait for old port to free up if needed
kill $(lsof -t -i :18080) 2>/dev/null || true

# Build the updater binary
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
UPDATER_DIR="${SCRIPT_DIR}/../updater"
cd "${UPDATER_DIR}"
cargo build --release

UPDATER_BIN="${UPDATER_DIR}/target/release/updater"

if [ ! -x "${UPDATER_BIN}" ]; then
    echo "FAIL: Expected updater binary at ${UPDATER_BIN}"
    false
fi

TEST_DIR=$(mktemp -d)
trap 'rm -rf "${TEST_DIR}"' EXIT

cd "${TEST_DIR}"

# Create candidate script
cat << 'SHELL' > candidate.sh
#!/bin/sh
while true; do
  sleep 1
done
SHELL
chmod +x candidate.sh

# Python script to act as the web server for downloading binaries and handling health checks
cat << 'PY' > server.py
from http.server import HTTPServer, BaseHTTPRequestHandler
import sys
import threading

# Simple mock server
class MockHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/bin':
            self.send_response(200)
            self.end_headers()
            with open("candidate.sh", "rb") as f:
                self.wfile.write(f.read())
        elif self.path == '/sig':
            self.send_response(200)
            self.end_headers()
            with open("signature.bin", "rb") as f:
                self.wfile.write(f.read())
        elif self.path == '/health':
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"OK")
        elif self.path == '/health_fail':
            self.send_response(500)
            self.end_headers()
            self.wfile.write(b"FAIL")
        else:
            self.send_response(404)
            self.end_headers()

def run_server(port):
    server = HTTPServer(('127.0.0.1', port), MockHandler)
    server.serve_forever()

if __name__ == '__main__':
    port = int(sys.argv[1])
    threading.Thread(target=run_server, args=(port,), daemon=True).start()
    while True:
        pass
PY

# We need to sign candidate.sh for the updater.
# We'll use a small Rust program to generate the keypair and signature,
# because ed25519 signatures are standard but hard to do from bash without a tool.

mkdir signer_crate
cd signer_crate
mkdir src

cat << 'RUST' > src/main.rs
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use std::fs;

fn main() {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    let public_key_bytes = verifying_key.to_bytes();

    let candidate_data = fs::read("../candidate.sh").unwrap();
    let signature = signing_key.sign(&candidate_data);

    fs::write("../public_key.hex", hex::encode(public_key_bytes)).unwrap();
    fs::write("../signature.bin", signature.to_bytes()).unwrap();
}
RUST

cat << 'CARGO' > Cargo.toml
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

cargo run --release

cd ..

PUBKEY=$(cat public_key.hex)
PORT=18080

# Start mock server
python3 server.py ${PORT} &
SERVER_PID=$!
sleep 2

# Setup target
TARGET_PATH="${TEST_DIR}/target_binary"
echo "Original Content" > "${TARGET_PATH}"
chmod +x "${TARGET_PATH}"

echo "--- Test: Updater CLI Success ---"
${UPDATER_BIN} \
    "http://127.0.0.1:${PORT}/bin" \
    "http://127.0.0.1:${PORT}/sig" \
    "${PUBKEY}" \
    "${TARGET_PATH}" \
    "http://127.0.0.1:${PORT}/health" \
    "2"

# Wait for candidate to be spawned
sleep 1

# Verify file updated
if ! grep -q "while true; do" "${TARGET_PATH}"; then
    echo "FAIL: target_binary was not updated"
    kill -9 $(pgrep -f "candidate.sh") || true
    kill -9 ${SERVER_PID} || true
    false
fi
echo "PASS: Updater CLI Success"

# Kill spawned candidate
kill -9 $(pgrep -f "candidate.sh") || true

# Restore original
echo "Original Content" > "${TARGET_PATH}"

echo "--- Test: Updater CLI Failure (Health Check) ---"
set +e
${UPDATER_BIN} \
    "http://127.0.0.1:${PORT}/bin" \
    "http://127.0.0.1:${PORT}/sig" \
    "${PUBKEY}" \
    "${TARGET_PATH}" \
    "http://127.0.0.1:${PORT}/health_fail" \
    "2"
EXIT_CODE=$?
set -e

if [ ${EXIT_CODE} -eq 0 ]; then
    echo "FAIL: Expected updater to fail on health check"
    kill -9 ${SERVER_PID} || true
    false
fi

if ! grep -q "Original Content" "${TARGET_PATH}"; then
    echo "FAIL: expected target_binary to rollback"
    kill -9 ${SERVER_PID} || true
    false
fi
echo "PASS: Updater CLI Failure (Rollback)"

kill -9 ${SERVER_PID} || true
echo "All Updater CLI tests passed."
