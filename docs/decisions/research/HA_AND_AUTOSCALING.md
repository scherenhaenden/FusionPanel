# HA and Autoscaling Feasibility

## Objective

This document evaluates the feasibility of Highly Available (HA) deployments and horizontal autoscaling for FusionPanel. It serves as a research-only foundation to model the failure and consistency requirements without promising unsupported behavior.

## Capabilities

The following capabilities are tracked as part of this research slice:

*   **CTR-13: Horizontal autoscaling** - `BLOCKED`
    *   Missing behavior: No shared session state or load balancer configuration exists to route traffic seamlessly across multiple control plane nodes.
*   **NODE-12: Highly available control plane** - `BLOCKED`
    *   Missing behavior: State is currently tied to a single node instance (e.g., local SQLite databases, local file system paths). There is no distributed consensus or replication mechanism established for metadata or secrets.

## Failure and Consistency Model

### Control Plane State

Currently, the FusionPanel control plane maintains state locally. An HA setup would require:
1.  **Database Replication:** A highly available datastore (e.g., clustered PostgreSQL or MariaDB) instead of isolated local SQLite instances.
2.  **Shared Storage:** A distributed file system or object storage for certificates, logs, and configuration files to ensure all nodes have a consistent view.
3.  **Leader Election / Consensus:** Mechanisms to prevent split-brain scenarios when orchestrating node agents or renewing certificates.

### Node Agents

Node agents apply host changes. In an HA setup:
1.  Control plane nodes must be able to securely dispatch commands to any node agent.
2.  Idempotency is required. If a control plane node fails mid-dispatch, a surviving node must be able to retry the operation safely.

## Conclusion

At present, HA and horizontal autoscaling are **NOT SUPPORTED**. The architecture lacks the distributed state management and consensus mechanisms required. These features remain blocked pending further architectural investments in shared state and leader election.
