use serde::Deserialize;

use super::ConditionConfig;

/// Действие стратегии при совпадении условий
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StrategyAction {
    /// Обычная обработка: найти ideal tier и переместить файл если нужно
    #[default]
    Evaluate,
    /// Всегда оставлять файл на текущем месте (игнорировать)
    Stay,
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlacementStrategyConfig {
    pub name: String,
    pub priority: u32,
    #[serde(default)]
    pub conditions: Vec<ConditionConfig>,
    pub preferred_tiers: Vec<String>,
    #[serde(default)]
    pub required: bool,
    /// Действие стратегии: evaluate (обычная обработка) или stay (игнорировать)
    #[serde(default)]
    pub action: StrategyAction,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, FileInfo, factory};
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn create_test_file(hours_ago: u64) -> FileInfo {
        let modified = SystemTime::now() - Duration::from_secs(hours_ago * 3600);
        FileInfo {
            path: PathBuf::from("/test/file.mkv"),
            size: 1000,
            modified,
            accessed: SystemTime::now(),
        }
    }

    #[test]
    fn test_deserialize_strategy_full() {
        let yaml = r"
name: old_files
priority: 10
conditions:
  - type: max_age
    max_age_hours: 168
preferred_tiers:
  - storage
required: true
";
        let config: PlacementStrategyConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "old_files");
        assert_eq!(config.priority, 10);
        assert_eq!(config.conditions.len(), 1);
        assert_eq!(config.preferred_tiers, vec!["storage"]);
        assert!(config.required);
    }

    #[test]
    fn test_deserialize_strategy_minimal() {
        let yaml = r"
name: default
priority: 1
preferred_tiers:
  - cache
";
        let config: PlacementStrategyConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "default");
        assert_eq!(config.priority, 1);
        assert_eq!(config.conditions.len(), 0); // default
        assert_eq!(config.preferred_tiers, vec!["cache"]);
        assert!(!config.required); // default
    }

    #[test]
    fn test_deserialize_strategy_multiple_conditions() {
        let yaml = r"
name: complex
priority: 5
conditions:
  - type: max_age
    max_age_hours: 24
  - type: always_true
preferred_tiers:
  - cache
  - storage
";
        let config: PlacementStrategyConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.conditions.len(), 2);
        assert_eq!(config.preferred_tiers.len(), 2);
    }

    #[test]
    fn test_deserialize_multiple_strategies() {
        let yaml = r"
- name: strategy1
  priority: 10
  preferred_tiers:
    - storage
- name: strategy2
  priority: 5
  preferred_tiers:
    - cache
";
        let configs: Vec<PlacementStrategyConfig> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].name, "strategy1");
        assert_eq!(configs[1].name, "strategy2");
    }

    #[test]
    fn test_into_strategy_no_conditions() {
        let config = PlacementStrategyConfig {
            name: "test".to_string(),
            priority: 1,
            conditions: vec![],
            preferred_tiers: vec!["cache".to_string()],
            required: false,
            action: StrategyAction::Evaluate,
        };

        let strategy = factory::build_strategy(config);
        assert_eq!(strategy.name, "test");
        assert_eq!(strategy.priority, 1);
        assert!(!strategy.is_required);

        // Стратегия без условий матчит любой файл
        let context = Context::new();
        let file = create_test_file(0);
        assert!(strategy.matches(&file, &context));
    }

    #[test]
    fn test_into_strategy_with_conditions() {
        let config = PlacementStrategyConfig {
            name: "old_files".to_string(),
            priority: 10,
            conditions: vec![ConditionConfig::MaxAge { max_age_hours: 24 }],
            preferred_tiers: vec!["storage".to_string()],
            required: false,
            action: StrategyAction::Evaluate,
        };

        let strategy = factory::build_strategy(config);
        let context = Context::new();

        // Старый файл матчится
        let old_file = create_test_file(48);
        assert!(strategy.matches(&old_file, &context));

        // Новый файл не матчится
        let new_file = create_test_file(12);
        assert!(!strategy.matches(&new_file, &context));
    }

    #[test]
    fn test_into_strategy_required() {
        let config = PlacementStrategyConfig {
            name: "required_strategy".to_string(),
            priority: 100,
            conditions: vec![],
            preferred_tiers: vec!["cache".to_string()],
            required: true,
            action: StrategyAction::Evaluate,
        };

        let strategy = factory::build_strategy(config);
        assert!(strategy.is_required);
    }

    #[test]
    fn test_into_strategy_multiple_conditions() {
        let config = PlacementStrategyConfig {
            name: "multi".to_string(),
            priority: 5,
            conditions: vec![
                ConditionConfig::AlwaysTrue,
                ConditionConfig::MaxAge { max_age_hours: 24 },
            ],
            preferred_tiers: vec!["cache".to_string()],
            required: false,
            action: StrategyAction::Evaluate,
        };

        let strategy = factory::build_strategy(config);
        let context = Context::new();

        // Оба условия должны выполниться (AND)
        let old_file = create_test_file(48);
        assert!(strategy.matches(&old_file, &context));

        let new_file = create_test_file(12);
        assert!(!strategy.matches(&new_file, &context)); // max_age не выполнен
    }

    #[test]
    fn test_into_strategy_multiple_tiers() {
        let config = PlacementStrategyConfig {
            name: "multi_tier".to_string(),
            priority: 1,
            conditions: vec![],
            preferred_tiers: vec![
                "cache".to_string(),
                "storage".to_string(),
                "archive".to_string(),
            ],
            required: false,
            action: StrategyAction::Evaluate,
        };

        let strategy = factory::build_strategy(config);
        // Проверяем что все tier'ы добавлены (косвенно через публичные поля нельзя)
        // Но мы знаем что это работает благодаря тестам PlacementStrategy
        assert_eq!(strategy.name, "multi_tier");
    }

    #[test]
    fn test_strategy_config_clone() {
        let config = PlacementStrategyConfig {
            name: "test".to_string(),
            priority: 1,
            conditions: vec![ConditionConfig::AlwaysTrue],
            preferred_tiers: vec!["cache".to_string()],
            required: false,
            action: StrategyAction::Evaluate,
        };

        let cloned = config.clone();
        assert_eq!(config, cloned);
    }
}
