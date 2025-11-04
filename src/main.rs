use clap::Parser;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tierflow::{
    Balancer, BalancingConfig, Cli, Commands, DryRunMover, Executor, Mover, MoverType,
    PlacementDecision, RsyncMover, TierLockGuard,
};

fn main() {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Parse CLI arguments
    let cli = Cli::parse();

    match cli.command {
        Commands::Rebalance { config, dry_run } => {
            if let Err(e) = run_rebalance(&config, dry_run) {
                log::error!("Error: {e}");
                process::exit(1);
            }
        }
        Commands::Daemon {
            config,
            dry_run,
            interval,
        } => {
            if let Err(e) = run_daemon(&config, dry_run, interval) {
                log::error!("Error: {e}");
                process::exit(1);
            }
        }
    }
}

fn run_rebalance(
    config_path: &std::path::Path,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Loading configuration from: {}", config_path.display());

    // Load configuration
    let config = BalancingConfig::from_file(config_path)?;

    // Extract config fields before consuming config
    let tautulli_config = config.tautulli.clone();
    let mover_config = config.mover.clone();

    // Convert configuration to runtime objects
    let tiers: Vec<_> = config
        .tiers
        .into_iter()
        .map(tierflow::TierConfig::into_tier)
        .collect::<Result<_, _>>()?;

    let strategies: Vec<_> = config
        .strategies
        .into_iter()
        .map(tierflow::PlacementStrategyConfig::into_strategy)
        .collect();

    log::info!(
        "Configuration loaded: {} tiers, {} strategies{}",
        tiers.len(),
        strategies.len(),
        if tautulli_config.is_some() {
            " (Tautulli enabled)"
        } else {
            ""
        }
    );

    // Acquire locks on all tiers before proceeding
    let _lock_guard = match TierLockGuard::try_lock_tiers(&tiers) {
        Ok(guard) => {
            log::info!("Acquired locks on {} tiers", guard.locked_tiers().len());
            guard
        }
        Err(tierflow::AppError::TierLocked {
            tier,
            owner_pid,
            owner_host,
            locked_for,
        }) => {
            eprintln!(
                "Error: Tier '{tier}' is locked by process {owner_pid} on {owner_host} (running for {locked_for:?})"
            );
            eprintln!("Another instance is already working with this tier. Exiting.");
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Error acquiring locks: {e}");
            process::exit(1);
        }
    };

    // Create Balancer
    let balancer = Balancer::new(tiers.clone(), strategies, tautulli_config);

    // Plan rebalance
    log::info!("Planning rebalance...");
    let plan = balancer.plan_rebalance();

    // Output plan
    print_plan(&plan);

    // Execute plan
    println!("\nExecuting plan...");

    // Choose mover based on config and dry-run flag
    let mover: Box<dyn Mover> = if dry_run {
        log::info!("Dry-run mode: using DryRunMover");
        Box::new(DryRunMover)
    } else {
        // Use mover from config
        match mover_config.mover_type {
            MoverType::Rsync => {
                log::info!("Using RsyncMover from config");
                Box::new(RsyncMover::with_args(mover_config.extra_args))
            }
            MoverType::DryRun => {
                log::info!("Using DryRunMover from config");
                Box::new(DryRunMover)
            }
        }
    };

    let result = Executor::execute_plan(&plan, mover.as_ref(), &tiers);

    if dry_run {
        println!("\n[DRY-RUN MODE] No files were actually moved");
    }

    println!("\nExecution complete:");
    println!("  Files moved: {}", result.files_moved);
    println!("  Files stayed: {}", result.files_stayed);
    println!(
        "  Bytes moved: {} ({:.2} GB)",
        result.bytes_moved,
        result.bytes_moved as f64 / 1_000_000_000.0
    );

    if !result.errors.is_empty() {
        println!("\nErrors ({}):", result.errors.len());
        for error in &result.errors {
            println!(
                "  {} -> {}: {}",
                error.from_tier, error.to_tier, error.error
            );
        }
    }

    Ok(())
}

fn run_daemon(
    config_path: &std::path::Path,
    dry_run: bool,
    interval: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!(
        "Starting daemon mode (interval: {}s, config: {})",
        interval,
        config_path.display()
    );

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    if let Err(e) = ctrlc::set_handler(move || {
        log::info!("Received interrupt signal, shutting down gracefully...");
        r.store(false, Ordering::SeqCst);
    }) {
        log::warn!("Failed to set Ctrl-C handler: {}", e);
    }

    let mut run_number = 1;

    while running.load(Ordering::SeqCst) {
        log::info!("===== Daemon run #{run_number} =====");

        match run_rebalance(config_path, dry_run) {
            Ok(()) => {
                log::info!("Rebalance completed successfully");
            }
            Err(e) => {
                log::error!("Rebalance failed: {e}");
                // Continue running even after errors
            }
        }

        if !running.load(Ordering::SeqCst) {
            break;
        }

        log::info!("Sleeping for {interval} seconds until next run...");

        // Sleep in smaller chunks to allow quick shutdown
        let sleep_chunk = Duration::from_secs(1);
        let chunks = interval;

        for _ in 0..chunks {
            if !running.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(sleep_chunk);
        }

        run_number += 1;
    }

    log::info!("Daemon stopped gracefully");
    Ok(())
}

fn print_plan(plan: &tierflow::BalancingPlan) {
    println!("\n=== Balancing Plan ===");

    // Warnings
    if !plan.warnings.is_empty() {
        println!("\nWarnings ({}):", plan.warnings.len());
        for warning in &plan.warnings {
            match warning {
                tierflow::PlanWarning::InsufficientSpace {
                    file,
                    strategy,
                    needed,
                    available,
                } => {
                    println!("  [INSUFFICIENT SPACE] {}", file.display());
                    println!("    Strategy: {strategy}");
                    println!("    Needed: {needed} bytes, Available: {available} bytes");
                }
                tierflow::PlanWarning::RequiredStrategyFailed {
                    strategy,
                    file,
                    reason,
                } => {
                    println!("  [REQUIRED STRATEGY FAILED] {}", file.display());
                    println!("    Strategy: {strategy}");
                    println!("    Reason: {reason}");
                }
            }
        }
    }

    // Tier projections
    println!("\nTier Usage Projections:");
    for (tier_name, projection) in &plan.projected_tier_usage {
        println!("  {tier_name}:");
        println!(
            "    Current:   {}% ({:.2} GB used, {:.2} GB free)",
            projection.current_percent,
            projection.current_used as f64 / 1_000_000_000.0,
            projection.current_free as f64 / 1_000_000_000.0
        );
        println!(
            "    Projected: {}% ({:.2} GB used, {:.2} GB free)",
            projection.projected_percent,
            projection.projected_used as f64 / 1_000_000_000.0,
            projection.projected_free as f64 / 1_000_000_000.0
        );

        let change = projection.projected_percent as i64 - projection.current_percent as i64;
        if change != 0 {
            let arrow = if change > 0 { "↑" } else { "↓" };
            println!("    Change: {} {}%", arrow, change.abs());
        }
    }

    // Decisions summary
    let promote_count = plan
        .decisions
        .iter()
        .filter(|d| matches!(d, PlacementDecision::Promote { .. }))
        .count();
    let demote_count = plan
        .decisions
        .iter()
        .filter(|d| matches!(d, PlacementDecision::Demote { .. }))
        .count();
    let stay_count = plan.stay_count();

    println!("\nDecisions Summary:");
    println!("  Total files: {}", plan.total_files());
    println!("  Promote: {promote_count}");
    println!("  Demote: {demote_count}");
    println!("  Stay: {stay_count}");

    // Show first 10 moves
    let moves: Vec<_> = plan
        .decisions
        .iter()
        .filter(|d| !matches!(d, PlacementDecision::Stay { .. }))
        .take(10)
        .collect();

    if !moves.is_empty() {
        println!(
            "\nPlanned Moves (showing first 10 of {}):",
            plan.move_count()
        );
        for decision in moves {
            match decision {
                PlacementDecision::Promote {
                    file,
                    from_tier,
                    to_tier,
                    strategy,
                    priority,
                } => {
                    println!("  ↑ PROMOTE [priority={priority}]");
                    println!("    File: {}", file.path.display());
                    println!("    {from_tier} -> {to_tier} (strategy: {strategy})");
                }
                PlacementDecision::Demote {
                    file,
                    from_tier,
                    to_tier,
                    strategy,
                    priority,
                } => {
                    println!("  ↓ DEMOTE [priority={priority}]");
                    println!("    File: {}", file.path.display());
                    println!("    {from_tier} -> {to_tier} (strategy: {strategy})");
                }
                PlacementDecision::Stay { .. } => {}
            }
        }

        if plan.move_count() > 10 {
            println!("  ... and {} more", plan.move_count() - 10);
        }
    }

    if plan.is_empty() {
        println!("\n✓ System is balanced, no moves needed");
    }
}
