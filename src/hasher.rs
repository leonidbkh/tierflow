//! File hashing implementations for integrity verification

use std::io;
use std::path::Path;

/// Trait for calculating file checksums/hashes
pub trait Hasher: Send + Sync {
    /// Calculate hash/checksum for a file
    fn calculate_hash(&self, path: &Path) -> io::Result<String>;

    /// Check if two files have identical content
    fn files_are_identical(&self, path1: &Path, path2: &Path) -> io::Result<bool> {
        // Default implementation: compare sizes first, then hashes
        let metadata1 = std::fs::metadata(path1)?;
        let metadata2 = std::fs::metadata(path2)?;

        if metadata1.len() != metadata2.len() {
            return Ok(false);
        }

        let hash1 = self.calculate_hash(path1)?;
        let hash2 = self.calculate_hash(path2)?;

        Ok(hash1 == hash2)
    }
}

/// Implementation that always returns a constant (for testing)
pub struct NoOpHasher;

impl Hasher for NoOpHasher {
    fn calculate_hash(&self, path: &Path) -> io::Result<String> {
        // Return path as "hash" for testing
        Ok(path.display().to_string())
    }
}

/// Native XXH3-128 hasher using xxhash-rust
pub struct Xxh3Hasher;

impl Xxh3Hasher {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Xxh3Hasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher for Xxh3Hasher {
    fn calculate_hash(&self, path: &Path) -> io::Result<String> {
        crate::mover::native::calculate_checksum_native(path)
    }
}

/// Hasher that uses external commands (sha256sum, md5sum, etc)
pub struct CommandHasher {
    command: String,
}

impl CommandHasher {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
        }
    }

    pub fn sha256() -> Self {
        Self::new("sha256sum")
    }

    pub fn md5() -> Self {
        Self::new("md5sum")
    }
}

impl Hasher for CommandHasher {
    fn calculate_hash(&self, path: &Path) -> io::Result<String> {
        use std::process::Command;

        let output = Command::new(&self.command).arg(path.as_os_str()).output()?;

        if !output.status.success() {
            return Err(io::Error::other(format!(
                "{} failed for {}",
                self.command,
                path.display()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .split_whitespace()
            .next()
            .map(String::from)
            .ok_or_else(|| io::Error::other(format!("Failed to parse {} output", self.command)))
    }
}

/// Smart hasher that tries fast algorithms first
pub struct SmartHasher {
    xxh3: Xxh3Hasher,
}

impl SmartHasher {
    pub const fn new() -> Self {
        Self {
            xxh3: Xxh3Hasher::new(),
        }
    }
}

impl Default for SmartHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl Hasher for SmartHasher {
    fn calculate_hash(&self, path: &Path) -> io::Result<String> {
        // Try native XXH3 first (fastest)
        match self.xxh3.calculate_hash(path) {
            Ok(hash) => Ok(hash),
            Err(e) => {
                // Log and return error - no fallback needed since XXH3 is built-in
                tracing::error!("Hash calculation failed for {}: {}", path.display(), e);
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_noop_hasher() {
        let hasher = NoOpHasher;
        let path = Path::new("/test/file.txt");

        let hash = hasher.calculate_hash(path).unwrap();
        assert_eq!(hash, "/test/file.txt");
    }

    #[test]
    fn test_files_are_identical_different_sizes() {
        let hasher = NoOpHasher;
        let temp_dir = std::env::temp_dir();
        let file1 = temp_dir.join("hash_test1.txt");
        let file2 = temp_dir.join("hash_test2.txt");

        File::create(&file1).unwrap().write_all(b"short").unwrap();
        File::create(&file2)
            .unwrap()
            .write_all(b"longer content")
            .unwrap();

        assert!(!hasher.files_are_identical(&file1, &file2).unwrap());

        std::fs::remove_file(&file1).unwrap();
        std::fs::remove_file(&file2).unwrap();
    }

    #[test]
    fn test_xxh3_hasher() {
        let hasher = Xxh3Hasher::new();
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("xxh3_test.txt");

        File::create(&test_file)
            .unwrap()
            .write_all(b"test content")
            .unwrap();

        let hash = hasher.calculate_hash(&test_file).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 32); // XXH3-128 = 128 bits = 16 bytes = 32 hex chars

        // Same file should give same hash
        let hash2 = hasher.calculate_hash(&test_file).unwrap();
        assert_eq!(hash, hash2);

        std::fs::remove_file(&test_file).unwrap();
    }

    #[test]
    fn test_smart_hasher() {
        let hasher = SmartHasher::new();
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("smart_test.txt");

        File::create(&test_file)
            .unwrap()
            .write_all(b"smart hasher test")
            .unwrap();

        let hash = hasher.calculate_hash(&test_file).unwrap();
        assert!(!hash.is_empty());

        std::fs::remove_file(&test_file).unwrap();
    }
}
