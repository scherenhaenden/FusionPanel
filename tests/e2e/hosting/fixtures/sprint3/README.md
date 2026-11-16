# Sprint 3 Websites Functional E2E Fixture

This fixture is the Sprint 3 API-to-agent-to-Nginx functional lane for real static-site and reverse-proxy lifecycle behavior. Run it only after the API, UI, and agent changes have been integrated and the matching API, CLI, agent, and frontend build artifacts exist in the checkout.

From the repository root, run:

```bash
bash tests/e2e/hosting/fixtures/sprint3/run.sh
```

The runner does not install host dependencies or build application artifacts. It checks the existing binaries, frontend build, Playwright dependency, Docker daemon, Docker usage, and free disk space before creating a uniquely named Compose project. It binds the API and managed Nginx ports to dynamically allocated loopback ports. Host writable fixture state is under a unique temporary directory; managed Nginx files live on a size-limited tmpfs inside the disposable node container. Compose creates no named volumes.

The three services are:

- `api`: the existing Sprint 2 API fixture image with the real API, CLI, and built frontend mounted read-only; SQLite is in a 64 MiB container tmpfs owned by the fixture UID. It shares only the disposable node's network namespace so the actual agent can use the API's required loopback HTTP boundary. The runner publishes the shared API and Nginx ports on dynamically assigned loopback-only host ports.
- `node`: the real agent process and a real foreground Nginx process in the same disposable container. Both run with the invoking user's UID/GID, and the container drops every Linux capability. A container-scoped `net.ipv4.ip_unprivileged_port_start=0` lets the generated `listen 80` vhosts work without giving either process a capability. Nginx uses Ubuntu `nginx` package `1.24.0-2ubuntu7.19` ([Ubuntu package listing](https://packages.ubuntu.com/noble/nginx)) on the repository-pinned `ubuntu:24.04` base. Candidate validation is `nginx -t`; reload is `nginx -s reload -c /etc/nginx/nginx.conf` against the same running server. The node entrypoint waits for the API to listen on loopback before starting the agent. No host Nginx or service manager is used.
- `upstream`: a small real HTTP service using the repository-pinned image `nginx:alpine@sha256:785835ea65db4bba6f021acc0f3be91b0fef76a53fe41f8588c6698d2aed0606`, running as UID 65532 on unprivileged port 8080.

The Playwright journey uses the UI to create a static site and a reverse-proxy site, checks `POST`/`DELETE` response envelopes `{site, job}`, the list envelope `{websites}`, the detail envelope `{website}`, typed agent operation payloads, and terminal job/site states. It checks real HTTP content through Nginx, then creates a reverse proxy to `http://unresolvable.invalid`. That URL is syntactically valid input; the real Nginx candidate check must reject it before promotion. Existing static/proxy traffic and the unrelated test vhost must keep serving. The test then deletes both active sites, checks their routes return 404, and checks the unrelated vhost still serves its marker.

Before the browser journey, the runner calls `nginx -t -q` in the real node container with the same literal `.invalid` `proxy_pass` form and requires the pinned Nginx binary to report `host not found in upstream`. The failure case relies on Nginx resolving a literal `proxy_pass` hostname during config validation; Nginx's own issue tracker records an unresolved upstream causing configuration test failure ([NGINX ticket #1040](https://trac.nginx.org/nginx/ticket/1040)). This direct probe is retained in the runner log because the agent captures Nginx subprocess output when validating its candidate. The exact Ubuntu Nginx package is pinned in the fixture and its `nginx -v` output is retained in the container logs.

The runner verifies the direct Nginx probe reports `host not found in upstream`, confirms the failed site leaves no candidate or enabled managed config, saves bounded/redacted container logs, tears down only its Compose project, removes only the two image tags it created, verifies its containers, network, and volumes are gone, and removes only its own temporary directory. It does not prune global Docker resources. Successful-run logs are retained in `RUNNER_TEMP` or the selected temporary directory for the release evidence record.

This lane proves candidate validation rejection before promotion. It does not claim to exercise a failure after promotion during Nginx reload.

## Package Acceptance Blocker

The API process, `fusionpanel-agent` development binary, Nginx process, and upstream all run as unprivileged users; the node container drops every Linux capability and uses the invoking UID for the managed tmpfs. This lane exercises the real HTTP job/worker/config/reload path without running as root. It proves API/worker behavior only; it is **not** evidence that the installed `fusionpanel` systemd service can modify its managed Nginx tree or reload Nginx. Package acceptance remains blocked until the privileged helper or systemd unit contract is implemented, the managed include is installed, and a separate fixture update runs that packaged boundary under its actual service identity and `ProtectSystem=strict` policy.
