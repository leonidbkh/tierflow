use std::path::{Path, PathBuf};
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

pub fn is_internal_artifact_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_internal_artifact_name)
}

pub fn is_internal_artifact_name(name: &str) -> bool {
    name == ".tierflow.lock"
        || name.starts_with(".tierflow-remove-check-")
        || name.ends_with(".partial")
        || has_backup_suffix(name)
}

fn has_backup_suffix(name: &str) -> bool {
    let Some((_, suffix)) = name.rsplit_once(".backup-") else {
        return false;
    };

    !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_internal_artifact_names() {
        assert!(is_internal_artifact_name(".tierflow.lock"));
        assert!(is_internal_artifact_name(
            ".tierflow-remove-check-123-1790000000-movie.mkv"
        ));
        assert!(is_internal_artifact_name("movie.mkv.partial"));
        assert!(is_internal_artifact_name("movie.mkv.backup-1790000000"));

        assert!(!is_internal_artifact_name("backup-note.txt"));
        assert!(!is_internal_artifact_name("movie.mkv.backup-latest"));
        assert!(!is_internal_artifact_name("movie.mkv.backup-"));
        assert!(!is_internal_artifact_name("movie.partial.mkv"));
    }

    #[test]
    fn test_internal_artifact_paths() {
        assert!(is_internal_artifact_path(Path::new(
            "/mnt/storage/movie.mkv.backup-1790000000"
        )));
        assert!(!is_internal_artifact_path(Path::new(
            "/mnt/storage/backup-note.txt"
        )));
    }
}
