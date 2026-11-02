# Performance and Failure Budget

This folder contains the integration tests for testing the performance, error rate budgets, and queue concurrency boundaries of the system.

## Tests included

We provide a lightweight runner within `tests/qualification/performance/` that validates:
- Target throughput capability under bounded concurrency.
- Tolerance and partial error processing (the failure budget).
- Recovery constraints and time.

These test scripts do not rely on local workstation modification and adhere to strictly disposable fixtures, using `tokio` for scalable concurrency tasks.

## To run

Execute:

```bash
cargo test --manifest-path tests/qualification/performance/Cargo.toml
```
