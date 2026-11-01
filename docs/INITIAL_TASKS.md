# Initial Programming Tasks

Five parallel tasks start from the same main commit and own disjoint paths. Each must submit a separate PR against main without merging.

## 01-domain: Domain types and lifecycle validation

Exclusive paths: `crates/domain/`.

Implement the domain contract, public typed errors, serde roundtrips, DNS validation and checked state transitions. Add focused positive/negative tests and package-level README documentation. Do not implement persistence or HTTP.

## 02-api: Control-plane API foundation

Exclusive paths: `crates/api/`.

Implement the HTTP contract using axum/Tokio or similarly maintained Rust libraries. Include startup config validation, bearer authentication, JSON errors, graceful shutdown, and route/auth tests. Token comparison must avoid a naive timing-leaking byte comparison. Do not add jobs, host operations or frontend.

## 03-agent-protocol: Versioned node protocol and validation

Exclusive paths: `crates/agent-protocol/`.

Implement strict typed request/response envelopes, validation and serialization for the agent protocol contract. Reject unknown fields where appropriate. Add roundtrip, malformed/version/operation/path/domain tests. No system command execution, privileged operations, network daemon or invented authentication.

## 04-jobs: Durable SQLite job lifecycle

Exclusive paths: `crates/jobs/`.

Implement the persistent job-store contract, typed errors and SQLite schema initialization. Test allowed and rejected transitions, reopen persistence, malformed data handling, atomic claims from separate connections and ordering. No scheduler loop, network service or sibling package dependencies.

## 05-cli: Administrative CLI foundation

Exclusive paths: `crates/cli/`.

Implement health/capabilities commands matching the HTTP contract using clap and an HTTP client. Test argument handling, human/JSON output, response parsing, authorization failures, request timeout/error behavior and token redaction using local mock HTTP fixtures. No actual production requests or mutation commands.

## Follow-up integration

After reviewing all five PRs, integrate adapters and public types, generate/review Cargo.lock, run workspace tests/clippy/format checks and establish CI. Review interfaces and dependencies before merging; connect jobs and the node protocol to the API only in a separate implementation task.
