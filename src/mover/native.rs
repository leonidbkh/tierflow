// Native Rust implementation using xxhash-rust for maximum performance

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;
use xxhash_rust::xxh3::xxh3_128;

/// Calculate XXH3-128 checksum of a file using streaming for memory efficiency
/// This is extremely fast - typically 30-60 GB/s on modern hardware
pub fn calculate_checksum_native(path: &Path) -> io::Result<String> {
    const BUFFER_SIZE: usize = 1024 * 1024; // 1MB buffer for streaming

    let file = File::open(path)?;
    let metadata = file.metadata()?;
    let file_size = metadata.len();
    let size_gb = file_size as f64 / (1024.0 * 1024.0 * 1024.0);

    // For small files, read all at once
    if file_size < 10 * 1024 * 1024 {
        // < 10MB
        let mut buffer = Vec::with_capacity(file_size as usize);
        let mut reader = BufReader::new(file);
        reader.read_to_end(&mut buffer)?;
        let hash = xxh3_128(&buffer);
        tracing::debug!(
            "XXH3-128 hash for {} ({:.3} MB): {:032x}",
            path.display(),
            file_size as f64 / (1024.0 * 1024.0),
            hash
        );
        return Ok(format!("{:032x}", hash));
    }

    // For large files, use streaming
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    let mut hasher = xxhash_rust::xxh3::Xxh3Builder::new().build();
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut total_read = 0u64;
    let mut last_log_gb = 0;

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
        total_read += bytes_read as u64;

        // Log progress for very large files
        let current_gb = total_read / (1024 * 1024 * 1024);
        if current_gb > last_log_gb && size_gb > 10.0 {
            tracing::debug!(
                "Hashing progress: {}/{} GB ({:.1}%)",
                current_gb,
                size_gb as u64,
                (total_read as f64 / file_size as f64) * 100.0
            );
            last_log_gb = current_gb;
        }
    }

    let hash = hasher.digest128();
    tracing::debug!(
        "XXH3-128 hash for {} ({:.2} GB): {:032x}",
        path.display(),
        size_gb,
        hash
    );

    Ok(format!("{:032x}", hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_small_file_hash() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_xxhash.txt");

        // Create test file
        let mut file = fs::File::create(&test_file).unwrap();
        file.write_all(b"Hello, World!").unwrap();
        file.sync_all().unwrap();
        drop(file);

        // Calculate hash
        let hash = calculate_checksum_native(&test_file).unwrap();

        // XXH3-128 hash of "Hello, World!" should be deterministic
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 32); // 128 bits = 16 bytes = 32 hex chars

        // Verify same content gives same hash
        let hash2 = calculate_checksum_native(&test_file).unwrap();
        assert_eq!(hash, hash2);

        // Clean up
        fs::remove_file(&test_file).unwrap();
    }

    #[test]
    fn test_large_file_streaming() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_xxhash_large.bin");

        // Create 20MB test file
        let mut file = fs::File::create(&test_file).unwrap();
        let data = vec![0xABu8; 20 * 1024 * 1024];
        file.write_all(&data).unwrap();
        file.sync_all().unwrap();
        drop(file);

        // Calculate hash (should use streaming)
        let hash = calculate_checksum_native(&test_file).unwrap();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 32);

        // Clean up
        fs::remove_file(&test_file).unwrap();
    }

    #[test]
    fn test_different_files_different_hashes() {
        let temp_dir = std::env::temp_dir();
        let file1 = temp_dir.join("test_hash1.txt");
        let file2 = temp_dir.join("test_hash2.txt");

        fs::write(&file1, b"Content 1").unwrap();
        fs::write(&file2, b"Content 2").unwrap();

        let hash1 = calculate_checksum_native(&file1).unwrap();
        let hash2 = calculate_checksum_native(&file2).unwrap();

        assert_ne!(hash1, hash2);

        fs::remove_file(&file1).unwrap();
        fs::remove_file(&file2).unwrap();
    }
}
