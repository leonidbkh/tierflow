mod decision;
mod plan;
mod state;

pub use decision::PlacementDecision;
pub use plan::{BalancingPlan, PlanWarning, TierUsageProjection};

use crate::{Context, FileInfo, FileStats, GlobalStats, PlacementStrategy, TautulliConfig, Tier};
use state::PlanningState;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Balancer {
    tiers: Vec<Tier>,
    strategies: Vec<PlacementStrategy>,
    tautulli_config: Option<TautulliConfig>,
}

impl Balancer {
    pub const fn new(
        tiers: Vec<Tier>,
        strategies: Vec<PlacementStrategy>,
        tautulli_config: Option<TautulliConfig>,
    ) -> Self {
        Self {
            tiers,
            strategies,
            tautulli_config,
        }
    }

    pub fn plan_rebalance(&self) -> BalancingPlan {
        let file_map = self.scan_all_tiers();

        // PASS 1: Collect statistics from all files
        log::info!("Pass 1: Collecting statistics from {} files...", file_map.len());
        let mut global_stats = self.collect_global_stats(file_map.keys());

        // Load Tautulli data if configured
        if let Some(tautulli_config) = &self.tautulli_config {
            log::info!("Loading Tautulli viewing history...");
            match self.load_tautulli_stats(file_map.keys(), tautulli_config) {
                Ok(tautulli_stats) => {
                    log::info!(
                        "Tautulli loaded: {} active episodes across {} users",
                        tautulli_stats.active_window_episodes.len(),
                        tautulli_stats.user_progress.len()
                    );
                    global_stats = global_stats.with_tautulli(tautulli_stats);
                }
                Err(e) => {
                    log::warn!("Failed to load Tautulli data: {e}. Continuing without it.");
                }
            }
        }

        let global_stats = Arc::new(global_stats);
        log::info!(
            "Statistics collected: {} directories",
            global_stats.file_stats.directory_files.len()
        );

        // PASS 2: Apply strategies with statistics
        log::info!("Pass 2: Planning file placement...");
        let mut state = PlanningState::new(&self.tiers);

        let files: Vec<_> = file_map.into_iter().collect();
        let files = self.sort_files_deterministically(files);

        let mut context = Context::new().with_global_stats(&global_stats);

        for (file, current_tier) in files {
            context.current_tier_path = Some(current_tier.path.clone());
            self.plan_file_placement(&file, current_tier, &context, &mut state);
        }

        state.decisions.sort_by(|d1, d2| {
            d2.sort_priority()
                .cmp(&d1.sort_priority())
                .then_with(|| d1.file_path().cmp(d2.file_path()))
        });

        let projected_usage = self.calculate_projected_usage(&state);

        BalancingPlan {
            decisions: state.decisions,
            projected_tier_usage: projected_usage,
            warnings: state.warnings,
        }
    }

    fn scan_all_tiers(&self) -> HashMap<FileInfo, &Tier> {
        let mut file_map = HashMap::new();
        for tier in &self.tiers {
            for file in tier.get_all_files() {
                file_map.insert(file, tier);
            }
        }
        file_map
    }

    /// Sorts files deterministically:
    /// 1. By size (larger first - more effective for freeing space)
    /// 2. By modification time (older first)
    /// 3. By path (lexicographically)
    /// 4. By tier name (for complete stability)
    fn sort_files_deterministically<'a>(
        &self,
        mut files: Vec<(FileInfo, &'a Tier)>,
    ) -> Vec<(FileInfo, &'a Tier)> {
        files.sort_by(|(f1, t1), (f2, t2)| {
            f2.size
                .cmp(&f1.size)
                .then_with(|| f1.modified.cmp(&f2.modified))
                .then_with(|| f1.path.cmp(&f2.path))
                .then_with(|| t1.name.cmp(&t2.name))
        });
        files
    }

    /// Finds strategy with highest priority (deterministic tie-breaking by name)
    fn find_matching_strategy(
        &self,
        file: &FileInfo,
        context: &Context,
    ) -> Option<&PlacementStrategy> {
        self.strategies
            .iter()
            .filter(|s| s.matches(file, context))
            .max_by(|s1, s2| {
                s1.priority
                    .cmp(&s2.priority)
                    .then_with(|| s1.name.cmp(&s2.name))
            })
    }

    /// Checks if tier can accept file considering simulated free space and `max_usage_percent`
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

    /// Finds ideal tier considering simulated free space and `max_usage_percent`
    fn find_ideal_tier_simulated(
        &self,
        strategy: &PlacementStrategy,
        file: &FileInfo,
        simulated_free_space: &HashMap<String, u64>,
    ) -> Option<&Tier> {
        strategy
            .preferred_tiers()
            .iter()
            .filter_map(|tier_name| self.tiers.iter().find(|t| &t.name == tier_name))
            .find(|tier| {
                simulated_free_space
                    .get(&tier.name)
                    .is_some_and(|&free| self.can_accept_file(tier, file.size, free))
            })
    }

    fn make_decision(
        &self,
        file: FileInfo,
        current_tier: &Tier,
        ideal_tier: &Tier,
        strategy: &PlacementStrategy,
    ) -> PlacementDecision {
        if current_tier.name == ideal_tier.name {
            PlacementDecision::Stay {
                file,
                current_tier: current_tier.name.clone(),
            }
        } else if ideal_tier.priority < current_tier.priority {
            PlacementDecision::Promote {
                file,
                from_tier: current_tier.name.clone(),
                to_tier: ideal_tier.name.clone(),
                strategy: strategy.name.clone(),
                priority: strategy.priority,
            }
        } else {
            PlacementDecision::Demote {
                file,
                from_tier: current_tier.name.clone(),
                to_tier: ideal_tier.name.clone(),
                strategy: strategy.name.clone(),
                priority: strategy.priority,
            }
        }
    }

    fn plan_file_placement(
        &self,
        file: &FileInfo,
        current_tier: &Tier,
        context: &Context,
        state: &mut PlanningState,
    ) {
        if let Some(strategy) = self.find_matching_strategy(file, context) {
            if let Some(ideal_tier) =
                self.find_ideal_tier_simulated(strategy, file, &state.tier_free_space)
            {
                let decision = self.make_decision(file.clone(), current_tier, ideal_tier, strategy);

                if !matches!(decision, PlacementDecision::Stay { .. }) {
                    state.apply_move(file.size, &current_tier.name, &ideal_tier.name);
                }

                state.decisions.push(decision);
            } else {
                state.decisions.push(PlacementDecision::Stay {
                    file: file.clone(),
                    current_tier: current_tier.name.clone(),
                });

                if strategy.is_required {
                    state.warnings.push(PlanWarning::RequiredStrategyFailed {
                        strategy: strategy.name.clone(),
                        file: file.path.clone(),
                        reason: "No tier with sufficient space".to_string(),
                    });
                }
            }
        } else {
            state.decisions.push(PlacementDecision::Stay {
                file: file.clone(),
                current_tier: current_tier.name.clone(),
            });
        }
    }

    fn calculate_projected_usage(
        &self,
        state: &PlanningState,
    ) -> HashMap<String, TierUsageProjection> {
        self.tiers
            .iter()
            .map(|tier| {
                let current_free = tier.get_free_space();
                let current_total = tier.get_total_space();
                let current_used = current_total.saturating_sub(current_free);
                let current_percent = tier.usage_percent();

                let projected_free = state
                    .tier_free_space
                    .get(&tier.name)
                    .copied()
                    .unwrap_or(current_free);
                let projected_used = current_total.saturating_sub(projected_free);
                let projected_percent = if current_total > 0 {
                    ((projected_used as f64 / current_total as f64) * 100.0) as u64
                } else {
                    0
                };

                (
                    tier.name.clone(),
                    TierUsageProjection {
                        tier_name: tier.name.clone(),
                        current_used,
                        current_free,
                        projected_used,
                        projected_free,
                        current_percent,
                        projected_percent,
                    },
                )
            })
            .collect()
    }

    /// Collect global statistics from all files (Pass 1)
    fn collect_global_stats<'a, I>(&self, files: I) -> GlobalStats
    where
        I: IntoIterator<Item = &'a FileInfo>,
    {
        let file_stats = FileStats::collect(files);
        GlobalStats::new(file_stats)
    }

    /// Load Tautulli viewing statistics (Pass 1)
    fn load_tautulli_stats<'a, I>(
        &self,
        files: I,
        config: &TautulliConfig,
    ) -> crate::Result<crate::TautulliStats>
    where
        I: IntoIterator<Item = &'a FileInfo>,
    {
        use crate::{build_progress, TautulliClient, TautulliStats};

        // Create Tautulli client
        let client = TautulliClient::new(config.url.clone(), config.api_key.clone())?;

        // Fetch viewing history
        let history = client.get_history(config.history_length)?;
        log::debug!("Fetched {} history items from Tautulli", history.len());

        // Build user watch progress
        let user_progress = build_progress(&history, config.days_back, config.watched_threshold);
        log::debug!(
            "Tracked {} show progress entries for {} unique users",
            user_progress.len(),
            user_progress
                .iter()
                .map(|p| &p.user)
                .collect::<std::collections::HashSet<_>>()
                .len()
        );

        // Build TautulliStats with viewing windows
        let tautulli_stats = TautulliStats::build(
            files,
            user_progress,
            config.backward_episodes,
            config.forward_episodes,
        );

        Ok(tautulli_stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    fn create_test_tier(name: &str, priority: u32, max_usage: Option<u64>) -> Tier {
        let temp_dir = env::temp_dir().join(format!("balancer_test_{name}"));
        fs::create_dir_all(&temp_dir).unwrap();
        Tier::new(name.to_string(), temp_dir, priority, max_usage).unwrap()
    }

    #[test]
    fn test_can_accept_file_sufficient_space() {
        let tier = create_test_tier("cache", 1, None);
        let balancer = Balancer::new(vec![tier.clone()], vec![], None);

        let free = tier.get_free_space();
        let file_size = 1024 * 1024; // 1MB

        assert!(
            balancer.can_accept_file(&tier, file_size, free),
            "Should accept file with sufficient space"
        );
    }

    #[test]
    fn test_can_accept_file_insufficient_space() {
        let tier = create_test_tier("cache", 1, None);
        let balancer = Balancer::new(vec![tier.clone()], vec![], None);

        let file_size = 1024 * 1024 * 1024; // 1GB
        let simulated_free = 100; // Only 100 bytes free

        assert!(
            !balancer.can_accept_file(&tier, file_size, simulated_free),
            "Should reject file with insufficient space"
        );
    }

    #[test]
    fn test_can_accept_file_respects_max_usage_percent() {
        let tier = create_test_tier("cache", 1, Some(50));
        let balancer = Balancer::new(vec![tier.clone()], vec![], None);

        let total = tier.get_total_space();
        let current_free = tier.get_free_space();
        let current_used = total - current_free;
        let max_allowed_used = total / 2;

        if current_used < max_allowed_used {
            let can_add = max_allowed_used - current_used;

            let small_file = can_add / 2;
            assert!(
                balancer.can_accept_file(&tier, small_file, current_free),
                "Should accept file within max_usage_percent limit"
            );

            let large_file = can_add + 1024 * 1024 * 1024;
            assert!(
                !balancer.can_accept_file(&tier, large_file, current_free),
                "Should reject file exceeding max_usage_percent limit"
            );
        }
    }

    #[test]
    fn test_can_accept_file_simulation_multiple_files() {
        let tier = create_test_tier("cache", 1, Some(80));
        let balancer = Balancer::new(vec![tier.clone()], vec![], None);

        let total = tier.get_total_space();
        let current_free = tier.get_free_space();
        let current_used = total - current_free;
        let current_percent = (current_used as f64 / total as f64 * 100.0) as u64;

        if current_percent >= 80 {
            return;
        }

        let file_size = total / 20;
        let mut simulated_free = current_free;

        for i in 1..=20 {
            let before_percent = ((total - simulated_free) as f64 / total as f64 * 100.0) as u64;
            let can_accept = balancer.can_accept_file(&tier, file_size, simulated_free);

            if can_accept {
                simulated_free = simulated_free.saturating_sub(file_size);
                let after_percent = ((total - simulated_free) as f64 / total as f64 * 100.0) as u64;

                assert!(
                    after_percent <= 80,
                    "File {i} was accepted but resulted in {after_percent}% usage (limit: 80%)"
                );
            } else {
                let would_be_free = simulated_free.saturating_sub(file_size);
                let would_be_percent = ((total - would_be_free) as f64 / total as f64 * 100.0) as u64;

                assert!(
                    would_be_percent > 80,
                    "File {i} was rejected at {before_percent}% but would only result in {would_be_percent}%"
                );

                break;
            }
        }
    }

    #[test]
    fn test_can_accept_file_no_limit() {
        let tier = create_test_tier("storage", 10, None);
        let balancer = Balancer::new(vec![tier.clone()], vec![], None);

        let free = tier.get_free_space();
        let file_size = free / 2;

        assert!(
            balancer.can_accept_file(&tier, file_size, free),
            "Should accept file when no max_usage_percent is set"
        );
    }
}
