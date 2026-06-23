use crate::conditions::{
    ActiveWindowCondition, AgeCondition, AlwaysTrueCondition, FileExtensionCondition,
    FileSizeCondition, FilenameContainsCondition, PathPrefixCondition,
};
use crate::config::{
    BlockersConfig, ConditionConfig, MoverConfig, MoverType, PlacementStrategyConfig,
};
use crate::error::Result;
use crate::move_blocker::{self, CompositeMoveBlocker, MoveBlocker, NoOpMoveBlocker};
use crate::{
    Condition, DryRunMover, FileChecker, Mover, PlacementStrategy, RsyncMover, SmartFileChecker,
};

pub fn build_strategy(config: PlacementStrategyConfig) -> PlacementStrategy {
    let mut strategy = PlacementStrategy::new(config.name, config.priority);

    for condition_config in config.conditions {
        strategy = strategy.add_condition(build_condition(condition_config));
    }

    for tier_name in config.preferred_tiers {
        strategy = strategy.add_preferred_tier(tier_name);
    }

    if config.required {
        strategy = strategy.required();
    }

    strategy.action = config.action;

    strategy
}

pub fn build_condition(config: ConditionConfig) -> Box<dyn Condition> {
    match config {
        ConditionConfig::AlwaysTrue => Box::new(AlwaysTrueCondition),
        ConditionConfig::Age {
            min_hours,
            max_hours,
        } => Box::new(AgeCondition::new(min_hours, max_hours)),
        ConditionConfig::FileSize {
            min_size_mb,
            max_size_mb,
        } => Box::new(FileSizeCondition::new(min_size_mb, max_size_mb)),
        ConditionConfig::FileExtension { extensions, mode } => Box::new(
            FileExtensionCondition::new_with_mode(extensions, mode.into()),
        ),
        ConditionConfig::PathPrefix { prefix, mode } => {
            Box::new(PathPrefixCondition::new_with_mode(prefix, mode.into()))
        }
        ConditionConfig::FilenameContains {
            patterns,
            mode,
            case_sensitive,
        } => {
            if case_sensitive {
                Box::new(FilenameContainsCondition::new_with_mode(
                    patterns,
                    mode.into(),
                ))
            } else {
                Box::new(FilenameContainsCondition::new_case_insensitive(
                    patterns,
                    mode.into(),
                ))
            }
        }
        ConditionConfig::ActiveWindow { name } => Box::new(ActiveWindowCondition::new(name)),
    }
}

/// Create a mover based on configuration
/// Uses a consistent hasher implementation across all movers
pub fn build_mover(config: Option<&MoverConfig>, dry_run: bool) -> Box<dyn Mover> {
    if dry_run {
        tracing::info!("Dry-run mode: using DryRunMover");
        return Box::new(DryRunMover);
    }

    if let Some(config) = config {
        match config.mover_type {
            MoverType::Rsync => {
                tracing::info!("Using RsyncMover");
                Box::new(RsyncMover::with_args(config.extra_args.clone()))
            }
            MoverType::DryRun => {
                tracing::info!("Using DryRunMover from config");
                Box::new(DryRunMover)
            }
        }
    } else {
        tracing::info!("Using RsyncMover (default)");
        Box::new(RsyncMover::new())
    }
}

/// Create a file checker with default implementation
pub fn build_file_checker() -> Box<dyn FileChecker> {
    Box::new(SmartFileChecker::new())
}

/// Create a move blocker from configuration.
pub fn build_move_blocker(config: Option<&BlockersConfig>) -> Result<Box<dyn MoveBlocker>> {
    let Some(config) = config else {
        tracing::info!("No move blockers configured");
        return Ok(Box::new(NoOpMoveBlocker));
    };

    if config.providers.is_empty() {
        tracing::info!("Move blockers section is empty");
        return Ok(Box::new(NoOpMoveBlocker));
    }

    let providers = config
        .providers
        .clone()
        .into_iter()
        .map(move_blocker::build_provider)
        .collect::<Result<Vec<_>>>()?;
    let on_error = move_blocker::build_blocker_error_policy(&config.on_error);

    tracing::info!("Configured {} move blocker provider(s)", providers.len());

    Ok(Box::new(CompositeMoveBlocker::new(providers, on_error)))
}
