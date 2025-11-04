use super::{Condition, Context};
use crate::FileInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainsMode {
    /// Whitelist: filename MUST contain one of the patterns
    Whitelist,
    /// Blacklist: filename must NOT contain any of the patterns
    Blacklist,
}

/// Condition that checks if filename contains specified substrings
///
/// Simple alternative to regex - just checks if any of the patterns
/// appear as substrings in the filename (not the full path).
///
/// Modes:
/// - Whitelist (default): returns true if filename contains one of the patterns
/// - Blacklist: returns true if filename does NOT contain any of the patterns
///
/// Example patterns: `["sample", "trailer", ".tmp.", "RARBG"]`
pub struct FilenameContainsCondition {
    patterns: Vec<String>,
    mode: ContainsMode,
    case_sensitive: bool,
}

impl FilenameContainsCondition {
    pub const fn new(patterns: Vec<String>) -> Self {
        Self {
            patterns,
            mode: ContainsMode::Whitelist,
            case_sensitive: true,
        }
    }

    pub const fn new_with_mode(patterns: Vec<String>, mode: ContainsMode) -> Self {
        Self {
            patterns,
            mode,
            case_sensitive: true,
        }
    }

    pub const fn new_case_insensitive(patterns: Vec<String>, mode: ContainsMode) -> Self {
        Self {
            patterns,
            mode,
            case_sensitive: false,
        }
    }
}

impl Condition for FilenameContainsCondition {
    fn matches(&self, file: &FileInfo, _context: &Context) -> bool {
        // Получаем только имя файла (без пути)
        let filename = match file.path.file_name() {
            Some(name) => name.to_string_lossy(),
            None => return false,
        };

        let contains_pattern = if self.case_sensitive {
            self.patterns
                .iter()
                .any(|pattern| filename.contains(pattern.as_str()))
        } else {
            let filename_lower = filename.to_lowercase();
            self.patterns
                .iter()
                .any(|pattern| filename_lower.contains(&pattern.to_lowercase()))
        };

        match self.mode {
            ContainsMode::Whitelist => contains_pattern,
            ContainsMode::Blacklist => !contains_pattern,
        }
    }

    fn name(&self) -> &'static str {
        "filename_contains"
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
    fn test_matches_single_pattern() {
        let condition = FilenameContainsCondition::new(vec!["sample".to_string()]);
        let context = Context::new();

        let sample_file = create_test_file("/movies/Movie.2023.sample.mkv");
        assert!(condition.matches(&sample_file, &context));

        let normal_file = create_test_file("/movies/Movie.2023.mkv");
        assert!(!condition.matches(&normal_file, &context));
    }

    #[test]
    fn test_matches_multiple_patterns() {
        let condition = FilenameContainsCondition::new(vec![
            "sample".to_string(),
            "trailer".to_string(),
            "preview".to_string(),
        ]);
        let context = Context::new();

        assert!(condition.matches(&create_test_file("/Movie.sample.mkv"), &context));
        assert!(condition.matches(&create_test_file("/Movie-trailer.mp4"), &context));
        assert!(condition.matches(&create_test_file("/preview_episode.mkv"), &context));
        assert!(!condition.matches(&create_test_file("/Movie.2023.mkv"), &context));
    }

    #[test]
    fn test_blacklist_mode() {
        let condition = FilenameContainsCondition::new_with_mode(
            vec!["sample".to_string(), "trailer".to_string()],
            ContainsMode::Blacklist,
        );
        let context = Context::new();

        // Обычные файлы проходят
        assert!(condition.matches(&create_test_file("/Movie.2023.mkv"), &context));
        assert!(condition.matches(&create_test_file("/Show.S01E01.mkv"), &context));

        // Файлы с blacklisted паттернами не проходят
        assert!(!condition.matches(&create_test_file("/Movie.sample.mkv"), &context));
        assert!(!condition.matches(&create_test_file("/Movie-trailer.mp4"), &context));
    }

    #[test]
    fn test_case_sensitive() {
        let condition = FilenameContainsCondition::new(vec!["SAMPLE".to_string()]);
        let context = Context::new();

        assert!(condition.matches(&create_test_file("/Movie.SAMPLE.mkv"), &context));
        assert!(!condition.matches(&create_test_file("/Movie.sample.mkv"), &context));
    }

    #[test]
    fn test_case_insensitive() {
        let condition = FilenameContainsCondition::new_case_insensitive(
            vec!["SAMPLE".to_string()],
            ContainsMode::Whitelist,
        );
        let context = Context::new();

        assert!(condition.matches(&create_test_file("/Movie.SAMPLE.mkv"), &context));
        assert!(condition.matches(&create_test_file("/Movie.sample.mkv"), &context));
        assert!(condition.matches(&create_test_file("/Movie.Sample.mkv"), &context));
    }

    #[test]
    fn test_partial_matches() {
        let condition = FilenameContainsCondition::new(vec!["tmp".to_string()]);
        let context = Context::new();

        // Паттерн может быть частью слова/имени файла
        assert!(condition.matches(&create_test_file("/file.tmp.mkv"), &context));
        assert!(condition.matches(&create_test_file("/tmp_file.dat"), &context));
        assert!(condition.matches(&create_test_file("/file_tmp_backup.zip"), &context));

        // Не содержит "tmp"
        assert!(!condition.matches(&create_test_file("/temporary.mkv"), &context));
        assert!(!condition.matches(&create_test_file("/file.mkv"), &context));
    }

    #[test]
    fn test_special_characters() {
        let condition = FilenameContainsCondition::new(vec![".tmp.".to_string()]);
        let context = Context::new();

        assert!(condition.matches(&create_test_file("/file.tmp.processing"), &context));
        assert!(!condition.matches(&create_test_file("/temporary.mkv"), &context));
    }

    #[test]
    fn test_rarbg_pattern() {
        let condition = FilenameContainsCondition::new(vec!["RARBG".to_string()]);
        let context = Context::new();

        assert!(condition.matches(
            &create_test_file("/Movie.2023.1080p.WEB-DL.RARBG.mkv"),
            &context
        ));
        assert!(!condition.matches(&create_test_file("/Movie.2023.1080p.WEB-DL.mkv"), &context));
    }

    #[test]
    fn test_empty_patterns() {
        let condition = FilenameContainsCondition::new(vec![]);
        let context = Context::new();

        // Пустой список паттернов - ничего не матчится в whitelist
        assert!(!condition.matches(&create_test_file("/any_file.mkv"), &context));
    }

    #[test]
    fn test_empty_patterns_blacklist() {
        let condition = FilenameContainsCondition::new_with_mode(vec![], ContainsMode::Blacklist);
        let context = Context::new();

        // Пустой blacklist - все проходит
        assert!(condition.matches(&create_test_file("/any_file.mkv"), &context));
    }

    #[test]
    fn test_only_filename_not_path() {
        let condition = FilenameContainsCondition::new(vec!["sample".to_string()]);
        let context = Context::new();

        // Паттерн в пути, но не в имени файла - не должен матчиться
        let file = create_test_file("/sample_folder/Movie.2023.mkv");
        assert!(!condition.matches(&file, &context));

        // Паттерн в имени файла - должен матчиться
        let file2 = create_test_file("/movies/Movie.sample.mkv");
        assert!(condition.matches(&file2, &context));
    }

    #[test]
    fn test_cyrillic_patterns() {
        let condition = FilenameContainsCondition::new(vec!["трейлер".to_string()]);
        let context = Context::new();

        assert!(condition.matches(&create_test_file("/фильмы/Кино.трейлер.mkv"), &context));
        assert!(!condition.matches(&create_test_file("/фильмы/Кино.mkv"), &context));
    }

    #[test]
    fn test_condition_name() {
        let condition = FilenameContainsCondition::new(vec!["test".to_string()]);
        assert_eq!(condition.name(), "filename_contains");
    }

    #[test]
    fn test_blacklist_temp_files() {
        let condition = FilenameContainsCondition::new_with_mode(
            vec![
                ".tmp".to_string(),
                ".temp".to_string(),
                ".part".to_string(),
                "~".to_string(),
            ],
            ContainsMode::Blacklist,
        );
        let context = Context::new();

        // Обычные файлы проходят
        assert!(condition.matches(&create_test_file("/document.pdf"), &context));
        assert!(condition.matches(&create_test_file("/movie.mkv"), &context));

        // Временные файлы не проходят
        assert!(!condition.matches(&create_test_file("/document.tmp"), &context));
        assert!(!condition.matches(&create_test_file("/movie.mkv.part"), &context));
        assert!(!condition.matches(&create_test_file("/~document.doc"), &context));
        assert!(!condition.matches(&create_test_file("/backup.temp.zip"), &context));
    }
}
