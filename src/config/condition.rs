use crate::{ContainsMode, ExtensionMode, PrefixMode};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExtensionModeConfig {
    Whitelist,
    Blacklist,
}

impl From<ExtensionModeConfig> for ExtensionMode {
    fn from(config: ExtensionModeConfig) -> Self {
        match config {
            ExtensionModeConfig::Whitelist => Self::Whitelist,
            ExtensionModeConfig::Blacklist => Self::Blacklist,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PrefixModeConfig {
    Whitelist,
    Blacklist,
}

impl From<PrefixModeConfig> for PrefixMode {
    fn from(config: PrefixModeConfig) -> Self {
        match config {
            PrefixModeConfig::Whitelist => Self::Whitelist,
            PrefixModeConfig::Blacklist => Self::Blacklist,
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContainsModeConfig {
    Whitelist,
    Blacklist,
}

impl From<ContainsModeConfig> for ContainsMode {
    fn from(config: ContainsModeConfig) -> Self {
        match config {
            ContainsModeConfig::Whitelist => Self::Whitelist,
            ContainsModeConfig::Blacklist => Self::Blacklist,
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConditionConfig {
    Age {
        #[serde(skip_serializing_if = "Option::is_none")]
        min_hours: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_hours: Option<u64>,
    },
    AlwaysTrue,
    FileExtension {
        extensions: Vec<String>,
        mode: ExtensionModeConfig,
    },
    PathPrefix {
        prefix: String,
        mode: PrefixModeConfig,
    },
    FileSize {
        #[serde(skip_serializing_if = "Option::is_none")]
        min_size_mb: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max_size_mb: Option<u64>,
    },
    FilenameContains {
        patterns: Vec<String>,
        mode: ContainsModeConfig,
        #[serde(default = "default_true")]
        case_sensitive: bool,
    },
    ActiveWindow {
        name: String,
    },
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Condition, Context, factory};
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn create_test_file(hours_ago: u64) -> crate::FileInfo {
        let modified = SystemTime::now() - Duration::from_secs(hours_ago * 3600);
        crate::FileInfo {
            path: PathBuf::from("/test/file.mkv"),
            size: 1000,
            modified,
            accessed: SystemTime::now(),
        }
    }

    #[test]
    fn test_deserialize_max_age() {
        let yaml = r"
type: age
min_hours: 168
";
        let config: ConditionConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config,
            ConditionConfig::Age {
                min_hours: Some(168),
                max_hours: None
            }
        );
    }

    #[test]
    fn test_deserialize_always_true() {
        let yaml = r"
type: always_true
";
        let config: ConditionConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config, ConditionConfig::AlwaysTrue);
    }

    #[test]
    fn test_deserialize_multiple_conditions() {
        let yaml = r"
- type: age
  min_hours: 24
- type: always_true
- type: age
  min_hours: 168
";
        let configs: Vec<ConditionConfig> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(configs.len(), 3);
        assert_eq!(
            configs[0],
            ConditionConfig::Age {
                min_hours: Some(24),
                max_hours: None
            }
        );
        assert_eq!(configs[1], ConditionConfig::AlwaysTrue);
        assert_eq!(
            configs[2],
            ConditionConfig::Age {
                min_hours: Some(168),
                max_hours: None
            }
        );
    }

    #[test]
    fn test_deserialize_unknown_type() {
        let yaml = r"
type: unknown_condition
";
        let result: Result<ConditionConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_into_condition_max_age() {
        let config = ConditionConfig::Age {
            min_hours: Some(24),
            max_hours: None,
        };
        let condition = factory::build_condition(config);
        let context = Context::new();

        // Старый файл должен матчиться
        let old_file = create_test_file(48);
        assert!(condition.matches(&old_file, &context));

        // Новый файл не должен
        let new_file = create_test_file(12);
        assert!(!condition.matches(&new_file, &context));
    }

    #[test]
    fn test_into_condition_always_true() {
        let config = ConditionConfig::AlwaysTrue;
        let condition = factory::build_condition(config);
        let context = Context::new();

        let file = create_test_file(0);
        assert!(condition.matches(&file, &context));

        let old_file = create_test_file(1000);
        assert!(condition.matches(&old_file, &context));
    }

    #[test]
    fn test_condition_config_clone() {
        let config = ConditionConfig::Age {
            min_hours: Some(168),
            max_hours: None,
        };
        let cloned = config.clone();
        assert_eq!(config, cloned);

        let config2 = ConditionConfig::AlwaysTrue;
        let cloned2 = config2.clone();
        assert_eq!(config2, cloned2);
    }

    #[test]
    fn test_convert_multiple_conditions() {
        let configs = vec![
            ConditionConfig::Age {
                min_hours: Some(24),
                max_hours: None,
            },
            ConditionConfig::AlwaysTrue,
        ];

        let conditions: Vec<Box<dyn Condition>> =
            configs.into_iter().map(factory::build_condition).collect();

        assert_eq!(conditions.len(), 2);

        // Проверяем что условия работают
        let context = Context::new();
        let old_file = create_test_file(48);

        assert!(conditions[0].matches(&old_file, &context));
        assert!(conditions[1].matches(&old_file, &context));
    }

    #[test]
    fn test_deserialize_file_extension() {
        let yaml = r#"
type: file_extension
extensions: ["mkv", "mp4", "avi"]
mode: whitelist
"#;
        let config: ConditionConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config,
            ConditionConfig::FileExtension {
                extensions: vec!["mkv".to_string(), "mp4".to_string(), "avi".to_string()],
                mode: ExtensionModeConfig::Whitelist,
            }
        );
    }

    #[test]
    fn test_deserialize_file_extension_single() {
        let yaml = r#"
type: file_extension
extensions: ["!qB"]
mode: whitelist
"#;
        let config: ConditionConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config,
            ConditionConfig::FileExtension {
                extensions: vec!["!qB".to_string()],
                mode: ExtensionModeConfig::Whitelist,
            }
        );
    }

    #[test]
    fn test_into_condition_file_extension() {
        let config = ConditionConfig::FileExtension {
            extensions: vec!["mkv".to_string(), "!qB".to_string()],
            mode: ExtensionModeConfig::Whitelist,
        };
        let condition = factory::build_condition(config);
        let context = Context::new();

        // MKV файл должен матчиться
        let mkv_file = crate::FileInfo {
            path: PathBuf::from("/test/movie.mkv"),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };
        assert!(condition.matches(&mkv_file, &context));

        // Incomplete файл должен матчиться
        let incomplete_file = crate::FileInfo {
            path: PathBuf::from("/test/movie.mkv.!qB"),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };
        assert!(condition.matches(&incomplete_file, &context));

        // MP4 файл не должен матчиться
        let mp4_file = crate::FileInfo {
            path: PathBuf::from("/test/movie.mp4"),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };
        assert!(!condition.matches(&mp4_file, &context));
    }

    #[test]
    fn test_file_extension_config_clone() {
        let config = ConditionConfig::FileExtension {
            extensions: vec!["mkv".to_string(), "mp4".to_string()],
            mode: ExtensionModeConfig::Whitelist,
        };
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_deserialize_file_extension_blacklist() {
        let yaml = r#"
type: file_extension
extensions: ["!qB", "part", "tmp"]
mode: blacklist
"#;
        let config: ConditionConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config,
            ConditionConfig::FileExtension {
                extensions: vec!["!qB".to_string(), "part".to_string(), "tmp".to_string()],
                mode: ExtensionModeConfig::Blacklist,
            }
        );
    }

    #[test]
    fn test_deserialize_file_extension_whitelist_explicit() {
        let yaml = r#"
type: file_extension
extensions: ["mkv", "mp4"]
mode: whitelist
"#;
        let config: ConditionConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config,
            ConditionConfig::FileExtension {
                extensions: vec!["mkv".to_string(), "mp4".to_string()],
                mode: ExtensionModeConfig::Whitelist,
            }
        );
    }

    #[test]
    fn test_into_condition_file_extension_blacklist() {
        let config = ConditionConfig::FileExtension {
            extensions: vec!["!qB".to_string(), "part".to_string()],
            mode: ExtensionModeConfig::Blacklist,
        };
        let condition = factory::build_condition(config);
        let context = Context::new();

        // MKV файл НЕ в blacklist → должен матчиться
        let mkv_file = crate::FileInfo {
            path: PathBuf::from("/test/movie.mkv"),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };
        assert!(condition.matches(&mkv_file, &context));

        // Incomplete файл в blacklist → НЕ должен матчиться
        let incomplete_file = crate::FileInfo {
            path: PathBuf::from("/test/movie.mkv.!qB"),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };
        assert!(!condition.matches(&incomplete_file, &context));
    }

    #[test]
    fn test_deserialize_path_prefix() {
        let yaml = r#"
type: path_prefix
prefix: "downloads"
mode: whitelist
"#;
        let config: ConditionConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config,
            ConditionConfig::PathPrefix {
                prefix: "downloads".to_string(),
                mode: PrefixModeConfig::Whitelist,
            }
        );
    }

    #[test]
    fn test_deserialize_path_prefix_multilevel() {
        let yaml = r#"
type: path_prefix
prefix: "media/movies"
mode: whitelist
"#;
        let config: ConditionConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config,
            ConditionConfig::PathPrefix {
                prefix: "media/movies".to_string(),
                mode: PrefixModeConfig::Whitelist,
            }
        );
    }

    #[test]
    fn test_into_condition_path_prefix() {
        let config = ConditionConfig::PathPrefix {
            prefix: "downloads".to_string(),
            mode: PrefixModeConfig::Whitelist,
        };
        let condition = factory::build_condition(config);
        let context = Context::new().with_tier_path(PathBuf::from("/mnt/cache"));

        // Файл в downloads должен матчиться
        let downloads_file = crate::FileInfo {
            path: PathBuf::from("/mnt/cache/downloads/movie.mkv"),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };
        assert!(condition.matches(&downloads_file, &context));

        // Файл в другой папке не должен матчиться
        let other_file = crate::FileInfo {
            path: PathBuf::from("/mnt/cache/series/show.mkv"),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };
        assert!(!condition.matches(&other_file, &context));
    }

    #[test]
    fn test_path_prefix_config_clone() {
        let config = ConditionConfig::PathPrefix {
            prefix: "downloads".to_string(),
            mode: PrefixModeConfig::Whitelist,
        };
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_deserialize_path_prefix_blacklist() {
        let yaml = r#"
type: path_prefix
prefix: "downloads"
mode: blacklist
"#;
        let config: ConditionConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config,
            ConditionConfig::PathPrefix {
                prefix: "downloads".to_string(),
                mode: PrefixModeConfig::Blacklist,
            }
        );
    }

    #[test]
    fn test_deserialize_path_prefix_whitelist_explicit() {
        let yaml = r#"
type: path_prefix
prefix: "series_lib"
mode: whitelist
"#;
        let config: ConditionConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(
            config,
            ConditionConfig::PathPrefix {
                prefix: "series_lib".to_string(),
                mode: PrefixModeConfig::Whitelist,
            }
        );
    }

    #[test]
    fn test_into_condition_path_prefix_blacklist() {
        let config = ConditionConfig::PathPrefix {
            prefix: "downloads".to_string(),
            mode: PrefixModeConfig::Blacklist,
        };
        let condition = factory::build_condition(config);
        let context = Context::new().with_tier_path(PathBuf::from("/mnt/cache"));

        // Файл НЕ в downloads → должен матчиться
        let series_file = crate::FileInfo {
            path: PathBuf::from("/mnt/cache/series_lib/show.mkv"),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };
        assert!(condition.matches(&series_file, &context));

        // Файл в downloads → НЕ должен матчиться
        let downloads_file = crate::FileInfo {
            path: PathBuf::from("/mnt/cache/downloads/movie.mkv"),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };
        assert!(!condition.matches(&downloads_file, &context));
    }
}
