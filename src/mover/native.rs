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

    #[cfg(target_os = "linux")]
    use std::os::unix::io::AsRawFd;

    // Get file descriptor for Linux optimizations
    #[cfg(target_os = "linux")]
    let fd = file.as_raw_fd();

    // Tell kernel we'll read sequentially (optimization hint)
    #[cfg(target_os = "linux")]
    unsafe {
        libc::posix_fadvise(fd, 0, 0, libc::POSIX_FADV_SEQUENTIAL);
    }

    tracing::info!("Hashing file: {} ({:.2} GB)", path.display(), size_gb);

    // For small files, read all at once
    if file_size < 10 * 1024 * 1024 {
        // < 10MB
        let mut buffer = Vec::with_capacity(file_size as usize);
        let mut reader = BufReader::new(file);
        reader.read_to_end(&mut buffer)?;
        let hash = xxh3_128(&buffer);

        // Drop cached pages for small files too
        #[cfg(target_os = "linux")]
        unsafe {
            libc::posix_fadvise(fd, 0, file_size as i64, libc::POSIX_FADV_DONTNEED);
        }

        tracing::info!(
            "Hash complete: {} ({:.3} MB) = {:032x}",
            path.display(),
            file_size as f64 / (1024.0 * 1024.0),
            hash
        );
        return Ok(format!("{:032x}", hash));
    }

    // For large files, use streaming and drop pages progressively
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    let mut hasher = xxhash_rust::xxh3::Xxh3Builder::new().build();
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut total_read = 0u64;
    let mut last_log_gb = 0;

    #[cfg(target_os = "linux")]
    let mut last_fadvise_offset = 0i64;

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
        total_read += bytes_read as u64;

        // Drop cached pages every 100MB to prevent page cache buildup
        #[cfg(target_os = "linux")]
        {
            const DROP_INTERVAL: i64 = 100 * 1024 * 1024; // 100MB
            let current_offset = total_read as i64;
            if current_offset - last_fadvise_offset >= DROP_INTERVAL {
                let drop_len = current_offset - last_fadvise_offset;
                unsafe {
                    libc::posix_fadvise(
                        fd,
                        last_fadvise_offset,
                        drop_len,
                        libc::POSIX_FADV_DONTNEED,
                    );
                }
                tracing::debug!(
                    "Dropped {}MB of page cache for {} (offset: {:.2}GB)",
                    drop_len / (1024 * 1024),
                    path.display(),
                    last_fadvise_offset as f64 / (1024.0 * 1024.0 * 1024.0)
                );
                last_fadvise_offset = current_offset;
            }
        }

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

    // Drop any remaining cached pages
    #[cfg(target_os = "linux")]
    if last_fadvise_offset < total_read as i64 {
        let remaining = total_read as i64 - last_fadvise_offset;
        unsafe {
            libc::posix_fadvise(
                fd,
                last_fadvise_offset,
                remaining,
                libc::POSIX_FADV_DONTNEED,
            );
        }
        tracing::debug!(
            "Dropped final {}MB of page cache for {}",
            remaining / (1024 * 1024),
            path.display()
        );
    }

    let hash = hasher.digest128();
    tracing::info!(
        "Hash complete: {} ({:.2} GB) = {:032x}",
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
