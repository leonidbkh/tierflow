use super::{Condition, Context};
use crate::FileInfo;

/// Condition that checks file size
///
/// Matches files within specified size range.
/// Sizes are specified in megabytes for convenience.
pub struct FileSizeCondition {
    min_size_mb: Option<u64>,
    max_size_mb: Option<u64>,
}

impl FileSizeCondition {
    pub const fn new(min_size_mb: Option<u64>, max_size_mb: Option<u64>) -> Self {
        Self {
            min_size_mb,
            max_size_mb,
        }
    }

    const fn mb_to_bytes(mb: u64) -> u64 {
        mb * 1024 * 1024
    }
}

impl Condition for FileSizeCondition {
    fn matches(&self, file: &FileInfo, _context: &Context) -> bool {
        let mut matches = true;

        if let Some(min_mb) = self.min_size_mb {
            matches = matches && file.size >= Self::mb_to_bytes(min_mb);
        }

        if let Some(max_mb) = self.max_size_mb {
            matches = matches && file.size <= Self::mb_to_bytes(max_mb);
        }

        matches
    }

    fn name(&self) -> &'static str {
        "file_size"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_file(size_mb: u64) -> FileInfo {
        FileInfo {
            path: PathBuf::from("/test/file.mkv"),
            size: size_mb * 1024 * 1024,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        }
    }

    #[test]
    fn test_matches_within_range() {
        let condition = FileSizeCondition::new(Some(100), Some(1000));
        let context = Context::new();

        // 500 MB - в диапазоне
        let file = create_test_file(500);
        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_matches_exact_min() {
        let condition = FileSizeCondition::new(Some(100), Some(1000));
        let context = Context::new();

        let file = create_test_file(100);
        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_matches_exact_max() {
        let condition = FileSizeCondition::new(Some(100), Some(1000));
        let context = Context::new();

        let file = create_test_file(1000);
        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_rejects_below_min() {
        let condition = FileSizeCondition::new(Some(100), Some(1000));
        let context = Context::new();

        let file = create_test_file(50);
        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_rejects_above_max() {
        let condition = FileSizeCondition::new(Some(100), Some(1000));
        let context = Context::new();

        let file = create_test_file(2000);
        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_min_only() {
        let condition = FileSizeCondition::new(Some(100), None);
        let context = Context::new();

        // Больше минимума - ок
        assert!(condition.matches(&create_test_file(500), &context));
        assert!(condition.matches(&create_test_file(100), &context));

        // Меньше минимума - нет
        assert!(!condition.matches(&create_test_file(50), &context));
    }

    #[test]
    fn test_max_only() {
        let condition = FileSizeCondition::new(None, Some(1000));
        let context = Context::new();

        // Меньше максимума - ок
        assert!(condition.matches(&create_test_file(500), &context));
        assert!(condition.matches(&create_test_file(1000), &context));

        // Больше максимума - нет
        assert!(!condition.matches(&create_test_file(2000), &context));
    }

    #[test]
    fn test_no_limits() {
        let condition = FileSizeCondition::new(None, None);
        let context = Context::new();

        // Без ограничений - всегда true
        assert!(condition.matches(&create_test_file(0), &context));
        assert!(condition.matches(&create_test_file(1), &context));
        assert!(condition.matches(&create_test_file(1000000), &context));
    }

    #[test]
    fn test_small_files() {
        let condition = FileSizeCondition::new(None, Some(1)); // До 1 MB
        let context = Context::new();

        let small_file = FileInfo {
            path: PathBuf::from("/test/small.txt"),
            size: 1024, // 1 KB
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };

        assert!(condition.matches(&small_file, &context));
    }

    #[test]
    fn test_exact_size_bytes() {
        let condition = FileSizeCondition::new(Some(1), Some(1));
        let context = Context::new();

        let file = FileInfo {
            path: PathBuf::from("/test/file.dat"),
            size: 1024 * 1024, // Ровно 1 MB
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };

        assert!(condition.matches(&file, &context));

        let file_smaller = FileInfo {
            path: PathBuf::from("/test/file.dat"),
            size: 1024 * 1024 - 1, // На 1 байт меньше
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };

        assert!(!condition.matches(&file_smaller, &context));
    }

    #[test]
    fn test_condition_name() {
        let condition = FileSizeCondition::new(Some(100), Some(1000));
        assert_eq!(condition.name(), "file_size");
    }

    #[test]
    fn test_large_files() {
        // Проверяем большие файлы (10+ GB)
        let condition = FileSizeCondition::new(Some(10000), None); // > 10 GB
        let context = Context::new();

        let large_file = FileInfo {
            path: PathBuf::from("/test/movie-4k.mkv"),
            size: 15 * 1024 * 1024 * 1024, // 15 GB
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        };

        assert!(condition.matches(&large_file, &context));
    }
}
