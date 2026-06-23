use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockersConfig {
    #[serde(default)]
    pub on_error: BlockerErrorPolicyConfig,

    #[serde(default)]
    pub providers: Vec<BlockerProviderConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockerErrorPolicyConfig {
    #[default]
    FailClosed,
    FailOpen,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BlockerProviderConfig {
    Tdarr(TdarrBlockerConfig),
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TdarrBlockerConfig {
    pub url: String,

    #[serde(default = "default_page_size")]
    pub page_size: usize,

    #[serde(default = "default_true")]
    pub block_active_workers: bool,

    #[serde(default = "default_true")]
    pub block_staged: bool,

    #[serde(default = "default_true")]
    pub block_queued_transcode: bool,

    #[serde(default)]
    pub block_queued_healthcheck: bool,

    #[serde(default)]
    pub path_mappings: Vec<PathMappingConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathMappingConfig {
    pub host_prefix: PathBuf,
    pub app_prefix: String,
}

const fn default_page_size() -> usize {
    500
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tdarr_blocker_defaults() {
        let yaml = r"
url: http://tdarr.local:8265
";

        let config: TdarrBlockerConfig = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(config.url, "http://tdarr.local:8265");
        assert_eq!(config.page_size, 500);
        assert!(config.block_active_workers);
        assert!(config.block_staged);
        assert!(config.block_queued_transcode);
        assert!(!config.block_queued_healthcheck);
        assert!(config.path_mappings.is_empty());
    }

    #[test]
    fn test_blockers_config_deserializes_tdarr_provider() {
        let yaml = r"
on_error: fail_open
providers:
  - type: tdarr
    url: http://tdarr.local:8265
    path_mappings:
      - host_prefix: /mnt/tier1/media/movies
        app_prefix: /media/movies
";

        let config: BlockersConfig = serde_yaml::from_str(yaml).unwrap();

        assert!(matches!(
            config.on_error,
            BlockerErrorPolicyConfig::FailOpen
        ));
        assert_eq!(config.providers.len(), 1);
        match &config.providers[0] {
            BlockerProviderConfig::Tdarr(tdarr) => {
                assert_eq!(tdarr.url, "http://tdarr.local:8265");
                assert_eq!(tdarr.path_mappings.len(), 1);
            }
        }
    }
}
