#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod balancer;
pub mod cli;
pub mod conditions;
pub mod config;
pub mod error;
pub mod executor;
pub mod file;
pub mod lock;
pub mod mover;
pub mod stats;
pub mod strategy;
pub mod tautulli;
pub mod tier;

pub use balancer::{Balancer, BalancingPlan, PlacementDecision, PlanWarning, TierUsageProjection};
pub use cli::{Cli, Commands, default_config_path};
pub use conditions::{
    ActiveWindowCondition, AlwaysTrueCondition, Condition, ContainsMode, Context, ExtensionMode,
    FileExtensionCondition, FileSizeCondition, FilenameContainsCondition, MaxAgeCondition,
    PathPrefixCondition, PrefixMode,
};
pub use config::{
    BalancingConfig, ConditionConfig, ConfigError, MoverConfig, MoverType, PlacementStrategyConfig,
    StrategyAction, TautulliConfig, TierConfig,
};
pub use error::{AppError, Result};
pub use executor::{ExecutionError, ExecutionResult, Executor};
pub use file::FileInfo;
pub use lock::TierLockGuard;
pub use mover::{DryRunMover, Mover, RsyncMover};
pub use stats::{FileStats, GlobalStats};
pub use strategy::PlacementStrategy;
pub use tautulli::{
    EpisodeInfo, HistoryItem, ShowProgress, TautulliClient, TautulliStats, build_progress,
    normalize_show_name, parse_episode,
};
pub use tier::Tier;
