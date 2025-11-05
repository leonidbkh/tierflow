use crate::{BalancingPlan, Mover, PlacementDecision, Tier};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    pub files_moved: usize,
    pub bytes_moved: u64,
    pub files_stayed: usize,
    pub errors: Vec<ExecutionError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionError {
    pub file: PathBuf,
    pub from_tier: String,
    pub to_tier: String,
    pub error: String,
}

pub struct Executor;

impl Executor {
    /// Выполняет план балансировки используя указанный Mover
    ///
    /// # Аргументы
    /// * `plan` - План балансировки от Balancer
    /// * `mover` - Реализация Mover trait (`DryRunMover`, `RsyncMover`, и т.д.)
    /// * `tiers` - Список tier'ов для построения полных путей
    pub fn execute_plan(
        plan: &BalancingPlan,
        mover: &dyn Mover,
        tiers: &[Tier],
    ) -> ExecutionResult {
        let tier_map: HashMap<String, &Tier> = tiers.iter().map(|t| (t.name.clone(), t)).collect();

        let mut result = ExecutionResult {
            files_moved: 0,
            bytes_moved: 0,
            files_stayed: 0,
            errors: Vec::new(),
        };

        for decision in &plan.decisions {
            match decision {
                PlacementDecision::Stay { .. } => {
                    result.files_stayed += 1;
                }
                PlacementDecision::Promote {
                    file,
                    from_tier,
                    to_tier,
                    strategy,
                    ..
                }
                | PlacementDecision::Demote {
                    file,
                    from_tier,
                    to_tier,
                    strategy,
                    ..
                } => {
                    let action = if matches!(decision, PlacementDecision::Promote { .. }) {
                        "Promoting"
                    } else {
                        "Demoting"
                    };

                    log::info!(
                        "{} file: {} (strategy: {}, {} -> {})",
                        action,
                        file.path.display(),
                        strategy,
                        from_tier,
                        to_tier
                    );

                    match Self::move_file_between_tiers(
                        &file.path, from_tier, to_tier, &tier_map, mover,
                    ) {
                        Ok(()) => {
                            result.files_moved += 1;
                            result.bytes_moved += file.size;
                        }
                        Err(e) => {
                            log::error!("Failed to move {}: {}", file.path.display(), e);
                            result.errors.push(ExecutionError {
                                file: file.path.clone(),
                                from_tier: from_tier.clone(),
                                to_tier: to_tier.clone(),
                                error: e.to_string(),
                            });
                        }
                    }
                }
            }
        }

        log::info!(
            "Execution complete: {} moved, {} stayed, {} errors",
            result.files_moved,
            result.files_stayed,
            result.errors.len()
        );

        result
    }

    /// Перемещает файл между tier'ами
    fn move_file_between_tiers(
        file_path: &Path,
        from_tier_name: &str,
        to_tier_name: &str,
        tier_map: &HashMap<String, &Tier>,
        mover: &dyn Mover,
    ) -> std::io::Result<()> {
        let from_tier = tier_map.get(from_tier_name).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Source tier not found: {from_tier_name}"),
            )
        })?;

        let to_tier = tier_map.get(to_tier_name).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Destination tier not found: {to_tier_name}"),
            )
        })?;

        // Вычисляем относительный путь от tier root
        let relative_path = file_path.strip_prefix(&from_tier.path).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "File {} is not under tier {}: {}",
                    file_path.display(),
                    from_tier_name,
                    e
                ),
            )
        })?;

        let destination_path = to_tier.path.join(relative_path);

        // Создаём директории если нужно
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Выполняем перемещение через Mover trait
        mover.move_file(file_path, &destination_path)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DryRunMover, FileInfo, PlacementDecision};
    use std::collections::HashMap;
    use std::time::SystemTime;

    fn create_test_tier(name: &str) -> Tier {
        let temp_dir = std::env::temp_dir().join(format!("executor_test_{name}"));
        std::fs::create_dir_all(&temp_dir).unwrap();
        Tier::new(name.to_string(), temp_dir, 1, None, None).unwrap()
    }

    fn create_test_file_in_tier(tier: &Tier, name: &str, size: u64) -> FileInfo {
        let path = tier.path.join(name);
        std::fs::write(&path, vec![0u8; size as usize]).unwrap();
        FileInfo {
            path,
            size,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        }
    }

    #[test]
    fn test_execute_empty_plan() {
        let plan = BalancingPlan {
            decisions: vec![],
            projected_tier_usage: HashMap::new(),
            warnings: vec![],
        };
        let mover = DryRunMover;
        let tiers = vec![];

        let result = Executor::execute_plan(&plan, &mover, &tiers);

        assert_eq!(result.files_moved, 0);
        assert_eq!(result.files_stayed, 0);
        assert_eq!(result.bytes_moved, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_execute_only_stay_decisions() {
        let cache = create_test_tier("cache");
        let file = create_test_file_in_tier(&cache, "test.mkv", 1000);

        let plan = BalancingPlan {
            decisions: vec![
                PlacementDecision::Stay {
                    file: file.clone(),
                    current_tier: "cache".to_string(),
                },
                PlacementDecision::Stay {
                    file,
                    current_tier: "cache".to_string(),
                },
            ],
            projected_tier_usage: HashMap::new(),
            warnings: vec![],
        };
        let mover = DryRunMover;
        let tiers = vec![cache];

        let result = Executor::execute_plan(&plan, &mover, &tiers);

        assert_eq!(result.files_moved, 0);
        assert_eq!(result.files_stayed, 2);
        assert_eq!(result.bytes_moved, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_execute_demote_decision() {
        let cache = create_test_tier("cache");
        let storage = create_test_tier("storage");
        let file = create_test_file_in_tier(&cache, "old.mkv", 5000);

        let plan = BalancingPlan {
            decisions: vec![PlacementDecision::Demote {
                file: file,
                from_tier: "cache".to_string(),
                to_tier: "storage".to_string(),
                strategy: "old_files".to_string(),
                priority: 10,
            }],
            projected_tier_usage: HashMap::new(),
            warnings: vec![],
        };
        let mover = DryRunMover;
        let tiers = vec![cache, storage];

        let result = Executor::execute_plan(&plan, &mover, &tiers);

        assert_eq!(result.files_moved, 1);
        assert_eq!(result.files_stayed, 0);
        assert_eq!(result.bytes_moved, 5000);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_execute_promote_decision() {
        let cache = create_test_tier("cache");
        let storage = create_test_tier("storage");
        let file = create_test_file_in_tier(&storage, "hot.mkv", 3000);

        let plan = BalancingPlan {
            decisions: vec![PlacementDecision::Promote {
                file: file,
                from_tier: "storage".to_string(),
                to_tier: "cache".to_string(),
                strategy: "hot_files".to_string(),
                priority: 20,
            }],
            projected_tier_usage: HashMap::new(),
            warnings: vec![],
        };
        let mover = DryRunMover;
        let tiers = vec![cache, storage];

        let result = Executor::execute_plan(&plan, &mover, &tiers);

        assert_eq!(result.files_moved, 1);
        assert_eq!(result.files_stayed, 0);
        assert_eq!(result.bytes_moved, 3000);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_execute_mixed_decisions() {
        let cache = create_test_tier("cache");
        let storage = create_test_tier("storage");
        let file1 = create_test_file_in_tier(&cache, "file1.mkv", 1000);
        let file2 = create_test_file_in_tier(&cache, "file2.mkv", 2000);
        let file3 = create_test_file_in_tier(&storage, "file3.mkv", 3000);

        let plan = BalancingPlan {
            decisions: vec![
                PlacementDecision::Stay {
                    file: file1,
                    current_tier: "cache".to_string(),
                },
                PlacementDecision::Demote {
                    file: file2,
                    from_tier: "cache".to_string(),
                    to_tier: "storage".to_string(),
                    strategy: "old".to_string(),
                    priority: 10,
                },
                PlacementDecision::Promote {
                    file: file3,
                    from_tier: "storage".to_string(),
                    to_tier: "cache".to_string(),
                    strategy: "hot".to_string(),
                    priority: 20,
                },
            ],
            projected_tier_usage: HashMap::new(),
            warnings: vec![],
        };
        let mover = DryRunMover;
        let tiers = vec![cache, storage];

        let result = Executor::execute_plan(&plan, &mover, &tiers);

        assert_eq!(result.files_moved, 2);
        assert_eq!(result.files_stayed, 1);
        assert_eq!(result.bytes_moved, 5000); // 2000 + 3000
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_execute_with_nested_directories() {
        let cache = create_test_tier("cache");
        let storage = create_test_tier("storage");

        // Создаём файл в поддиректории
        let subdir = cache.path.join("tv_shows/show1");
        std::fs::create_dir_all(&subdir).unwrap();
        let file_path = subdir.join("episode.mkv");
        std::fs::write(&file_path, vec![0u8; 1000]).unwrap();

        let file = FileInfo {
            path: file_path,
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };

        let plan = BalancingPlan {
            decisions: vec![PlacementDecision::Demote {
                file,
                from_tier: "cache".to_string(),
                to_tier: "storage".to_string(),
                strategy: "old".to_string(),
                priority: 10,
            }],
            projected_tier_usage: HashMap::new(),
            warnings: vec![],
        };
        let mover = DryRunMover;
        let tiers = vec![cache, storage];

        let result = Executor::execute_plan(&plan, &mover, &tiers);

        assert_eq!(result.files_moved, 1);
        assert_eq!(result.bytes_moved, 1000);
        assert!(result.errors.is_empty());
    }
}
