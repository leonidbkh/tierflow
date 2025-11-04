use crate::FileInfo;
use crate::tautulli::TautulliStats;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

/// Global statistics collected during the first pass of file processing
///
/// This allows conditions to make decisions based on information about other files,
/// directory contents, relative ages, etc.
#[derive(Debug, Clone)]
pub struct GlobalStats {
    /// File statistics
    pub file_stats: FileStats,

    /// Tautulli statistics (optional, only if Tautulli is configured)
    pub tautulli_stats: Option<TautulliStats>,
}

impl GlobalStats {
    pub const fn new(file_stats: FileStats) -> Self {
        Self {
            file_stats,
            tautulli_stats: None,
        }
    }

    pub fn with_tautulli(mut self, tautulli_stats: TautulliStats) -> Self {
        self.tautulli_stats = Some(tautulli_stats);
        self
    }
}

/// Basic file statistics collected from scanning all tiers
#[derive(Debug, Clone, Default)]
pub struct FileStats {
    /// All files grouped by their parent directory
    pub directory_files: HashMap<PathBuf, Vec<FileInfo>>,

    /// Newest (most recently modified) file per directory
    pub newest_file_per_dir: HashMap<PathBuf, SystemTime>,

    /// Oldest (least recently modified) file per directory
    pub oldest_file_per_dir: HashMap<PathBuf, SystemTime>,

    /// Total size of all files in each directory
    pub directory_total_size: HashMap<PathBuf, u64>,

    /// Number of files in each directory
    pub directory_file_count: HashMap<PathBuf, usize>,
}

impl FileStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Collect statistics from a list of files
    pub fn collect<'a, I>(files: I) -> Self
    where
        I: IntoIterator<Item = &'a FileInfo>,
    {
        let mut stats = Self::new();

        for file in files {
            stats.add_file(file);
        }

        stats
    }

    /// Add a single file to the statistics
    fn add_file(&mut self, file: &FileInfo) {
        // Get parent directory (use root if no parent)
        let parent = file
            .path
            .parent()
            .map_or_else(|| PathBuf::from("/"), std::path::Path::to_path_buf);

        // Add to directory files list
        self.directory_files
            .entry(parent.clone())
            .or_default()
            .push(file.clone());

        // Update newest file
        let should_update_newest = self
            .newest_file_per_dir
            .get(&parent)
            .is_none_or(|&newest| file.modified > newest);
        if should_update_newest {
            self.newest_file_per_dir
                .insert(parent.clone(), file.modified);
        }

        // Update oldest file
        let should_update_oldest = self
            .oldest_file_per_dir
            .get(&parent)
            .is_none_or(|&oldest| file.modified < oldest);
        if should_update_oldest {
            self.oldest_file_per_dir
                .insert(parent.clone(), file.modified);
        }

        // Update total size
        *self.directory_total_size.entry(parent.clone()).or_insert(0) += file.size;

        // Update file count
        *self.directory_file_count.entry(parent).or_insert(0) += 1;
    }

    /// Get all files in a specific directory
    pub fn get_directory_files(&self, dir: &PathBuf) -> Option<&Vec<FileInfo>> {
        self.directory_files.get(dir)
    }

    /// Get the newest file timestamp in a directory
    pub fn get_newest_in_directory(&self, dir: &PathBuf) -> Option<SystemTime> {
        self.newest_file_per_dir.get(dir).copied()
    }

    /// Get the oldest file timestamp in a directory
    pub fn get_oldest_in_directory(&self, dir: &PathBuf) -> Option<SystemTime> {
        self.oldest_file_per_dir.get(dir).copied()
    }

    /// Get total size of all files in a directory
    pub fn get_directory_size(&self, dir: &PathBuf) -> u64 {
        self.directory_total_size.get(dir).copied().unwrap_or(0)
    }

    /// Get number of files in a directory
    pub fn get_directory_file_count(&self, dir: &PathBuf) -> usize {
        self.directory_file_count.get(dir).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_file(path: &str, size: u64, hours_ago: u64) -> FileInfo {
        let modified = SystemTime::now() - Duration::from_secs(hours_ago * 3600);
        FileInfo {
            path: PathBuf::from(path),
            size,
            modified,
            accessed: SystemTime::now(),
        }
    }

    #[test]
    fn test_file_stats_collect_empty() {
        let stats = FileStats::collect(&[]);
        assert_eq!(stats.directory_files.len(), 0);
        assert_eq!(stats.newest_file_per_dir.len(), 0);
        assert_eq!(stats.oldest_file_per_dir.len(), 0);
    }

    #[test]
    fn test_file_stats_collect_single_file() {
        let file = create_test_file("/test/dir/file.mkv", 1000, 24);
        let stats = FileStats::collect(&[file]);

        let dir = PathBuf::from("/test/dir");
        assert_eq!(stats.directory_files.len(), 1);
        assert_eq!(stats.get_directory_files(&dir).unwrap().len(), 1);
        assert_eq!(stats.get_directory_size(&dir), 1000);
        assert_eq!(stats.get_directory_file_count(&dir), 1);
    }

    #[test]
    fn test_file_stats_multiple_files_same_directory() {
        let files = vec![
            create_test_file("/test/dir/file1.mkv", 1000, 48),
            create_test_file("/test/dir/file2.mkv", 2000, 24),
            create_test_file("/test/dir/file3.mkv", 3000, 72),
        ];

        let stats = FileStats::collect(&files);
        let dir = PathBuf::from("/test/dir");

        assert_eq!(stats.get_directory_files(&dir).unwrap().len(), 3);
        assert_eq!(stats.get_directory_size(&dir), 6000);
        assert_eq!(stats.get_directory_file_count(&dir), 3);

        // file2 is newest (24 hours ago)
        let newest = stats.get_newest_in_directory(&dir).unwrap();
        assert!(newest > files[0].modified);
        assert!(newest > files[2].modified);

        // file3 is oldest (72 hours ago)
        let oldest = stats.get_oldest_in_directory(&dir).unwrap();
        assert!(oldest < files[0].modified);
        assert!(oldest < files[1].modified);
    }

    #[test]
    fn test_file_stats_multiple_directories() {
        let files = vec![
            create_test_file("/test/dir1/file1.mkv", 1000, 24),
            create_test_file("/test/dir1/file2.mkv", 2000, 48),
            create_test_file("/test/dir2/file3.mkv", 3000, 12),
            create_test_file("/test/dir2/file4.mkv", 4000, 36),
        ];

        let stats = FileStats::collect(&files);

        let dir1 = PathBuf::from("/test/dir1");
        let dir2 = PathBuf::from("/test/dir2");

        assert_eq!(stats.directory_files.len(), 2);
        assert_eq!(stats.get_directory_file_count(&dir1), 2);
        assert_eq!(stats.get_directory_file_count(&dir2), 2);
        assert_eq!(stats.get_directory_size(&dir1), 3000);
        assert_eq!(stats.get_directory_size(&dir2), 7000);
    }

    #[test]
    fn test_file_stats_get_nonexistent_directory() {
        let files = vec![create_test_file("/test/dir/file.mkv", 1000, 24)];
        let stats = FileStats::collect(&files);

        let nonexistent = PathBuf::from("/nonexistent");
        assert!(stats.get_directory_files(&nonexistent).is_none());
        assert_eq!(stats.get_directory_size(&nonexistent), 0);
        assert_eq!(stats.get_directory_file_count(&nonexistent), 0);
    }

    #[test]
    fn test_global_stats_creation() {
        let file_stats = FileStats::collect(&[create_test_file("/test/file.mkv", 1000, 24)]);
        let global_stats = GlobalStats::new(file_stats.clone());

        assert_eq!(
            global_stats.file_stats.directory_files.len(),
            file_stats.directory_files.len()
        );
    }

    #[test]
    fn test_file_stats_nested_directories() {
        let files = vec![
            create_test_file("/test/a/file1.mkv", 1000, 24),
            create_test_file("/test/a/b/file2.mkv", 2000, 48),
            create_test_file("/test/a/b/c/file3.mkv", 3000, 72),
        ];

        let stats = FileStats::collect(&files);

        // Each file is in its own directory
        assert_eq!(stats.directory_files.len(), 3);

        let dir_a = PathBuf::from("/test/a");
        let dir_b = PathBuf::from("/test/a/b");
        let dir_c = PathBuf::from("/test/a/b/c");

        assert_eq!(stats.get_directory_file_count(&dir_a), 1);
        assert_eq!(stats.get_directory_file_count(&dir_b), 1);
        assert_eq!(stats.get_directory_file_count(&dir_c), 1);
    }

    #[test]
    fn test_file_stats_root_files() {
        let files = vec![
            create_test_file("/file1.mkv", 1000, 24),
            create_test_file("/file2.mkv", 2000, 48),
        ];

        let stats = FileStats::collect(&files);

        let root = PathBuf::from("/");
        assert_eq!(stats.get_directory_file_count(&root), 2);
        assert_eq!(stats.get_directory_size(&root), 3000);
    }

    #[test]
    fn test_file_stats_clone() {
        let files = vec![create_test_file("/test/file.mkv", 1000, 24)];
        let stats = FileStats::collect(&files);
        let cloned = stats.clone();

        assert_eq!(stats.directory_files.len(), cloned.directory_files.len());
        assert_eq!(stats.newest_file_per_dir, cloned.newest_file_per_dir);
    }
}
