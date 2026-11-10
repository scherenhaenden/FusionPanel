#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeLifecycleState {
    Active,
    Quarantined,
    Draining,
    Evicted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvictionEvent {
    WorkloadDrainRequested,
    CertificateRevocationRequired,
    NodePurged,
}

#[derive(Debug, thiserror::Error)]
pub enum EvictionError {
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: NodeLifecycleState,
        to: NodeLifecycleState,
    },
    #[error("Cannot transition to Evicted while {workloads} workloads remain")]
    WorkloadsRemaining { workloads: usize },
}

pub struct NodeEvictionCoordinator {
    state: NodeLifecycleState,
    workloads: usize,
}

impl NodeEvictionCoordinator {
    pub fn new(workloads: usize) -> Self {
        Self {
            state: NodeLifecycleState::Active,
            workloads,
        }
    }

    pub fn state(&self) -> NodeLifecycleState {
        self.state
    }

    pub fn workloads(&self) -> usize {
        self.workloads
    }

    pub fn quarantine(&mut self) -> Result<(), EvictionError> {
        if self.state != NodeLifecycleState::Active {
            return Err(EvictionError::InvalidTransition {
                from: self.state,
                to: NodeLifecycleState::Quarantined,
            });
        }
        self.state = NodeLifecycleState::Quarantined;
        Ok(())
    }

    pub fn start_drain(&mut self) -> Result<EvictionEvent, EvictionError> {
        if self.state != NodeLifecycleState::Quarantined && self.state != NodeLifecycleState::Active
        {
            return Err(EvictionError::InvalidTransition {
                from: self.state,
                to: NodeLifecycleState::Draining,
            });
        }
        self.state = NodeLifecycleState::Draining;
        Ok(EvictionEvent::WorkloadDrainRequested)
    }

    pub fn complete_workload(&mut self) {
        self.workloads = self.workloads.saturating_sub(1);
    }

    pub fn evict(&mut self) -> Result<Vec<EvictionEvent>, EvictionError> {
        if self.state != NodeLifecycleState::Draining {
            return Err(EvictionError::InvalidTransition {
                from: self.state,
                to: NodeLifecycleState::Evicted,
            });
        }

        if self.workloads > 0 {
            return Err(EvictionError::WorkloadsRemaining {
                workloads: self.workloads,
            });
        }

        self.state = NodeLifecycleState::Evicted;
        Ok(vec![
            EvictionEvent::CertificateRevocationRequired,
            EvictionEvent::NodePurged,
        ])
    }
}
