use super::{Condition, Context};
use crate::FileInfo;

/// Condition that matches files within active viewing windows
///
/// This condition uses Tautulli integration to identify episodes that users
/// are currently watching. It keeps recent episodes and upcoming episodes
/// on fast storage tiers.
///
/// Requires Tautulli to be configured and `TautulliStats` to be present in `GlobalStats`.
#[derive(Debug, Clone)]
pub struct ActiveWindowCondition {
    name: String,
}

impl ActiveWindowCondition {
    pub const fn new(name: String) -> Self {
        Self { name }
    }
}

impl Condition for ActiveWindowCondition {
    fn matches(&self, file: &FileInfo, context: &Context) -> bool {
        // Get TautulliStats from GlobalStats
        // Note: If Tautulli is not available, Balancer already logged a warning in Pass 1
        let tautulli_stats = match &context.global_stats {
            Some(global_stats) => match &global_stats.tautulli_stats {
                Some(stats) => stats,
                None => return false, // Tautulli not configured or failed to load
            },
            None => return false, // GlobalStats not available
        };

        // Check if file is in active viewing window
        tautulli_stats.is_in_active_window(&file.path)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tautulli::{ShowProgress, TautulliStats};
    use crate::{FileStats, GlobalStats};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::SystemTime;

    fn create_file_info(path: &str) -> FileInfo {
        FileInfo {
            path: PathBuf::from(path),
            size: 1024,
            modified: SystemTime::now(),
            accessed: SystemTime::now(),
        }
    }

    fn create_progress(show: &str, season: u32, episode: u32) -> ShowProgress {
        let global = (season - 1) * 100 + episode;
        ShowProgress {
            user: "alice".to_string(),
            show_name: show.to_string(),
            last_watched_season: season,
            last_watched_episode: episode,
            last_watched_global: global,
            last_watched_time: 1234567890,
        }
    }

    #[test]
    fn test_active_window_matches_file_in_window() {
        let files = [
            create_file_info("/mnt/cache/Breaking.Bad.S01E05.mkv"),
            create_file_info("/mnt/cache/Breaking.Bad.S01E06.mkv"),
            create_file_info("/mnt/cache/Breaking.Bad.S01E07.mkv"),
        ];

        let progress = vec![create_progress("Breaking Bad", 1, 6)];

        let tautulli_stats = TautulliStats::build(files.iter(), progress, 1, 1);

        let mut global_stats = GlobalStats::new(FileStats::new());
        global_stats.tautulli_stats = Some(tautulli_stats);

        let context = Context::new().with_global_stats(&Arc::new(global_stats));

        let condition = ActiveWindowCondition::new("test".to_string());

        // Episode 5 is in window (6-1)
        let file = create_file_info("/mnt/cache/Breaking.Bad.S01E05.mkv");
        assert!(condition.matches(&file, &context));

        // Episode 6 is in window (currently watching)
        let file = create_file_info("/mnt/cache/Breaking.Bad.S01E06.mkv");
        assert!(condition.matches(&file, &context));

        // Episode 7 is in window (6+1)
        let file = create_file_info("/mnt/cache/Breaking.Bad.S01E07.mkv");
        assert!(condition.matches(&file, &context));
    }

    #[test]
    fn test_active_window_does_not_match_outside_window() {
        let files = [
            create_file_info("/mnt/cache/Breaking.Bad.S01E01.mkv"),
            create_file_info("/mnt/cache/Breaking.Bad.S01E06.mkv"),
            create_file_info("/mnt/cache/Breaking.Bad.S01E10.mkv"),
        ];

        let progress = vec![create_progress("Breaking Bad", 1, 6)];

        let tautulli_stats = TautulliStats::build(files.iter(), progress, 1, 1);

        let mut global_stats = GlobalStats::new(FileStats::new());
        global_stats.tautulli_stats = Some(tautulli_stats);

        let context = Context::new().with_global_stats(&Arc::new(global_stats));

        let condition = ActiveWindowCondition::new("test".to_string());

        // Episode 1 is outside window (6-1 = 5)
        let file = create_file_info("/mnt/cache/Breaking.Bad.S01E01.mkv");
        assert!(!condition.matches(&file, &context));

        // Episode 10 is outside window (6+1 = 7)
        let file = create_file_info("/mnt/cache/Breaking.Bad.S01E10.mkv");
        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_active_window_no_tautulli_stats() {
        let context =
            Context::new().with_global_stats(&Arc::new(GlobalStats::new(FileStats::new())));

        let condition = ActiveWindowCondition::new("test".to_string());

        let file = create_file_info("/mnt/cache/Breaking.Bad.S01E05.mkv");

        // Should return false when TautulliStats not present
        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_active_window_no_global_stats() {
        let context = Context::new();

        let condition = ActiveWindowCondition::new("test".to_string());

        let file = create_file_info("/mnt/cache/Breaking.Bad.S01E05.mkv");

        // Should return false when GlobalStats not present
        assert!(!condition.matches(&file, &context));
    }

    #[test]
    fn test_active_window_non_episode_file() {
        let files = [create_file_info("/mnt/cache/Movie.2021.1080p.mkv")];

        let progress = vec![create_progress("Breaking Bad", 1, 6)];

        let tautulli_stats = TautulliStats::build(files.iter(), progress, 1, 1);

        let mut global_stats = GlobalStats::new(FileStats::new());
        global_stats.tautulli_stats = Some(tautulli_stats);

        let context = Context::new().with_global_stats(&Arc::new(global_stats));

        let condition = ActiveWindowCondition::new("test".to_string());

        let file = create_file_info("/mnt/cache/Movie.2021.1080p.mkv");

        // Movies should not match active window condition
        assert!(!condition.matches(&file, &context));
    }
}
