use std::path::PathBuf;
use std::time::SystemTime;
use std::{fs, io};

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub size: u64,
    pub modified: SystemTime,
    pub accessed: SystemTime,
}

impl FileInfo {
    pub fn from_path(path: PathBuf) -> io::Result<Self> {
        let metadata = fs::metadata(&path)?;

        Ok(Self {
            path,
            size: metadata.len(),
            modified: metadata.modified()?,
            accessed: metadata.accessed()?,
        })
    }

    pub fn display(&self) -> String {
        format!(
            "{}: {} bytes, modified_at: {}, accessed_at: {}",
            self.path.display(),
            self.size,
            self.modified_timestamp(),
            self.accessed_timestamp()
        )
    }

    pub fn modified_timestamp(&self) -> u64 {
        self.modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs()
    }

    /// Returns last access time as Unix timestamp (seconds)
    pub fn accessed_timestamp(&self) -> u64 {
        self.accessed
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs()
    }
}

// Implement Eq/Hash/PartialEq for FileInfo based only on its path so it can be used in HashSet/HashMap
impl PartialEq for FileInfo {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for FileInfo {}

impl std::hash::Hash for FileInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Hash only by path to keep identity by file path
        std::hash::Hash::hash(&self.path, state);
    }
}
