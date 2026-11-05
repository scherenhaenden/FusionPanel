#![forbid(unsafe_code)]
// Copyright 2024 FusionPanel Contributors
// SPDX-License-Identifier: AGPL-3.0-only

//! ADOPT-05: Backup and Certificate-Renewal Ownership Handoff
//!
//! Models ownership handoff invariants: inventory user/system schedules, timezone,
//! exclusions, destinations, retention, credentials, and last successful run.

#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerState {
    Active,
    Stopped,
}

#[derive(Debug, Clone)]
pub struct BackupConfig {
    pub schedule: String,
    pub timezone: String,
    pub exclusions: Vec<String>,
    pub destinations: Vec<String>,
    pub retention_policy: String,
    pub credentials_valid: bool,
    pub last_successful_run: Option<String>,
    pub successor_verified: bool,
}

#[derive(Debug, Clone)]
pub struct CertRenewalConfig {
    pub schedule: String,
    pub successor_verified: bool,
    pub independently_verified: bool,
}

#[derive(Debug, Clone)]
pub struct ControlPlane {
    pub backup_config: BackupConfig,
    pub cert_config: CertRenewalConfig,
    pub scheduler_state: SchedulerState,
    pub successor_scheduler_state: SchedulerState,
    pub independent_restore_confirmed: bool,
}

impl ControlPlane {
    /// Checks if the control plane can be decommissioned based on successor verification.
    pub fn can_decommission(&self) -> Result<(), &'static str> {
        if !self.backup_config.successor_verified {
            return Err("Backup retention has no verified successor owner.");
        }
        if !self.cert_config.successor_verified {
            return Err("Certificate renewal has no verified successor owner.");
        }
        Ok(())
    }

    /// Executes the handover process.
    pub fn execute_handover(&mut self) -> Result<(), &'static str> {
        self.can_decommission()?;

        self.scheduler_state = SchedulerState::Stopped;
        self.successor_scheduler_state = SchedulerState::Active;
        self.cert_config.independently_verified = true;
        self.independent_restore_confirmed = true;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> ControlPlane {
        ControlPlane {
            backup_config: BackupConfig {
                schedule: "0 0 * * *".into(),
                timezone: "UTC".into(),
                exclusions: vec!["/tmp".into(), "/var/cache".into()],
                destinations: vec!["s3://fusionpanel-backups".into()],
                retention_policy: "30d".into(),
                credentials_valid: true,
                last_successful_run: Some("2023-10-01T12:00:00Z".into()),
                successor_verified: false,
            },
            cert_config: CertRenewalConfig {
                schedule: "0 0 1 * *".into(),
                successor_verified: false,
                independently_verified: false,
            },
            scheduler_state: SchedulerState::Active,
            successor_scheduler_state: SchedulerState::Stopped,
            independent_restore_confirmed: false,
        }
    }

    #[test]
    fn test_invariants_inventory() {
        let cp = default_config();
        assert_eq!(cp.backup_config.schedule, "0 0 * * *");
        assert_eq!(cp.backup_config.timezone, "UTC");
        assert_eq!(cp.backup_config.exclusions.len(), 2);
        assert_eq!(cp.backup_config.destinations[0], "s3://fusionpanel-backups");
        assert_eq!(cp.backup_config.retention_policy, "30d");
        assert!(cp.backup_config.credentials_valid);
        assert_eq!(
            cp.backup_config.last_successful_run.unwrap(),
            "2023-10-01T12:00:00Z"
        );
    }

    #[test]
    fn test_decommission_blocked_no_backup_successor() {
        let cp = default_config();
        assert_eq!(
            cp.can_decommission(),
            Err("Backup retention has no verified successor owner.")
        );
    }

    #[test]
    fn test_decommission_blocked_no_cert_successor() {
        let mut cp = default_config();
        cp.backup_config.successor_verified = true;
        assert_eq!(
            cp.can_decommission(),
            Err("Certificate renewal has no verified successor owner.")
        );
    }

    #[test]
    fn test_handover_execution() {
        let mut cp = default_config();
        cp.backup_config.successor_verified = true;
        cp.cert_config.successor_verified = true;

        assert!(cp.can_decommission().is_ok());

        assert!(cp.execute_handover().is_ok());

        assert_eq!(cp.scheduler_state, SchedulerState::Stopped);
        assert_eq!(cp.successor_scheduler_state, SchedulerState::Active);
        assert!(cp.cert_config.independently_verified);
        assert!(cp.independent_restore_confirmed);
    }
}
