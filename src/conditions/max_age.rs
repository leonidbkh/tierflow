use super::{Condition, Context};
use crate::FileInfo;
use std::time::{Duration, SystemTime};

/// Condition that checks file age (returns true if file is older than specified hours)
pub struct MaxAgeCondition {
    max_age_hours: u64,
}

impl MaxAgeCondition {
    pub const fn new(max_age_hours: u64) -> Self {
        Self { max_age_hours }
    }
}

impl Condition for MaxAgeCondition {
    fn matches(&self, file: &FileInfo, _context: &Context) -> bool {
        let now = SystemTime::now();
        if let Ok(duration) = now.duration_since(file.modified) {
            let max_age = Duration::from_secs(self.max_age_hours * 3600);
            duration >= max_age
        } else {
            false
        }
    }

    fn name(&self) -> &'static str {
        "max_age"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
    fn test_max_age_matches_old_file() {
        let condition = MaxAgeCondition::new(24);
        let file = create_test_file(48);
        let context = Context::new();

        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_max_age_rejects_new_file() {
        let condition = MaxAgeCondition::new(24);
        let file = create_test_file(12);
        let context = Context::new();

        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_max_age_boundary() {
        let condition = MaxAgeCondition::new(24);
        let file = create_test_file(24);
        let context = Context::new();

        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_max_age_future_file() {
        let condition = MaxAgeCondition::new(24);
        let context = Context::new();

        let file = FileInfo {
            path: PathBuf::from("/test/file.mkv"),
            size: 1000,
            modified: SystemTime::now() + Duration::from_secs(3600),
            accessed: SystemTime::now(),
        };

        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_max_age_name() {
        let condition = MaxAgeCondition::new(24);
        assert_eq!(condition.name(), "max_age");
    }
}
