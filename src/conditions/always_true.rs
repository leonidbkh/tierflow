use super::{Condition, Context};
use crate::FileInfo;

/// Condition that always returns true (for default strategies that apply to all files)
pub struct AlwaysTrueCondition;

impl Condition for AlwaysTrueCondition {
    fn matches(&self, _file: &FileInfo, _context: &Context) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "always_true"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_file() -> FileInfo {
        FileInfo {
            path: PathBuf::from("/test/file.mkv"),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        }
    }

    #[test]
    fn test_always_true_matches() {
        let condition = AlwaysTrueCondition;
        let file = create_test_file();
        let context = Context::new();

        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_always_true_name() {
        let condition = AlwaysTrueCondition;
        assert_eq!(condition.name(), "always_true");
    }

    #[test]
    fn test_always_true_any_file() {
        let condition = AlwaysTrueCondition;
        let context = Context::new();

        for size in [0, 1, 1000, u64::MAX] {
            let file = FileInfo {
                path: PathBuf::from("/test/file.mkv"),
                size,
                modified: SystemTime::now(),
                accessed: SystemTime::now(),
            };
            assert!(condition.matches(&file, &context));
        }
    }
}
