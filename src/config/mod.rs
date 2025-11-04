mod condition;
mod error;
mod strategy;
mod tautulli;
mod tier;

pub use condition::ConditionConfig;
pub use error::{ConfigError, Result};
pub use strategy::PlacementStrategyConfig;
pub use tautulli::TautulliConfig;
pub use tier::TierConfig;

use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoverType {
    Rsync,
    DryRun,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MoverConfig {
    #[serde(rename = "type")]
    pub mover_type: MoverType,
    #[serde(default)]
    pub extra_args: Vec<String>,
}

impl Default for MoverConfig {
    fn default() -> Self {
        Self {
            mover_type: MoverType::Rsync,
            extra_args: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct BalancingConfig {
    pub tiers: Vec<TierConfig>,
    pub strategies: Vec<PlacementStrategyConfig>,
    #[serde(default)]
    pub mover: MoverConfig,
    pub tautulli: Option<TautulliConfig>,
}

impl BalancingConfig {
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: Self = serde_yaml::from_str(&contents)?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.tiers.is_empty() {
            return Err(ConfigError::NoTiers);
        }

        if self.strategies.is_empty() {
            return Err(ConfigError::NoStrategies);
        }

        let mut tier_names = HashSet::new();
        for tier in &self.tiers {
            if !tier_names.insert(&tier.name) {
                return Err(ConfigError::DuplicateTierName {
                    name: tier.name.clone(),
                });
            }
        }

        let mut strategy_names = HashSet::new();
        for strategy in &self.strategies {
            if !strategy_names.insert(&strategy.name) {
                return Err(ConfigError::DuplicateStrategyName {
                    name: strategy.name.clone(),
                });
            }
        }

        for strategy in &self.strategies {
            for tier_name in &strategy.preferred_tiers {
                if !tier_names.contains(tier_name) {
                    return Err(ConfigError::UnknownTier {
                        strategy: strategy.name.clone(),
                        tier: tier_name.clone(),
                    });
                }
            }
        }

        // Validate mover availability
        match self.mover.mover_type {
            MoverType::Rsync => {
                // Check if rsync is available
                let result = Command::new("rsync")
                    .arg("--version")
                    .output();

                match result {
                    Ok(output) if output.status.success() => {
                        log::debug!("Rsync is available");
                    }
                    _ => {
                        return Err(ConfigError::MoverUnavailable {
                            mover: "rsync".to_string(),
                            reason: "rsync command not found or not executable".to_string(),
                        });
                    }
                }
            }
            MoverType::DryRun => {
                // DryRun mover is always available
            }
        }

        // Validate Tautulli configuration if active_window conditions are used
        if self.has_active_window_conditions() {
            if let Some(tautulli_config) = &self.tautulli {
                log::info!("Validating Tautulli configuration (active_window conditions detected)");

                // Perform health check
                use crate::TautulliClient;
                let client = TautulliClient::new(
                    tautulli_config.url.clone(),
                    tautulli_config.api_key.clone(),
                )?;

                client.health_check().map_err(|e| {
                    ConfigError::TautulliUnavailable {
                        reason: format!("Tautulli health check failed: {e}"),
                    }
                })?;
            } else {
                return Err(ConfigError::TautulliRequired {
                    reason: "active_window condition is used but tautulli is not configured".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Check if any strategy uses `active_window` condition
    fn has_active_window_conditions(&self) -> bool {
        self.strategies.iter().any(|strategy| {
            strategy.conditions.iter().any(|condition| {
                matches!(condition, ConditionConfig::ActiveWindow { .. })
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_deserialize_full_config() {
        let yaml = r"
tiers:
  - name: cache
    path: /mnt/cache
    priority: 1
  - name: storage
    path: /mnt/storage
    priority: 10

strategies:
  - name: old_files
    priority: 10
    conditions:
      - type: max_age
        max_age_hours: 168
    preferred_tiers:
      - storage
    required: false

  - name: default
    priority: 1
    preferred_tiers:
      - cache
";
        let config: BalancingConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.tiers.len(), 2);
        assert_eq!(config.strategies.len(), 2);
    }

    #[test]
    fn test_from_file_valid() {
        let yaml = r"
tiers:
  - name: cache
    path: /tmp
    priority: 1

strategies:
  - name: default
    priority: 1
    preferred_tiers:
      - cache
";
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(yaml.as_bytes()).unwrap();

        let config = BalancingConfig::from_file(temp_file.path()).unwrap();
        assert_eq!(config.tiers.len(), 1);
        assert_eq!(config.strategies.len(), 1);
    }

    #[test]
    fn test_from_file_not_found() {
        let result = BalancingConfig::from_file(Path::new("/nonexistent.yaml"));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::Io(_)));
    }

    #[test]
    fn test_from_file_invalid_yaml() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"invalid: yaml: content:").unwrap();

        let result = BalancingConfig::from_file(temp_file.path());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::Parse(_)));
    }

    #[test]
    fn test_validate_no_tiers() {
        let config = BalancingConfig {
            tiers: vec![],
            strategies: vec![PlacementStrategyConfig {
                name: "test".to_string(),
                priority: 1,
                conditions: vec![],
                preferred_tiers: vec!["cache".to_string()],
                required: false,
            }],
            mover: MoverConfig::default(),
            tautulli: None,
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::NoTiers));
    }

    #[test]
    fn test_validate_no_strategies() {
        let config = BalancingConfig {
            tiers: vec![TierConfig {
                name: "cache".to_string(),
                path: "/tmp".into(),
                priority: 1,
                max_usage_percent: None,
            }],
            strategies: vec![],
            mover: MoverConfig::default(),
            tautulli: None,
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::NoStrategies));
    }

    #[test]
    fn test_validate_duplicate_tier_names() {
        let config = BalancingConfig {
            tiers: vec![
                TierConfig {
                    name: "cache".to_string(),
                    path: "/tmp".into(),
                    priority: 1,
                    max_usage_percent: None,
                },
                TierConfig {
                    name: "cache".to_string(),
                    path: "/tmp2".into(),
                    priority: 2,
                    max_usage_percent: None,
                },
            ],
            strategies: vec![PlacementStrategyConfig {
                name: "test".to_string(),
                priority: 1,
                conditions: vec![],
                preferred_tiers: vec!["cache".to_string()],
                required: false,
            }],
            mover: MoverConfig::default(),
            tautulli: None,
        };

        let result = config.validate();
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::DuplicateTierName { name } => assert_eq!(name, "cache"),
            _ => panic!("Expected DuplicateTierName error"),
        }
    }

    #[test]
    fn test_validate_duplicate_strategy_names() {
        let config = BalancingConfig {
            tiers: vec![TierConfig {
                name: "cache".to_string(),
                path: "/tmp".into(),
                priority: 1,
                max_usage_percent: None,
            }],
            strategies: vec![
                PlacementStrategyConfig {
                    name: "test".to_string(),
                    priority: 1,
                    conditions: vec![],
                    preferred_tiers: vec!["cache".to_string()],
                    required: false,
                },
                PlacementStrategyConfig {
                    name: "test".to_string(),
                    priority: 2,
                    conditions: vec![],
                    preferred_tiers: vec!["cache".to_string()],
                    required: false,
                },
            ],
            mover: MoverConfig::default(),
            tautulli: None,
        };

        let result = config.validate();
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::DuplicateStrategyName { name } => assert_eq!(name, "test"),
            _ => panic!("Expected DuplicateStrategyName error"),
        }
    }

    #[test]
    fn test_validate_unknown_tier() {
        let config = BalancingConfig {
            tiers: vec![TierConfig {
                name: "cache".to_string(),
                path: "/tmp".into(),
                priority: 1,
                max_usage_percent: None,
            }],
            strategies: vec![PlacementStrategyConfig {
                name: "test".to_string(),
                priority: 1,
                conditions: vec![],
                preferred_tiers: vec!["nonexistent".to_string()],
                required: false,
            }],
            mover: MoverConfig::default(),
            tautulli: None,
        };

        let result = config.validate();
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::UnknownTier { strategy, tier } => {
                assert_eq!(strategy, "test");
                assert_eq!(tier, "nonexistent");
            }
            _ => panic!("Expected UnknownTier error"),
        }
    }

    #[test]
    fn test_validate_valid_config() {
        let config = BalancingConfig {
            tiers: vec![
                TierConfig {
                    name: "cache".to_string(),
                    path: "/tmp".into(),
                    priority: 1,
                    max_usage_percent: None,
                },
                TierConfig {
                    name: "storage".to_string(),
                    path: "/tmp2".into(),
                    priority: 10,
                    max_usage_percent: None,
                },
            ],
            strategies: vec![
                PlacementStrategyConfig {
                    name: "old_files".to_string(),
                    priority: 10,
                    conditions: vec![],
                    preferred_tiers: vec!["storage".to_string()],
                    required: false,
                },
                PlacementStrategyConfig {
                    name: "default".to_string(),
                    priority: 1,
                    conditions: vec![],
                    preferred_tiers: vec!["cache".to_string(), "storage".to_string()],
                    required: false,
                },
            ],
            mover: MoverConfig::default(),
            tautulli: None,
        };

        let result = config.validate();
        assert!(result.is_ok());
    }
}
