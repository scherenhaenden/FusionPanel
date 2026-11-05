# ADOPT-05: Backup and Certificate-Renewal Ownership Handoff

## Objective
Ensure a secure and reliable transition of backup scheduling and certificate renewal responsibilities when a source control plane is decommissioned and handed over to a successor owner.

## Invariants Modeled

During the handover process, the following state and configurations are strictly inventoried and verified:
* **Schedules**: User and system-defined schedules (e.g., cron expressions) for backups and certificate renewals.
* **Timezone**: The timezone for the schedules to ensure consistency across systems.
* **Exclusions**: Directories, files, or data omitted from backups.
* **Destinations**: Targets for the backups (e.g., S3, local storage).
* **Retention**: Retention policies determining the lifespan of backups.
* **Credentials**: Validation of credentials for backup destinations.
* **Last Successful Run**: Timestamp of the last successful backup or certificate renewal.

## Pre-Decommission Requirements

Decommissioning of the source control plane is blocked if:
* The backup retention system does not have a verified successor owner.
* The certificate renewal system does not have a verified successor owner.

## Handover Execution

The actual handover process executes the following steps:
1. **Old Scheduler Stopped**: The backup and certificate renewal scheduler on the source control plane is disabled.
2. **New Scheduler Active**: The scheduler on the successor control plane takes over.
3. **Certificate Renewal Verified**: The certificate renewal process is independently verified by the successor.
4. **Independent Restore Confirmed**: A backup restore is independently confirmed by the successor to ensure data integrity.
