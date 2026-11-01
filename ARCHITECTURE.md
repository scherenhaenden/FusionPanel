# Architecture

## Foundation decisions

Use a Rust 2021 workspace with separate domain, API, agent protocol, jobs and CLI packages. The first development batch has no cross-package dependencies so each package can be validated independently. Integration is a separate task after contracts and tests are reviewed.

The control plane is unprivileged. Host changes ultimately belong to an authenticated node agent using typed, validated requests. The initial protocol package executes no system commands and opens no listening socket. Real hosting operations and privileged execution are later milestones.

## Components

| Package | Responsibility |
|---|---|
| `fusionpanel-domain` | Validated resource identifiers, ownership and lifecycle types. |
| `fusionpanel-api` | HTTP health, authenticated capability discovery, configuration and structured errors. |
| `fusionpanel-agent-protocol` | Versioned requests/responses, input validation and safe operation allowlists. |
| `fusionpanel-jobs` | SQLite-backed job records, lifecycle transitions, atomic claims and restart persistence. |
| `fusionpanel-cli` | HTTP administration client, consistent human/JSON output and safe token handling. |

Use a shared identity model, authorization rules, resource inventory, job events and audit trail. Each Nginx configuration, database instance, DNS zone, container stack and firewall policy must have one authoritative manager. Never permit multiple modules to overwrite a shared service without reconciliation.

SQLite is selected for the initial job-store library only. The final control-state topology and multi-node consistency model remain open. Frontend, remote transport authentication and deployment packaging require later decisions. Remote authentication is not provided merely by defining a protocol envelope.

See [foundation contracts](docs/FOUNDATION_CONTRACTS.md) for the first implementation interfaces.
