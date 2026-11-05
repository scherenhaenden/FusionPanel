# Security Qualification

These tests qualify a bounded part of W04-45 (Jules session `10269760512157044324`, issue #55). They exercise the real local identity routes and secret-store library, plus privileged firewall cases only inside an isolated network namespace. They do not certify the full security capability set.

## Executable cases

- **Session revocation:** Uses the real `integration-identity` login and logout routes with a temporary SQLite database. It proves an authenticated session works before logout, is rejected after logout and service restart, and a newly issued session remains usable.
- **Secret storage and rotation:** Uses the real `modules/secrets` store with generated keys and synthetic secret material in a temporary directory. It checks ciphertext at rest, scope denial, file permissions, rotation to a new key, and rejection of the old key. The implementation uses ChaCha20-Poly1305; this is not an AES-GCM test.
- **Emergency SSH lockout:** Starts a disposable loopback `sshd` with a generated host key and applies drop policies only after `unshare --net` creates an isolated network namespace. A shell trap terminates the server and removes its temporary files; the namespace is destroyed when its runner exits.
- **Firewall rule lifecycle:** Adds and verifies a temporary drop rule using iptables or nftables inside a new network namespace. Cleanup is registered before mutation and deletes the rule or temporary nftables table.

The token and secret-store cases use only generated credentials and temporary files. The tests do not query ambient PostgreSQL configuration, invoke a host `fusionpanel-api`, or apply rules to the host firewall.

## Run and interpret

Run `cargo test --manifest-path tests/e2e/security/Cargo.toml` in a Linux test environment. The identity and secret-store tests need only local runtime support. SSH/firewall cases require `sshd`, `ssh-keygen`, `nc`, `ip`, iptables or nftables, and permission to create a network namespace. Missing prerequisites produce a nonzero `RESULT=BLOCKED`; that result is not a pass and keeps the privileged qualification lane unverified.

## Remaining qualification gaps

W04-45 also calls for secret injection into a real service, credential rotation observed by that service, WAF configuration plus benign/malicious traffic checks, and an independent recovery path for lockout scenarios. This repository does not expose a production secret-injection route or supply the required WAF/full-OS fixture in this package, so those paths remain blocked and are not represented by local-library tests.
