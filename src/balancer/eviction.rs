use super::{PlacementDecision, state::BlockedPlacement};
use crate::Tier;
use std::collections::HashMap;
use std::sync::Arc;

pub struct EvictionPlanner<'a> {
    tiers: &'a [Tier],
}

impl<'a> EvictionPlanner<'a> {
    pub fn new(tiers: &'a [Tier]) -> Self {
        Self { tiers }
    }

    pub fn evict_to_make_space(
        &self,
        decisions: &mut [PlacementDecision],
        blocked_placements: Vec<BlockedPlacement>,
        tier_free_space: &mut HashMap<String, u64>,
    ) {
        if blocked_placements.is_empty() {
            return;
        }

        let by_tier = self.group_by_tier(blocked_placements);

        for (tier_name, blocked_list) in by_tier {
            self.evict_from_tier(&tier_name, blocked_list, decisions, tier_free_space);
        }
    }

    fn group_by_tier(
        &self,
        blocked_placements: Vec<BlockedPlacement>,
    ) -> HashMap<String, Vec<BlockedPlacement>> {
        let mut by_tier: HashMap<String, Vec<BlockedPlacement>> = HashMap::new();
        for blocked in blocked_placements {
            by_tier
                .entry(blocked.desired_tier.clone())
                .or_default()
                .push(blocked);
        }
        by_tier
    }

    fn evict_from_tier(
        &self,
        tier_name: &str,
        mut blocked_list: Vec<BlockedPlacement>,
        decisions: &mut [PlacementDecision],
        tier_free_space: &mut HashMap<String, u64>,
    ) {
        blocked_list.sort_by_key(|b| std::cmp::Reverse(b.strategy_priority));

        let candidates = self.find_eviction_candidates(tier_name, decisions);
        let needed_space = self.calculate_needed_space(&blocked_list);
        let to_evict = self.select_files_to_evict(candidates, needed_space, &blocked_list);

        self.apply_evictions(&to_evict, decisions, tier_free_space);
        self.replan_blocked_files(&blocked_list, decisions, tier_free_space);
    }

    fn find_eviction_candidates(
        &self,
        tier_name: &str,
        decisions: &[PlacementDecision],
    ) -> Vec<(usize, u32, u64)> {
        let mut candidates: Vec<(usize, u32, u64)> = decisions
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                matches!(d, PlacementDecision::Stay { .. }) && d.current_tier() == tier_name
            })
            .map(|(idx, d)| (idx, d.strategy_priority(), d.file_size()))
            .collect();

        candidates.sort_by_key(|(_, priority, size)| (*priority, std::cmp::Reverse(*size)));
        candidates
    }

    fn calculate_needed_space(&self, blocked_list: &[BlockedPlacement]) -> u64 {
        blocked_list.iter().map(|b| b.file.size).sum()
    }

    fn select_files_to_evict(
        &self,
        candidates: Vec<(usize, u32, u64)>,
        needed_space: u64,
        blocked_list: &[BlockedPlacement],
    ) -> Vec<usize> {
        let max_blocked_priority = blocked_list
            .iter()
            .map(|b| b.strategy_priority)
            .max()
            .unwrap_or(0);

        let mut freed_space = 0u64;
        let mut to_evict = Vec::new();

        for (idx, candidate_priority, file_size) in candidates {
            if freed_space >= needed_space {
                break;
            }

            if candidate_priority < max_blocked_priority {
                to_evict.push(idx);
                freed_space += file_size;
            }
        }

        to_evict
    }

    fn apply_evictions(
        &self,
        to_evict: &[usize],
        decisions: &mut [PlacementDecision],
        tier_free_space: &mut HashMap<String, u64>,
    ) {
        for &evict_idx in to_evict.iter().rev() {
            if let Some(PlacementDecision::Stay {
                file,
                current_tier,
                strategy,
                priority,
            }) = decisions.get(evict_idx).cloned()
                && let Some(fallback_tier) =
                    self.find_fallback_tier(&current_tier, tier_free_space, file.size)
            {
                tracing::debug!(
                    "Evicting {} from {} to {} (priority {} < required priority)",
                    file.path.display(),
                    current_tier,
                    fallback_tier.name,
                    priority
                );

                decisions[evict_idx] = PlacementDecision::Demote {
                    file: Arc::clone(&file),
                    from_tier: current_tier.clone(),
                    to_tier: fallback_tier.name.clone(),
                    strategy,
                    priority,
                };

                self.apply_move(
                    tier_free_space,
                    file.size,
                    &current_tier,
                    &fallback_tier.name,
                );
            }
        }
    }

    fn replan_blocked_files(
        &self,
        blocked_list: &[BlockedPlacement],
        decisions: &mut [PlacementDecision],
        tier_free_space: &mut HashMap<String, u64>,
    ) {
        for blocked in blocked_list {
            if let Some(target_tier) = self.find_tier(&blocked.desired_tier)
                && let Some(current_tier) = self.find_tier(&blocked.current_tier)
                && tier_free_space
                    .get(&target_tier.name)
                    .is_some_and(|&free| self.can_accept_file(target_tier, blocked.file.size, free))
                && let Some(decision_idx) = decisions
                    .iter()
                    .position(|d| d.file().path == blocked.file.path)
            {
                tracing::debug!(
                    "Re-planning {} to {} after eviction (priority {})",
                    blocked.file.path.display(),
                    target_tier.name,
                    blocked.strategy_priority
                );

                let decision = if target_tier.priority < current_tier.priority {
                    PlacementDecision::Promote {
                        file: Arc::clone(&blocked.file),
                        from_tier: blocked.current_tier.clone(),
                        to_tier: blocked.desired_tier.clone(),
                        strategy: blocked.strategy_name.clone(),
                        priority: blocked.strategy_priority,
                    }
                } else {
                    PlacementDecision::Demote {
                        file: Arc::clone(&blocked.file),
                        from_tier: blocked.current_tier.clone(),
                        to_tier: blocked.desired_tier.clone(),
                        strategy: blocked.strategy_name.clone(),
                        priority: blocked.strategy_priority,
                    }
                };

                self.apply_move(
                    tier_free_space,
                    blocked.file.size,
                    &blocked.current_tier,
                    &blocked.desired_tier,
                );
                decisions[decision_idx] = decision;
            }
        }
    }

    fn find_tier(&self, name: &str) -> Option<&Tier> {
        self.tiers.iter().find(|t| t.name == name)
    }

    fn find_fallback_tier(
        &self,
        current_tier: &str,
        tier_free_space: &HashMap<String, u64>,
        file_size: u64,
    ) -> Option<&Tier> {
        let current_tier_obj = self.find_tier(current_tier)?;

        self.tiers
            .iter()
            .filter(|t| t.priority > current_tier_obj.priority)
            .min_by_key(|t| t.priority)
            .filter(|tier| {
                tier_free_space
                    .get(&tier.name)
                    .is_some_and(|&free| self.can_accept_file(tier, file_size, free))
            })
    }

    fn can_accept_file(&self, tier: &Tier, file_size: u64, simulated_free: u64) -> bool {
        if simulated_free < file_size {
            return false;
        }

        if let Some(max_percent) = tier.max_usage_percent {
            let total = tier.get_total_space();
            let after_free = simulated_free.saturating_sub(file_size);
            let after_used = total - after_free;
            let after_percent = (after_used as f64 / total as f64 * 100.0) as u64;

            if after_percent > max_percent {
                return false;
            }
        }

        true
    }

    fn apply_move(
        &self,
        tier_free_space: &mut HashMap<String, u64>,
        file_size: u64,
        from_tier: &str,
        to_tier: &str,
    ) {
        if let Some(free) = tier_free_space.get_mut(from_tier) {
            *free = free.saturating_add(file_size);
        }

        if let Some(free) = tier_free_space.get_mut(to_tier) {
            *free = free.saturating_sub(file_size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileInfo;
    use std::env;
    use std::fs;

    fn create_test_tier(name: &str, priority: u32, max_usage: Option<u64>) -> Tier {
        let temp_dir = env::temp_dir().join(format!("eviction_test_{name}"));
        fs::create_dir_all(&temp_dir).unwrap();
        Tier::new(name.to_string(), temp_dir, priority, max_usage, None).unwrap()
    }

    #[test]
    fn test_eviction_frees_space_for_high_priority() {
        let cache = create_test_tier("cache", 1, Some(80));
        let storage = create_test_tier("storage", 10, None);
        let tiers = vec![cache, storage];

        let eviction_planner = EvictionPlanner::new(&tiers);

        let low_priority_file = FileInfo {
            path: std::path::PathBuf::from("/cache/low_priority.mkv"),
            size: 1000,
            modified: std::time::SystemTime::now(),
            accessed: std::time::SystemTime::now(),
        };

        let high_priority_file = FileInfo {
            path: std::path::PathBuf::from("/storage/high_priority.mkv"),
            size: 500,
            modified: std::time::SystemTime::now(),
            accessed: std::time::SystemTime::now(),
        };

        let mut decisions = vec![PlacementDecision::Stay {
            file: Arc::new(low_priority_file.clone()),
            current_tier: "cache".to_string(),
            strategy: "low_priority".to_string(),
            priority: 10,
        }];

        let blocked = vec![BlockedPlacement {
            file: Arc::new(high_priority_file.clone()),
            current_tier: "storage".to_string(),
            desired_tier: "cache".to_string(),
            strategy_name: "high_priority".to_string(),
            strategy_priority: 90,
        }];

        let mut tier_free_space = HashMap::new();
        tier_free_space.insert("cache".to_string(), 100);
        tier_free_space.insert("storage".to_string(), 10000);

        eviction_planner.evict_to_make_space(&mut decisions, blocked, &mut tier_free_space);

        let low_priority_decision = decisions
            .iter()
            .find(|d| d.file().path == low_priority_file.path);

        assert!(
            matches!(
                low_priority_decision,
                Some(PlacementDecision::Demote { .. })
            ),
            "Low priority file should be demoted"
        );
    }

    #[test]
    fn test_eviction_respects_priority() {
        let cache = create_test_tier("cache", 1, None);
        let storage = create_test_tier("storage", 10, None);
        let tiers = vec![cache, storage];

        let eviction_planner = EvictionPlanner::new(&tiers);

        let high_priority_file = FileInfo {
            path: std::path::PathBuf::from("/cache/high.mkv"),
            size: 1000,
            modified: std::time::SystemTime::now(),
            accessed: std::time::SystemTime::now(),
        };

        let mut decisions = vec![PlacementDecision::Stay {
            file: Arc::new(high_priority_file.clone()),
            current_tier: "cache".to_string(),
            strategy: "high".to_string(),
            priority: 90,
        }];

        let blocked = vec![BlockedPlacement {
            file: Arc::new(FileInfo {
                path: std::path::PathBuf::from("/storage/medium.mkv"),
                size: 500,
                modified: std::time::SystemTime::now(),
                accessed: std::time::SystemTime::now(),
            }),
            current_tier: "storage".to_string(),
            desired_tier: "cache".to_string(),
            strategy_name: "medium".to_string(),
            strategy_priority: 50,
        }];

        let mut tier_free_space = HashMap::new();
        tier_free_space.insert("cache".to_string(), 1000);
        tier_free_space.insert("storage".to_string(), 10000);

        eviction_planner.evict_to_make_space(&mut decisions, blocked, &mut tier_free_space);

        let high_priority_decision = decisions
            .iter()
            .find(|d| d.file().path == high_priority_file.path);

        assert!(
            matches!(high_priority_decision, Some(PlacementDecision::Stay { .. })),
            "High priority file should not be evicted for lower priority file"
        );
    }
}
