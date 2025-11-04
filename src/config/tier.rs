use crate::Tier;
use serde::Deserialize;
use std::io;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct TierConfig {
    pub name: String,
    pub path: PathBuf,
    pub priority: u32,
    /// Maximum tier usage percent (0-100). If not specified, tier can fill to 100%
    #[serde(default)]
    pub max_usage_percent: Option<u64>,
}

impl TierConfig {
    pub fn into_tier(self) -> io::Result<Tier> {
        Tier::new(
            self.name,
            self.path,
            self.priority,
            self.max_usage_percent,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_tier_config() {
        let yaml = r"
name: cache
path: /mnt/cache
priority: 1
";
        let config: TierConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "cache");
        assert_eq!(config.path, PathBuf::from("/mnt/cache"));
        assert_eq!(config.priority, 1);
    }

    #[test]
    fn test_deserialize_multiple_tiers() {
        let yaml = r"
- name: cache
  path: /mnt/cache
  priority: 1
- name: storage
  path: /mnt/storage
  priority: 10
";
        let configs: Vec<TierConfig> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].name, "cache");
        assert_eq!(configs[1].name, "storage");
    }

    #[test]
    fn test_into_tier_valid_path() {
        let temp_dir = std::env::temp_dir();
        let config = TierConfig {
            name: "test".to_string(),
            path: temp_dir.clone(),
            priority: 1,
            max_usage_percent: None,
        };

        let tier = config.into_tier().unwrap();
        assert_eq!(tier.name, "test");
        assert_eq!(tier.path, temp_dir);
        assert_eq!(tier.priority, 1);
    }

    #[test]
    fn test_into_tier_invalid_path() {
        let config = TierConfig {
            name: "test".to_string(),
            path: PathBuf::from("/nonexistent/path"),
            priority: 1,
            max_usage_percent: None,
        };

        let result = config.into_tier();
        assert!(result.is_err());
    }

    #[test]
    fn test_tier_config_clone() {
        let config = TierConfig {
            name: "cache".to_string(),
            path: PathBuf::from("/mnt/cache"),
            priority: 1,
            max_usage_percent: Some(85),
        };

        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_deserialize_tier_with_max_usage() {
        let yaml = r"
name: cache
path: /mnt/cache
priority: 1
max_usage_percent: 85
";
        let config: TierConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "cache");
        assert_eq!(config.max_usage_percent, Some(85));
    }

    #[test]
    fn test_deserialize_tier_without_max_usage() {
        let yaml = r"
name: storage
path: /mnt/storage
priority: 10
";
        let config: TierConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "storage");
        assert_eq!(config.max_usage_percent, None);
    }
}
