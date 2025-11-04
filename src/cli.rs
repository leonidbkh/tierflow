use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Find default config path with priority:
/// 1. /etc/tierflow/config.yaml (system-wide, preferred)
/// 2. ~/.config/tierflow/config.yaml (user-specific)
/// 3. Fallback to /etc even if doesn't exist
pub fn default_config_path() -> PathBuf {
    let etc_path = PathBuf::from("/etc/tierflow/config.yaml");

    // Check system-wide config first
    if etc_path.exists() {
        return etc_path;
    }

    // Check user config
    if let Some(config_dir) = dirs::config_dir() {
        let user_path = config_dir.join("tierflow/config.yaml");
        if user_path.exists() {
            return user_path;
        }
    }

    // Fallback to /etc (will show clear error if missing)
    etc_path
}

#[derive(Parser)]
#[command(name = "tierflow")]
#[command(version)]
#[command(about = "Automatic file balancing for tiered storage", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Plan and execute file rebalancing between tiers
    Rebalance {
        /// Path to configuration file
        #[arg(short, long, value_name = "FILE", default_value_os_t = default_config_path())]
        config: PathBuf,

        /// Dry-run mode: show plan without executing moves
        #[arg(short = 'n', long)]
        dry_run: bool,
    },

    /// Run in daemon mode with periodic rebalancing
    Daemon {
        /// Path to configuration file
        #[arg(short, long, value_name = "FILE", default_value_os_t = default_config_path())]
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
        let result = Cli::command().try_get_matches_from(vec!["tierflow", "--help"]);
        // --help вызывает DisplayHelp error (это нормально)
        assert!(result.is_err());
    }

    #[test]
    fn test_rebalance_with_config() {
        let cli = Cli::parse_from(vec!["tierflow", "rebalance", "-c", "test.yaml"]);
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
        let cli = Cli::parse_from(vec!["tierflow", "rebalance"]);
        match cli.command {
            Commands::Rebalance { config, dry_run } => {
                // Should use default_config_path() which prefers /etc
                assert!(config.to_string_lossy().contains("tierflow"));
                assert!(!dry_run);
            }
            _ => panic!("Expected Rebalance command"),
        }
    }

    #[test]
    fn test_rebalance_dry_run() {
        let cli = Cli::parse_from(vec!["tierflow", "rebalance", "--dry-run"]);
        match cli.command {
            Commands::Rebalance { config, dry_run } => {
                assert!(config.to_string_lossy().contains("tierflow"));
                assert!(dry_run);
            }
            _ => panic!("Expected Rebalance command"),
        }
    }

    #[test]
    fn test_rebalance_short_flags() {
        let cli = Cli::parse_from(vec!["tierflow", "rebalance", "-c", "custom.yaml", "-n"]);
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
        let cli = Cli::parse_from(vec!["tierflow", "daemon"]);
        match cli.command {
            Commands::Daemon {
                config,
                dry_run,
                interval,
            } => {
                assert!(config.to_string_lossy().contains("tierflow"));
                assert!(!dry_run);
                assert_eq!(interval, 3600);
            }
            _ => panic!("Expected Daemon command"),
        }
    }

    #[test]
    fn test_daemon_with_interval() {
        let cli = Cli::parse_from(vec!["tierflow", "daemon", "-i", "600"]);
        match cli.command {
            Commands::Daemon {
                config,
                dry_run,
                interval,
            } => {
                assert!(config.to_string_lossy().contains("tierflow"));
                assert!(!dry_run);
                assert_eq!(interval, 600);
            }
            _ => panic!("Expected Daemon command"),
        }
    }

    #[test]
    fn test_daemon_all_flags() {
        let cli = Cli::parse_from(vec![
            "tierflow",
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
