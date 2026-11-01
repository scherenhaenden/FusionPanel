# Laboratory and Delivery Prerequisites

| Resource | Required by | Qualification consequence if absent |
|---|---|---|
| Jules access/concurrency and available daily quota | Every remote session | Keep unsent tasks queued with actual provider error; do not infer an account limit. |
| Owner review and integration merge authorization | S06 and dependent work | PRs remain separate; runtime integration cannot be claimed on master. |
| CI runner with Rust/JS/Python and container support | S07/S41 and adapter suites | Unit tests can run; actual dependency integration is blocked. |
| Disposable Debian full-OS VM and snapshot/restore capacity | S24/S37/S38/S51/S53 | Cannot qualify systemd, firewall, quotas, SSH, reboot or machine rollback from containers alone. |
| Two isolated node VMs | S34/S49 | Multi-node partition/move/revocation acceptance is blocked. |
| Controlled domains and scoped DNS/provider credentials | External TLS/DNS/mail | Local fixtures only establish local behavior, not public delivery or renewal. |
| Hardware for each advertised GPU vendor/model | GPU passthrough | Unsupported/untested hardware must remain explicit; do not generalize across vendors. |
| Protected artifact and secret storage | Backups, logs and credential tests | Use generated isolated fixture secrets; never place production material in the public repository. |
| Version-matched sanitized migration export | S18/S35/S53 | Synthetic schema tests do not establish actual source-version compatibility. |

Use existing authorized local/CI fixtures where possible. Do not incur new hosting/provider costs, change production or expose credentials merely to make a test appear runnable. Record the precise blocked test and required input.
