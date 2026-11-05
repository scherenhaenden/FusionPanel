# W04 Release Decision

## Overview
This document outlines the final integrated release outcomes for W04. The release spans numerous capability domains including adoption, decommissioning, application catalogues, backups, operations, security, and machine recovery. 

## Capability Qualification Status

### Unit & Integration Evidence
- **Manifests & Parsing**: Release manifest and component schemas are rigorously tested for validation and structure constraints.
- **Update Mechanism**: Tested recovery markers and atomic component swapping via `.recovery-marker`.
- **Packaging/Systemd**: Verified clean build of the workspace and correct execution. 
- **Dependencies**: Bounded and isolated tests correctly utilize temporary target environments, preventing host mutation.

### E2E / Realistic Execution State
- **CLI Missing Dependency**: Tests evaluating end-to-end capabilities like `fusionpanel-cli` panic correctly with `BLOCKED: Missing dependency 'fusionpanel-cli'`, keeping unsupported/unprovisioned dependencies clearly visible.
- **Provider / Full OS Capabilities**: Due to missing explicit full disposable lab environments (like complete multi-node VMs and T4 real deployment endpoints), various E2E tests across security, network, container domains and recovery cannot execute successfully. These capabilities remain BLOCKED.

## Blocker List
1. **Missing Lab (T3/T4)**: Absence of T3 multi-node VMs for testing full host configurations, reboots, kernel quotas, firewall policies, and physical/logical restoration of an entire node state.
2. **Missing CLI Binaries**: E2E journeys are unable to execute the main operator interfaces because the `fusionpanel-cli` binary is not provisioned or configured correctly in the CI runner environment.
3. **External Dependencies**: Authoritative DNS and functional Mailbox tests (W04-42) cannot execute fully due to missing verified provider test instances. E2E asserts fail closed.
4. **Research Features**: Highly-available (HA) configurations and auto-scaling logic are unverified because appropriate real-world cluster topologies are missing. They remain unsupported.

## Decision: NO-RELEASE
Based on the failure/blocking of complete real-world journeys—specifically, the lack of full OS test labs and functional CLI integration—the evidence gate has failed. We explicitly produce a **NO-RELEASE** decision. We cannot claim complete implementation of capability families, and no partial release should be shipped as final until full T3/T4 test paths are unblocked and cleared.
