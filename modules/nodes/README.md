# Nodes Module

This standalone Rust package contains bounded, in-memory node lifecycle and placement models. It does not register workers, persist control state, authenticate requests, or move live site data.

## Capability status

Status here is specific to this package. The repository capability ledger remains authoritative for product acceptance.

| Capability | Package status | Implemented behavior | Missing behavior / follow-up |
|---|---|---|---|
| NODE-01: Rust core | Partial | Checked in-memory invitation, enrollment, revocation, placement, and movement-plan types | Persisted/recoverable state and integrated runtime are not implemented. |
| NODE-02: Lightweight frontend | Blocked | None | Node UI is not implemented. |
| NODE-03: Agent separation | Blocked | None | Node agent is not integrated. |
| NODE-04: Restricted privileged surface | Blocked | None | Privileged-operation boundary is not implemented. |
| NODE-05: Authenticated RPC | Blocked | None | Authenticated RPC is not integrated; see [S34 multi-node runtime, issue #37](https://github.com/scherenhaenden/FusionPanel/issues/37). |
| NODE-06: Master and worker node management | Partial | Ephemeral invitation, enrollment, identity, and revocation state | Registration, authorization, persistence, and real master/worker operation remain unimplemented; see issue #37. |
| NODE-07: Move and copy sites between nodes | Partial | Validated plan generation for moving one site between distinct node IDs | No copy workflow or live data/traffic movement is implemented; runtime integration remains in issue #37. |
| NODE-08: Load-aware placement | Partial | Selects the lowest-scoring candidate from supplied normalized metrics while respecting CPU, memory, and post-placement site limits | Metric collection, reservations, scheduling, and placement enforcement are not implemented. |
| NODE-09: LIFO provisioning compensation | Blocked | None | Provisioning and compensation execution are not implemented. |
| NODE-10: Control-state persistence | Blocked | None | Durable storage and recovery are not implemented; see issue #37. |
| NODE-11: Durable jobs | Blocked | None | Job creation, execution, retry, and recovery are not integrated; see issue #37. |
| NODE-13: OpenAPI contract | Blocked | None | No API contract is implemented. |
| NODE-14: Operational and recovery CLI | Blocked | None | No CLI is implemented. |
| NODE-15: AGPL licensing and source compliance | Partial | This package declares `AGPL-3.0-only` | Repository-wide source, dependency, and notice compliance has not been audited. |

## Package API

- `Node::new_invited` rejects zero, negative, and unrepresentable invitation lifetimes.
- `Node::enroll` requires a non-empty key identity and checks invitation expiry against one captured timestamp.
- `Node::revoke` preserves the enrolled node identity so a revocation check can still identify the node.
- Lifecycle fields are private and can only be changed through checked methods. `Node` is serializable for output; deserialization is intentionally unavailable until persisted-state recovery has a validated contract.
- `plan_placement` accepts finite normalized CPU/memory loads in `0.0..=1.0`; a candidate must remain below its post-placement site limit.
- `plan_movement` rejects identical source and target nodes and returns an ordered plan. It does not execute the steps. Target verification precedes traffic cutover in the plan.

The `key_identity` value is an identity or public-key fingerprint, never private key material. Do not pass credentials or secret keys to this API or include them in test-driver output.

## Verification

Run the local unit suite:

```sh
cargo test --offline --manifest-path modules/nodes/Cargo.toml --lib
```

The actual-service integration and E2E lanes are executable dependency gates, not ignored placeholders. Each requires an executable fixture driver that uses disposable real services, checks the named outcome, exits successfully, and prints exactly the expected result marker on stdout. Drivers must keep secrets out of arguments and output; evidence collection and fixture cleanup remain the driver's responsibility.

Integration case: two real local node identities communicate through the signed transport fixture.

```sh
FUSIONPANEL_NODES_INTEGRATION_DRIVER=/absolute/path/to/driver \
  cargo test --offline --manifest-path modules/nodes/Cargo.toml --test integration -- --nocapture
```

The driver receives `FUSIONPANEL_NODES_TEST_SCENARIO=two-identities-signed-transport` and must print `RESULT=PASS;scenario=two-identities-signed-transport` only after asserting the actual outcome.

E2E case: enroll two disposable nodes, revoke one, then verify that its signed requests are rejected by the real API/agent path.

```sh
FUSIONPANEL_NODES_E2E_DRIVER=/absolute/path/to/driver \
  cargo test --offline --manifest-path modules/nodes/Cargo.toml --test e2e -- --nocapture
```

The driver receives `FUSIONPANEL_NODES_TEST_SCENARIO=enroll-two-revoke-one-reject-requests` and must print `RESULT=PASS;scenario=enroll-two-revoke-one-reject-requests` only after asserting the actual user-visible and service-side outcomes.

Without those drivers, the respective test exits non-zero with `RESULT=BLOCKED` and names the missing driver and runtime requirement. The lanes currently remain BLOCKED: this package has no integrated node agent, authenticated API, signed transport fixture, or disposable two-node runtime. A blocked result is not a pass. A passing driver run still needs recorded commit, service/tool versions, fixture identity, sanitized evidence, exit status, and cleanup results before the capability ledger can advance.

## Unsupported behavior

- State is in-memory only and has no database recovery or durable job support.
- No node agent, authenticated transport, master/worker API, UI, or CLI is implemented here.
- Placement and movement calls return decisions/plans only; they do not reserve capacity, copy data, switch traffic, or clean up resources.
- Copying sites between nodes is not implemented.

## Proposed shared changes

None. Runtime integration belongs to the separately scoped S34 work package and issue #37.
