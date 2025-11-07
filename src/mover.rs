use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

/// Trait for moving files between tiers
/// Different implementations can use rsync, cp, mv, etc.
pub trait Mover {
    /// Move file from source to destination
    ///
    /// # Arguments
    /// * `source` - Full path to source file
    /// * `destination` - Full path to destination file
    ///
    /// # Errors
    /// Returns `io::Error` if operation fails
    fn move_file(&self, source: &Path, destination: &Path) -> io::Result<()>;
}

/// `DryRun` implementation - only logs operations without actual movement
pub struct DryRunMover;

/// Rsync-based mover for actual file movement
/// Uses rsync with --remove-source-files to move files efficiently
pub struct RsyncMover {
    /// Additional rsync arguments (e.g., bandwidth limiting)
    extra_args: Vec<String>,
}

impl RsyncMover {
    /// Create a new `RsyncMover` with default options
    pub const fn new() -> Self {
        Self {
            extra_args: Vec::new(),
        }
    }

    /// Create a new `RsyncMover` with custom rsync arguments
    pub const fn with_args(args: Vec<String>) -> Self {
        Self { extra_args: args }
    }

    /// Calculate SHA256 checksum of a file
    fn calculate_checksum(path: &Path) -> io::Result<String> {
        use std::io::Read;

        // Use xxhsum or sha256sum command for performance on large files
        let output = Command::new("sha256sum").arg(path.as_os_str()).output();

        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // sha256sum output format: "hash  filename"
                if let Some(hash) = stdout.split_whitespace().next() {
                    return Ok(hash.to_string());
                }
            }
            _ => {
                // Fallback to calculating in Rust if sha256sum is not available
                let mut file = fs::File::open(path)?;
                let mut buffer = Vec::new();

                // For very large files, this might use a lot of memory
                // Consider streaming hash calculation for production
                file.read_to_end(&mut buffer)?;

                // Simple checksum using std hash
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                let mut hasher = DefaultHasher::new();
                buffer.hash(&mut hasher);
                let hash = hasher.finish();

                return Ok(format!("{hash:016x}"));
            }
        }

        Err(io::Error::other("Failed to calculate checksum"))
    }
}

impl Default for RsyncMover {
    fn default() -> Self {
        Self::new()
    }
}

impl RsyncMover {
    /// Check if file is currently open by any process
    fn is_file_in_use(path: &Path) -> io::Result<bool> {
        #[cfg(target_os = "linux")]
        {
            // Use fuser to check if any process has the file open
            let output = Command::new("fuser").arg(path.as_os_str()).output()?;

            // fuser returns exit code 0 if processes found
            Ok(output.status.success())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = path; // Silence unused warning
            // On non-Linux, skip the check (no reliable method)
            Ok(false)
        }
    }
}

impl Mover for RsyncMover {
    fn move_file(&self, source: &Path, destination: &Path) -> io::Result<()> {
        // Check if source exists
        if !source.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Source file does not exist: {}", source.display()),
            ));
        }

        // Check if source file is currently in use
        if Self::is_file_in_use(source)? {
            return Err(io::Error::new(
                io::ErrorKind::ResourceBusy,
                format!("Source file is currently in use: {}", source.display()),
            ));
        }

        // Check if destination already exists
        if destination.exists() {
            // Compare files to see if they're identical
            let source_metadata = fs::metadata(source)?;
            let dest_metadata = fs::metadata(destination)?;

            // If sizes match, check checksums
            if source_metadata.len() == dest_metadata.len() {
                let source_checksum = Self::calculate_checksum(source)?;
                let dest_checksum = Self::calculate_checksum(destination)?;

                if source_checksum == dest_checksum {
                    // Files are identical, just remove source
                    log::info!(
                        "Destination already exists and is identical: {} (checksum: {})",
                        destination.display(),
                        source_checksum
                    );
                    fs::remove_file(source)?;
                    return Ok(());
                }
            }

            // Files are different - rename destination with timestamp
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let backup_path = destination.with_extension(format!(
                "{}.backup-{}",
                destination
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy(),
                timestamp
            ));

            log::warn!(
                "Destination already exists but differs: {} -> Backing up to: {}",
                destination.display(),
                backup_path.display()
            );

            fs::rename(destination, backup_path)?;
        }

        // Ensure destination directory exists
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }

        // Step 1: Copy file with rsync (without removing source)
        let mut cmd = Command::new("rsync");

        // Base arguments for file copy (no --remove-source-files!)
        cmd.arg("-av") // Archive mode with verbose
            .arg("--checksum") // Use checksums for verification
            .arg("--progress"); // Show progress during transfer

        // Add any extra arguments
        for arg in &self.extra_args {
            cmd.arg(arg);
        }

        // Add source and destination
        cmd.arg(source.as_os_str()).arg(destination.as_os_str());

        log::info!(
            "Copying file: {} -> {}",
            source.display(),
            destination.display()
        );

        // Execute rsync
        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            log::error!(
                "Rsync failed for {} -> {}\nstdout: {}\nstderr: {}",
                source.display(),
                destination.display(),
                stdout,
                stderr
            );

            return Err(io::Error::other(format!(
                "rsync failed with exit code {:?}: {}",
                output.status.code(),
                stderr
            )));
        }

        // Step 2: Verify the file was copied correctly
        if !destination.exists() {
            return Err(io::Error::other(format!(
                "Destination file was not created: {}",
                destination.display()
            )));
        }

        // Step 3: Verify file sizes match
        let source_metadata = fs::metadata(source)?;
        let dest_metadata = fs::metadata(destination)?;

        if source_metadata.len() != dest_metadata.len() {
            // Try to clean up the incomplete copy
            let _ = fs::remove_file(destination);
            return Err(io::Error::other(format!(
                "File size mismatch after copy: source={} bytes, dest={} bytes",
                source_metadata.len(),
                dest_metadata.len()
            )));
        }

        // Step 4: Calculate checksums for both files
        let source_checksum = Self::calculate_checksum(source)?;
        let dest_checksum = Self::calculate_checksum(destination)?;

        if source_checksum != dest_checksum {
            // Try to clean up the corrupted copy
            let _ = fs::remove_file(destination);
            return Err(io::Error::other(format!(
                "Checksum mismatch after copy: source={source_checksum}, dest={dest_checksum}"
            )));
        }

        // Step 5: Verify source file hasn't been modified during copy
        // (Protection against concurrent modifications)
        let source_metadata_after = fs::metadata(source)?;

        if source_metadata_after.len() != source_metadata.len() {
            log::warn!(
                "Source file size changed during copy! Before: {} bytes, After: {} bytes. Aborting delete.",
                source_metadata.len(),
                source_metadata_after.len()
            );
            return Err(io::Error::other(format!(
                "Source file was modified during copy (size changed). Not deleting for safety: {}",
                source.display()
            )));
        }

        // Check if source file is in use before deleting
        if Self::is_file_in_use(source)? {
            log::warn!(
                "Source file is now in use by another process. Not deleting: {}",
                source.display()
            );
            return Err(io::Error::other(format!(
                "Source file became in use during copy. Not deleting for safety: {}",
                source.display()
            )));
        }

        // Step 6: Only now, after all verification, remove the source
        fs::remove_file(source)?;

        log::info!(
            "Successfully moved: {} -> {} (checksum: {})",
            source.display(),
            destination.display(),
            source_checksum
        );

        Ok(())
    }
}

impl Mover for DryRunMover {
    fn move_file(&self, source: &Path, destination: &Path) -> io::Result<()> {
        log::info!(
            "[DRY-RUN] Would move: {} -> {}",
            source.display(),
            destination.display()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_dry_run_mover_success() {
        let mover = DryRunMover;
        let source = PathBuf::from("/source/file.txt");
        let dest = PathBuf::from("/dest/file.txt");

        let result = mover.move_file(&source, &dest);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dry_run_mover_multiple_calls() {
        let mover = DryRunMover;

        for i in 0..5 {
            let source = PathBuf::from(format!("/source/file{i}.txt"));
            let dest = PathBuf::from(format!("/dest/file{i}.txt"));
            assert!(mover.move_file(&source, &dest).is_ok());
        }
    }

    #[test]
    fn test_mover_trait_object() {
        let mover: &dyn Mover = &DryRunMover;
        let source = PathBuf::from("/test/source.txt");
        let dest = PathBuf::from("/test/dest.txt");

        assert!(mover.move_file(&source, &dest).is_ok());
    }

    #[test]
    fn test_rsync_mover_new() {
        let mover = RsyncMover::new();
        assert!(mover.extra_args.is_empty());
    }

    #[test]
    fn test_rsync_mover_with_args() {
        let args = vec!["--bwlimit=1000".to_string()];
        let mover = RsyncMover::with_args(args.clone());
        assert_eq!(mover.extra_args, args);
    }

    #[test]
    fn test_rsync_mover_default() {
        let mover = RsyncMover::default();
        assert!(mover.extra_args.is_empty());
    }

    // Integration test - only run if rsync is available
    #[test]
    #[ignore = "requires rsync, run with --ignored"]
    fn test_rsync_mover_actual_move() {
        let temp_dir = TempDir::new().unwrap();
        let source_path = temp_dir.path().join("source.txt");
        let dest_dir = temp_dir.path().join("dest");
        let dest_path = dest_dir.join("source.txt");

        // Create source file
        fs::write(&source_path, "test content").unwrap();

        // Move file
        let mover = RsyncMover::new();
        let result = mover.move_file(&source_path, &dest_path);

        assert!(result.is_ok());
        assert!(!source_path.exists(), "Source file should be removed");
        assert!(dest_path.exists(), "Destination file should exist");

        // Check content
        let content = fs::read_to_string(&dest_path).unwrap();
        assert_eq!(content, "test content");
    }
}
