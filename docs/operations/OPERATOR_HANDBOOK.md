# FusionPanel Operator Handbook

## Introduction

This handbook provides standard operating procedures for operating and maintaining FusionPanel instances, including backup key management, maintenance mode cutovers, recovery operations, and rollbacks.

## Maintenance Mode and Cutovers

To minimize downtime and ensure consistency during migrations or major updates, follow this cutover sequence:

1. **Verify Prerequisites**:
   - Backup keys are securely exported and available.
   - The target host (if applicable) is cleanly provisioned.
2. **Initiate Maintenance Mode**:
   ```bash
   fusionpanel-cli maintenance enable --reason "Scheduled Cutover"
   ```
3. **Execute Cutover Job**:
   - All asynchronous jobs are gracefully drained or paused.
   - Control plane API blocks mutating requests.
4. **Validation**:
   - Check `GET /v1/health` and verify maintenance status.

## Backup Keys

FusionPanel encrypts backups using AES-256-GCM.

- **Key Generation**: Backup keys are generated automatically upon configuring a backup destination.
- **Custody**:
  - Ensure keys are exported and stored in a secure offline location.
  - The secret rotation mechanism manages active key aliases.

## Recovery Runbook

In the event of a catastrophic failure, follow this sequence:

1. **Provision Disposable Machine**:
   - Provision a fresh host with matching OS and network topology.
2. **Deploy Control Plane**:
   - Execute the installer script.
   - Boot `fusionpanel-api` using a temporary root token.
3. **Restore Backup**:
   - Provide the offline backup keys to the system.
   - Execute the restore from the configured destination.
   ```bash
   fusionpanel-cli recovery restore --destination s3-main --key-file /path/to/keys
   ```
4. **Independent Verification**:
   - Manually verify DNS responses, TLS endpoints, and database connection grants on the restored nodes.
   - Do not redirect production traffic until verification completes.

## In-Place Updates and Rollbacks

If a migration or self-update fails:

1. **Identify the Failure Domain**: Determine if the failure occurred at the node agent level or control plane.
2. **Crash Recovery Marker Check**:
   - Check for `.<executable-name>.recovery-marker` and `<executable-name>.backup` beside the executable. The older shared `.recovery-marker` format is ambiguous unless exactly one matching backup exists.
   - Stop the updater process before recovery. The release package exposes `Updater::recover()`, which restores the previous binary from its backup or removes a candidate when no previous target existed. This library method is not wired to a startup command or CLI, so operators must not restart the service on the strength of the marker check alone.
   - Preserve markers and backups if the marker is invalid, the previous binary is missing, or more than one legacy backup exists; resolve the artifacts manually before restarting.
3. **Revert the Release Directory**:
   - For control plane, stop `fusionpanel-api.service`, switch the active release symlink to the previous build, and restart.
4. **Job Reconciliation**:
   - Restarted jobs may need manual intervention if stuck in an incomplete state. Check SQLite backend `job_records`.
