# Mail Module (`modules/mail`)

## Objective
Mail domain/mailbox/relay config, credentials and service ownership; actual local delivery path.

## Capability Ownership and Status

- **NET-01: DNS zone management**: Blocked / Owned by S13 (DNS management module). Follow-on: Integration with DNS module for MX/SPF/DKIM record publication.
- **NET-02: DNSSEC**: Blocked / Owned by S13. Follow-on: Bind DNSSEC keys with DKIM verification in DNS module.
- **NET-03: DNS templates**: Blocked / Owned by S13. Follow-on: Standard mail DNS template generation (MX, SPF, DKIM, DMARC, Autodiscover).
- **NET-04: Hosted mailboxes**: Partial. Implemented domain, mailbox, and alias models, credentials custody with `RedactedSecret`, quota invariants, and local Maildir delivery/storage. Missing: Real IMAP/POP3 service integration (Dovecot daemon reload/auth). Follow-on item: "Implement Dovecot service reload and live auth file update".
- **NET-05: Webmail**: Blocked. Missing behavior: Webmail client (e.g. Roundcube / Snappymail) integration. Follow-on item: "Implement webmail container provisioning and SSO configuration".
- **NET-06: Outgoing SMTP**: Partial. Implemented relay configuration models with secret redaction and Postfix config plan generation (`MailConfigPlan`). Missing: Live MTA process execution / Postfix reload. Follow-on item: "Implement Postfix service management adapter and spool monitoring".
- **NET-07: DKIM and SPF controls**: Partial. Implemented SPF string validation and DKIM selector validation and domain configuration models. Missing: Cryptographic keypair generation (OpenSSL / ed25519/rsa) and milter configuration (Rspamd/OpenDKIM). Follow-on item: "Implement DKIM key generation and OpenDKIM/Rspamd key table sync".
- **NET-08: Antispam**: Blocked. Missing behavior: Spam detection engine (e.g. Rspamd/SpamAssassin) integration and filtering rules. Follow-on item: "Define antispam rule templates and Rspamd proxy worker configuration".
- **NET-09: Mail logs**: Blocked. Missing behavior: Mail server log ingestion, parsing, and queue inspection. Follow-on item: "Implement Postfix/Dovecot log parsing and mail queue inspection API".
- **NET-10: SMTP relay**: Partial. Implemented relay configuration, upstream authentication models, and credential redaction. Missing: External relay delivery verification and fallbacks. Follow-on item: "Implement relay health check and active probe delivery".

## Supported and Unsupported Features

### Supported:
- Strict canonical ASCII DNS domain name validation (`MailDomainName`), rejecting Unicode/IDNA U-labels, IP address literals, consecutive dots, leading/trailing hyphens.
- Mail address validation (`MailAddress`) parsing `<local-part>@<domain>` with RFC 5322 local-part checks and directive injection prevention.
- Mail domain (`MailDomain`), mailbox (`Mailbox`), and alias (`MailAlias`) models with path traversal defenses.
- Secret custody (`RedactedSecret` and `MailboxCredentials`): prevents secret leakage in `Debug`, `Display`, and `Serialize` implementations.
- Distinct contracts for `MailConfigPlan`, `AppliedResult`, and `ObservedState` distinguishing configuration planning from actual host application.
- Real local Maildir delivery (`MaildirDelivery`): creates `tmp/`, `new/`, `cur/` structures, writes message to `tmp/` and atomically moves to `new/`, and retrieves stored messages.

### Unsupported:
- IDNA/Unicode U-labels are explicitly rejected (punycode ASCII A-labels e.g. `xn--...` are supported).
- Live host modification of Postfix or Dovecot configuration files on workstation or production environments is not performed.

## Package API

- `MailDomainName::parse(name)`: Validates canonical ASCII domain names.
- `MailAddress::parse(email)`: Validates email address format.
- `RedactedSecret::new(secret)`: Secure secret custody wrapper that masks contents from logs and serializations.
- `MailConfigPlan::generate(domains, mailboxes, aliases, relay)`: Generates validated Postfix virtual domain/mailbox/alias maps and Dovecot password file records.
- `MaildirDelivery::new(root_path)`: Provides local message delivery and retrieval using the standard Maildir structure.
- `MaildirDelivery::deliver_message(recipient, sender, content)`: Atomically delivers a message into recipient's `new/` Maildir folder.
- `MaildirDelivery::read_message(recipient, folder, filename)`: Reads message from recipient's Maildir folder.

## Verification Evidence

Recorded on Linux x86_64 host with `rustc 1.94.0` and `cargo 1.94.0`.

### Commands and Results

| Command | Result |
|---|---|
| `cargo fmt --manifest-path modules/mail/Cargo.toml -- --check` | PASS, exit 0 |
| `cargo clippy --manifest-path modules/mail/Cargo.toml --all-targets -- -D warnings` | PASS, exit 0 |
| `cargo test --manifest-path modules/mail/Cargo.toml` | PASS (8 unit tests passed, 1 e2e passed, 1 e2e ignored as BLOCKED, 3 integration passed with BLOCKED dependency gates reported) |

### Runtime Dependency Probes

| Probe | Observed Result |
|---|---|
| `postfix status` | NOT INSTALLED |
| `dovecot --version` | NOT INSTALLED |
| `docker info` | Docker daemon overlayfs mount restricted / invalid argument in sandbox |
| `openssl version` | OpenSSL 3.0.13 |

Accordingly, full live SMTP/IMAP network delivery integration and E2E lanes are marked **BLOCKED** due to missing disposable Postfix/Dovecot services and container daemon mount restrictions in the sandbox environment.

## Proposed Shared Changes

- The root workspace `Cargo.toml` should add `"modules/mail"` to its `members` list during the root workspace integration task.
