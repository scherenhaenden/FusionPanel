#![forbid(unsafe_code)]

#[derive(Debug, PartialEq)]
pub enum RecoveryState {
    Ready,
    MaintenanceMode,
    Recovering,
    Restored,
    Failed,
}

pub struct RecoveryPlan {
    pub keys_available: bool,
    pub target_host_clean: bool,
}

impl RecoveryPlan {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.keys_available {
            return Err("Missing backup keys");
        }
        if !self.target_host_clean {
            return Err("Target host must be cleanly provisioned");
        }
        Ok(())
    }
}

pub fn initiate_cutover(plan: &RecoveryPlan) -> Result<RecoveryState, &'static str> {
    plan.validate()?;
    Ok(RecoveryState::Ready)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_recovery_plan() {
        let plan = RecoveryPlan {
            keys_available: true,
            target_host_clean: true,
        };
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn test_missing_keys_plan() {
        let plan = RecoveryPlan {
            keys_available: false,
            target_host_clean: true,
        };
        assert_eq!(plan.validate(), Err("Missing backup keys"));
    }

    #[test]
    fn test_dirty_host_plan() {
        let plan = RecoveryPlan {
            keys_available: true,
            target_host_clean: false,
        };
        assert_eq!(plan.validate(), Err("Target host must be cleanly provisioned"));
    }

    #[test]
    fn test_initiate_cutover_success() {
        let plan = RecoveryPlan {
            keys_available: true,
            target_host_clean: true,
        };
        assert_eq!(initiate_cutover(&plan), Ok(RecoveryState::Ready));
    }
}
