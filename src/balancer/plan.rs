use super::PlacementDecision;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct BalancingPlan {
    pub decisions: Vec<PlacementDecision>,
    pub projected_tier_usage: HashMap<String, TierUsageProjection>,
    pub warnings: Vec<PlanWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TierUsageProjection {
    pub tier_name: String,
    pub current_used: u64,
    pub current_free: u64,
    pub projected_used: u64,
    pub projected_free: u64,
    pub current_percent: u64,
    pub projected_percent: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanWarning {
    /// Стратегия требует переместить файл, но нет места
    InsufficientSpace {
        file: PathBuf,
        strategy: String,
        needed: u64,
        available: u64,
    },

    /// Required strategy не может быть выполнена
    RequiredStrategyFailed {
        strategy: String,
        file: PathBuf,
        reason: String,
    },
}

impl BalancingPlan {
    /// Проверяет, пуст ли план (все решения - Stay)
    pub fn is_empty(&self) -> bool {
        self.decisions
            .iter()
            .all(|d| matches!(d, PlacementDecision::Stay { .. }))
    }

    /// Количество файлов для перемещения
    pub fn move_count(&self) -> usize {
        self.decisions
            .iter()
            .filter(|d| !matches!(d, PlacementDecision::Stay { .. }))
            .count()
    }

    /// Количество файлов, которые остаются на месте
    pub fn stay_count(&self) -> usize {
        self.decisions
            .iter()
            .filter(|d| matches!(d, PlacementDecision::Stay { .. }))
            .count()
    }

    /// Общее количество файлов в плане
    pub const fn total_files(&self) -> usize {
        self.decisions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileInfo;
    use std::time::SystemTime;

    fn create_test_file(name: &str) -> FileInfo {
        FileInfo {
            path: PathBuf::from(format!("/test/{name}")),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        }
    }

    #[test]
    fn test_is_empty_all_stay() {
        let plan = BalancingPlan {
            decisions: vec![
                PlacementDecision::Stay {
                    file: create_test_file("file1.mkv"),
                    current_tier: "cache".to_string(),
                },
                PlacementDecision::Stay {
                    file: create_test_file("file2.mkv"),
                    current_tier: "cache".to_string(),
                },
            ],
            projected_tier_usage: HashMap::new(),
            warnings: vec![],
        };

        assert!(plan.is_empty());
        assert_eq!(plan.move_count(), 0);
        assert_eq!(plan.stay_count(), 2);
    }

    #[test]
    fn test_is_empty_with_moves() {
        let plan = BalancingPlan {
            decisions: vec![
                PlacementDecision::Stay {
                    file: create_test_file("file1.mkv"),
                    current_tier: "cache".to_string(),
                },
                PlacementDecision::Demote {
                    file: create_test_file("file2.mkv"),
                    from_tier: "cache".to_string(),
                    to_tier: "storage".to_string(),
                    strategy: "old_files".to_string(),
                    priority: 10,
                },
            ],
            projected_tier_usage: HashMap::new(),
            warnings: vec![],
        };

        assert!(!plan.is_empty());
        assert_eq!(plan.move_count(), 1);
        assert_eq!(plan.stay_count(), 1);
    }

    #[test]
    fn test_move_count() {
        let plan = BalancingPlan {
            decisions: vec![
                PlacementDecision::Promote {
                    file: create_test_file("file1.mkv"),
                    from_tier: "storage".to_string(),
                    to_tier: "cache".to_string(),
                    strategy: "hot".to_string(),
                    priority: 10,
                },
                PlacementDecision::Demote {
                    file: create_test_file("file2.mkv"),
                    from_tier: "cache".to_string(),
                    to_tier: "storage".to_string(),
                    strategy: "cold".to_string(),
                    priority: 5,
                },
                PlacementDecision::Stay {
                    file: create_test_file("file3.mkv"),
                    current_tier: "cache".to_string(),
                },
            ],
            projected_tier_usage: HashMap::new(),
            warnings: vec![],
        };

        assert_eq!(plan.move_count(), 2);
        assert_eq!(plan.stay_count(), 1);
        assert_eq!(plan.total_files(), 3);
    }

    #[test]
    fn test_total_files() {
        let plan = BalancingPlan {
            decisions: vec![],
            projected_tier_usage: HashMap::new(),
            warnings: vec![],
        };

        assert_eq!(plan.total_files(), 0);
        assert_eq!(plan.move_count(), 0);
        assert_eq!(plan.stay_count(), 0);
        assert!(plan.is_empty());
    }

    #[test]
    fn test_tier_usage_projection() {
        let projection = TierUsageProjection {
            tier_name: "cache".to_string(),
            current_used: 500_000_000_000,   // 500GB
            current_free: 500_000_000_000,   // 500GB
            projected_used: 400_000_000_000, // 400GB
            projected_free: 600_000_000_000, // 600GB
            current_percent: 50,
            projected_percent: 40,
        };

        assert_eq!(projection.tier_name, "cache");
        assert_eq!(projection.current_percent, 50);
        assert_eq!(projection.projected_percent, 40);
    }

    #[test]
    fn test_plan_warning_insufficient_space() {
        let warning = PlanWarning::InsufficientSpace {
            file: PathBuf::from("/test/large.mkv"),
            strategy: "move_to_storage".to_string(),
            needed: 100_000_000_000,
            available: 50_000_000_000,
        };

        match warning {
            PlanWarning::InsufficientSpace {
                needed, available, ..
            } => {
                assert!(needed > available);
            }
            _ => panic!("Expected InsufficientSpace"),
        }
    }

    #[test]
    fn test_plan_warning_required_strategy_failed() {
        let warning = PlanWarning::RequiredStrategyFailed {
            strategy: "critical_files".to_string(),
            file: PathBuf::from("/test/important.dat"),
            reason: "No tier with sufficient space".to_string(),
        };

        match warning {
            PlanWarning::RequiredStrategyFailed { strategy, .. } => {
                assert_eq!(strategy, "critical_files");
            }
            _ => panic!("Expected RequiredStrategyFailed"),
        }
    }

    #[test]
    fn test_plan_with_warnings() {
        let plan = BalancingPlan {
            decisions: vec![PlacementDecision::Stay {
                file: create_test_file("file1.mkv"),
                current_tier: "cache".to_string(),
            }],
            projected_tier_usage: HashMap::new(),
            warnings: vec![
                PlanWarning::InsufficientSpace {
                    file: PathBuf::from("/test/large.mkv"),
                    strategy: "test".to_string(),
                    needed: 1000,
                    available: 500,
                },
                PlanWarning::RequiredStrategyFailed {
                    strategy: "required".to_string(),
                    file: PathBuf::from("/test/file.mkv"),
                    reason: "No space".to_string(),
                },
            ],
        };

        assert_eq!(plan.warnings.len(), 2);
    }

    #[test]
    fn test_plan_with_projected_usage() {
        let mut projected_usage = HashMap::new();
        projected_usage.insert(
            "cache".to_string(),
            TierUsageProjection {
                tier_name: "cache".to_string(),
                current_used: 500_000_000_000,
                current_free: 500_000_000_000,
                projected_used: 400_000_000_000,
                projected_free: 600_000_000_000,
                current_percent: 50,
                projected_percent: 40,
            },
        );

        let plan = BalancingPlan {
            decisions: vec![],
            projected_tier_usage: projected_usage.clone(),
            warnings: vec![],
        };

        assert_eq!(plan.projected_tier_usage.len(), 1);
        assert!(plan.projected_tier_usage.contains_key("cache"));
    }
}
