use crate::{Condition, Context, FileInfo, Tier};

pub use crate::config::StrategyAction;

pub struct PlacementStrategy {
    pub name: String,
    pub priority: u32,
    conditions: Vec<Box<dyn Condition>>,
    preferred_tiers: Vec<String>,
    pub is_required: bool,
    pub action: StrategyAction,
}

impl PlacementStrategy {
    pub fn new(name: String, priority: u32) -> Self {
        Self {
            name,
            priority,
            is_required: false,
            conditions: Vec::new(),
            preferred_tiers: Vec::new(),
            action: StrategyAction::Evaluate,
        }
    }
    pub fn add_condition(mut self, condition: Box<dyn Condition>) -> Self {
        self.conditions.push(condition);
        self
    }
    pub fn add_preferred_tier(mut self, tier_name: String) -> Self {
        self.preferred_tiers.push(tier_name);
        self
    }

    pub const fn required(mut self) -> Self {
        self.is_required = true;
        self
    }

    pub fn matches(&self, file: &FileInfo, context: &Context) -> bool {
        self.conditions.iter().all(|c| c.matches(file, context))
    }

    pub fn get_ideal_tier<'a>(
        &self,
        available_tiers: &'a [Tier],
        file: &FileInfo,
    ) -> Option<&'a Tier> {
        self.preferred_tiers
            .iter()
            .filter_map(|name| available_tiers.iter().find(|t| &t.name == name))
            .find(|tier| tier.has_space_for(file.size))
    }

    /// Возвращает список `preferred_tiers`
    pub fn preferred_tiers(&self) -> &[String] {
        &self.preferred_tiers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conditions::{AgeCondition, AlwaysTrueCondition};
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    fn create_test_file(hours_ago: u64, size: u64) -> FileInfo {
        let modified = SystemTime::now() - Duration::from_secs(hours_ago * 3600);
        FileInfo {
            path: PathBuf::from("/test/file.mkv"),
            size,
            modified,
            accessed: SystemTime::now(),
        }
    }

    // Test constants
    const TB: u64 = 1024 * 1024 * 1024 * 1024;

    fn create_test_tier(name: &str) -> Tier {
        // Use fixed 1TB disk at 0% usage for predictable tests
        Tier::new_mock_with_usage(name, 1, None, TB, 0)
    }

    #[test]
    fn test_new_strategy() {
        let strategy = PlacementStrategy::new("test".to_string(), 1);
        assert_eq!(strategy.name, "test");
        assert_eq!(strategy.priority, 1);
        assert!(!strategy.is_required);
        assert!(strategy.conditions.is_empty());
        assert!(strategy.preferred_tiers.is_empty());
    }

    #[test]
    fn test_builder_pattern() {
        let condition = AgeCondition::new(Some(10), None);
        let strategy = PlacementStrategy::new("test".to_string(), 1)
            .add_condition(Box::new(condition))
            .add_preferred_tier("tier1".to_string())
            .required();

        assert_eq!(strategy.conditions.len(), 1);
        assert_eq!(strategy.preferred_tiers.len(), 1);
        assert!(strategy.is_required);
        assert_eq!(strategy.priority, 1);
        assert_eq!(strategy.name, "test");
    }

    #[test]
    fn test_builder_multiple_conditions() {
        let strategy = PlacementStrategy::new("multi".to_string(), 5)
            .add_condition(Box::new(AlwaysTrueCondition))
            .add_condition(Box::new(AgeCondition::new(Some(24), None)))
            .add_preferred_tier("cache".to_string())
            .add_preferred_tier("storage".to_string());

        assert_eq!(strategy.conditions.len(), 2);
        assert_eq!(strategy.preferred_tiers.len(), 2);
    }

    #[test]
    fn test_matches_with_no_conditions() {
        let strategy = PlacementStrategy::new("empty".to_string(), 1);
        let file = create_test_file(0, 1000);
        let context = Context::new();

        // Нет условий = все файлы подходят (all() на пустом итераторе = true)
        assert!(strategy.matches(&file, &context));
    }

    #[test]
    fn test_matches_with_single_condition_true() {
        let strategy = PlacementStrategy::new("test".to_string(), 1)
            .add_condition(Box::new(AlwaysTrueCondition));

        let file = create_test_file(0, 1000);
        let context = Context::new();

        assert!(strategy.matches(&file, &context));
    }

    #[test]
    fn test_matches_with_single_condition_false() {
        let strategy = PlacementStrategy::new("test".to_string(), 1)
            .add_condition(Box::new(AgeCondition::new(Some(24), None)));

        let file = create_test_file(12, 1000); // Файл 12 часов назад
        let context = Context::new();

        assert!(!strategy.matches(&file, &context));
    }

    #[test]
    fn test_matches_with_multiple_conditions_all_true() {
        let strategy = PlacementStrategy::new("test".to_string(), 1)
            .add_condition(Box::new(AlwaysTrueCondition))
            .add_condition(Box::new(AgeCondition::new(Some(24), None)));

        let file = create_test_file(48, 1000); // Файл 48 часов назад
        let context = Context::new();

        // Оба условия true
        assert!(strategy.matches(&file, &context));
    }

    #[test]
    fn test_matches_with_multiple_conditions_one_false() {
        let strategy = PlacementStrategy::new("test".to_string(), 1)
            .add_condition(Box::new(AlwaysTrueCondition))
            .add_condition(Box::new(AgeCondition::new(Some(24), None)));

        let file = create_test_file(12, 1000); // Файл 12 часов назад
        let context = Context::new();

        // Одно условие false = весь результат false
        assert!(!strategy.matches(&file, &context));
    }

    #[test]
    fn test_get_ideal_tier_no_preferred_tiers() {
        let strategy = PlacementStrategy::new("test".to_string(), 1);
        let tier = create_test_tier("cache");
        let tiers = vec![tier];
        let file = create_test_file(0, 1000);

        // Нет preferred_tiers = нет результата
        assert!(strategy.get_ideal_tier(&tiers, &file).is_none());
    }

    #[test]
    fn test_get_ideal_tier_tier_not_found() {
        let strategy =
            PlacementStrategy::new("test".to_string(), 1).add_preferred_tier("missing".to_string());

        let tier = create_test_tier("cache");
        let tiers = vec![tier];
        let file = create_test_file(0, 1000);

        // Tier "missing" не существует
        assert!(strategy.get_ideal_tier(&tiers, &file).is_none());
    }

    #[test]
    fn test_get_ideal_tier_first_match() {
        let strategy = PlacementStrategy::new("test".to_string(), 1)
            .add_preferred_tier("cache".to_string())
            .add_preferred_tier("storage".to_string());

        let cache = create_test_tier("cache");
        let storage = create_test_tier("storage");
        let tiers = vec![cache, storage];
        let file = create_test_file(0, 1000);

        // Должен вернуть первый tier (cache)
        let result = strategy.get_ideal_tier(&tiers, &file);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "cache");
    }

    #[test]
    fn test_get_ideal_tier_skip_no_space() {
        let strategy = PlacementStrategy::new("test".to_string(), 1)
            .add_preferred_tier("cache".to_string())
            .add_preferred_tier("storage".to_string());

        let cache = create_test_tier("cache");
        let storage = create_test_tier("storage");
        let tiers = vec![cache, storage];

        // Файл размером больше чем свободное место на любом диске
        let huge_file = create_test_file(0, u64::MAX);

        // Нет tier с достаточным местом
        assert!(strategy.get_ideal_tier(&tiers, &huge_file).is_none());
    }

    #[test]
    fn test_get_ideal_tier_order_matters() {
        let strategy = PlacementStrategy::new("test".to_string(), 1)
            .add_preferred_tier("storage".to_string())
            .add_preferred_tier("cache".to_string());

        let cache = create_test_tier("cache");
        let storage = create_test_tier("storage");
        let tiers = vec![cache, storage];
        let file = create_test_file(0, 1000);

        // Должен вернуть storage (первый в preferred_tiers)
        let result = strategy.get_ideal_tier(&tiers, &file);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "storage");
    }

    #[test]
    fn test_strategy_with_all_features() {
        let strategy = PlacementStrategy::new("full_test".to_string(), 10)
            .add_condition(Box::new(AgeCondition::new(Some(24), None)))
            .add_condition(Box::new(AlwaysTrueCondition))
            .add_preferred_tier("cache".to_string())
            .add_preferred_tier("storage".to_string())
            .required();

        assert_eq!(strategy.name, "full_test");
        assert_eq!(strategy.priority, 10);
        assert!(strategy.is_required);
        assert_eq!(strategy.conditions.len(), 2);
        assert_eq!(strategy.preferred_tiers.len(), 2);

        // Проверяем matches
        let old_file = create_test_file(48, 1000);
        let context = Context::new();
        assert!(strategy.matches(&old_file, &context));

        // Проверяем get_ideal_tier
        let cache = create_test_tier("cache");
        let storage = create_test_tier("storage");
        let tiers = vec![cache, storage];
        let result = strategy.get_ideal_tier(&tiers, &old_file);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "cache");
    }
}
