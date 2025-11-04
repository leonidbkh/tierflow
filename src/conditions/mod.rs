mod active_window;
mod always_true;
mod file_extension;
mod file_size;
mod filename_contains;
mod max_age;
mod path_prefix;

pub use active_window::ActiveWindowCondition;
pub use always_true::AlwaysTrueCondition;
pub use file_extension::{ExtensionMode, FileExtensionCondition};
pub use file_size::FileSizeCondition;
pub use filename_contains::{ContainsMode, FilenameContainsCondition};
pub use max_age::MaxAgeCondition;
pub use path_prefix::{PathPrefixCondition, PrefixMode};

use crate::{FileInfo, GlobalStats};
use std::path::PathBuf;
use std::sync::Arc;

/// Execution context for conditions, allows passing additional information
#[derive(Debug, Clone)]
pub struct Context {
    /// Path to current tier root (for computing relative paths)
    pub current_tier_path: Option<PathBuf>,

    /// Global statistics collected from all files (optional)
    /// Shared across all condition evaluations via Arc
    pub global_stats: Option<Arc<GlobalStats>>,
}

impl Context {
    pub const fn new() -> Self {
        Self {
            current_tier_path: None,
            global_stats: None,
        }
    }

    pub fn with_tier_path(mut self, tier_path: PathBuf) -> Self {
        self.current_tier_path = Some(tier_path);
        self
    }

    pub fn with_global_stats(mut self, stats: &Arc<GlobalStats>) -> Self {
        self.global_stats = Some(Arc::clone(stats));
        self
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for conditions that determine if a file matches a strategy
pub trait Condition: Send + Sync {
    fn matches(&self, file: &FileInfo, context: &Context) -> bool;
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let context = Context::new();
        let _ = format!("{context:?}");
    }

    #[test]
    fn test_context_default() {
        let context = Context::default();
        let _ = format!("{context:?}");
    }

    #[test]
    fn test_conditions_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn Condition>>();
    }
}
