use super::{HistoryItem, normalize_show_name};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// User's watch progress for a show
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowProgress {
    pub user: String,
    pub show_name: String,
    pub last_watched_season: u32,
    pub last_watched_episode: u32,
    pub last_watched_global: u32,
    pub last_watched_time: u64,
}

impl ShowProgress {
    const fn new(
        user: String,
        show_name: String,
        season: u32,
        episode: u32,
        timestamp: u64,
    ) -> Self {
        let global_index = (season - 1) * 100 + episode;
        Self {
            user,
            show_name,
            last_watched_season: season,
            last_watched_episode: episode,
            last_watched_global: global_index,
            last_watched_time: timestamp,
        }
    }
}

/// Build user watch progress from history items
///
/// # Arguments
/// * `history` - History items from Tautulli API
/// * `days_back` - Only consider episodes watched in last N days
/// * `watched_threshold` - Minimum percent complete to consider "watched" (0-100)
///
/// # Returns
/// Vector of `ShowProgress`, one per (user, show) combination
pub fn build_progress(
    history: &[HistoryItem],
    days_back: u32,
    watched_threshold: u8,
) -> Vec<ShowProgress> {
    let cutoff_time = calculate_cutoff_time(days_back);

    // Group by (user, normalized_show_name) and track latest episode
    let mut progress_map: HashMap<(String, String), ShowProgress> = HashMap::new();

    for item in history {
        // Skip if show name is empty
        if item.grandparent_title.is_empty() {
            continue;
        }

        // Skip if not watched enough
        if item.percent_complete < watched_threshold {
            continue;
        }

        // Skip if too old
        if item.stopped < cutoff_time {
            continue;
        }

        // Normalize show name for comparison
        let normalized_show = normalize_show_name(&item.grandparent_title);

        // Key: (user, normalized_show_name)
        let key = (item.user.clone(), normalized_show);

        let global_index = (item.parent_media_index - 1) * 100 + item.media_index;

        // Update if this is the latest episode for this user+show
        progress_map
            .entry(key)
            .and_modify(|existing| {
                if global_index > existing.last_watched_global {
                    *existing = ShowProgress::new(
                        item.user.clone(),
                        item.grandparent_title.clone(),
                        item.parent_media_index,
                        item.media_index,
                        item.stopped,
                    );
                }
            })
            .or_insert_with(|| {
                ShowProgress::new(
                    item.user.clone(),
                    item.grandparent_title.clone(),
                    item.parent_media_index,
                    item.media_index,
                    item.stopped,
                )
            });
    }

    progress_map.into_values().collect()
}

/// Calculate Unix timestamp for N days ago
fn calculate_cutoff_time(days_back: u32) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| {
            tracing::warn!("System time is before Unix epoch, using 0");
            Duration::from_secs(0)
        })
        .as_secs();

    let seconds_back = u64::from(days_back) * 24 * 60 * 60;
    now.saturating_sub(seconds_back)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_history_item(
        user: &str,
        show: &str,
        season: u32,
        episode: u32,
        percent: u8,
        stopped: u64,
    ) -> HistoryItem {
        HistoryItem {
            user: user.to_string(),
            rating_key: "12345".to_string(),
            grandparent_title: show.to_string(),
            parent_media_index: season,
            media_index: episode,
            percent_complete: percent,
            stopped,
        }
    }

    #[test]
    fn test_build_progress_basic() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let history = vec![
            create_history_item("alice", "Breaking Bad", 1, 5, 95, now - 100),
            create_history_item("alice", "Breaking Bad", 1, 6, 98, now - 50),
            create_history_item("bob", "Breaking Bad", 2, 1, 100, now - 200),
        ];

        let progress = build_progress(&history, 1, 90);

        assert_eq!(progress.len(), 2); // alice and bob

        // Find alice's progress
        let alice_progress = progress.iter().find(|p| p.user == "alice").unwrap();
        assert_eq!(alice_progress.last_watched_season, 1);
        assert_eq!(alice_progress.last_watched_episode, 6); // Latest episode
        assert_eq!(alice_progress.last_watched_global, 6);

        // Find bob's progress
        let bob_progress = progress.iter().find(|p| p.user == "bob").unwrap();
        assert_eq!(bob_progress.last_watched_season, 2);
        assert_eq!(bob_progress.last_watched_episode, 1);
        assert_eq!(bob_progress.last_watched_global, 101); // (2-1)*100 + 1
    }

    #[test]
    fn test_build_progress_filters_unwatched() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let history = vec![
            create_history_item("alice", "Breaking Bad", 1, 5, 50, now - 100), // Too low percent
            create_history_item("alice", "Breaking Bad", 1, 6, 95, now - 50),
        ];

        let progress = build_progress(&history, 1, 90);

        assert_eq!(progress.len(), 1);
        let alice_progress = &progress[0];
        assert_eq!(alice_progress.last_watched_episode, 6); // Episode 5 was skipped
    }

    #[test]
    fn test_build_progress_filters_old() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let old_time = now - (10 * 24 * 60 * 60); // 10 days ago

        let history = vec![
            create_history_item("alice", "Breaking Bad", 1, 5, 95, old_time),
            create_history_item("alice", "Breaking Bad", 1, 6, 95, now - 100),
        ];

        let progress = build_progress(&history, 7, 90); // Only last 7 days

        assert_eq!(progress.len(), 1);
        let alice_progress = &progress[0];
        assert_eq!(alice_progress.last_watched_episode, 6); // Old episode was skipped
    }

    #[test]
    fn test_build_progress_multiple_users() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let history = vec![
            create_history_item("alice", "Breaking Bad", 1, 5, 95, now - 100),
            create_history_item("bob", "Breaking Bad", 1, 3, 98, now - 50),
            create_history_item("charlie", "Breaking Bad", 2, 10, 100, now - 200),
        ];

        let progress = build_progress(&history, 1, 90);

        assert_eq!(progress.len(), 3);

        // Each user should have their own progress
        let alice = progress.iter().find(|p| p.user == "alice").unwrap();
        assert_eq!(alice.last_watched_episode, 5);

        let bob = progress.iter().find(|p| p.user == "bob").unwrap();
        assert_eq!(bob.last_watched_episode, 3);

        let charlie = progress.iter().find(|p| p.user == "charlie").unwrap();
        assert_eq!(charlie.last_watched_episode, 10);
    }

    #[test]
    fn test_build_progress_multiple_shows() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let history = vec![
            create_history_item("alice", "Breaking Bad", 1, 5, 95, now - 100),
            create_history_item("alice", "The Office", 2, 3, 98, now - 50),
        ];

        let progress = build_progress(&history, 1, 90);

        assert_eq!(progress.len(), 2); // 2 shows

        let bb = progress
            .iter()
            .find(|p| p.show_name == "Breaking Bad")
            .unwrap();
        assert_eq!(bb.last_watched_episode, 5);

        let office = progress
            .iter()
            .find(|p| p.show_name == "The Office")
            .unwrap();
        assert_eq!(office.last_watched_episode, 3);
    }

    #[test]
    fn test_build_progress_empty_show_name() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let history = vec![create_history_item("alice", "", 1, 5, 95, now - 100)];

        let progress = build_progress(&history, 1, 90);

        assert_eq!(progress.len(), 0); // Should be filtered out
    }

    #[test]
    fn test_build_progress_cross_season() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let history = vec![
            create_history_item("alice", "Breaking Bad", 1, 5, 95, now - 300),
            create_history_item("alice", "Breaking Bad", 2, 1, 95, now - 200),
            create_history_item("alice", "Breaking Bad", 2, 3, 95, now - 100),
        ];

        let progress = build_progress(&history, 1, 90);

        assert_eq!(progress.len(), 1);
        let alice = &progress[0];
        assert_eq!(alice.last_watched_season, 2);
        assert_eq!(alice.last_watched_episode, 3);
        assert_eq!(alice.last_watched_global, 103); // (2-1)*100 + 3
    }
}
