# Migration and Adoption

CloudPanel import is a product requirement and the initial initial deployment adoption path. Define versioned source adapters, a canonical import representation, a dry-run planner and target-node jobs that use normal provisioning/restore services.

Inventory source OS/panel version, domains, runtime types, files, databases/users/grants, aliases, TLS, cron, ownership, SSH access, DNS/mail dependencies, backup destinations and resource capacity. Store sensitive data outside this repository.

Rehearse discovery, explicit create/skip/conflict/unsupported results, export/import, application verification and final sync in disposable environments. Validate credential rewrites, scheduled work, TLS and permissions. In-place conversion requires its own service/port ownership rehearsal.

Per-resource compensation does not restore a whole server. Prove independent machine and data restore, including post-cutover write reconciliation. Set downtime/data-loss objectives and rollback triggers before preparing a production cutover. Production execution requires explicit authorization after reviewing a concrete runbook and evidence.
