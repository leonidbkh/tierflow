use clap::Parser;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tierflow::{
    Balancer, BalancingConfig, Cli, Commands, Executor, OutputFormat, PlacementDecision,
    TierLockGuard, factory,
};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;

fn main() {
    // Parse CLI arguments first to get logging settings
    let cli = Cli::parse();

    // Setup tracing based on CLI flags
    match &cli.command {
        Commands::Rebalance { verbose, quiet, .. } | Commands::Daemon { verbose, quiet, .. } => {
            setup_tracing(*verbose, *quiet);
        }
    }

    match cli.command {
        Commands::Rebalance {
            config,
            dry_run,
            format,
            ..
        } => {
            if let Err(e) = run_rebalance(&config, dry_run, format) {
                tracing::error!("Error: {e}");
                process::exit(1);
            }
        }
        Commands::Daemon {
            config,
            dry_run,
            interval,
            format,
            ..
        } => {
            if let Err(e) = run_daemon(&config, dry_run, interval, format) {
                tracing::error!("Error: {e}");
                process::exit(1);
            }
        }
    }
}

/// Setup tracing subscriber based on verbosity level
fn setup_tracing(verbose: u8, quiet: bool) {
    let filter = if quiet {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("error"))
    } else {
        let level = match verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        };
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(format!("tierflow={level}")))
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_span_events(FmtSpan::NONE)
        .with_writer(std::io::stderr) // All logs to stderr
        .init();
}

fn run_rebalance(
    config_path: &std::path::Path,
    dry_run: bool,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Loading configuration from: {}", config_path.display());

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
        .map(tierflow::factory::build_strategy)
        .collect();

    tracing::info!(
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
            tracing::info!("Acquired lock for {} tiers", tiers.len());
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
    tracing::info!("Planning rebalance...");
    let plan = balancer.plan_rebalance();

    // Output plan to stderr (for human consumption)
    if !matches!(format, OutputFormat::Json | OutputFormat::Yaml) {
        print_plan(&plan);
    }

    // Execute plan
    tracing::info!("Executing plan...");

    // Use factory functions for consistent initialization
    let mover = factory::build_mover(Some(&mover_config), dry_run);
    let file_checker = factory::build_file_checker();
    let result = Executor::execute_plan(&plan, mover.as_ref(), &tiers, file_checker.as_ref());

    // Output result to stdout based on format
    match format {
        OutputFormat::Json => {
            let output = serde_json::json!({
                "files_moved": result.files_moved,
                "files_stayed": result.files_stayed,
                "bytes_moved": result.bytes_moved,
                "dry_run": dry_run,
                "errors": result.errors.iter().map(|e| serde_json::json!({
                    "file": e.file.display().to_string(),
                    "from_tier": &e.from_tier,
                    "to_tier": &e.to_tier,
                    "error": &e.error,
                })).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        }
        OutputFormat::Yaml => {
            let output = serde_json::json!({
                "files_moved": result.files_moved,
                "files_stayed": result.files_stayed,
                "bytes_moved": result.bytes_moved,
                "dry_run": dry_run,
                "errors": result.errors.iter().map(|e| serde_json::json!({
                    "file": e.file.display().to_string(),
                    "from_tier": &e.from_tier,
                    "to_tier": &e.to_tier,
                    "error": &e.error,
                })).collect::<Vec<_>>(),
            });
            println!("{}", serde_yaml::to_string(&output)?);
        }
        OutputFormat::Text => {
            if dry_run {
                eprintln!("\n[DRY-RUN MODE] No files were actually moved");
            }

            eprintln!("\nExecution complete:");
            eprintln!("  Files moved: {}", result.files_moved);
            eprintln!("  Files stayed: {}", result.files_stayed);
            eprintln!(
                "  Bytes moved: {} ({:.2} GB)",
                result.bytes_moved,
                result.bytes_moved as f64 / 1_000_000_000.0
            );

            if !result.errors.is_empty() {
                eprintln!("\nErrors ({}):", result.errors.len());
                for error in &result.errors {
                    eprintln!(
                        "  {} -> {}: {}",
                        error.from_tier, error.to_tier, error.error
                    );
                }
            }
        }
    }

    Ok(())
}

fn run_daemon(
    config_path: &std::path::Path,
    dry_run: bool,
    interval: u64,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(
        "Starting daemon mode (interval: {}s, config: {})",
        interval,
        config_path.display()
    );

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    if let Err(e) = ctrlc::set_handler(move || {
        tracing::info!("Received interrupt signal, shutting down gracefully...");
        r.store(false, Ordering::SeqCst);
    }) {
        tracing::warn!("Failed to set Ctrl-C handler: {}", e);
    }

    let mut run_number = 1;

    while running.load(Ordering::SeqCst) {
        tracing::info!("===== Daemon run #{run_number} =====");

        match run_rebalance(config_path, dry_run, format) {
            Ok(()) => {
                tracing::info!("Rebalance completed successfully");
            }
            Err(e) => {
                tracing::error!("Rebalance failed: {e}");
                // Continue running even after errors
            }
        }

        if !running.load(Ordering::SeqCst) {
            break;
        }

        tracing::info!("Sleeping for {interval} seconds until next run...");

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

    tracing::info!("Daemon stopped gracefully");
    Ok(())
}

fn print_plan(plan: &tierflow::BalancingPlan) {
    eprintln!("\n=== Balancing Plan ===");

    // Warnings
    if !plan.warnings.is_empty() {
        eprintln!("\nWarnings ({}):", plan.warnings.len());
        for warning in &plan.warnings {
            match warning {
                tierflow::PlanWarning::InsufficientSpace {
                    file,
                    strategy,
                    needed,
                    available,
                } => {
                    eprintln!("  [INSUFFICIENT SPACE] {}", file.display());
                    eprintln!("    Strategy: {strategy}");
                    eprintln!("    Needed: {needed} bytes, Available: {available} bytes");
                }
                tierflow::PlanWarning::RequiredStrategyFailed {
                    strategy,
                    file,
                    reason,
                } => {
                    eprintln!("  [REQUIRED STRATEGY FAILED] {}", file.display());
                    eprintln!("    Strategy: {strategy}");
                    eprintln!("    Reason: {reason}");
                }
            }
        }
    }

    // Tier projections
    eprintln!("\nTier Usage Projections:");
    for (tier_name, projection) in &plan.projected_tier_usage {
        eprintln!("  {tier_name}:");
        eprintln!(
            "    Current:   {}% ({:.2} GB used, {:.2} GB free)",
            projection.current_percent,
            projection.current_used as f64 / 1_000_000_000.0,
            projection.current_free as f64 / 1_000_000_000.0
        );
        eprintln!(
            "    Projected: {}% ({:.2} GB used, {:.2} GB free)",
            projection.projected_percent,
            projection.projected_used as f64 / 1_000_000_000.0,
            projection.projected_free as f64 / 1_000_000_000.0
        );

        let change = projection.projected_percent as i64 - projection.current_percent as i64;
        if change != 0 {
            let arrow = if change > 0 { "↑" } else { "↓" };
            eprintln!("    Change: {} {}%", arrow, change.abs());
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

    eprintln!("\nDecisions Summary:");
    eprintln!("  Total files: {}", plan.total_files());
    eprintln!("  Promote: {promote_count}");
    eprintln!("  Demote: {demote_count}");
    eprintln!("  Stay: {stay_count}");

    // Show first 10 moves
    let moves: Vec<_> = plan
        .decisions
        .iter()
        .filter(|d| !matches!(d, PlacementDecision::Stay { .. }))
        .take(10)
        .collect();

    if !moves.is_empty() {
        eprintln!(
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
                    eprintln!("  ↑ PROMOTE [priority={priority}]");
                    eprintln!("    File: {}", file.path.display());
                    eprintln!("    {from_tier} -> {to_tier} (strategy: {strategy})");
                }
                PlacementDecision::Demote {
                    file,
                    from_tier,
                    to_tier,
                    strategy,
                    priority,
                } => {
                    eprintln!("  ↓ DEMOTE [priority={priority}]");
                    eprintln!("    File: {}", file.path.display());
                    eprintln!("    {from_tier} -> {to_tier} (strategy: {strategy})");
                }
                PlacementDecision::Stay { .. } => {}
            }
        }

        if plan.move_count() > 10 {
            eprintln!("  ... and {} more", plan.move_count() - 10);
        }
    }

    if plan.is_empty() {
        eprintln!("\n✓ System is balanced, no moves needed");
    }
}
