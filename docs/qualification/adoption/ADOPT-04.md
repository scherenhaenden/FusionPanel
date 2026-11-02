# ADOPT-04: Multi-Database Export, Restore, and Ownership

## Objective
The purpose of this qualification standard (ADOPT-04) is to ensure that when a site is exported, restored, or ownership is transferred, all associated databases are preserved correctly. A site may have zero, one, or multiple databases. The system must correctly identify, export, and restore all of them without relying on heuristics like name-based ownership inference or first-item lookups.

## Requirements

1. **Multi-Database Preservation:**
   - A single site can be associated with multiple databases.
   - When a site is exported or restored, all associated databases must be preserved.
   - The preservation must include independent ownership metadata, database users, grants (permissions), secret references, and application connection mappings.

2. **Enumeration and Association:**
   - The system must enumerate all databases associated with a site via explicit associations (e.g., connection mappings or site-level metadata).
   - The system must not rely on "first-item lookup" (e.g., only exporting the first database found).
   - The system must not use "name-based ownership inference" (e.g., assuming a database belongs to a site just because its name starts with the site's name).

3. **Failure Verification:**
   - If a site has multiple associated databases and any one of them fails to export or fails verification, the overall site export process must fail and report the error.
   - Omission of an associated database during the export process must trigger a failure verification.

## Test Scenarios

- **Scenario 1: Full Preservation of Multiple Databases**
  Verify that a site with multiple databases, each having distinct users, grants, and connection mappings, can be successfully exported and restored with all properties intact.
- **Scenario 2: Strict Enumeration Verification**
  Verify that the export process explicitly uses relational associations and does not rely on name matching or single-item limitations.
- **Scenario 3: Omission Failure**
  Verify that if an associated database is missing from the export or fails to export, the verification step detects the failure and marks the site export as failed.
