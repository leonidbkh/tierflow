use super::{PlacementDecision, PlanWarning};
use crate::Tier;
use std::collections::HashMap;

/// Internal planning state with free space simulation
pub(super) struct PlanningState {
    /// Simulated free space after all planned operations
    pub tier_free_space: HashMap<String, u64>,
    pub decisions: Vec<PlacementDecision>,
    pub warnings: Vec<PlanWarning>,
}

impl PlanningState {
    pub fn new(tiers: &[Tier]) -> Self {
        Self {
            tier_free_space: tiers
                .iter()
                .map(|t| (t.name.clone(), t.get_free_space()))
                .collect(),
            decisions: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Updates simulated state after planning a move
    pub fn apply_move(&mut self, file_size: u64, from_tier: &str, to_tier: &str) {
        if let Some(free) = self.tier_free_space.get_mut(from_tier) {
            *free = free.saturating_add(file_size);
        }

        if let Some(free) = self.tier_free_space.get_mut(to_tier) {
            *free = free.saturating_sub(file_size);
        }
    }

    #[cfg(test)]
    pub fn get_simulated_free_space(&self, tier_name: &str) -> Option<u64> {
        self.tier_free_space.get(tier_name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn create_test_tier(name: &str, _size: u64) -> Tier {
        let temp_dir = std::env::temp_dir().join(format!("tier_state_test_{name}"));
        fs::create_dir_all(&temp_dir).unwrap();
        Tier::new(name.to_string(), temp_dir, 1, None).unwrap()
    }

    #[test]
    fn test_new_planning_state() {
        let cache = create_test_tier("cache", 1000);
        let storage = create_test_tier("storage", 2000);
        let tiers = vec![cache, storage];

        let state = PlanningState::new(&tiers);

        assert_eq!(state.tier_free_space.len(), 2);
        assert!(state.tier_free_space.contains_key("cache"));
        assert!(state.tier_free_space.contains_key("storage"));
        assert!(state.decisions.is_empty());
        assert!(state.warnings.is_empty());
    }

    #[test]
    fn test_apply_move() {
        let cache = create_test_tier("cache", 1000);
        let storage = create_test_tier("storage", 2000);
        let tiers = vec![cache, storage];

        let mut state = PlanningState::new(&tiers);

        let cache_free_before = state.get_simulated_free_space("cache").unwrap();
        let storage_free_before = state.get_simulated_free_space("storage").unwrap();

        state.apply_move(500, "cache", "storage");

        let cache_free_after = state.get_simulated_free_space("cache").unwrap();
        let storage_free_after = state.get_simulated_free_space("storage").unwrap();

        assert_eq!(cache_free_after, cache_free_before + 500);
        assert_eq!(storage_free_after, storage_free_before - 500);
    }

    #[test]
    fn test_apply_move_multiple() {
        let cache = create_test_tier("cache", 1000);
        let storage = create_test_tier("storage", 2000);
        let tiers = vec![cache, storage];

        let mut state = PlanningState::new(&tiers);

        let initial_cache = state.get_simulated_free_space("cache").unwrap();
        let initial_storage = state.get_simulated_free_space("storage").unwrap();

        state.apply_move(100, "cache", "storage");
        state.apply_move(200, "cache", "storage");
        state.apply_move(50, "storage", "cache");

        let final_cache = state.get_simulated_free_space("cache").unwrap();
        let final_storage = state.get_simulated_free_space("storage").unwrap();

        assert_eq!(final_cache, initial_cache + 250);
        assert_eq!(final_storage, initial_storage - 250);
    }

    #[test]
    fn test_apply_move_saturating_sub() {
        let cache = create_test_tier("cache", 1000);
        let tiers = vec![cache];

        let mut state = PlanningState::new(&tiers);

        let cache_free = state.get_simulated_free_space("cache").unwrap();

        state.apply_move(cache_free + 1000, "storage", "cache");

        let cache_free_after = state.get_simulated_free_space("cache").unwrap();

        assert_eq!(cache_free_after, 0);
    }

    #[test]
    fn test_apply_move_saturating_add() {
        let cache = create_test_tier("cache", 1000);
        let tiers = vec![cache];

        let mut state = PlanningState::new(&tiers);

        state.apply_move(u64::MAX - 100, "cache", "storage");

        let cache_free = state.get_simulated_free_space("cache").unwrap();

        assert!(cache_free > 0);
    }

    #[test]
    fn test_get_simulated_free_space() {
        let cache = create_test_tier("cache", 1000);
        let tiers = vec![cache];

        let state = PlanningState::new(&tiers);

        assert!(state.get_simulated_free_space("cache").is_some());
        assert!(state.get_simulated_free_space("nonexistent").is_none());
    }

    #[test]
    fn test_apply_move_unknown_tier() {
        let cache = create_test_tier("cache", 1000);
        let tiers = vec![cache];

        let mut state = PlanningState::new(&tiers);

        let cache_free_before = state.get_simulated_free_space("cache").unwrap();

        state.apply_move(100, "cache", "nonexistent");
        state.apply_move(100, "nonexistent", "cache");

        let cache_free_after = state.get_simulated_free_space("cache").unwrap();
        assert_eq!(cache_free_after, cache_free_before);
    }

    #[test]
    fn test_state_with_decisions_and_warnings() {
        let cache = create_test_tier("cache", 1000);
        let tiers = vec![cache];

        let mut state = PlanningState::new(&tiers);

        state.decisions.push(PlacementDecision::Stay {
            file: crate::FileInfo {
                path: PathBuf::from("/test/file.mkv"),
                size: 1000,
                modified: std::time::SystemTime::now(),
                accessed: std::time::SystemTime::now(),
            },
            current_tier: "cache".to_string(),
        });

        state.warnings.push(PlanWarning::InsufficientSpace {
            file: PathBuf::from("/test/large.mkv"),
            strategy: "test".to_string(),
            needed: 1000,
            available: 500,
        });

        assert_eq!(state.decisions.len(), 1);
        assert_eq!(state.warnings.len(), 1);
    }
}
