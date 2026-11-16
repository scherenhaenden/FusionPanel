# Development packages

The package workflow builds the control-plane API, operator CLI, and node agent from locked Rust dependencies, then builds and bundles the frontend from `frontend/package-lock.json`. The archive contains `fusionpanel-api`, `fusionpanel-agent`, `fusionpanel-cli`, and `frontend/dist/`. Pull requests and manual runs upload a development artifact to GitHub Actions for review. These artifacts are not published releases and do not imply production readiness. The package currently targets x86_64 Linux with a Debian-compatible runtime.

An annotated release tag has the exact form `yyyy.MM.dd.HH.mm.sss.g<12-character-commit-prefix>` and must pass `tools/versioning/version.py verify`. A tag-triggered build includes the annotation's JSON manifest in the archive, exposes its full version in the API build, and publishes the artifacts as a GitHub prerelease. The release also contains `RELEASE_MANIFEST.json` and `SHA256SUMS`. Follow the project's versioning policy and verify the reviewed commit before creating a tag.

To install a published prerelease, specify its **complete** tag:

```sh
./packaging/install.sh --version '2026.11.08.16.30.123.gabcdef123456'
```

That tag is an example; use an actual published tag. Each release uses a fixed asset name such as `fusionpanel-debian-x86_64.tar.gz`, and the installer falls back to the historical versioned filename. GitHub's `releases/latest` endpoint excludes prereleases, so `--version latest` does not select these development packages.

The installer writes `/etc/fusionpanel/fusionpanel.env` and `/etc/fusionpanel/fusionpanel-agent.env` as `root:root 0600`; systemd reads each environment before dropping privileges for its service. The API environment sets `FUSIONPANEL_FRONTEND_DIR` to `/usr/local/lib/fusionpanel/versions/current/bin/frontend/dist`, so the installed API serves the bundled UI regardless of its working directory. The installer and updater preserve an existing regular agent environment during upgrades so remote API URLs and node credentials survive; symlinked or non-regular environment files are rejected. Versioned config copies are root-owned and mode `0600`. The API binds to loopback and stores SQLite data at `/var/lib/fusionpanel/fusionpanel.db`. The local HTTP agent uses that loopback API URL, the generated admin token, and a sanitized node name derived from the system hostname. The agent unit starts after the API and requests it when started directly; it remains active across API restarts and retries transient API failures. Remote agent API endpoints must use HTTPS and a pre-provisioned node ID/token; the global admin token must not be configured on a remote agent.

For legacy remote WebSocket deployments, use `wss://` and a server certificate chain trusted by the host's system certificate store, with a hostname matching the certificate. The agent uses Rustls with certificate verification enabled. Plain `ws://` is accepted only for loopback/localhost.

To configure a remote HTTP agent, register it once from a trusted operator host against the control plane's existing admin endpoint. Keep the admin token on that trusted host and handle the response token as a secret:

```sh
curl --fail --silent --show-error \
  --proto '=https' --proto-redir '=https' \
  -H "Authorization: Bearer $FUSIONPANEL_ADMIN_TOKEN" \
  -H 'Content-Type: application/json' \
  --data '{"name":"edge-node-1"}' \
  "$FUSIONPANEL_CONTROL_PLANE_URL/v1/nodes/register"
```

The response contains `node_id` and a one-time `token`. Provision those as `FUSIONPANEL_NODE_ID` and `FUSIONPANEL_NODE_TOKEN` in `/etc/fusionpanel/fusionpanel-agent.env` on the remote host, set `FUSIONPANEL_AGENT_MODE=http` and `FUSIONPANEL_API_URL=https://<control-plane-host>`, and remove `FUSIONPANEL_ADMIN_TOKEN` from that agent environment. Do not place the response in shell history, CI logs, or chat. Heartbeats and Jobs operations authenticate with the node-scoped token; the API never returns it from node-listing routes. Re-registering the same node name rotates its token, so update the agent environment before restarting it if credentials are rotated. The installer preserves this configuration during package upgrades.

The API and agent services currently share the `fusionpanel` Unix account and are treated as one host trust boundary. Root-only environment files prevent reading credentials from disk by the service account, but a compromised same-UID process may inspect a peer process's runtime credentials. A node token only authorizes that node's heartbeat and Jobs operations; separate service accounts would be needed for stronger process isolation.

The installer and updater use the same version layout: `/usr/local/lib/fusionpanel/versions/<version>/bin/` contains the three executables and `frontend/dist/`, while `config/` holds root-only environment copies. `/usr/local/lib/fusionpanel/versions/current` is atomically switched to the active version. Stable commands in `/usr/local/bin` resolve through that alias. The updater derives its health URL from the saved `FUSIONPANEL_LISTEN` value; `FUSIONPANEL_HEALTH_URL` can override it explicitly. Any update failure after stopping services triggers rollback and a service restart. The installer keeps token-bearing environment copies in root-owned versioned release directories at mode `0600` and reapplies that ownership and permission to copies from earlier versions during upgrades.

To build locally, provide Rust, Node.js 20 with npm, a non-release label, and an output directory. The root Cargo lockfile, frontend npm lockfile, and standalone agent lockfile are all used in locked builds:

```sh
bash packaging/build-package.sh ci-local /tmp/fusionpanel-package
```
