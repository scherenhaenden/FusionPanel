# FusionPanel

FusionPanel is an early-stage, self-hosted control panel for Linux servers. Its goal is to manage websites, containers, databases, DNS, and mail through a shared web UI, HTTP API, and CLI. The backend is written in Rust, and the frontend uses React, TypeScript, and Vite.

## Project status

The quickest way to try FusionPanel today is to build the control-plane API and use the CLI to check health and list available API capabilities.

| Area | Available in the repository | Qualification still needed |
| --- | --- | --- |
| Control plane | The main API exposes `GET /v1/health` and `GET /v1/capabilities`. The CLI supports both. | Mount identity and authorization in the main API and verify the full operator session. |
| Web UI | The frontend has a login screen and a dashboard for API health and capabilities. | Verify sign-in, dashboard access, and sign-out against the running main API. Identity routes currently live in a separate integration package. |
| Hosting and services | The repository contains components for hosting and WordPress, containers, databases, deployments, backups, DNS, mail, security, observability, and node management. | Verify supported workflows and modes against real services. See the capability ledger for evidence and open gaps. |
| Installation and recovery | The repository contains an installer, systemd units, and an updater. | Qualify clean installation, reboot, update, and recovery in a disposable Debian VM. |

FusionPanel remains in development. Use disposable environments for evaluation. The [W04 release decision](docs/execution/waves/W04_2026-11-03/final/RELEASE_DECISION.md) records a no-release decision. The [product delivery plan](docs/execution/PRODUCT_DELIVERY_PLAN.md) starts with a working operator session and then moves to clean installation and administration.

## Try the API and CLI locally

Use a Rust toolchain with Cargo, a POSIX-compatible shell, and OpenSSL to generate a disposable token. Run these commands from the repository root.

```sh
cargo build --locked -p fusionpanel-api -p fusionpanel-cli
```

Start the API on loopback in the background.

```sh
export FUSIONPANEL_ADMIN_TOKEN="$(openssl rand -hex 32)"
export FUSIONPANEL_LISTEN=127.0.0.1:8080
./target/debug/fusionpanel-api &
fusionpanel_api_pid=$!
```

Once the API prints `Listening on 127.0.0.1:8080`, use the same terminal to check health and list capabilities. The CLI defaults to `http://127.0.0.1:8080`. Use `--server` before the subcommand if you changed the address.

```sh
./target/debug/fusionpanel-cli health
FUSIONPANEL_TOKEN="$FUSIONPANEL_ADMIN_TOKEN" ./target/debug/fusionpanel-cli --json capabilities
```

The health command reports the service status and version. The capabilities command requires the token and lists the operations advertised by the running API. When finished, stop the API and clear the local configuration.

```sh
kill "$fusionpanel_api_pid"
unset fusionpanel_api_pid FUSIONPANEL_ADMIN_TOKEN FUSIONPANEL_LISTEN
```

## Development and testing

Run the core Rust workspace tests from the repository root.

```sh
cargo test --locked --workspace
```

For frontend development, use Node.js 20.19+ in the 20.x series or Node.js 22.12+ and npm.

```sh
cd frontend
npm ci
npm test -- --runInBand
npm run build
```

The root Rust workspace covers the foundation crates. Service modules and integration suites can have their own manifests and prerequisites. End-to-end suites use real binaries and services in disposable environments. A missing prerequisite is a blocked qualification run. See the [real-system test strategy](docs/testing/TEST_STRATEGY.md), [test plan](TEST_PLAN.md), and [performance qualification notes](docs/qualification/performance/README.md).

## Architecture and documentation

The architecture separates the control plane from a node agent that applies host changes. The core workspace contains domain types, the HTTP API and CLI, a versioned agent protocol, and SQLite-backed job persistence. The [architecture](ARCHITECTURE.md) describes the intended boundaries and data flows.

- [Project charter](PROJECT_CHARTER.md), [capability catalog](CAPABILITY_CATALOG.md), and [roadmap](ROADMAP.md)
- [Readiness snapshots](docs/execution/READINESS.md), [capability and evidence ledger](docs/execution/CAPABILITY_LEDGER.md), and [work packages](docs/execution/DELIVERY_PLAN.md)
- [Migration plan](ARCHITECTURE_AND_MIGRATION.md) and [open decisions](DECISIONS_AND_OPEN_QUESTIONS.md)
- [Feature preservation](docs/engineering/FEATURE_PRESERVATION.md) and [selectable workflow modes](docs/engineering/VARIANT_DESIGN.md)
- [Contributing](CONTRIBUTING.md) and [repository instructions](AGENTS.md)

## License

FusionPanel is licensed under the AGPL-3.0-only license. See [LICENSE](LICENSE).
