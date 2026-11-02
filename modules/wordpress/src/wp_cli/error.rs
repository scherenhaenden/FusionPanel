use thiserror::Error;

#[derive(Error, Debug)]
pub enum WpCliError {
    #[error("WP-CLI command timed out after {0} seconds")]
    Timeout(u64),

    #[error("Failed to execute WP-CLI command: {0}")]
    ExecutionFailed(#[from] std::io::Error),

    #[error("WP-CLI command exited with failure status {0}. Stderr: {1}")]
    CommandFailed(i32, String),

    #[error("WP-CLI command was terminated by a signal")]
    TerminatedBySignal,

    #[error("Failed to parse WP-CLI output: {0}")]
    ParsingError(#[from] serde_json::Error),

    #[error("Invalid argument provided to WP-CLI command: {0}")]
    InvalidArgument(String),
}
