use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "beads-workers")]
#[command(version, about = "Parallel task executor for beads issue tracker", long_about = None)]
pub struct Cli {
    /// Path to beads directory
    #[arg(long, env = "BEADS_DIR", global = true)]
    pub beads_dir: Option<PathBuf>,

    /// Path to config file
    #[arg(long, env = "BEADS_CONFIG", global = true)]
    pub config: Option<PathBuf>,

    /// Maximum number of concurrent workers
    #[arg(short = 'j', long, env = "BEADS_MAX_WORKERS", global = true)]
    pub max_workers: Option<usize>,

    /// Enable verbose logging
    #[arg(short, long, env = "BEADS_VERBOSE", global = true)]
    pub verbose: bool,

    /// Enable debug logging
    #[arg(short, long, env = "BEADS_DEBUG", global = true)]
    pub debug: bool,

    /// Log level (overrides verbose/debug)
    #[arg(long, env = "BEADS_LOG_LEVEL", global = true)]
    pub log_level: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the worker daemon
    Start {
        /// Start in background mode
        #[arg(short, long)]
        daemon: bool,

        /// Force restart if already running
        #[arg(short, long)]
        force: bool,
    },

    /// Stop the worker daemon
    Stop {
        /// Force stop (kill immediately)
        #[arg(short, long)]
        force: bool,

        /// Wait for all tasks to complete before stopping
        #[arg(short, long)]
        wait: bool,
    },

    /// Restart the worker daemon
    Restart {
        /// Start in background mode
        #[arg(short, long)]
        daemon: bool,

        /// Force restart
        #[arg(short, long)]
        force: bool,
    },

    /// Show worker status
    Status {
        /// Output in JSON format
        #[arg(long)]
        json: bool,

        /// Show detailed status
        #[arg(long)]
        detailed: bool,
    },

    /// Watch worker status in real-time (live dashboard)
    Watch {
        /// Refresh interval in seconds
        #[arg(short, long, default_value = "1")]
        interval: u64,
    },

    /// View worker logs
    Logs {
        /// Number of lines to show
        #[arg(short = 'n', long, default_value = "50")]
        lines: usize,

        /// Follow log output
        #[arg(short, long)]
        follow: bool,

        /// Filter by log level
        #[arg(short = 'l', long)]
        level: Option<String>,

        /// Filter by worker ID
        #[arg(short = 'w', long)]
        worker: Option<String>,
    },

    /// Execute a command directly (for testing)
    Exec {
        /// Command to execute
        command: Vec<String>,

        /// Timeout in seconds
        #[arg(short, long)]
        timeout: Option<u64>,

        /// Working directory
        #[arg(short = 'C', long)]
        dir: Option<PathBuf>,
    },

    /// Show or modify configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// View performance metrics
    Metrics {
        /// Output in JSON format
        #[arg(long)]
        json: bool,

        /// Show detailed metrics
        #[arg(long)]
        detailed: bool,

        /// Reset metrics
        #[arg(long)]
        reset: bool,

        /// Archive current metrics
        #[arg(long)]
        archive: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration
    Show {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,

        /// Configuration value
        value: String,
    },

    /// Reset configuration to defaults
    Reset {
        /// Skip confirmation
        #[arg(short, long)]
        yes: bool,
    },
}

impl Cli {
    /// Parse command line arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Get the beads directory path
    pub fn get_beads_dir(&self) -> PathBuf {
        self.beads_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from(".beads"))
    }

    /// Get the config file path
    pub fn get_config_path(&self) -> PathBuf {
        if let Some(ref config) = self.config {
            config.clone()
        } else {
            // Default to .beads-workers.yaml in current directory
            PathBuf::from(".beads-workers.yaml")
        }
    }

    /// Determine the log level based on flags and environment
    pub fn get_log_level(&self) -> String {
        if let Some(ref level) = self.log_level {
            level.clone()
        } else if self.debug {
            "debug".to_string()
        } else if self.verbose {
            "trace".to_string()
        } else {
            "info".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse() {
        let cli = Cli::parse_from(["beads-workers", "status"]);
        assert!(matches!(cli.command, Commands::Status { .. }));
    }

    #[test]
    fn test_log_level_priority() {
        let cli = Cli::parse_from(["beads-workers", "status"]);
        assert_eq!(cli.get_log_level(), "info");

        let cli = Cli::parse_from(["beads-workers", "-v", "status"]);
        assert_eq!(cli.get_log_level(), "trace");

        let cli = Cli::parse_from(["beads-workers", "-d", "status"]);
        assert_eq!(cli.get_log_level(), "debug");
    }

    #[test]
    fn test_beads_dir_default() {
        let cli = Cli::parse_from(["beads-workers", "status"]);
        assert_eq!(cli.get_beads_dir(), PathBuf::from(".beads"));
    }

    #[test]
    fn test_config_path_default() {
        let cli = Cli::parse_from(["beads-workers", "status"]);
        assert_eq!(
            cli.get_config_path(),
            PathBuf::from(".beads-workers.yaml")
        );
    }
}
