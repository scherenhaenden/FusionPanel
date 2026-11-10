#![forbid(unsafe_code)]

pub mod backup;
pub mod error;
pub mod hardlink_pruner;
pub mod models;
pub mod restore;
pub mod retention;
pub mod retention_tiers;

pub use hardlink_pruner::*;
pub use retention_tiers::*;

#[cfg(test)]
mod tests {
    use super::*;
    use models::*;

    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn test_manifest_serialization() {
        let manifest = Manifest {
            version: 1,
            backup_id: Uuid::new_v4().to_string(),
            timestamp: "2026-11-02T12:00:00Z".to_string(),
            backup_type: BackupType::Mixed,
            files: vec![FileInfo {
                path: "test.txt".to_string(),
                size: 1024,
                checksum_sha256: "somehash".to_string(),
            }],
            databases: vec![DbInfo {
                engine: "postgresql".to_string(),
                database_name: "testdb".to_string(),
                dump_file: "testdb.sql".to_string(),
            }],
            metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let decoded: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, decoded);
    }
}
