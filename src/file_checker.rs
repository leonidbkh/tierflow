//! File usage checking implementations

use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

/// Trait for checking if a file is currently in use by any process
pub trait FileChecker: Send + Sync {
    /// Check if the file at the given path is currently open/in use
    fn is_file_in_use(&self, path: &Path) -> io::Result<bool>;
}

/// Implementation that always returns false (for testing/dry-run)
pub struct NoOpFileChecker;

impl FileChecker for NoOpFileChecker {
    fn is_file_in_use(&self, _path: &Path) -> io::Result<bool> {
        Ok(false)
    }
}

/// Implementation using lsof command (most reliable on Unix systems)
pub struct LsofFileChecker;

impl FileChecker for LsofFileChecker {
    fn is_file_in_use(&self, path: &Path) -> io::Result<bool> {
        // lsof -t returns PIDs of processes using the file
        // Exit code 0 = file is open, 1 = file not open
        match Command::new("lsof")
            .arg("-t")
            .arg(path.as_os_str())
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    tracing::debug!("File {} is in use (lsof found processes)", path.display());
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                tracing::warn!(
                    "lsof command not found. Cannot verify if files are in use. \
                     Consider installing lsof package for safer file operations."
                );
                // Fallback to false to not block operations
                Ok(false)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to check if file {} is in use: {}",
                    path.display(),
                    e
                );
                // Conservative: assume not in use to not block operations
                Ok(false)
            }
        }
    }
}

/// Implementation using file locking (cross-platform but less reliable)
pub struct FileLockChecker;

impl FileChecker for FileLockChecker {
    fn is_file_in_use(&self, path: &Path) -> io::Result<bool> {
        use fs2::FileExt;

        match fs::File::open(path) {
            Ok(file) => {
                // Try to get exclusive lock (non-blocking)
                match file.try_lock_exclusive() {
                    Ok(()) => {
                        // Got the lock, file is likely not in use
                        let _ = file.unlock();
                        Ok(false)
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        // Lock would block = file is in use
                        tracing::debug!("File {} is in use (lock would block)", path.display());
                        Ok(true)
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Could not check lock for {}: {}. Assuming not in use.",
                            path.display(),
                            e
                        );
                        Ok(false)
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                // Permission denied might mean file is locked
                tracing::debug!(
                    "File {} might be in use (permission denied)",
                    path.display()
                );
                Ok(true)
            }
            Err(_) => {
                // File doesn't exist or other error
                Ok(false)
            }
        }
    }
}

/// Smart implementation that tries lsof first, then falls back to file locking
pub struct SmartFileChecker {
    lsof_available: std::sync::Once,
    use_lsof: std::sync::atomic::AtomicBool,
}

impl SmartFileChecker {
    pub fn new() -> Self {
        Self {
            lsof_available: std::sync::Once::new(),
            use_lsof: std::sync::atomic::AtomicBool::new(true),
        }
    }

    fn check_lsof_availability(&self) {
        self.lsof_available.call_once(|| {
            // Check if lsof is available
            let available = Command::new("lsof")
                .arg("-v")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            self.use_lsof
                .store(available, std::sync::atomic::Ordering::Relaxed);

            if !available {
                tracing::info!(
                    "lsof not found, will use file locking to check file usage. \
                     For better reliability, consider installing lsof."
                );
            }
        });
    }
}

impl Default for SmartFileChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl FileChecker for SmartFileChecker {
    fn is_file_in_use(&self, path: &Path) -> io::Result<bool> {
        self.check_lsof_availability();

        if self.use_lsof.load(std::sync::atomic::Ordering::Relaxed) {
            // Try lsof first
            if let Ok(output) = Command::new("lsof")
                .arg("-t")
                .arg(path.as_os_str())
                .output()
            {
                return Ok(output.status.success());
            }
            // lsof failed, fall back to file locking
        }

        // Fallback to file locking
        FileLockChecker.is_file_in_use(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_noop_checker() {
        let checker = NoOpFileChecker;
        let temp_file = std::env::temp_dir().join("test_noop.txt");
        File::create(&temp_file).unwrap();

        assert!(!checker.is_file_in_use(&temp_file).unwrap());

        std::fs::remove_file(&temp_file).unwrap();
    }

    #[test]
    fn test_file_lock_checker() {
        let checker = FileLockChecker;
        let temp_file = std::env::temp_dir().join("test_lock.txt");
        File::create(&temp_file).unwrap();

        // File not locked initially
        assert!(!checker.is_file_in_use(&temp_file).unwrap());

        // Note: Actually locking the file and testing would require
        // multi-threading or multi-process testing

        std::fs::remove_file(&temp_file).unwrap();
    }

    #[test]
    fn test_smart_checker() {
        let checker = SmartFileChecker::new();
        let temp_file = std::env::temp_dir().join("test_smart.txt");

        let mut file = File::create(&temp_file).unwrap();
        file.write_all(b"test").unwrap();
        drop(file);

        // Should work with either lsof or file locking
        let _ = checker.is_file_in_use(&temp_file);

        std::fs::remove_file(&temp_file).unwrap();
    }
}
