use super::{Condition, Context};
use crate::FileInfo;
use std::time::{Duration, SystemTime};

/// Condition that checks file age
///
/// Matches files within specified age range (based on modification time).
/// Ages are specified in hours for convenience.
pub struct AgeCondition {
    min_hours: Option<u64>,
    max_hours: Option<u64>,
}

impl AgeCondition {
    pub const fn new(min_hours: Option<u64>, max_hours: Option<u64>) -> Self {
        Self {
            min_hours,
            max_hours,
        }
    }
}

impl Condition for AgeCondition {
    fn matches(&self, file: &FileInfo, _context: &Context) -> bool {
        let now = SystemTime::now();
        if let Ok(file_age) = now.duration_since(file.modified) {
            let mut matches = true;

            // Check minimum age (file must be AT LEAST this old)
            if let Some(min_h) = self.min_hours {
                let min_age = Duration::from_secs(min_h * 3600);
                matches = matches && file_age >= min_age;
            }

            // Check maximum age (file must be AT MOST this old)
            if let Some(max_h) = self.max_hours {
                let max_age = Duration::from_secs(max_h * 3600);
                matches = matches && file_age <= max_age;
            }

            matches
        } else {
            // File modified in the future - don't match
            false
        }
    }

    fn name(&self) -> &'static str {
        "age"
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
    fn test_age_min_only() {
        // Files older than 24 hours
        let condition = AgeCondition::new(Some(24), None);

        let old_file = create_test_file(48);
        assert!(condition.matches(&old_file, &Context::new()));

        let young_file = create_test_file(12);
        assert!(!condition.matches(&young_file, &Context::new()));

        // Use 25 hours to avoid timing issues at exact boundary
        let near_boundary_file = create_test_file(25);
        assert!(condition.matches(&near_boundary_file, &Context::new()));
    }

    #[test]
    fn test_age_max_only() {
        // Files younger than 24 hours
        let condition = AgeCondition::new(None, Some(24));

        let young_file = create_test_file(12);
        assert!(condition.matches(&young_file, &Context::new()));

        let old_file = create_test_file(48);
        assert!(!condition.matches(&old_file, &Context::new()));

        // Use 23 hours to avoid timing issues at exact boundary
        let near_boundary_file = create_test_file(23);
        assert!(condition.matches(&near_boundary_file, &Context::new()));
    }

    #[test]
    fn test_age_range() {
        // Files between 6 and 24 hours old
        let condition = AgeCondition::new(Some(6), Some(24));

        let too_young = create_test_file(3);
        assert!(!condition.matches(&too_young, &Context::new()));

        let in_range_low = create_test_file(7);
        assert!(condition.matches(&in_range_low, &Context::new()));

        let in_range_mid = create_test_file(12);
        assert!(condition.matches(&in_range_mid, &Context::new()));

        // Use 23 hours to avoid timing issues at exact boundary
        let in_range_high = create_test_file(23);
        assert!(condition.matches(&in_range_high, &Context::new()));

        let too_old = create_test_file(48);
        assert!(!condition.matches(&too_old, &Context::new()));
    }

    #[test]
    fn test_age_no_constraints() {
        // No min or max - matches all files
        let condition = AgeCondition::new(None, None);

        let file = create_test_file(12);
        assert!(condition.matches(&file, &Context::new()));
    }

    #[test]
    fn test_age_future_file() {
        let condition = AgeCondition::new(Some(24), None);
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
    fn test_age_name() {
        let condition = AgeCondition::new(Some(24), None);
        assert_eq!(condition.name(), "age");
    }
}
