1. **Understand the problem**:
    * Implement an ACME HTTP-01 Challenge Responder & TLS Certificate Renewal Manager.
    * It belongs in `modules/hosting/src/acme.rs`.
    * Generates `.well-known/acme-challenge/<token>` responders, validates challenge tokens, and monitors x509 cert expiry with renewal triggers.
    * Strict Safe Rust: `#![forbid(unsafe_code)]`.
    * Unit tests in `modules/hosting/tests/unit_acme.rs`.
    * Non-interactive execution, run `cargo test --manifest-path modules/hosting/Cargo.toml --test unit_acme`.
2. **Review existing implementation**: I already sketched `modules/hosting/src/acme.rs` and `modules/hosting/tests/unit_acme.rs`.
    * In `acme.rs`, the `AcmeResponder` struct handles generating, validating, and removing challenge files.
    * In `acme.rs`, the `CertMonitor` struct uses `openssl` (via `std::process::Command` to avoid unsafe native bindings, thus complying with `#![forbid(unsafe_code)]` as per guidelines for complex operations like X.509 checking without pure Rust crates available or if we want to rely on the system tool) to check certificate expiry.
3. **Refine `CertMonitor`**:
    * The instruction says "monitors x509 cert expiry with renewal triggers." I should make sure that it meets this.
    * The requirements and memories mention "When restricted from adding external dependencies via Cargo.toml for required cryptographic operations, either embed a pure safe-Rust implementation ... or invoke standard system tools like openssl via std::process::Command ... to comply with #![forbid(unsafe_code)]."
    * The tests pass. The implementations look solid.
4. **Integration**: `modules/hosting/src/lib.rs` is already updated to `pub mod acme;`.
5. **Execution of tests**: `cargo test --manifest-path modules/hosting/Cargo.toml --test unit_acme` passes.
6. **Pre-commit checks**:
    * Follow `pre_commit_instructions`.
7. **Submit**: Create PR against master.

I should also make sure the generated challenge is accessible. Wait, it writes to a file, which the web server (Nginx/etc) will serve.
