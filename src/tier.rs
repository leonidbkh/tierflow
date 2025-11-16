use crate::file::FileInfo;
use fs2::statvfs;
use std::io;
use std::path::PathBuf;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Tier {
    pub name: String,
    pub path: PathBuf,
    pub priority: u32,
    pub max_usage_percent: Option<u64>,
    pub min_usage_percent: Option<u64>,
}

impl Tier {
    pub fn new(
        name: String,
        path: PathBuf,
        priority: u32,
        max_usage_percent: Option<u64>,
        min_usage_percent: Option<u64>,
    ) -> io::Result<Self> {
        if !path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Path does not exist: {}", path.display()),
            ));
        }
        if !path.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Path is not a directory: {}", path.display()),
            ));
        }

        if let Some(max) = max_usage_percent {
            if max == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("max_usage_percent must be at least 1%, got {max}"),
                ));
            }
            if max > 100 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("max_usage_percent must be <= 100, got {max}"),
                ));
            }
        }

        if let Some(min) = min_usage_percent
            && min > 100
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("min_usage_percent must be <= 100, got {min}"),
            ));
        }

        // Validate min < max if both set
        if let (Some(min), Some(max)) = (min_usage_percent, max_usage_percent)
            && min >= max
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("min_usage_percent ({min}) must be less than max_usage_percent ({max})"),
            ));
        }

        Ok(Self {
            name,
            path,
            priority,
            max_usage_percent,
            min_usage_percent,
        })
    }

    pub fn get_free_space(&self) -> u64 {
        statvfs(&self.path)
            .map(|stat| stat.available_space())
            .unwrap_or(0)
    }

    pub fn get_total_space(&self) -> u64 {
        statvfs(&self.path)
            .map(|stat| stat.total_space())
            .unwrap_or(1)
    }

    pub fn usage_percent(&self) -> u64 {
        let total = self.get_total_space();
        let free = self.get_free_space();
        if total == 0 {
            return 0;
        }
        ((total - free) as f64 / total as f64 * 100.0) as u64
    }

    /// Check if files can be demoted from this tier
    /// Returns false if tier usage is below `min_usage_percent` threshold
    pub fn can_demote(&self) -> bool {
        if let Some(min) = self.min_usage_percent {
            self.usage_percent() >= min
        } else {
            true
        }
    }

    pub fn has_space_for(&self, size: u64) -> bool {
        if self.get_free_space() < size {
            return false;
        }

        if let Some(max_percent) = self.max_usage_percent {
            let total = self.get_total_space();
            let current_used = total - self.get_free_space();
            let after_used = current_used + size;
            let after_percent = (after_used as f64 / total as f64 * 100.0) as u64;

            if after_percent > max_percent {
                return false;
            }
        }

        true
    }

    pub fn get_all_files(&self) -> Vec<FileInfo> {
        WalkDir::new(&self.path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| match e {
                Ok(entry) => Some(entry),
                Err(err) => {
                    // Log the error but continue processing other files
                    tracing::warn!(
                        "Failed to read directory entry in tier '{}': {}",
                        self.name,
                        err
                    );
                    None
                }
            })
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                // Skip lock file
                if let Some(name) = e.file_name().to_str() {
                    name != ".tierflow.lock"
                } else {
                    true
                }
            })
            .filter_map(|e| {
                let path = e.path().to_path_buf();
                match FileInfo::from_path(path.clone()) {
                    Ok(info) => Some(info),
                    Err(err) => {
                        tracing::warn!(
                            "Failed to get file info for '{}' in tier '{}': {}",
                            path.display(),
                            self.name,
                            err
                        );
                        None
                    }
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn test_tier_creation_valid_path() {
        let temp_dir = env::temp_dir();
        let tier = Tier::new("test-tier".to_string(), temp_dir.clone(), 1, None, None);
        assert!(tier.is_ok(), "Tier should be created successfully");

        let tier = tier.unwrap();
        assert_eq!(tier.name, "test-tier");
        assert_eq!(tier.path, temp_dir);
        assert_eq!(tier.priority, 1);
        assert_eq!(tier.max_usage_percent, None);
    }

    #[test]
    fn test_tier_creation_invalid_path() {
        let nonexistent = PathBuf::from("/nonexistent/path/that/does/not/exist");
        let result = Tier::new("test".to_string(), nonexistent, 1, None, None);

        assert!(result.is_err(), "Should return error for nonexistent path");
        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn test_tier_creation_file_not_directory() {
        let temp_dir = env::temp_dir();
        let temp_file = temp_dir.join("test_file.txt");
        fs::write(&temp_file, b"test").unwrap();

        let result = Tier::new("test".to_string(), temp_file.clone(), 1, None, None);
        assert!(result.is_err(), "Should return error when path is a file");

        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);

        fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_tier_disk_space_methods() {
        let temp_dir = env::temp_dir();
        let tier = Tier::new("test".to_string(), temp_dir, 1, None, None).unwrap();

        let total = tier.get_total_space();
        let free = tier.get_free_space();
        let usage = tier.usage_percent();

        assert!(total > 0, "Total space should be > 0");
        assert!(free > 0, "Free space should be > 0");
        assert!(free <= total, "Free space should be <= total space");
        assert!(usage <= 100, "Usage percent should be <= 100");

        println!("Total: {total} bytes");
        println!("Free: {free} bytes");
        println!("Usage: {usage}%");
    }

    #[test]
    fn test_tier_has_space_for() {
        let temp_dir = env::temp_dir();
        let tier = Tier::new("test".to_string(), temp_dir, 1, None, None).unwrap();

        assert!(tier.has_space_for(1024), "Should have space for 1KB");

        let huge_size = u64::MAX;
        assert!(
            !tier.has_space_for(huge_size),
            "Should not have space for u64::MAX bytes"
        );
    }

    #[test]
    fn test_tier_get_all_files_empty() {
        let temp_dir = env::temp_dir().join("test_tier_empty");
        fs::create_dir_all(&temp_dir).unwrap();

        let tier = Tier::new("test".to_string(), temp_dir.clone(), 1, None, None).unwrap();
        let files = tier.get_all_files();

        assert_eq!(files.len(), 0, "Empty directory should have 0 files");

        fs::remove_dir_all(temp_dir).ok();
    }

    #[test]
    fn test_tier_get_all_files_with_files() {
        let temp_dir = env::temp_dir().join("test_tier_files");
        fs::create_dir_all(&temp_dir).unwrap();

        fs::write(temp_dir.join("file1.txt"), b"content1").unwrap();
        fs::write(temp_dir.join("file2.txt"), b"content2").unwrap();

        let subdir = temp_dir.join("subdir");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("file3.txt"), b"content3").unwrap();

        let tier = Tier::new("test".to_string(), temp_dir.clone(), 1, None, None).unwrap();
        let files = tier.get_all_files();

        assert_eq!(files.len(), 3, "Should find 3 files");

        fs::remove_dir_all(temp_dir).ok();
    }

    #[test]
    fn test_tier_clone() {
        let temp_dir = env::temp_dir();
        let tier1 = Tier::new("original".to_string(), temp_dir, 1, Some(85), None).unwrap();

        let tier2 = tier1.clone();

        assert_eq!(tier1.name, tier2.name);
        assert_eq!(tier1.path, tier2.path);
        assert_eq!(tier1.priority, tier2.priority);
        assert_eq!(tier1.max_usage_percent, tier2.max_usage_percent);
    }

    #[test]
    fn test_tier_max_usage_percent_validation() {
        let temp_dir = env::temp_dir();

        // Test max > 100 - should fail
        let result = Tier::new("test".to_string(), temp_dir.clone(), 1, Some(101), None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);

        // Test max = 0 - should fail (new validation)
        let result = Tier::new("test".to_string(), temp_dir.clone(), 1, Some(0), None);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);

        // Test valid values - should succeed
        let result = Tier::new("test".to_string(), temp_dir.clone(), 1, Some(1), None);
        assert!(result.is_ok());

        let result = Tier::new("test".to_string(), temp_dir.clone(), 1, Some(50), None);
        assert!(result.is_ok());

        let result = Tier::new("test".to_string(), temp_dir, 1, Some(100), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tier_has_space_for_with_max_usage() {
        let temp_dir = env::temp_dir();
        let tier = Tier::new("test".to_string(), temp_dir, 1, Some(50), None).unwrap();

        let total = tier.get_total_space();
        let current_free = tier.get_free_space();
        let current_used = total - current_free;
        let max_allowed_used = total / 2;

        // Test behavior based on current disk state
        if current_used < max_allowed_used {
            // Disk has room - test that we can add within limit but not exceed it
            let can_add = max_allowed_used - current_used;

            let small_file = can_add / 2;
            assert!(
                tier.has_space_for(small_file),
                "Should have space for file within limit"
            );

            // Large file should exceed max_usage even if there's physical space
            // Use total size to ensure it definitely exceeds the 50% limit
            let large_file = (total / 2) + 1024;
            assert!(
                !tier.has_space_for(large_file),
                "Should not have space for file exceeding limit"
            );
        } else {
            // Disk already over limit - should reject any file
            assert!(
                !tier.has_space_for(1024),
                "Should not have space when already over max_usage_percent"
            );
        }
    }
}
