#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationPhase {
    PreCopyFiles,
    SetupTargetVhost,
    ExportDatabase,
    SyncDatabase,
    DeltaSyncFiles,
    DnsCutover,
    Verification,
    FinalizeCleanup,
}

#[derive(Debug, Clone)]
pub struct SiteMigrationPlanner {
    pub domain: String,
    pub source_size_bytes: u64,
    pub target_disk_space_bytes: u64,
    pub user_mapping_valid: bool,
}

#[derive(Debug)]
pub enum MigrationError {
    InvalidDomain,
    InsufficientSpace,
    InvalidUserMapping,
}

impl SiteMigrationPlanner {
    pub fn new(
        domain: String,
        source_size_bytes: u64,
        target_disk_space_bytes: u64,
        user_mapping_valid: bool,
    ) -> Self {
        Self {
            domain,
            source_size_bytes,
            target_disk_space_bytes,
            user_mapping_valid,
        }
    }

    pub fn plan(&self) -> Result<Vec<MigrationPhase>, MigrationError> {
        self.check_prerequisites()?;

        Ok(vec![
            MigrationPhase::PreCopyFiles,
            MigrationPhase::SetupTargetVhost,
            MigrationPhase::ExportDatabase,
            MigrationPhase::SyncDatabase,
            MigrationPhase::DeltaSyncFiles,
            MigrationPhase::DnsCutover,
            MigrationPhase::Verification,
            MigrationPhase::FinalizeCleanup,
        ])
    }

    pub fn generate_rollback_sequence(
        completed_phases: &[MigrationPhase],
    ) -> Vec<MigrationPhase> {
        completed_phases.iter().cloned().rev().collect()
    }

    fn check_prerequisites(&self) -> Result<(), MigrationError> {
        if !self.is_valid_domain(&self.domain) {
            return Err(MigrationError::InvalidDomain);
        }

        let required_space = (self.source_size_bytes as f64 * 1.2) as u64;
        if self.target_disk_space_bytes < required_space {
            return Err(MigrationError::InsufficientSpace);
        }

        if !self.user_mapping_valid {
            return Err(MigrationError::InvalidUserMapping);
        }

        Ok(())
    }

    fn is_valid_domain(&self, domain: &str) -> bool {
        if domain.is_empty() || domain.len() > 253 {
            return false;
        }
        let parts: Vec<&str> = domain.split('.').collect();
        if parts.len() < 2 {
            return false;
        }
        for part in parts {
            if part.is_empty() || part.len() > 63 {
                return false;
            }
            if part.starts_with('-') || part.ends_with('-') {
                return false;
            }
            if !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return false;
            }
        }
        true
    }
}
