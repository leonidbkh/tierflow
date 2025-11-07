use serde::Deserialize;

/// Tautulli configuration for tracking Plex viewing progress
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TautulliConfig {
    /// Base URL of Tautulli instance (e.g., "<http://localhost:8181>")
    pub url: String,

    /// API key for authentication
    pub api_key: String,

    /// Number of history items to fetch (default: 1000)
    #[serde(default = "default_history_length")]
    pub history_length: u32,

    /// Percent complete threshold to consider episode "watched" (default: 90)
    #[serde(default = "default_watched_threshold")]
    pub watched_threshold: u8,

    /// Only consider episodes watched in last N days (default: 30)
    #[serde(default = "default_days_back")]
    pub days_back: u32,

    /// Number of episodes to keep before currently watched (default: 2)
    #[serde(default = "default_backward_episodes")]
    pub backward_episodes: u32,

    /// Number of episodes to keep after currently watched (default: 5)
    #[serde(default = "default_forward_episodes")]
    pub forward_episodes: u32,
}

const fn default_history_length() -> u32 {
    1000
}

const fn default_watched_threshold() -> u8 {
    90
}

const fn default_days_back() -> u32 {
    30
}

const fn default_backward_episodes() -> u32 {
    2
}

const fn default_forward_episodes() -> u32 {
    5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tautulli_config_defaults() {
        let yaml = r#"
url: "http://localhost:8181"
api_key: "test-key"
"#;

        let config: TautulliConfig = serde_yaml::from_str(yaml).expect("Should parse");

        assert_eq!(config.url, "http://localhost:8181");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.history_length, 1000);
        assert_eq!(config.watched_threshold, 90);
        assert_eq!(config.days_back, 30);
        assert_eq!(config.backward_episodes, 2);
        assert_eq!(config.forward_episodes, 5);
    }

    #[test]
    fn test_tautulli_config_custom_values() {
        let yaml = r#"
url: "http://192.168.1.100:8181"
api_key: "my-api-key"
history_length: 500
watched_threshold: 85
days_back: 14
backward_episodes: 3
forward_episodes: 10
"#;

        let config: TautulliConfig = serde_yaml::from_str(yaml).expect("Should parse");

        assert_eq!(config.url, "http://192.168.1.100:8181");
        assert_eq!(config.api_key, "my-api-key");
        assert_eq!(config.history_length, 500);
        assert_eq!(config.watched_threshold, 85);
        assert_eq!(config.days_back, 14);
        assert_eq!(config.backward_episodes, 3);
        assert_eq!(config.forward_episodes, 10);
    }
}
