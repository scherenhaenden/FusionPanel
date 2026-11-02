# ADOPT-06: OS Username Preservation and Identity Mapping

## Objective
To outline the qualification tests and specifications for ensuring proper OS username preservation and collision-safe identity mappings within the platform.

## Specifications

### Username Validation
The platform must strictly validate all OS usernames to prevent security risks (like command injection via option-like flags) and avoid conflicts with essential system services.

- **Allowed Characters:** Alphanumeric characters, hyphens (`-`), and underscores (`_`).
- **Forbidden Characters:** Any non-alphanumeric character other than `-` and `_` (e.g., `@`, `/`, `\`, `*`).
- **Length Constraint:** Usernames must not exceed 32 characters. Empty usernames are strictly rejected.
- **Prefix Constraints:** Usernames must not begin with a hyphen (`-`) to avoid being parsed as option flags by command-line utilities.
- **Protected Accounts:** System and reserved accounts are forbidden to prevent privilege escalation or disruption. This includes: `root`, `daemon`, `bin`, `sys`, `sync`, `games`, `man`, `lp`, `mail`, `news`, `uucp`, `proxy`, `www-data`, `backup`, `list`, `irc`, `gnats`, `nobody`, `systemd-network`, `systemd-resolve`, `systemd-timesync`, `messagebus`, `syslog`, `mysql`, `postgres`.

### Normalization and Collision Handling
Hyphens (`-`) in usernames are explicitly **never silently normalized** to underscores (`_`) upon creation or internal representation. The exact spelling is preserved.

However, to avoid ambiguity and security risks where external systems or utilities might conflate `foo-bar` and `foo_bar`, the platform enforces **collision detection**:
- When adding a new user, its normalized form (where hyphens are temporarily treated as underscores for comparison) is checked against the normalized forms of existing users.
- If a collision occurs (e.g., trying to create `foo_bar` when `foo-bar` already exists), the creation is rejected.

### Separation of Identity and Ownership Boundaries
The system maintains strict separation between account identifiers and numeric mapping.
- **Account Names:** Human-readable string identifiers used for login and reference.
- **UIDs/GIDs:** Distinct numeric values assigned by the OS, serving as the definitive identity for processes and file ownership.
- **Resource Boundaries:** Ownership of files and resources is tightly bound to these unique numeric UIDs/GIDs. Tests assert that resources owned by distinct users are mathematically verifiable as separate, preventing cross-tenant access.
