# W04-49 Performance Qualification

This package provides bounded evidence for issue #62. It does not qualify the full W04-49 acceptance criteria.

## Executable cases

- `test_actual_api_load` sends 1,000 requests at concurrency 10 to the real FusionPanel API health endpoint at `127.0.0.1:8080/v1/health`. It records total duration, request errors, and p50/p95/p99 request latency. An unavailable local API produces a nonzero `BLOCKED` result; a failed health response or request fails the test.
- `test_worker_failure_is_persisted_by_real_runtime` enqueues a valid operation in the real SQLite job store, executes it through the real `integration-jobs::Worker` and `SubprocessAgentClient`, and uses a disposable local agent process to return one deterministic protocol error. It measures the time to persist the failed state, closes and reopens SQLite, and verifies the state, worker ID, and error text.
- Unit tests check percentile calculation over completed request latencies, including the empty sample case.

All worker fixtures use a temporary directory and SQLite database. No Docker daemon, public service, production credential, or host service is used.

## Run and interpret

Run `cargo test --manifest-path tests/qualification/performance/Cargo.toml` on Linux. The worker and percentile cases use only local disposable fixtures. The real API load case requires a disposable FusionPanel API process listening on `127.0.0.1:8080`; without it the test fails with `BLOCKED`, not a passing skip.

## Remaining qualification gaps

- `integration-jobs::Worker` does not automatically recover jobs left in `Running` after a worker restart. Its current contract exposes those jobs for operator reconciliation. Restart recovery, measured RTO/RPO, and guarantees for acknowledged writes or duplicate effects therefore remain blocked.
- The API exposes no controlled fault-injection route. This package does not manufacture random failures after an unrelated health probe; real API outage/failure-budget measurements remain blocked until a disposable fault-injection fixture exists.
- Resource-use measurements, declared hardware/mode baselines, and service-level performance budgets are not supplied by this bounded package. The local health workload does not establish production capacity or SLO compliance.

Jules task: W04-49, session `2726729400535846353`. Issue: #62.
