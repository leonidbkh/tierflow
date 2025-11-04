use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{Duration, SystemTime};

use crate::{Tier, error::AppError};

#[derive(Debug, Serialize, Deserialize)]
struct LockInfo {
    pid: u32,
    hostname: String,
    started_at: SystemTime,
    command: String,
}

pub struct TierLockGuard {
    locks: Vec<(PathBuf, File)>,
}

impl TierLockGuard {
    /// Try to acquire exclusive locks on all tiers atomically
    pub fn try_lock_tiers(tiers: &[Tier]) -> Result<Self, AppError> {
        // Sort tiers by path to ensure consistent lock order and prevent deadlocks
        // All processes will always lock in the same order
        let mut sorted_tiers: Vec<&Tier> = tiers.iter().collect();
        sorted_tiers.sort_by(|a, b| a.path.cmp(&b.path));

        let mut locks = Vec::new();

        for tier in sorted_tiers {
            let lock_path = tier.path.join(".mergerfs-balancer.lock");

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
                };

                lock_file.set_len(0).ok(); // Truncate
                lock_file
                    .write_all(serde_json::to_string(&info)?.as_bytes())
                    .map_err(|e| AppError::LockError {
                        message: format!("Failed to write lock info: {e}"),
                    })?;
                lock_file.sync_all().ok();

                locks.push((lock_path, lock_file));
            } else {
                // Lock is held by another process - get info about owner
                let owner_info = Self::read_lock_info(&lock_path);

                // Release all acquired locks before returning error
                for (path, lock) in locks {
                    let _ = lock.unlock();
                    let _ = fs::remove_file(path);
                }

                if let Some(info) = owner_info {
                    let duration = SystemTime::now()
                        .duration_since(info.started_at)
                        .unwrap_or_default();

                    return Err(AppError::TierLocked {
                        tier: tier.name.clone(),
                        owner_pid: info.pid,
                        owner_host: info.hostname,
                        locked_for: duration,
                    });
                }
                return Err(AppError::TierLocked {
                    tier: tier.name.clone(),
                    owner_pid: 0,
                    owner_host: "unknown".to_string(),
                    locked_for: Duration::from_secs(0),
                });
            }
        }

        Ok(Self { locks })
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
                        log::warn!(
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

    /// Get list of locked tiers for display
    pub fn locked_tiers(&self) -> Vec<String> {
        self.locks
            .iter()
            .filter_map(|(path, _)| {
                path.parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().to_string())
            })
            .collect()
    }
}

impl Drop for TierLockGuard {
    fn drop(&mut self) {
        // Release all locks and remove lock files
        for (path, lock) in &self.locks {
            let _ = lock.unlock();
            // Remove lock file on clean exit
            if let Err(e) = fs::remove_file(path) {
                log::warn!("Failed to remove lock file {}: {}", path.display(), e);
            }
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
        Tier::new(name.to_string(), temp_dir, 1, None).unwrap()
    }

    #[test]
    fn test_single_lock_success() {
        let tier = create_test_tier("single");
        let guard = TierLockGuard::try_lock_tiers(&[tier.clone()]).unwrap();

        assert_eq!(guard.locks.len(), 1);

        // Lock file should exist
        let lock_path = tier.path.join(".mergerfs-balancer.lock");
        assert!(lock_path.exists());

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

        assert_eq!(guard.locks.len(), 2);

        // Both lock files should exist
        assert!(tier1.path.join(".mergerfs-balancer.lock").exists());
        assert!(tier2.path.join(".mergerfs-balancer.lock").exists());

        drop(guard);

        // Both lock files should be removed
        assert!(!tier1.path.join(".mergerfs-balancer.lock").exists());
        assert!(!tier2.path.join(".mergerfs-balancer.lock").exists());

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

        if let Err(AppError::TierLocked {
            tier: tier_name, ..
        }) = result
        {
            assert_eq!(tier_name, "conflict");
        } else {
            panic!("Expected TierLocked error");
        }

        // Cleanup
        fs::remove_dir_all(&tier.path).ok();
    }

    #[test]
    fn test_atomic_locking() {
        let tier1 = create_test_tier("atomic1");
        let tier2 = create_test_tier("atomic2");

        // Lock first tier manually with proper LockInfo
        let lock_path1 = tier1.path.join(".mergerfs-balancer.lock");
        let mut manual_lock = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .open(&lock_path1)
            .unwrap();

        // Acquire the lock
        manual_lock.try_lock_exclusive().unwrap();

        // Write valid LockInfo so it won't be cleaned up as stale
        let lock_info = LockInfo {
            pid: process::id(), // Use current process ID so it won't be seen as stale
            hostname: "test-host".to_string(),
            started_at: SystemTime::now(),
            command: "test".to_string(),
        };
        manual_lock
            .write_all(serde_json::to_string(&lock_info).unwrap().as_bytes())
            .unwrap();
        manual_lock.sync_all().unwrap();

        // Try to lock both tiers - should fail atomically
        let result = TierLockGuard::try_lock_tiers(&[tier1.clone(), tier2.clone()]);
        assert!(result.is_err());

        // Second tier should not have a lock file (atomic failure)
        let lock_path2 = tier2.path.join(".mergerfs-balancer.lock");
        if lock_path2.exists() {
            // If it exists, it should not be locked
            let test_lock = OpenOptions::new().write(true).open(&lock_path2).unwrap();
            assert!(test_lock.try_lock_exclusive().is_ok());
        }

        // Cleanup
        manual_lock.unlock().ok();
        fs::remove_dir_all(&tier1.path).ok();
        fs::remove_dir_all(&tier2.path).ok();
    }

    #[test]
    fn test_stale_lock_cleanup() {
        let tier = create_test_tier("stale");
        let lock_path = tier.path.join(".mergerfs-balancer.lock");

        // Create a stale lock file with fake PID
        let stale_info = LockInfo {
            pid: 999999999, // Non-existent PID
            hostname: "test-host".to_string(),
            started_at: SystemTime::now(),
            command: "test".to_string(),
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
