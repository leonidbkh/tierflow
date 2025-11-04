use super::{Condition, Context};
use crate::FileInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionMode {
    /// Whitelist: file MUST have one of the extensions (default)
    Whitelist,
    /// Blacklist: file must NOT have any of the extensions
    Blacklist,
}

impl Default for ExtensionMode {
    fn default() -> Self {
        Self::Whitelist
    }
}

/// Condition that checks file extension
///
/// Modes:
/// - Whitelist (default): returns true if file has one of the extensions
/// - Blacklist: returns true if file does NOT have any of the extensions
///
/// Extensions are checked with suffixes (e.g., ".mkv.!qB" matches ".!qB")
pub struct FileExtensionCondition {
    extensions: Vec<String>,
    mode: ExtensionMode,
}

impl FileExtensionCondition {
    pub const fn new(extensions: Vec<String>) -> Self {
        Self {
            extensions,
            mode: ExtensionMode::Whitelist,
        }
    }

    pub const fn new_with_mode(extensions: Vec<String>, mode: ExtensionMode) -> Self {
        Self { extensions, mode }
    }
}

impl Condition for FileExtensionCondition {
    fn matches(&self, file: &FileInfo, _context: &Context) -> bool {
        let file_name = file.path.to_string_lossy();

        let has_extension = self.extensions.iter().any(|ext| {
            let ext_clean = ext.strip_prefix('.').unwrap_or(ext);
            file_name.ends_with(&format!(".{ext_clean}"))
        });

        match self.mode {
            ExtensionMode::Whitelist => has_extension,
            ExtensionMode::Blacklist => !has_extension,
        }
    }

    fn name(&self) -> &'static str {
        "file_extension"
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

    #[test]
    fn test_matches_single_extension() {
        let condition = FileExtensionCondition::new(vec!["mkv".to_string()]);
        let file = create_test_file("/test/movie.mkv");
        let context = Context::new();

        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_matches_with_dot_prefix() {
        let condition = FileExtensionCondition::new(vec![".mkv".to_string()]);
        let file = create_test_file("/test/movie.mkv");
        let context = Context::new();

        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_matches_multiple_extensions() {
        let condition = FileExtensionCondition::new(vec![
            "mkv".to_string(),
            "mp4".to_string(),
            "avi".to_string(),
        ]);
        let context = Context::new();

        assert!(condition.matches(&create_test_file("/test/movie.mkv"), &context));
        assert!(condition.matches(&create_test_file("/test/movie.mp4"), &context));
        assert!(condition.matches(&create_test_file("/test/movie.avi"), &context));
    }

    #[test]
    fn test_does_not_match_wrong_extension() {
        let condition = FileExtensionCondition::new(vec!["mkv".to_string()]);
        let file = create_test_file("/test/movie.mp4");
        let context = Context::new();

        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_matches_incomplete_downloads() {
        let condition = FileExtensionCondition::new(vec!["!qB".to_string()]);
        let context = Context::new();

        // qBittorrent incomplete file
        let file1 = create_test_file("/test/movie.mkv.!qB");
        assert!(condition.matches(&file1, &context));

        // Завершённый файл
        let file2 = create_test_file("/test/movie.mkv");
        assert!(!condition.matches(&file2, &context));
    }

    #[test]
    fn test_matches_partial_downloads() {
        let condition = FileExtensionCondition::new(vec![
            "!qB".to_string(),
            "part".to_string(),
            "tmp".to_string(),
        ]);
        let context = Context::new();

        assert!(condition.matches(&create_test_file("/test/file.mkv.!qB"), &context));
        assert!(condition.matches(&create_test_file("/test/file.mkv.part"), &context));
        assert!(condition.matches(&create_test_file("/test/file.tmp"), &context));
        assert!(!condition.matches(&create_test_file("/test/file.mkv"), &context));
    }

    #[test]
    fn test_no_extension() {
        let condition = FileExtensionCondition::new(vec!["mkv".to_string()]);
        let file = create_test_file("/test/movie");
        let context = Context::new();

        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_empty_extensions_list() {
        let condition = FileExtensionCondition::new(vec![]);
        let file = create_test_file("/test/movie.mkv");
        let context = Context::new();

        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_case_sensitive() {
        let condition = FileExtensionCondition::new(vec!["mkv".to_string()]);
        let context = Context::new();

        assert!(condition.matches(&create_test_file("/test/movie.mkv"), &context));
        assert!(!condition.matches(&create_test_file("/test/movie.MKV"), &context));
    }

    #[test]
    fn test_condition_name() {
        let condition = FileExtensionCondition::new(vec!["mkv".to_string()]);
        assert_eq!(condition.name(), "file_extension");
    }

    #[test]
    fn test_cyrillic_filename() {
        let condition = FileExtensionCondition::new(vec!["mkv".to_string()]);
        let file = create_test_file("/test/фильм.mkv");
        let context = Context::new();

        assert!(condition.matches(&file, &context));
    }

    // Blacklist mode tests
    #[test]
    fn test_blacklist_mode_matches_non_blacklisted() {
        let condition = FileExtensionCondition::new_with_mode(
            vec!["!qB".to_string(), "part".to_string()],
            ExtensionMode::Blacklist,
        );
        let context = Context::new();

        let mkv_file = create_test_file("/test/movie.mkv");
        assert!(condition.matches(&mkv_file, &context));
    }

    #[test]
    fn test_blacklist_mode_rejects_blacklisted() {
        let condition = FileExtensionCondition::new_with_mode(
            vec!["!qB".to_string(), "part".to_string()],
            ExtensionMode::Blacklist,
        );
        let context = Context::new();

        let incomplete_file = create_test_file("/test/movie.mkv.!qB");
        assert!(!condition.matches(&incomplete_file, &context));

        let part_file = create_test_file("/test/movie.mkv.part");
        assert!(!condition.matches(&part_file, &context));
    }

    #[test]
    fn test_blacklist_mode_multiple_extensions() {
        let condition = FileExtensionCondition::new_with_mode(
            vec!["!qB".to_string(), "part".to_string(), "tmp".to_string()],
            ExtensionMode::Blacklist,
        );
        let context = Context::new();

        assert!(condition.matches(&create_test_file("/test/movie.mkv"), &context));
        assert!(condition.matches(&create_test_file("/test/movie.mp4"), &context));
        assert!(condition.matches(&create_test_file("/test/movie.avi"), &context));

        assert!(!condition.matches(&create_test_file("/test/movie.mkv.!qB"), &context));
        assert!(!condition.matches(&create_test_file("/test/movie.mkv.part"), &context));
        assert!(!condition.matches(&create_test_file("/test/movie.tmp"), &context));
    }

    #[test]
    fn test_whitelist_mode_default() {
        let condition = FileExtensionCondition::new(vec!["mkv".to_string()]);
        let context = Context::new();

        assert!(condition.matches(&create_test_file("/test/movie.mkv"), &context));
        assert!(!condition.matches(&create_test_file("/test/movie.mp4"), &context));
    }

    #[test]
    fn test_extension_mode_default() {
        assert_eq!(ExtensionMode::default(), ExtensionMode::Whitelist);
    }
}
