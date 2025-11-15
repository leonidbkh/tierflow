use crate::conditions::{
    ActiveWindowCondition, AlwaysTrueCondition, FileExtensionCondition, FileSizeCondition,
    FilenameContainsCondition, MaxAgeCondition, PathPrefixCondition,
};
use crate::config::{ConditionConfig, PlacementStrategyConfig};
use crate::{Condition, PlacementStrategy};

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
        ConditionConfig::MaxAge { max_age_hours } => Box::new(MaxAgeCondition::new(max_age_hours)),
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
