use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use thiserror::Error;
use tokio::time::timeout;

use fusionpanel_agent_protocol::{Request, Response, ResponseResult};
use fusionpanel_jobs::{JobState, JobStore, JobStoreError, MAX_ERROR_TEXT_BYTES};

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("Job store error: {0}")]
    Store(#[from] JobStoreError),
    #[error("Agent error: {0}")]
    Agent(String),
}

/// Abstract client for dispatching agent operations.
#[async_trait]
pub trait AgentClient: Send + Sync {
    /// Execute the requested agent operation and return the response.
    async fn execute(&self, request: Request) -> Result<Response, String>;
}

/// A worker that polls the job store, claims operations, and dispatches them via the agent client.
pub struct Worker {
    pub worker_id: String,
    pub store: Arc<Mutex<JobStore>>,
    pub agent_client: Arc<dyn AgentClient>,
    pub execution_timeout: Duration,
}

impl Worker {
    pub fn new(
        worker_id: String,
        store: Arc<Mutex<JobStore>>,
        agent_client: Arc<dyn AgentClient>,
        execution_timeout: Duration,
    ) -> Self {
        Self {
            worker_id,
            store,
            agent_client,
            execution_timeout,
        }
    }

    /// Run one iteration of the worker: claim a job if available, execute it, and record the result.
    /// Returns true if a job was processed, false otherwise.
    pub async fn run_once(&self) -> Result<bool, WorkerError> {
        let job = {
            let mut store = self.store.lock().unwrap();
            store.claim(&self.worker_id)?
        };

        let job = match job {
            Some(j) => j,
            None => return Ok(false),
        };

        // Parse payload
        let request: Request = match serde_json::from_value(job.payload.clone()) {
            Ok(req) => req,
            Err(e) => {
                self.transition_to_failed(&job.id, format!("Invalid payload: {}", e))?;
                return Ok(true);
            }
        };

        // Execute via agent client with timeout
        let result = timeout(self.execution_timeout, self.agent_client.execute(request)).await;

        match result {
            Ok(Ok(response)) => {
                match response.result {
                    ResponseResult::Result(_) => {
                        let mut store = self.store.lock().unwrap();
                        let _ = store.transition(&job.id, JobState::Succeeded, None);
                    }
                    ResponseResult::Error(err) => {
                        self.transition_to_failed(&job.id, format!("Agent error: {}", err.message))?;
                    }
                }
            }
            Ok(Err(e)) => {
                self.transition_to_failed(&job.id, format!("Agent client error: {}", e))?;
            }
            Err(_) => {
                self.transition_to_failed(&job.id, "Execution timed out".to_string())?;
            }
        }

        Ok(true)
    }

    fn transition_to_failed(&self, id: &str, mut msg: String) -> Result<(), WorkerError> {
        if msg.len() > MAX_ERROR_TEXT_BYTES {
            let mut truncate_at = MAX_ERROR_TEXT_BYTES;
            while !msg.is_char_boundary(truncate_at) {
                truncate_at -= 1;
            }
            msg.truncate(truncate_at);
        }
        let mut store = self.store.lock().unwrap();
        // Disregard if it's an invalid transition (e.g. cancelled while running)
        let _ = store.transition(&id.to_string(), JobState::Failed, Some(msg));
        Ok(())
    }
}
