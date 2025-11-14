use crate::FileInfo;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementDecision {
    Stay {
        file: Arc<FileInfo>,
        current_tier: String,
        strategy: String,
        priority: u32,
    },
    Promote {
        file: Arc<FileInfo>,
        from_tier: String,
        to_tier: String,
        strategy: String,
        priority: u32,
    },
    Demote {
        file: Arc<FileInfo>,
        from_tier: String,
        to_tier: String,
        strategy: String,
        priority: u32,
    },
}

impl PlacementDecision {
    /// Приоритет для сортировки: Demote > Promote (освобождаем место сначала)
    pub fn sort_priority(&self) -> u32 {
        match self {
            Self::Stay { .. } => 0,
            Self::Demote { priority, .. } => 1000 + priority,
            Self::Promote { priority, .. } => *priority,
        }
    }

    /// Путь к файлу (для детерминированной сортировки)
    pub fn file_path(&self) -> &PathBuf {
        match self {
            Self::Stay { file, .. } => &file.path,
            Self::Promote { file, .. } => &file.path,
            Self::Demote { file, .. } => &file.path,
        }
    }

    pub fn file_size(&self) -> u64 {
        match self {
            Self::Stay { file, .. } => file.size,
            Self::Promote { file, .. } => file.size,
            Self::Demote { file, .. } => file.size,
        }
    }

    pub fn strategy_priority(&self) -> u32 {
        match self {
            Self::Stay { priority, .. } => *priority,
            Self::Promote { priority, .. } => *priority,
            Self::Demote { priority, .. } => *priority,
        }
    }

    pub fn strategy_name(&self) -> &str {
        match self {
            Self::Stay { strategy, .. } => strategy,
            Self::Promote { strategy, .. } => strategy,
            Self::Demote { strategy, .. } => strategy,
        }
    }

    pub fn current_tier(&self) -> &str {
        match self {
            Self::Stay { current_tier, .. } => current_tier,
            Self::Promote { from_tier, .. } => from_tier,
            Self::Demote { from_tier, .. } => from_tier,
        }
    }

    pub fn file(&self) -> &Arc<FileInfo> {
        match self {
            Self::Stay { file, .. } => file,
            Self::Promote { file, .. } => file,
            Self::Demote { file, .. } => file,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn create_test_file(name: &str, size: u64) -> FileInfo {
        FileInfo {
            path: PathBuf::from(format!("/test/{name}")),
            size,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        }
    }

    #[test]
    fn test_sort_priority_stay() {
        let decision = PlacementDecision::Stay {
            file: Arc::new(create_test_file("test.mkv", 1000)),
            current_tier: "cache".to_string(),
            strategy: "test".to_string(),
            priority: 10,
        };
        assert_eq!(decision.sort_priority(), 0);
    }

    #[test]
    fn test_sort_priority_promote() {
        let decision = PlacementDecision::Promote {
            file: Arc::new(create_test_file("test.mkv", 1000)),
            from_tier: "storage".to_string(),
            to_tier: "cache".to_string(),
            strategy: "hot_files".to_string(),
            priority: 10,
        };
        assert_eq!(decision.sort_priority(), 10);
    }

    #[test]
    fn test_sort_priority_demote() {
        let decision = PlacementDecision::Demote {
            file: Arc::new(create_test_file("test.mkv", 1000)),
            from_tier: "cache".to_string(),
            to_tier: "storage".to_string(),
            strategy: "old_files".to_string(),
            priority: 10,
        };
        assert_eq!(decision.sort_priority(), 1010); // 1000 + 10
    }

    #[test]
    fn test_demote_priority_always_higher() {
        let promote = PlacementDecision::Promote {
            file: Arc::new(create_test_file("test.mkv", 1000)),
            from_tier: "storage".to_string(),
            to_tier: "cache".to_string(),
            strategy: "hot".to_string(),
            priority: 100,
        };

        let demote = PlacementDecision::Demote {
            file: Arc::new(create_test_file("test2.mkv", 1000)),
            from_tier: "cache".to_string(),
            to_tier: "storage".to_string(),
            strategy: "cold".to_string(),
            priority: 1,
        };

        // Demote с priority=1 должен быть выше чем Promote с priority=100
        assert!(demote.sort_priority() > promote.sort_priority());
    }

    #[test]
    fn test_file_path() {
        let file = Arc::new(create_test_file("test.mkv", 1000));
        let path = file.path.clone();

        let stay = PlacementDecision::Stay {
            file: Arc::clone(&file),
            current_tier: "cache".to_string(),
            strategy: "test".to_string(),
            priority: 1,
        };
        assert_eq!(stay.file_path(), &path);

        let promote = PlacementDecision::Promote {
            file: Arc::clone(&file),
            from_tier: "storage".to_string(),
            to_tier: "cache".to_string(),
            strategy: "test".to_string(),
            priority: 1,
        };
        assert_eq!(promote.file_path(), &path);

        let demote = PlacementDecision::Demote {
            file: Arc::clone(&file),
            from_tier: "cache".to_string(),
            to_tier: "storage".to_string(),
            strategy: "test".to_string(),
            priority: 1,
        };
        assert_eq!(demote.file_path(), &path);
    }

    #[test]
    fn test_file_size() {
        let file = Arc::new(create_test_file("test.mkv", 5_000_000_000)); // 5GB

        let stay = PlacementDecision::Stay {
            file: Arc::clone(&file),
            current_tier: "cache".to_string(),
            strategy: "test".to_string(),
            priority: 1,
        };
        assert_eq!(stay.file_size(), 5_000_000_000);

        let promote = PlacementDecision::Promote {
            file: Arc::clone(&file),
            from_tier: "storage".to_string(),
            to_tier: "cache".to_string(),
            strategy: "test".to_string(),
            priority: 1,
        };
        assert_eq!(promote.file_size(), 5_000_000_000);
    }

    #[test]
    fn test_decision_equality() {
        let file = Arc::new(create_test_file("test.mkv", 1000));

        let stay1 = PlacementDecision::Stay {
            file: Arc::clone(&file),
            current_tier: "cache".to_string(),
            strategy: "test".to_string(),
            priority: 1,
        };

        let stay2 = PlacementDecision::Stay {
            file: Arc::clone(&file),
            current_tier: "cache".to_string(),
            strategy: "test".to_string(),
            priority: 1,
        };

        assert_eq!(stay1, stay2);
    }

    #[test]
    fn test_decision_clone() {
        let decision = PlacementDecision::Promote {
            file: Arc::new(create_test_file("test.mkv", 1000)),
            from_tier: "storage".to_string(),
            to_tier: "cache".to_string(),
            strategy: "test".to_string(),
            priority: 10,
        };

        let cloned = decision.clone();
        assert_eq!(decision, cloned);
    }
}
