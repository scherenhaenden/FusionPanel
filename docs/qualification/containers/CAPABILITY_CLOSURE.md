# Container and Deployment Capability Closure Matrix

This document tracks the automated qualification and capability closure for Container Management (CTR) and Deployments (DEP) modules.

## Container Management (CTR)

| ID | Description | Status | Automated Test | Notes |
|---|---|---|---|---|
| CTR-01 | Compose File Parsing | Closed | `test_ctr_01_parse_compose` | Basic YAML extraction validated. |
| CTR-02 | Host Root Mount Bypasses | Closed | `test_ctr_02_prevent_host_root_mount` | Path traversal (`../`) and absolute root (`/`) blocked. |
| CTR-03 | Privileged Mode Override | Closed | `test_ctr_03_reject_privileged_mode` | Rejected unless explicit override flag is true. |
| CTR-04 | Host Port Allocation | Closed | `test_ctr_04_prevent_overlapping_host_ports` | Overlapping ports are correctly detected and rejected. |
| CTR-05 | Container Lifecycle | Closed | `test_ctr_05_container_lifecycle` | Validates stopped -> running -> stopped transitions. |
| CTR-06 | Health Checking | Closed | `test_ctr_06_health_checking` | Ensures healthy states can be read. |
| CTR-07 | Restart Policies & Stateful Volumes | Closed | `test_ctr_07_restart_policies_and_stateful_volumes` | Asserts standard restart policies and host-to-container volume mappings. |

## Deployments (DEP)

| ID | Description | Status | Automated Test | Notes |
|---|---|---|---|---|
| DEP-01 | Atomic Symlink Deploy | Closed | `test_dep_01_atomic_symlink_deploy` | Verifies pointers shift correctly to releases. |
| DEP-02 | Rollback State Machine | Closed | `test_dep_02_rollback_state_machine` | Failure during deployment triggers `rolling_back` -> `rolled_back`. |
| DEP-03 | Dependency Allocation | Closed | `test_dep_03_dependency_allocation` | Ensures required dependencies like DBs are allocated. |
| DEP-04 | Configuration Injection | Closed | `test_dep_04_configuration_injection` | Environment variables map properly. |
| DEP-05 | CPU Limit Enforcement | Closed | `test_dep_05_cpu_limit_enforcement` | Boundaries checked against maximum allowed CPU cores. |
| DEP-06 | Memory Limit Enforcement | Closed | `test_dep_06_memory_limit_enforcement` | Boundaries checked for min/max limits. |

_All capabilities documented here are verified automatically by the test suite in `tests/closure/containers/`._
