use std::sync::{Arc, Mutex};
use thiserror::Error;
use uuid::Uuid;

use fusionpanel_agent_protocol::{Operation, Request};
use fusionpanel_jobs::{Job, JobId, JobState, JobStore, JobStoreError};

#[derive(Debug, Error)]
pub enum OperationServiceError {
    #[error("Job store error: {0}")]
    Store(#[from] JobStoreError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

/// Service that exposes operation enqueuing and reconciliation.
#[derive(Clone)]
pub struct OperationService {
    store: Arc<Mutex<JobStore>>,
}

impl OperationService {
    pub fn new(store: Arc<Mutex<JobStore>>) -> Self {
        Self { store }
    }

    /// Enqueue a new operation.
    pub fn enqueue_operation(&self, operation: Operation) -> Result<JobId, OperationServiceError> {
        let req = Request {
            version: 1,
            request_id: Uuid::new_v4(),
            operation,
        };
        let payload = serde_json::to_value(&req)?;

        let store = self.store.lock().unwrap();
        let id = store.create("agent_operation", &payload)?;
        Ok(id)
    }

    /// Cancel an operation if it is queued or running.
    pub fn cancel_operation(&self, id: &JobId) -> Result<(), OperationServiceError> {
        let mut store = self.store.lock().unwrap();
        store.transition(
            id,
            JobState::Cancelled,
            Some("Cancelled by user".to_string()),
        )?;
        Ok(())
    }

    /// Reconcile interrupted jobs (jobs in Running state).
    /// These are not blindly replayed; they must be manually reconciled by the operator.
    pub fn get_running_operations(&self) -> Result<Vec<Job>, OperationServiceError> {
        let store = self.store.lock().unwrap();
        let running = store.list_running()?;
        Ok(running)
    }

    pub fn get_operation(&self, id: &JobId) -> Result<Job, OperationServiceError> {
        let store = self.store.lock().unwrap();
        let job = store.get(id)?;
        Ok(job)
    }
}
