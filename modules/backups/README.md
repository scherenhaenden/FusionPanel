# Backups module

## Capabilities
- BAK-01: Scheduled backups (Partial)
- BAK-02: File backups (Implemented)
- BAK-03: Database backups (Partial)
- BAK-07: Backup encryption (Partial)
- BAK-08: Retention policies (Implemented)
- BAK-09: Selective restore (Implemented)
- BAK-10: Backup verification (Implemented)
- BAK-11: Restore drills (Partial)
- BAK-12: Restic snapshots (Partial)
- BAK-13: Pre-update backups (Partial)
- BAK-14: Restore to another domain (Partial)
- BAK-15: Restore confidence dashboard (Partial)

## Implementation Details
This module provides a backup and recovery core. It includes generating backup manifests, recording file checksums, implementing retention policies, and independent restore functionality for file backups with basic database manifest inclusion.

## Proposed shared changes
None at this time.
