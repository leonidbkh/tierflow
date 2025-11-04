use std::io;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Tier '{tier}' is locked by process {owner_pid} on {owner_host} for {locked_for:?}")]
    TierLocked {
        tier: String,
        owner_pid: u32,
        owner_host: String,
        locked_for: Duration,
    },

    #[error("Failed to acquire lock: {message}")]
    LockError { message: String },

    #[error("Another instance is already running")]
    AlreadyRunning,

    #[error("Failed to move file {from:?} to {to:?}: {reason}")]
    MoveFailed {
        from: PathBuf,
        to: PathBuf,
        reason: String,
    },

    #[error("Command failed: {command}. Exit code: {exit_code}")]
    CommandFailed { command: String, exit_code: i32 },

    #[error("External service error: {0}")]
    External(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
