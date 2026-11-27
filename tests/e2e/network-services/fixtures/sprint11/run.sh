#!/usr/bin/env bash
set -Eeuo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../../../../.." && pwd)
browser_dir="$repo_root/tests/e2e/browser"

suffix=$(tr -dc 'a-f0-9' </dev/urandom | head -c 12 || true)
[ -n "$suffix" ] || suffix=$(date +%s)
project_name="fp-s11-mail-e2e-${suffix}"
state_dir=$(mktemp -d /tmp/fusionpanel-sprint11-e2e.XXXXXX)
api_ctx="$state_dir/api-ctx"
node_ctx="$state_dir/node-ctx"
mail_ctx="$state_dir/mail-ctx"
dns_dir="$state_dir/dns"
mail_dir="$state_dir/mail"
api_state_dir="$state_dir/api-state"
rndc_key="$state_dir/rndc.key"
log_file="/tmp/fusionpanel-sprint11-e2e-${suffix}.log"
compose_invoked=0

export FUSIONPANEL_S11_REPO_ROOT="$repo_root"
export FUSIONPANEL_S11_UID=$(id -u)
export FUSIONPANEL_S11_GID=$(id -g)
export FUSIONPANEL_S11_SUFFIX="$suffix"
export FUSIONPANEL_S11_PROJECT="$project_name"
export FUSIONPANEL_S11_COMPOSE_FILE="$script_dir/docker-compose.yml"
export FUSIONPANEL_S11_DNS_DIR="$dns_dir"
export FUSIONPANEL_S11_MAIL_DIR="$mail_dir"
export FUSIONPANEL_S11_API_STATE_DIR="$api_state_dir"
export FUSIONPANEL_S11_RNDC_KEY="$rndc_key"
export FUSIONPANEL_S11_API_IMAGE="fusionpanel-s11-api:${suffix}"
export FUSIONPANEL_S11_NODE_IMAGE="fusionpanel-s11-node:${suffix}"
export FUSIONPANEL_S11_MAIL_IMAGE="fusionpanel-s11-mail:${suffix}"
export FUSIONPANEL_S11_API_BUILD_CONTEXT="$api_ctx"
export FUSIONPANEL_S11_NODE_BUILD_CONTEXT="$node_ctx"
export FUSIONPANEL_S11_MAIL_BUILD_CONTEXT="$mail_ctx"
export FUSIONPANEL_E2E_ADMIN_TOKEN=$(head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n')
export FUSIONPANEL_E2E_USERNAME="e2e-admin"
export FUSIONPANEL_E2E_PASSWORD=$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')

compose() {
  docker compose -p "$project_name" -f "$script_dir/docker-compose.yml" "$@"
}

cleanup() {
  local rc=$?
  trap - EXIT INT TERM
  if [ "$compose_invoked" -eq 1 ]; then
    compose logs --no-color >"$log_file" 2>&1 || true
    compose down -v --remove-orphans || rc=1
  fi
  docker image rm -f "$FUSIONPANEL_S11_API_IMAGE" "$FUSIONPANEL_S11_NODE_IMAGE" "$FUSIONPANEL_S11_MAIL_IMAGE" >/dev/null 2>&1 || true
  rm -rf -- "$state_dir"
  docker system df || true
  printf 'Sprint 11 Mail E2E log: %s\n' "$log_file"
  exit "$rc"
}

trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

printf 'Docker usage before Sprint 11 Mail E2E:\n'
docker system df

mkdir -p "$api_ctx" "$node_ctx" "$mail_ctx" "$dns_dir/zones" "$mail_dir/state" "$api_state_dir"
: > "$dns_dir/named.conf.fusionpanel"
chmod -R 0777 "$dns_dir" "$mail_dir" "$api_state_dir"

cat > "$rndc_key" <<'EOF'
key "rndc-key" {
    algorithm hmac-sha256;
    secret "c3ByaW50MTEtZTJlLXRlc3Qtcm5kYy1rZXktMTIzNDU=";
};
EOF
chmod 0644 "$rndc_key"

cp "$repo_root/tests/e2e/browser/fixtures/sprint2/Dockerfile" "$api_ctx/Dockerfile"
cp "$repo_root/tests/e2e/browser/fixtures/sprint2/seed-admin.sh" "$api_ctx/seed-admin.sh"
cp "$script_dir/Dockerfile.node" "$node_ctx/Dockerfile"
cp "$script_dir/Dockerfile.mail" "$mail_ctx/Dockerfile"
cp "$script_dir/mail-entrypoint.sh" "$mail_ctx/mail-entrypoint.sh"

RUST_MIN_STACK=33554432 cargo build --locked --offline --manifest-path "$repo_root/Cargo.toml" -p fusionpanel-api -p fusionpanel-cli
RUST_MIN_STACK=33554432 cargo build --locked --offline --manifest-path "$repo_root/crates/agent/Cargo.toml" --bin fusionpanel-agent
npm --prefix "$repo_root/frontend" run build

DOCKER_BUILDKIT=0 docker build --file "$api_ctx/Dockerfile" --tag "$FUSIONPANEL_S11_API_IMAGE" "$api_ctx"
DOCKER_BUILDKIT=0 docker build --file "$node_ctx/Dockerfile" --tag "$FUSIONPANEL_S11_NODE_IMAGE" "$node_ctx"
DOCKER_BUILDKIT=0 docker build --file "$mail_ctx/Dockerfile" --tag "$FUSIONPANEL_S11_MAIL_IMAGE" "$mail_ctx"

compose config --quiet
compose_invoked=1
compose up --detach --wait --wait-timeout 180

api_addr=$(compose port api 8080)
api_port=${api_addr##*:}
bind9_addr=$(compose port --protocol udp bind9 53)
bind9_port=${bind9_addr##*:}
smtp_addr=$(compose port mail 25)
smtp_port=${smtp_addr##*:}
submission_addr=$(compose port mail 587)
submission_port=${submission_addr##*:}
imap_addr=$(compose port mail 143)
imap_port=${imap_addr##*:}

export FUSIONPANEL_FRONTEND_URL="http://127.0.0.1:${api_port}"
export FUSIONPANEL_S11_BIND9_PORT="$bind9_port"
export FUSIONPANEL_S11_SMTP_PORT="$smtp_port"
export FUSIONPANEL_S11_SUBMISSION_PORT="$submission_port"
export FUSIONPANEL_S11_IMAP_PORT="$imap_port"

"$browser_dir/node_modules/.bin/playwright" test \
  --config "$script_dir/playwright.sprint11.config.mjs"
