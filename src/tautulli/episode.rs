use lazy_regex::{Regex, regex};
use std::path::Path;

// Regex patterns for episode parsing
// Compiled and validated at compile-time (similar to Python's re.compile())
// If regex is invalid, compilation will fail with clear error message

/// Episode information parsed from filename
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EpisodeInfo {
    pub show_name: String,
    pub season: u32,
    pub episode: u32,
}

impl EpisodeInfo {
    /// Calculate global episode index (for cross-season windows)
    /// Assumes 100 episodes per season for simplicity
    pub const fn global_index(&self) -> u32 {
        (self.season - 1) * 100 + self.episode
    }

    /// Create from global index (inverse of `global_index`)
    pub const fn from_global_index(show_name: String, global_idx: u32) -> Self {
        let season = (global_idx / 100) + 1;
        let episode = global_idx % 100;
        Self {
            show_name,
            season,
            episode,
        }
    }
}

/// Parse episode information from file path
///
/// Supports multiple formats:
/// - Plex: "Show Name - s01e05 - Title.mkv"
/// - Scene: "Show.Name.S01E05.1080p.mkv"
/// - Simple: "Show Name S01E05.mkv"
///
/// Returns None if no episode pattern found
pub fn parse_episode(path: &Path) -> Option<EpisodeInfo> {
    let filename = path.file_name()?.to_str()?;

    // Case-insensitive regex: S01E05 or s01e05 (compiled at compile-time)
    let re: &Regex = regex!(r"(?i)[sS](\d{1,2})[eE](\d{1,2})");

    let captures = re.captures(filename)?;

    let season: u32 = captures.get(1)?.as_str().parse().ok()?;
    let episode: u32 = captures.get(2)?.as_str().parse().ok()?;

    // Extract show name from filename (everything before season/episode)
    let show_start = captures.get(0)?.start();
    let show_name_raw = &filename[..show_start];

    // Clean up show name: remove trailing dots, dashes, spaces
    let show_name = show_name_raw
        .trim()
        .trim_end_matches(['.', '-', ' '])
        .replace('.', " ")
        .trim()
        .to_string();

    if show_name.is_empty() {
        return None;
    }

    Some(EpisodeInfo {
        show_name,
        season,
        episode,
    })
}

/// Normalize show name for comparison (lowercase, no spaces/dots/years)
pub fn normalize_show_name(name: &str) -> String {
    // Remove year patterns like (2016), [2016], or just 2016 at the end (compiled at compile-time)
    let re: &Regex = regex!(r"\s*[\(\[]?\d{4}[\)\]]?\s*$");

    let without_year = re.replace(name, "");

    // Convert to lowercase and keep only alphanumeric characters
    without_year
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_plex_format() {
        let path = PathBuf::from("/mnt/cache/Breaking Bad - s01e05 - Gray Matter.mkv");
        let episode = parse_episode(&path).expect("Should parse Plex format");

        assert_eq!(episode.show_name, "Breaking Bad");
        assert_eq!(episode.season, 1);
        assert_eq!(episode.episode, 5);
    }

    #[test]
    fn test_parse_scene_format() {
        let path = PathBuf::from("/mnt/cache/Breaking.Bad.S01E05.1080p.WEB-DL.mkv");
        let episode = parse_episode(&path).expect("Should parse Scene format");

        assert_eq!(episode.show_name, "Breaking Bad");
        assert_eq!(episode.season, 1);
        assert_eq!(episode.episode, 5);
    }

    #[test]
    fn test_parse_simple_format() {
        let path = PathBuf::from("/mnt/cache/Breaking Bad S02E13.mkv");
        let episode = parse_episode(&path).expect("Should parse simple format");

        assert_eq!(episode.show_name, "Breaking Bad");
        assert_eq!(episode.season, 2);
        assert_eq!(episode.episode, 13);
    }

    #[test]
    fn test_parse_case_insensitive() {
        let path1 = PathBuf::from("/mnt/cache/Show.s01e05.mkv");
        let path2 = PathBuf::from("/mnt/cache/Show.S01E05.mkv");

        let ep1 = parse_episode(&path1).expect("Should parse lowercase");
        let ep2 = parse_episode(&path2).expect("Should parse uppercase");

        assert_eq!(ep1.season, ep2.season);
        assert_eq!(ep1.episode, ep2.episode);
    }

    #[test]
    fn test_parse_single_digit_season() {
        let path = PathBuf::from("/mnt/cache/Show.S1E05.mkv");
        let episode = parse_episode(&path).expect("Should parse single digit season");

        assert_eq!(episode.season, 1);
        assert_eq!(episode.episode, 5);
    }

    #[test]
    fn test_parse_single_digit_episode() {
        let path = PathBuf::from("/mnt/cache/Show.S01E5.mkv");
        let episode = parse_episode(&path).expect("Should parse single digit episode");

        assert_eq!(episode.season, 1);
        assert_eq!(episode.episode, 5);
    }

    #[test]
    fn test_parse_double_digit_season() {
        let path = PathBuf::from("/mnt/cache/Show.S12E25.mkv");
        let episode = parse_episode(&path).expect("Should parse double digit season");

        assert_eq!(episode.season, 12);
        assert_eq!(episode.episode, 25);
    }

    #[test]
    fn test_parse_no_episode_pattern() {
        let path = PathBuf::from("/mnt/cache/Movie.2021.1080p.mkv");
        let episode = parse_episode(&path);

        assert!(episode.is_none(), "Should not parse movie files");
    }

    #[test]
    fn test_global_index_calculation() {
        let ep1 = EpisodeInfo {
            show_name: "Test".to_string(),
            season: 1,
            episode: 5,
        };
        assert_eq!(ep1.global_index(), 5);

        let ep2 = EpisodeInfo {
            show_name: "Test".to_string(),
            season: 2,
            episode: 10,
        };
        assert_eq!(ep2.global_index(), 110); // (2-1)*100 + 10

        let ep3 = EpisodeInfo {
            show_name: "Test".to_string(),
            season: 3,
            episode: 1,
        };
        assert_eq!(ep3.global_index(), 201); // (3-1)*100 + 1
    }

    #[test]
    fn test_from_global_index() {
        let ep = EpisodeInfo::from_global_index("Test".to_string(), 110);
        assert_eq!(ep.season, 2);
        assert_eq!(ep.episode, 10);

        let ep2 = EpisodeInfo::from_global_index("Test".to_string(), 5);
        assert_eq!(ep2.season, 1);
        assert_eq!(ep2.episode, 5);
    }

    #[test]
    fn test_normalize_show_name() {
        assert_eq!(
            normalize_show_name("Breaking Bad"),
            normalize_show_name("breaking bad")
        );

        assert_eq!(
            normalize_show_name("Breaking Bad"),
            normalize_show_name("Breaking.Bad")
        );

        assert_eq!(
            normalize_show_name("Breaking Bad"),
            normalize_show_name("BREAKING BAD")
        );

        // Year stripping
        assert_eq!(
            normalize_show_name("Stranger Things"),
            normalize_show_name("Stranger Things (2016)")
        );

        assert_eq!(
            normalize_show_name("Stranger Things"),
            normalize_show_name("Stranger Things [2016]")
        );

        assert_eq!(
            normalize_show_name("Stranger Things"),
            normalize_show_name("Stranger Things 2016")
        );

        // Country codes should be kept (for now)
        assert_eq!(normalize_show_name("The Office (US)"), "theofficeus");

        // All should match
        assert_eq!(normalize_show_name("The Wire (2002)"), "thewire");
        assert_eq!(normalize_show_name("The Wire"), "thewire");
    }

    #[test]
    fn test_cyrillic_show_names() {
        let path = PathBuf::from("/mnt/cache/Сериал - S01E05.mkv");
        let episode = parse_episode(&path).expect("Should parse Cyrillic names");

        assert_eq!(episode.show_name, "Сериал");
        assert_eq!(episode.season, 1);
        assert_eq!(episode.episode, 5);
    }
}
