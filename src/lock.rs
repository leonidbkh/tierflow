use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, SystemTime};

use crate::{Tier, error::AppError};

const LOCK_DIR: &str = "/tmp/tierflow-locks";

#[derive(Debug, Serialize, Deserialize)]
struct LockInfo {
    pid: u32,
    hostname: String,
    started_at: SystemTime,
    command: String,
    tier_paths: Vec<PathBuf>,
}

pub struct TierLockGuard {
    lock_path: PathBuf,
    lock_file: File,
}

impl TierLockGuard {
    /// Generate unique lock path based on tier paths
    fn generate_lock_path(tiers: &[Tier]) -> PathBuf {
        // Sort tier paths for consistent hashing
        let mut paths: Vec<_> = tiers.iter().map(|t| &t.path).collect();
        paths.sort();

        // Hash the sorted paths
        let mut hasher = DefaultHasher::new();
        for path in paths {
            path.hash(&mut hasher);
        }
        let hash = hasher.finish();

        PathBuf::from(LOCK_DIR).join(format!("lock-{:016x}.lock", hash))
    }

    /// Try to acquire exclusive lock for this tier configuration
    pub fn try_lock_tiers(tiers: &[Tier]) -> Result<Self, AppError> {
        // Ensure lock directory exists
        fs::create_dir_all(LOCK_DIR).map_err(|e| AppError::LockError {
            message: format!("Failed to create lock directory {}: {}", LOCK_DIR, e),
        })?;

        let lock_path = Self::generate_lock_path(tiers);

        // Collect tier paths for lock info
        let tier_paths: Vec<PathBuf> = tiers.iter().map(|t| t.path.clone()).collect();

        // Clean up stale locks from dead processes
        if lock_path.exists() {
            Self::cleanup_stale_lock(&lock_path);
        }

        // Create or open lock file
        let mut lock_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .read(true)
            .open(&lock_path)
            .map_err(|e| AppError::LockError {
                message: format!("Failed to open lock file {}: {}", lock_path.display(), e),
            })?;

        // Try to acquire exclusive lock (non-blocking)
        if matches!(lock_file.try_lock_exclusive(), Ok(())) {
            // Write process info to lock file
            let info = LockInfo {
                pid: process::id(),
                hostname: hostname::get().map_or_else(
                    |_| "unknown".to_string(),
                    |h| h.to_string_lossy().to_string(),
                ),
                started_at: SystemTime::now(),
                command: std::env::args().collect::<Vec<_>>().join(" "),
                tier_paths: tier_paths.clone(),
            };

            lock_file.set_len(0).ok(); // Truncate
            lock_file
                .write_all(serde_json::to_string(&info)?.as_bytes())
                .map_err(|e| AppError::LockError {
                    message: format!("Failed to write lock info: {e}"),
                })?;
            lock_file.sync_all().ok();

            Ok(Self {
                lock_path,
                lock_file,
            })
        } else {
            // Lock is held by another process - get info about owner
            let owner_info = Self::read_lock_info(&lock_path);

            if let Some(info) = owner_info {
                let duration = SystemTime::now()
                    .duration_since(info.started_at)
                    .unwrap_or_default();

                return Err(AppError::TierLocked {
                    tier: format!("Tiers: {:?}", tier_paths),
                    owner_pid: info.pid,
                    owner_host: info.hostname,
                    locked_for: duration,
                });
            }
            Err(AppError::TierLocked {
                tier: format!("Tiers: {:?}", tier_paths),
                owner_pid: 0,
                owner_host: "unknown".to_string(),
                locked_for: Duration::from_secs(0),
            })
        }
    }

    /// Clean up stale locks from dead processes
    fn cleanup_stale_lock(lock_path: &Path) {
        if let Ok(file) = OpenOptions::new().write(true).read(true).open(lock_path) {
            // If we can acquire the lock, the owning process is dead
            if file.try_lock_exclusive().is_ok() {
                // Read lock info for logging
                if let Some(info) = Self::read_lock_info(lock_path) {
                    // Check if process is still alive
                    if !Self::is_process_alive(info.pid) {
                        tracing::warn!(
                            "Removing stale lock from dead process {} ({}) at {}",
                            info.pid,
                            info.hostname,
                            lock_path.display()
                        );
                        // Unlock before removing
                        let _ = file.unlock();
                        drop(file);
                        fs::remove_file(lock_path).ok();
                        return;
                    }
                }
                // Process is still alive, release the lock
                let _ = file.unlock();
            }
        }
    }

    /// Read lock info from file
    fn read_lock_info(lock_path: &Path) -> Option<LockInfo> {
        if let Ok(mut file) = File::open(lock_path) {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
                serde_json::from_str(&contents).ok()
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Check if a process is still alive
    fn is_process_alive(pid: u32) -> bool {
        #[cfg(unix)]
        {
            // On Unix, send signal 0 to check if process exists
            use nix::sys::signal::kill;
            use nix::unistd::Pid;

            let pid = Pid::from_raw(pid as i32);
            kill(pid, None).is_ok()
        }

        #[cfg(not(unix))]
        {
            // On Windows, we can't reliably check, so assume it's alive
            // The flock will still work correctly
            true
        }
    }

    /// Get lock file path for display
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

impl Drop for TierLockGuard {
    fn drop(&mut self) {
        // Release lock and remove lock file
        let _ = self.lock_file.unlock();
        // Remove lock file on clean exit
        if let Err(e) = fs::remove_file(&self.lock_path) {
            tracing::warn!(
                "Failed to remove lock file {}: {}",
                self.lock_path.display(),
                e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn create_test_tier(name: &str) -> Tier {
        let temp_dir = env::temp_dir().join(format!("lock_test_{name}"));
        fs::create_dir_all(&temp_dir).unwrap();
        Tier::new(name.to_string(), temp_dir, 1, None, None).unwrap()
    }

    #[test]
    fn test_single_lock_success() {
        let tier = create_test_tier("single");
        let guard = TierLockGuard::try_lock_tiers(&[tier.clone()]).unwrap();

        // Lock file should exist in /tmp
        let lock_path = guard.lock_path().to_path_buf();
        assert!(lock_path.exists());
        assert!(lock_path.starts_with(LOCK_DIR));

        drop(guard);

        // Lock file should be removed after drop
        assert!(!lock_path.exists());

        // Cleanup
        fs::remove_dir_all(&tier.path).ok();
    }

    #[test]
    fn test_multiple_tiers_lock() {
        let tier1 = create_test_tier("multi1");
        let tier2 = create_test_tier("multi2");

        let guard = TierLockGuard::try_lock_tiers(&[tier1.clone(), tier2.clone()]).unwrap();

        // Single lock file should exist in /tmp
        let lock_path = guard.lock_path().to_path_buf();
        assert!(lock_path.exists());
        assert!(lock_path.starts_with(LOCK_DIR));

        drop(guard);

        // Lock file should be removed after drop
        assert!(!lock_path.exists());

        // Cleanup
        fs::remove_dir_all(&tier1.path).ok();
        fs::remove_dir_all(&tier2.path).ok();
    }

    #[test]
    fn test_lock_conflict() {
        let tier = create_test_tier("conflict");

        // First lock should succeed
        let _guard1 = TierLockGuard::try_lock_tiers(&[tier.clone()]).unwrap();

        // Second lock should fail
        let result = TierLockGuard::try_lock_tiers(&[tier.clone()]);
        assert!(result.is_err());

        if let Err(AppError::TierLocked { .. }) = result {
            // Expected error
        } else {
            panic!("Expected TierLocked error");
        }

        // Cleanup
        fs::remove_dir_all(&tier.path).ok();
    }

    #[test]
    fn test_different_tier_configs_different_locks() {
        let tier1 = create_test_tier("atomic1");
        let tier2 = create_test_tier("atomic2");

        // Lock with tier1 only
        let guard1 = TierLockGuard::try_lock_tiers(&[tier1.clone()]).unwrap();
        let lock_path1 = guard1.lock_path().to_path_buf();

        // Lock with tier2 only - should succeed (different config)
        let guard2 = TierLockGuard::try_lock_tiers(&[tier2.clone()]).unwrap();
        let lock_path2 = guard2.lock_path().to_path_buf();

        // Different tier configurations should have different lock files
        assert_ne!(lock_path1, lock_path2);

        drop(guard1);
        drop(guard2);

        // Cleanup
        fs::remove_dir_all(&tier1.path).ok();
        fs::remove_dir_all(&tier2.path).ok();
    }

    #[test]
    fn test_stale_lock_cleanup() {
        let tier = create_test_tier("stale");

        // Ensure lock directory exists
        fs::create_dir_all(LOCK_DIR).unwrap();

        let lock_path = TierLockGuard::generate_lock_path(&[tier.clone()]);

        // Create a stale lock file with fake PID
        let stale_info = LockInfo {
            pid: 999999999, // Non-existent PID
            hostname: "test-host".to_string(),
            started_at: SystemTime::now(),
            command: "test".to_string(),
            tier_paths: vec![tier.path.clone()],
        };

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(&lock_path)
            .unwrap();
        file.write_all(serde_json::to_string(&stale_info).unwrap().as_bytes())
            .unwrap();
        drop(file);

        // Should succeed after cleaning up stale lock
        let guard = TierLockGuard::try_lock_tiers(&[tier.clone()]);
        assert!(guard.is_ok());

        // Cleanup
        fs::remove_dir_all(&tier.path).ok();
    }
}
