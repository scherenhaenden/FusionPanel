# FusionPanel Job Store

SQLite-backed durable job records with transactional claims and checked lifecycle transitions. Claiming decodes the row before committing, so a malformed row produces an error without persisting a Running state or worker assignment. Running jobs remain available for explicit reconciliation; no automatic retry is performed.

Error messages supplied to `transition` are limited to `MAX_ERROR_TEXT_BYTES` (4096 UTF-8 bytes). Oversized messages return `JobStoreError::ErrorTextTooLong` without truncation or job mutation. The bound applies to bytes, including multibyte text. Callers must redact secrets before passing error messages to the store.

Run `cargo test -p fusionpanel-jobs`. Integration tests exercise real SQLite persistence, multiple connections, genuinely simultaneous claims, malformed data and rollback, lifecycle rejection, and the error-text limit.
