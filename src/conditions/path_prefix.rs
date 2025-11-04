use super::{Condition, Context};
use crate::FileInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixMode {
    /// Whitelist: file MUST have the prefix (default)
    Whitelist,
    /// Blacklist: file must NOT have the prefix
    Blacklist,
}

impl Default for PrefixMode {
    fn default() -> Self {
        Self::Whitelist
    }
}

/// Condition that checks file path prefix relative to tier root
///
/// Modes:
/// - Whitelist (default): returns true if file has the prefix
/// - Blacklist: returns true if file does NOT have the prefix
///
/// Example:
/// - Tier path: `/mnt/cache`
/// - File path: `/mnt/cache/downloads/movie.mkv`
/// - Relative path: `downloads/movie.mkv`
/// - Prefix: `downloads` → matches ✓ (whitelist) / ✗ (blacklist)
///
/// Used to create rules for different subdirectories within a tier
pub struct PathPrefixCondition {
    prefix: String,
    mode: PrefixMode,
}

impl PathPrefixCondition {
    pub const fn new(prefix: String) -> Self {
        Self {
            prefix,
            mode: PrefixMode::Whitelist,
        }
    }

    pub const fn new_with_mode(prefix: String, mode: PrefixMode) -> Self {
        Self { prefix, mode }
    }
}

impl Condition for PathPrefixCondition {
    fn matches(&self, file: &FileInfo, context: &Context) -> bool {
        let tier_path = if let Some(path) = &context.current_tier_path { path } else {
            log::warn!(
                "PathPrefixCondition requires current_tier_path in context, but it's None"
            );
            return false;
        };

        let relative_path = if let Ok(rel) = file.path.strip_prefix(tier_path) { rel } else {
            log::warn!(
                "File {} is not under tier {}",
                file.path.display(),
                tier_path.display()
            );
            return false;
        };

        let relative_str = relative_path.to_string_lossy();
        let prefix_normalized = self.prefix.trim_end_matches('/');

        if prefix_normalized.is_empty() {
            return true;
        }

        // Ensure exact directory match (e.g., "down" doesn't match "downloads")
        let has_prefix = if let Some(after_prefix) = relative_str.strip_prefix(prefix_normalized) {
            after_prefix.is_empty() || after_prefix.starts_with('/')
        } else {
            false
        };

        match self.mode {
            PrefixMode::Whitelist => has_prefix,
            PrefixMode::Blacklist => !has_prefix,
        }
    }

    fn name(&self) -> &'static str {
        "path_prefix"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    fn create_test_file(path: &str) -> FileInfo {
        FileInfo {
            path: PathBuf::from(path),
            size: 1000,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        }
    }

    fn create_context(tier_path: &str) -> Context {
        Context::new().with_tier_path(PathBuf::from(tier_path))
    }

    #[test]
    fn test_matches_direct_subfolder() {
        let condition = PathPrefixCondition::new("downloads".to_string());
        let file = create_test_file("/mnt/cache/downloads/movie.mkv");
        let context = create_context("/mnt/cache");

        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_matches_nested_path() {
        let condition = PathPrefixCondition::new("downloads".to_string());
        let file = create_test_file("/mnt/cache/downloads/series/show/s01e01.mkv");
        let context = create_context("/mnt/cache");

        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_does_not_match_different_folder() {
        let condition = PathPrefixCondition::new("downloads".to_string());
        let file = create_test_file("/mnt/cache/series_lib/show/s01e01.mkv");
        let context = create_context("/mnt/cache");

        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_matches_with_trailing_slash() {
        let condition = PathPrefixCondition::new("downloads/".to_string());
        let file = create_test_file("/mnt/cache/downloads/movie.mkv");
        let context = create_context("/mnt/cache");

        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_does_not_partial_match() {
        // "down" не должен матчиться на "downloads"
        let condition = PathPrefixCondition::new("down".to_string());
        let file = create_test_file("/mnt/cache/downloads/movie.mkv");
        let context = create_context("/mnt/cache");

        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_matches_exact_folder_name() {
        let condition = PathPrefixCondition::new("downloads".to_string());
        let file = create_test_file("/mnt/cache/downloads_old/movie.mkv");
        let context = create_context("/mnt/cache");

        // "downloads" не должен матчиться на "downloads_old"
        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_matches_multilevel_prefix() {
        let condition = PathPrefixCondition::new("media/movies".to_string());
        let file = create_test_file("/mnt/storage/media/movies/action/film.mkv");
        let context = create_context("/mnt/storage");

        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_does_not_match_partial_multilevel() {
        let condition = PathPrefixCondition::new("media/movies".to_string());
        let file = create_test_file("/mnt/storage/media/series/show.mkv");
        let context = create_context("/mnt/storage");

        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_no_context_tier_path() {
        let condition = PathPrefixCondition::new("downloads".to_string());
        let file = create_test_file("/mnt/cache/downloads/movie.mkv");
        let context = Context::new(); // Без tier_path

        // Должен вернуть false если нет tier_path в context
        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_file_not_under_tier() {
        let condition = PathPrefixCondition::new("downloads".to_string());
        let file = create_test_file("/mnt/storage/downloads/movie.mkv");
        let context = create_context("/mnt/cache"); // Другой tier

        // Файл не под указанным tier'ом
        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_cyrillic_path() {
        let condition = PathPrefixCondition::new("загрузки".to_string());
        let file = create_test_file("/mnt/cache/загрузки/фильм.mkv");
        let context = create_context("/mnt/cache");

        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_condition_name() {
        let condition = PathPrefixCondition::new("downloads".to_string());
        assert_eq!(condition.name(), "path_prefix");
    }

    #[test]
    fn test_root_file() {
        let condition = PathPrefixCondition::new("downloads".to_string());
        let file = create_test_file("/mnt/cache/file.mkv"); // Файл в корне tier
        let context = create_context("/mnt/cache");

        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_empty_prefix() {
        let condition = PathPrefixCondition::new(String::new());
        let file = create_test_file("/mnt/cache/downloads/movie.mkv");
        let context = create_context("/mnt/cache");

        // Пустой prefix должен матчить все файлы
        assert!(condition.matches(&file, &context));
    }

    // Blacklist mode tests
    #[test]
    fn test_blacklist_mode_matches_non_prefixed() {
        let condition =
            PathPrefixCondition::new_with_mode("downloads".to_string(), PrefixMode::Blacklist);
        let context = create_context("/mnt/cache");

        // Файл НЕ в downloads → должен матчиться
        let series_file = create_test_file("/mnt/cache/series_lib/show.mkv");
        assert!(condition.matches(&series_file, &context));
    }

    #[test]
    fn test_blacklist_mode_rejects_prefixed() {
        let condition =
            PathPrefixCondition::new_with_mode("downloads".to_string(), PrefixMode::Blacklist);
        let context = create_context("/mnt/cache");

        // Файл в downloads → НЕ должен матчиться
        let downloads_file = create_test_file("/mnt/cache/downloads/movie.mkv");
        assert!(!condition.matches(&downloads_file, &context));
    }

    #[test]
    fn test_blacklist_mode_multiple_prefixes() {
        let condition1 =
            PathPrefixCondition::new_with_mode("downloads".to_string(), PrefixMode::Blacklist);
        let condition2 =
            PathPrefixCondition::new_with_mode("series_lib".to_string(), PrefixMode::Blacklist);
        let context = create_context("/mnt/cache");

        // Файл в movies (не в blacklist) → оба условия матчатся
        let movies_file = create_test_file("/mnt/cache/movies/film.mkv");
        assert!(condition1.matches(&movies_file, &context));
        assert!(condition2.matches(&movies_file, &context));

        // Файл в downloads (в blacklist для condition1)
        let downloads_file = create_test_file("/mnt/cache/downloads/movie.mkv");
        assert!(!condition1.matches(&downloads_file, &context));
        assert!(condition2.matches(&downloads_file, &context));

        // Файл в series_lib (в blacklist для condition2)
        let series_file = create_test_file("/mnt/cache/series_lib/show.mkv");
        assert!(condition1.matches(&series_file, &context));
        assert!(!condition2.matches(&series_file, &context));
    }

    #[test]
    fn test_blacklist_mode_root_file() {
        let condition =
            PathPrefixCondition::new_with_mode("downloads".to_string(), PrefixMode::Blacklist);
        let file = create_test_file("/mnt/cache/file.mkv"); // Файл в корне tier
        let context = create_context("/mnt/cache");

        // Файл в корне НЕ в downloads → должен матчиться
        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_whitelist_mode_default() {
        // По умолчанию должен быть whitelist mode
        let condition = PathPrefixCondition::new("downloads".to_string());
        let context = create_context("/mnt/cache");

        // Файл в downloads → матчится
        assert!(condition.matches(
            &create_test_file("/mnt/cache/downloads/movie.mkv"),
            &context
        ));

        // Файл НЕ в downloads → НЕ матчится
        assert!(!condition.matches(&create_test_file("/mnt/cache/series/show.mkv"), &context));
    }

    #[test]
    fn test_prefix_mode_default() {
        assert_eq!(PrefixMode::default(), PrefixMode::Whitelist);
    }

    #[test]
    fn test_blacklist_mode_cyrillic() {
        let condition =
            PathPrefixCondition::new_with_mode("загрузки".to_string(), PrefixMode::Blacklist);
        let context = create_context("/mnt/cache");

        // Файл НЕ в загрузки → матчится
        assert!(condition.matches(&create_test_file("/mnt/cache/фильмы/кино.mkv"), &context));

        // Файл в загрузки → НЕ матчится
        assert!(!condition.matches(&create_test_file("/mnt/cache/загрузки/файл.mkv"), &context));
    }
}
