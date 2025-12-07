use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Maximum number of concurrent workers
    pub max_workers: usize,

    /// Path to beads database
    pub db_path: Option<PathBuf>,

    /// Path to beads JSONL file
    pub jsonl_path: Option<PathBuf>,

    /// Default timeout for tasks in seconds
    pub default_timeout: Option<u64>,

    /// Retry configuration
    pub retry: RetryConfig,

    /// Logging configuration
    pub logging: LoggingConfig,

    /// IPC configuration
    pub ipc: IpcConfig,

    /// Worker configuration
    pub worker: WorkerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RetryConfig {
    /// Maximum number of retries for failed tasks
    pub max_retries: usize,

    /// Initial backoff duration in seconds
    pub initial_backoff_secs: u64,

    /// Maximum backoff duration in seconds
    pub max_backoff_secs: u64,

    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// Log format (pretty, json, compact)
    pub format: String,

    /// Log file path
    pub file: Option<PathBuf>,

    /// Enable ANSI colors
    pub colors: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IpcConfig {
    /// Socket path for IPC communication
    pub socket_path: Option<PathBuf>,

    /// Enable IPC server
    pub enabled: bool,

    /// Connection timeout in seconds
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkerConfig {
    /// Poll interval in milliseconds
    pub poll_interval_ms: u64,

    /// Maximum task queue size per worker
    pub queue_size: usize,

    /// Enable task prefetching
    pub prefetch: bool,

    /// Number of tasks to prefetch
    pub prefetch_count: usize,

    /// Graceful shutdown timeout in seconds
    pub shutdown_timeout_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_workers: num_cpus(),
            db_path: None,
            jsonl_path: None,
            default_timeout: Some(300), // 5 minutes
            retry: RetryConfig::default(),
            logging: LoggingConfig::default(),
            ipc: IpcConfig::default(),
            worker: WorkerConfig::default(),
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_secs: 1,
            max_backoff_secs: 60,
            backoff_multiplier: 2.0,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
            file: None,
            colors: true,
        }
    }
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            socket_path: None,
            enabled: true,
            timeout_secs: 30,
        }
    }
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 100,
            queue_size: 1000,
            prefetch: true,
            prefetch_count: 10,
            shutdown_timeout_secs: 30,
        }
    }
}

impl Config {
    /// Load configuration from a file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        if !path.exists() {
            tracing::debug!("Config file not found at {:?}, using defaults", path);
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        let config: Self = serde_yaml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {:?}", path))?;

        tracing::debug!("Loaded config from {:?}", path);
        Ok(config)
    }

    /// Save configuration to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }

        let contents = serde_yaml::to_string(self)
            .context("Failed to serialize config")?;

        fs::write(path, contents)
            .with_context(|| format!("Failed to write config file: {:?}", path))?;

        tracing::debug!("Saved config to {:?}", path);
        Ok(())
    }

    /// Merge configuration with CLI overrides
    pub fn merge_with_cli(&mut self, cli: &crate::cli::Cli) {
        // Override max_workers if provided via CLI
        if let Some(max_workers) = cli.max_workers {
            self.max_workers = max_workers;
        }

        // Override log level if provided via CLI
        let log_level = cli.get_log_level();
        self.logging.level = log_level;
    }

    /// Get the database path, falling back to default
    pub fn get_db_path(&self, beads_dir: &Path) -> PathBuf {
        self.db_path
            .clone()
            .unwrap_or_else(|| beads_dir.join("beads.db"))
    }

    /// Get the JSONL path, falling back to default
    pub fn get_jsonl_path(&self, beads_dir: &Path) -> PathBuf {
        self.jsonl_path
            .clone()
            .unwrap_or_else(|| beads_dir.join("issues.jsonl"))
    }

    /// Get the IPC socket path, falling back to default
    pub fn get_socket_path(&self, beads_dir: &Path) -> PathBuf {
        self.ipc
            .socket_path
            .clone()
            .unwrap_or_else(|| beads_dir.join("beads-workers.sock"))
    }
}

/// Get the number of CPUs, with a reasonable default
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.max_workers > 0);
        assert_eq!(config.retry.max_retries, 3);
        assert_eq!(config.logging.level, "info");
    }

    #[test]
    fn test_load_nonexistent_config() {
        let result = Config::load("/nonexistent/path/config.yaml");
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.max_workers, Config::default().max_workers);
    }

    #[test]
    fn test_save_and_load_config() {
        let mut config = Config::default();
        config.max_workers = 8;
        config.logging.level = "debug".to_string();

        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();

        config.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded.max_workers, 8);
        assert_eq!(loaded.logging.level, "debug");
    }

    #[test]
    fn test_yaml_format() {
        let config = Config::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("max_workers"));
        assert!(yaml.contains("retry"));
        assert!(yaml.contains("logging"));
    }

    #[test]
    fn test_merge_with_cli() {
        use crate::cli::Cli;
        use clap::Parser;

        let mut config = Config::default();
        let cli = Cli::parse_from(["beads-workers", "-j", "16", "-v", "status"]);

        config.merge_with_cli(&cli);

        assert_eq!(config.max_workers, 16);
        assert_eq!(config.logging.level, "trace");
    }
}
