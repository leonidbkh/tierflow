use std::io;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to parse YAML: {0}")]
    Parse(#[from] serde_yaml::Error),

    #[error("Invalid tier path '{path}': {reason}")]
    InvalidTierPath { path: PathBuf, reason: String },

    #[error("Duplicate tier name: {name}")]
    DuplicateTierName { name: String },

    #[error("Duplicate strategy name: {name}")]
    DuplicateStrategyName { name: String },

    #[error("Strategy '{strategy}' references unknown tier: {tier}")]
    UnknownTier { strategy: String, tier: String },

    #[error("No tiers defined in configuration")]
    NoTiers,

    #[error("No strategies defined in configuration")]
    NoStrategies,

    #[error("Mover '{mover}' is unavailable: {reason}")]
    MoverUnavailable { mover: String, reason: String },

    #[error("Tautulli is required: {reason}")]
    TautulliRequired { reason: String },

    #[error("Tautulli is unavailable: {reason}")]
    TautulliUnavailable { reason: String },

    #[error("Application error: {0}")]
    App(#[from] crate::AppError),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_io() {
        let err = ConfigError::Io(io::Error::new(io::ErrorKind::NotFound, "file not found"));
        let msg = format!("{err}");
        assert!(msg.contains("Failed to read config file"));
    }

    #[test]
    fn test_error_display_invalid_tier_path() {
        let err = ConfigError::InvalidTierPath {
            path: PathBuf::from("/invalid/path"),
            reason: "does not exist".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("Invalid tier path"));
        assert!(msg.contains("/invalid/path"));
        assert!(msg.contains("does not exist"));
    }

    #[test]
    fn test_error_display_duplicate_tier() {
        let err = ConfigError::DuplicateTierName {
            name: "cache".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("Duplicate tier name"));
        assert!(msg.contains("cache"));
    }

    #[test]
    fn test_error_display_unknown_tier() {
        let err = ConfigError::UnknownTier {
            strategy: "old_files".to_string(),
            tier: "archive".to_string(),
        };
        let msg = format!("{err}");
        assert!(msg.contains("references unknown tier"));
        assert!(msg.contains("old_files"));
        assert!(msg.contains("archive"));
    }

    #[test]
    fn test_error_display_no_tiers() {
        let err = ConfigError::NoTiers;
        let msg = format!("{err}");
        assert!(msg.contains("No tiers defined"));
    }

    #[test]
    fn test_error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
        let config_err: ConfigError = io_err.into();
        assert!(matches!(config_err, ConfigError::Io(_)));
    }

    #[test]
    fn test_result_type() {
        // Просто проверяем что Result тип работает
        let success: Result<i32> = Ok(42);
        assert_eq!(success.unwrap(), 42);

        let failure: Result<i32> = Err(ConfigError::NoTiers);
        assert!(failure.is_err());
    }
}
