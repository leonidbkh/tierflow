//! Disk operations abstraction for testing and flexibility

use std::path::Path;

/// Trait for disk space operations
///
/// Allows different implementations for production (real disk) and testing (mock)
pub trait DiskOperations: Send + Sync {
    /// Get total disk space in bytes
    fn get_total_space(&self, path: &Path) -> u64;

    /// Get free disk space in bytes
    fn get_free_space(&self, path: &Path) -> u64;
}

/// Real disk operations using fs2 and system calls
pub struct RealDisk;

impl RealDisk {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for RealDisk {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskOperations for RealDisk {
    fn get_total_space(&self, path: &Path) -> u64 {
        use fs2::statvfs;

        statvfs(path).map_or_else(
            |e| {
                tracing::warn!("Failed to get total space for {}: {}", path.display(), e);
                1 // Return 1 to avoid division by zero
            },
            |stat| stat.total_space(),
        )
    }

    fn get_free_space(&self, path: &Path) -> u64 {
        use fs2::statvfs;

        statvfs(path).map_or_else(
            |e| {
                tracing::warn!("Failed to get free space for {}: {}", path.display(), e);
                0
            },
            |stat| stat.available_space(),
        )
    }
}

/// Mock disk operations for testing
#[cfg(test)]
pub struct MockDisk {
    total: u64,
    free: u64,
}

#[cfg(test)]
impl MockDisk {
    /// Create a mock disk with specific total and free space
    pub const fn new(total: u64, free: u64) -> Self {
        Self { total, free }
    }

    /// Create a mock disk with a specific usage percentage
    ///
    /// # Arguments
    /// * `total` - Total disk space in bytes
    /// * `used_percent` - Percentage of disk space used (0-100)
    pub fn with_usage_percent(total: u64, used_percent: u8) -> Self {
        let used = (total as f64 * f64::from(used_percent) / 100.0) as u64;
        let free = total.saturating_sub(used);
        Self { total, free }
    }
}

#[cfg(test)]
impl DiskOperations for MockDisk {
    fn get_total_space(&self, _path: &Path) -> u64 {
        self.total
    }

    fn get_free_space(&self, _path: &Path) -> u64 {
        self.free
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_mock_disk_new() {
        let disk = MockDisk::new(1000, 600);
        let path = PathBuf::from("/test");

        assert_eq!(disk.get_total_space(&path), 1000);
        assert_eq!(disk.get_free_space(&path), 600);
    }

    #[test]
    fn test_mock_disk_with_usage_percent() {
        let disk = MockDisk::with_usage_percent(1000, 30);
        let path = PathBuf::from("/test");

        assert_eq!(disk.get_total_space(&path), 1000);
        assert_eq!(disk.get_free_space(&path), 700); // 70% free
    }

    #[test]
    fn test_mock_disk_full() {
        let disk = MockDisk::with_usage_percent(1000, 100);
        let path = PathBuf::from("/test");

        assert_eq!(disk.get_total_space(&path), 1000);
        assert_eq!(disk.get_free_space(&path), 0);
    }

    #[test]
    fn test_mock_disk_empty() {
        let disk = MockDisk::with_usage_percent(1000, 0);
        let path = PathBuf::from("/test");

        assert_eq!(disk.get_total_space(&path), 1000);
        assert_eq!(disk.get_free_space(&path), 1000);
    }
}
