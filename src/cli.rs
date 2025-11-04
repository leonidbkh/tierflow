use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mergerfs-balancer")]
#[command(version = "2.0.0")]
#[command(about = "Strategy-based intelligent file balancing for mergerfs", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Plan and execute file rebalancing between tiers
    Rebalance {
        /// Path to configuration file
        #[arg(short, long, value_name = "FILE", default_value = "config.yaml")]
        config: PathBuf,

        /// Dry-run mode: show plan without executing moves
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Run in daemon mode with periodic rebalancing
    Daemon {
        /// Path to configuration file
        #[arg(short, long, value_name = "FILE", default_value = "config.yaml")]
        config: PathBuf,

        /// Dry-run mode: show plan without executing moves
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Interval between rebalance runs (in seconds)
        #[arg(short, long, value_name = "SECONDS", default_value = "3600")]
        interval: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_verify() {
        // Проверяет что CLI правильно сконфигурирован
        Cli::command().debug_assert();
    }

    #[test]
    fn test_cli_help() {
        let result = Cli::command().try_get_matches_from(vec!["mergerfs-balancer", "--help"]);
        // --help вызывает DisplayHelp error (это нормально)
        assert!(result.is_err());
    }

    #[test]
    fn test_rebalance_with_config() {
        let cli = Cli::parse_from(vec!["mergerfs-balancer", "rebalance", "-c", "test.yaml"]);
        match cli.command {
            Commands::Rebalance { config, dry_run } => {
                assert_eq!(config, PathBuf::from("test.yaml"));
                assert!(!dry_run);
            }
            _ => panic!("Expected Rebalance command"),
        }
    }

    #[test]
    fn test_rebalance_default_config() {
        let cli = Cli::parse_from(vec!["mergerfs-balancer", "rebalance"]);
        match cli.command {
            Commands::Rebalance { config, dry_run } => {
                assert_eq!(config, PathBuf::from("config.yaml"));
                assert!(!dry_run);
            }
            _ => panic!("Expected Rebalance command"),
        }
    }

    #[test]
    fn test_rebalance_dry_run() {
        let cli = Cli::parse_from(vec!["mergerfs-balancer", "rebalance", "--dry-run"]);
        match cli.command {
            Commands::Rebalance { config, dry_run } => {
                assert_eq!(config, PathBuf::from("config.yaml"));
                assert!(dry_run);
            }
            _ => panic!("Expected Rebalance command"),
        }
    }

    #[test]
    fn test_rebalance_short_flags() {
        let cli = Cli::parse_from(vec![
            "mergerfs-balancer",
            "rebalance",
            "-c",
            "custom.yaml",
            "-n",
        ]);
        match cli.command {
            Commands::Rebalance { config, dry_run } => {
                assert_eq!(config, PathBuf::from("custom.yaml"));
                assert!(dry_run);
            }
            _ => panic!("Expected Rebalance command"),
        }
    }

    #[test]
    fn test_daemon_default() {
        let cli = Cli::parse_from(vec!["mergerfs-balancer", "daemon"]);
        match cli.command {
            Commands::Daemon {
                config,
                dry_run,
                interval,
            } => {
                assert_eq!(config, PathBuf::from("config.yaml"));
                assert!(!dry_run);
                assert_eq!(interval, 3600);
            }
            _ => panic!("Expected Daemon command"),
        }
    }

    #[test]
    fn test_daemon_with_interval() {
        let cli = Cli::parse_from(vec!["mergerfs-balancer", "daemon", "-i", "600"]);
        match cli.command {
            Commands::Daemon {
                config,
                dry_run,
                interval,
            } => {
                assert_eq!(config, PathBuf::from("config.yaml"));
                assert!(!dry_run);
                assert_eq!(interval, 600);
            }
            _ => panic!("Expected Daemon command"),
        }
    }

    #[test]
    fn test_daemon_all_flags() {
        let cli = Cli::parse_from(vec![
            "mergerfs-balancer",
            "daemon",
            "-c",
            "custom.yaml",
            "-n",
            "-i",
            "1800",
        ]);
        match cli.command {
            Commands::Daemon {
                config,
                dry_run,
                interval,
            } => {
                assert_eq!(config, PathBuf::from("custom.yaml"));
                assert!(dry_run);
                assert_eq!(interval, 1800);
            }
            _ => panic!("Expected Daemon command"),
        }
    }
}
