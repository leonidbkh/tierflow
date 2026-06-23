use crate::error::Result;
use crate::tdarr::TdarrMoveBlocker;
use std::path::{Path, PathBuf};

const NO_OP_MOVE_BLOCKER_NAME: &str = "none";
const COMPOSITE_MOVE_BLOCKER_NAME: &str = "composite";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockReason {
    pub provider: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockDecision {
    Allowed,
    Blocked(BlockReason),
}

pub trait MoveBlocker: Send + Sync {
    fn name(&self) -> &str;
    fn snapshot(&self, candidates: &[PathBuf]) -> Result<Box<dyn MoveBlockerSnapshot>>;
}

pub trait MoveBlockerSnapshot: Send + Sync {
    fn check(&self, path: &Path) -> BlockDecision;
}

pub struct NoOpMoveBlocker;

impl MoveBlocker for NoOpMoveBlocker {
    fn name(&self) -> &str {
        NO_OP_MOVE_BLOCKER_NAME
    }

    fn snapshot(&self, _candidates: &[PathBuf]) -> Result<Box<dyn MoveBlockerSnapshot>> {
        Ok(Box::new(NoOpMoveBlockerSnapshot))
    }
}

struct NoOpMoveBlockerSnapshot;

impl MoveBlockerSnapshot for NoOpMoveBlockerSnapshot {
    fn check(&self, _path: &Path) -> BlockDecision {
        BlockDecision::Allowed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockerErrorPolicy {
    FailClosed,
    FailOpen,
}

pub struct CompositeMoveBlocker {
    blockers: Vec<Box<dyn MoveBlocker>>,
    on_error: BlockerErrorPolicy,
}

impl CompositeMoveBlocker {
    pub fn new(blockers: Vec<Box<dyn MoveBlocker>>, on_error: BlockerErrorPolicy) -> Self {
        Self { blockers, on_error }
    }
}

impl MoveBlocker for CompositeMoveBlocker {
    fn name(&self) -> &str {
        COMPOSITE_MOVE_BLOCKER_NAME
    }

    fn snapshot(&self, candidates: &[PathBuf]) -> Result<Box<dyn MoveBlockerSnapshot>> {
        let mut snapshots = Vec::new();
        let mut fail_closed_reason = None;

        for blocker in &self.blockers {
            match blocker.snapshot(candidates) {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(err) => match self.on_error {
                    BlockerErrorPolicy::FailClosed => {
                        let reason = BlockReason {
                            provider: blocker.name().to_string(),
                            reason: format!("blocker snapshot failed: {err}"),
                        };
                        tracing::error!(
                            "Move blocker '{}' failed; blocking candidate moves: {}",
                            blocker.name(),
                            err
                        );
                        fail_closed_reason = Some(reason);
                    }
                    BlockerErrorPolicy::FailOpen => {
                        tracing::warn!(
                            "Move blocker '{}' failed; proceeding without it: {}",
                            blocker.name(),
                            err
                        );
                    }
                },
            }
        }

        Ok(Box::new(CompositeMoveBlockerSnapshot {
            snapshots,
            fail_closed_reason,
        }))
    }
}

struct CompositeMoveBlockerSnapshot {
    snapshots: Vec<Box<dyn MoveBlockerSnapshot>>,
    fail_closed_reason: Option<BlockReason>,
}

impl MoveBlockerSnapshot for CompositeMoveBlockerSnapshot {
    fn check(&self, path: &Path) -> BlockDecision {
        if let Some(reason) = &self.fail_closed_reason {
            return BlockDecision::Blocked(reason.clone());
        }

        for snapshot in &self.snapshots {
            let decision = snapshot.check(path);
            if matches!(decision, BlockDecision::Blocked(_)) {
                return decision;
            }
        }

        BlockDecision::Allowed
    }
}

pub fn build_blocker_error_policy(
    value: &crate::config::BlockerErrorPolicyConfig,
) -> BlockerErrorPolicy {
    match value {
        crate::config::BlockerErrorPolicyConfig::FailClosed => BlockerErrorPolicy::FailClosed,
        crate::config::BlockerErrorPolicyConfig::FailOpen => BlockerErrorPolicy::FailOpen,
    }
}

pub fn build_provider(
    config: crate::config::BlockerProviderConfig,
) -> Result<Box<dyn MoveBlocker>> {
    match config {
        crate::config::BlockerProviderConfig::Tdarr(config) => {
            Ok(Box::new(TdarrMoveBlocker::new(config)?))
        }
    }
}

pub struct StaticMoveBlocker {
    blocked_paths: Vec<PathBuf>,
    provider: String,
    reason: String,
}

impl StaticMoveBlocker {
    pub fn new(blocked_paths: Vec<PathBuf>, provider: String, reason: String) -> Self {
        Self {
            blocked_paths,
            provider,
            reason,
        }
    }
}

impl MoveBlocker for StaticMoveBlocker {
    fn name(&self) -> &str {
        &self.provider
    }

    fn snapshot(&self, _candidates: &[PathBuf]) -> Result<Box<dyn MoveBlockerSnapshot>> {
        Ok(Box::new(StaticMoveBlockerSnapshot {
            blocked_paths: self.blocked_paths.clone(),
            reason: BlockReason {
                provider: self.provider.clone(),
                reason: self.reason.clone(),
            },
        }))
    }
}

struct StaticMoveBlockerSnapshot {
    blocked_paths: Vec<PathBuf>,
    reason: BlockReason,
}

impl MoveBlockerSnapshot for StaticMoveBlockerSnapshot {
    fn check(&self, path: &Path) -> BlockDecision {
        if self.blocked_paths.iter().any(|blocked| blocked == path) {
            BlockDecision::Blocked(self.reason.clone())
        } else {
            BlockDecision::Allowed
        }
    }
}

pub fn snapshot_or_fail_closed(
    blocker: &dyn MoveBlocker,
    candidates: &[PathBuf],
) -> Box<dyn MoveBlockerSnapshot> {
    match blocker.snapshot(candidates) {
        Ok(snapshot) => snapshot,
        Err(err) => Box::new(FailClosedMoveBlockerSnapshot {
            reason: BlockReason {
                provider: blocker.name().to_string(),
                reason: format!("blocker snapshot failed: {err}"),
            },
        }),
    }
}

struct FailClosedMoveBlockerSnapshot {
    reason: BlockReason,
}

impl MoveBlockerSnapshot for FailClosedMoveBlockerSnapshot {
    fn check(&self, _path: &Path) -> BlockDecision {
        BlockDecision::Blocked(self.reason.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_move_blocker_blocks_exact_path() {
        let blocked = PathBuf::from("/media/movie.mkv");
        let blocker = StaticMoveBlocker::new(
            vec![blocked.clone()],
            "test".to_string(),
            "queued".to_string(),
        );

        let snapshot = blocker.snapshot(&[]).unwrap();

        assert_eq!(
            snapshot.check(&blocked),
            BlockDecision::Blocked(BlockReason {
                provider: "test".to_string(),
                reason: "queued".to_string(),
            })
        );
        assert_eq!(
            snapshot.check(Path::new("/media/other.mkv")),
            BlockDecision::Allowed
        );
    }
}
