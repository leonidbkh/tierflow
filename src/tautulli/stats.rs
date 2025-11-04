use super::{EpisodeInfo, ShowProgress, normalize_show_name, parse_episode};
use crate::FileInfo;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Tautulli statistics for file placement decisions
#[derive(Debug, Clone)]
pub struct TautulliStats {
    /// Episodes that should be kept in active viewing window
    /// Key: (`normalized_show_name`, season, episode)
    pub active_window_episodes: HashSet<(String, u32, u32)>,

    /// Mapping from file path to parsed episode info
    pub episode_map: HashMap<PathBuf, EpisodeInfo>,

    /// User watch progress (for debugging/logging)
    pub user_progress: Vec<ShowProgress>,
}

impl TautulliStats {
    /// Build `TautulliStats` from files and user progress
    ///
    /// # Arguments
    /// * `files` - All files to parse for episodes
    /// * `user_progress` - User watch progress from Tautulli history
    /// * `backward_episodes` - Number of episodes to keep before last watched
    /// * `forward_episodes` - Number of episodes to keep after last watched
    pub fn build<'a, I>(
        files: I,
        user_progress: Vec<ShowProgress>,
        backward_episodes: u32,
        forward_episodes: u32,
    ) -> Self
    where
        I: IntoIterator<Item = &'a FileInfo>,
    {
        // Parse episodes from file paths
        let episode_map: HashMap<PathBuf, EpisodeInfo> = files
            .into_iter()
            .filter_map(|file| {
                parse_episode(&file.path).map(|episode| (file.path.clone(), episode))
            })
            .collect();

        // Calculate active viewing windows
        let active_window_episodes =
            calculate_viewing_windows(&user_progress, backward_episodes, forward_episodes);

        Self {
            active_window_episodes,
            episode_map,
            user_progress,
        }
    }

    /// Check if file is in any active viewing window
    pub fn is_in_active_window(&self, file_path: &PathBuf) -> bool {
        if let Some(episode) = self.episode_map.get(file_path) {
            let key = (
                normalize_show_name(&episode.show_name),
                episode.season,
                episode.episode,
            );
            self.active_window_episodes.contains(&key)
        } else {
            false
        }
    }
}

/// Calculate viewing windows for all users
///
/// For each user's last watched episode, creates a window of episodes to keep.
/// Windows are merged across all users.
fn calculate_viewing_windows(
    user_progress: &[ShowProgress],
    backward_episodes: u32,
    forward_episodes: u32,
) -> HashSet<(String, u32, u32)> {
    let mut windows = HashSet::new();

    for progress in user_progress {
        let normalized_show = normalize_show_name(&progress.show_name);

        // Calculate window range using global indices
        let start_global = progress
            .last_watched_global
            .saturating_sub(backward_episodes);
        let end_global = progress.last_watched_global + forward_episodes;

        // Convert back to season/episode pairs
        for global_idx in start_global..=end_global {
            let season = (global_idx / 100) + 1;
            let episode = global_idx % 100;

            // Skip episode 0 (would happen if global_idx % 100 == 0)
            if episode == 0 {
                continue;
            }

            windows.insert((normalized_show.clone(), season, episode));
        }
    }

    windows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
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
    fn test_calculate_viewing_windows_basic() {
        let progress = vec![create_progress("Breaking Bad", 1, 5)];

        let windows = calculate_viewing_windows(&progress, 2, 3);

        // Should have episodes 3, 4, 5, 6, 7, 8 (5 - 2 to 5 + 3)
        assert_eq!(windows.len(), 6);

        let normalized_show = normalize_show_name("Breaking Bad");
        assert!(windows.contains(&(normalized_show.clone(), 1, 3)));
        assert!(windows.contains(&(normalized_show.clone(), 1, 4)));
        assert!(windows.contains(&(normalized_show.clone(), 1, 5)));
        assert!(windows.contains(&(normalized_show.clone(), 1, 6)));
        assert!(windows.contains(&(normalized_show.clone(), 1, 7)));
        assert!(windows.contains(&(normalized_show, 1, 8)));
    }

    #[test]
    fn test_calculate_viewing_windows_cross_season() {
        let progress = vec![create_progress("Breaking Bad", 2, 2)];

        let windows = calculate_viewing_windows(&progress, 5, 3);

        // Should span from S01E97 to S02E05
        let normalized_show = normalize_show_name("Breaking Bad");

        // Check S01E97, S01E98, S01E99
        assert!(windows.contains(&(normalized_show.clone(), 1, 97)));
        assert!(windows.contains(&(normalized_show.clone(), 1, 98)));
        assert!(windows.contains(&(normalized_show.clone(), 1, 99)));

        // Check S02E01, S02E02, S02E03, S02E04, S02E05
        assert!(windows.contains(&(normalized_show.clone(), 2, 1)));
        assert!(windows.contains(&(normalized_show.clone(), 2, 2)));
        assert!(windows.contains(&(normalized_show.clone(), 2, 3)));
        assert!(windows.contains(&(normalized_show.clone(), 2, 4)));
        assert!(windows.contains(&(normalized_show, 2, 5)));
    }

    #[test]
    fn test_calculate_viewing_windows_multiple_users() {
        let progress = vec![
            create_progress("Breaking Bad", 1, 5),
            create_progress("Breaking Bad", 1, 10),
        ];

        let windows = calculate_viewing_windows(&progress, 2, 2);

        let normalized_show = normalize_show_name("Breaking Bad");

        // Should have union of both windows
        // User 1: 3,4,5,6,7
        // User 2: 8,9,10,11,12
        // Union: 3-12
        assert!(windows.contains(&(normalized_show.clone(), 1, 3)));
        assert!(windows.contains(&(normalized_show.clone(), 1, 7)));
        assert!(windows.contains(&(normalized_show.clone(), 1, 8)));
        assert!(windows.contains(&(normalized_show, 1, 12)));
    }

    #[test]
    fn test_calculate_viewing_windows_different_shows() {
        let progress = vec![
            create_progress("Breaking Bad", 1, 5),
            create_progress("The Office", 2, 3),
        ];

        let windows = calculate_viewing_windows(&progress, 1, 1);

        assert_eq!(windows.len(), 6); // 3 episodes per show

        let bb = normalize_show_name("Breaking Bad");
        let office = normalize_show_name("The Office");

        assert!(windows.contains(&(bb.clone(), 1, 4)));
        assert!(windows.contains(&(bb.clone(), 1, 5)));
        assert!(windows.contains(&(bb, 1, 6)));

        assert!(windows.contains(&(office.clone(), 2, 2)));
        assert!(windows.contains(&(office.clone(), 2, 3)));
        assert!(windows.contains(&(office, 2, 4)));
    }

    #[test]
    fn test_tautulli_stats_build() {
        let files = [
            create_file_info("/mnt/cache/Breaking.Bad.S01E05.mkv"),
            create_file_info("/mnt/cache/Breaking.Bad.S01E06.mkv"),
            create_file_info("/mnt/cache/Breaking.Bad.S01E10.mkv"),
        ];

        let progress = vec![create_progress("Breaking Bad", 1, 6)];

        let stats = TautulliStats::build(files.iter(), progress, 1, 1);

        // Should have 3 files in episode_map
        assert_eq!(stats.episode_map.len(), 3);

        // Episode 5,6,7 should be in active window
        assert_eq!(stats.active_window_episodes.len(), 3);

        let normalized_show = normalize_show_name("Breaking Bad");
        assert!(
            stats
                .active_window_episodes
                .contains(&(normalized_show.clone(), 1, 5))
        );
        assert!(
            stats
                .active_window_episodes
                .contains(&(normalized_show.clone(), 1, 6))
        );
        assert!(
            stats
                .active_window_episodes
                .contains(&(normalized_show, 1, 7))
        );
    }

    #[test]
    fn test_is_in_active_window() {
        let files = [
            create_file_info("/mnt/cache/Breaking.Bad.S01E05.mkv"),
            create_file_info("/mnt/cache/Breaking.Bad.S01E10.mkv"),
        ];

        let progress = vec![create_progress("Breaking Bad", 1, 6)];

        let stats = TautulliStats::build(files.iter(), progress, 1, 1);

        // Episode 5 should be in window (6-1)
        assert!(stats.is_in_active_window(&PathBuf::from("/mnt/cache/Breaking.Bad.S01E05.mkv")));

        // Episode 10 should NOT be in window (6+1 = 7, but 10 > 7)
        assert!(!stats.is_in_active_window(&PathBuf::from("/mnt/cache/Breaking.Bad.S01E10.mkv")));
    }

    #[test]
    fn test_is_in_active_window_not_episode() {
        let files = [create_file_info("/mnt/cache/Movie.2021.1080p.mkv")];

        let progress = vec![create_progress("Breaking Bad", 1, 6)];

        let stats = TautulliStats::build(files.iter(), progress, 1, 1);

        // Movie files should not be in active window
        assert!(!stats.is_in_active_window(&PathBuf::from("/mnt/cache/Movie.2021.1080p.mkv")));
    }
}
