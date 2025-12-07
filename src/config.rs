use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
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

    /// Merge configuration with environment variables and CLI overrides
    /// Priority: CLI flags > Environment variables > Config file > Defaults
    pub fn merge_with_cli(&mut self, cli: &crate::cli::Cli) {
        // Apply environment variable overrides first
        self.apply_env_overrides();

        // Then apply CLI overrides (highest priority)
        if let Some(max_workers) = cli.max_workers {
            self.max_workers = max_workers;
        }

        // Override log level if provided via CLI
        let log_level = cli.get_log_level();
        self.logging.level = log_level;

        // Override beads paths if provided
        if let Some(ref beads_dir) = cli.beads_dir {
            if self.db_path.is_none() {
                self.db_path = Some(beads_dir.join("beads.db"));
            }
            if self.jsonl_path.is_none() {
                self.jsonl_path = Some(beads_dir.join("issues.jsonl"));
            }
        }
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) {
        // Core settings
        if let Ok(val) = env::var("BEADS_MAX_WORKERS") {
            if let Ok(n) = val.parse() {
                self.max_workers = n;
            }
        }

        if let Ok(val) = env::var("BEADS_DB_PATH") {
            self.db_path = Some(PathBuf::from(val));
        }

        if let Ok(val) = env::var("BEADS_JSONL_PATH") {
            self.jsonl_path = Some(PathBuf::from(val));
        }

        if let Ok(val) = env::var("BEADS_DEFAULT_TIMEOUT") {
            if let Ok(n) = val.parse() {
                self.default_timeout = Some(n);
            }
        }

        // Retry settings
        if let Ok(val) = env::var("BEADS_MAX_RETRIES") {
            if let Ok(n) = val.parse() {
                self.retry.max_retries = n;
            }
        }

        if let Ok(val) = env::var("BEADS_INITIAL_BACKOFF_SECS") {
            if let Ok(n) = val.parse() {
                self.retry.initial_backoff_secs = n;
            }
        }

        if let Ok(val) = env::var("BEADS_MAX_BACKOFF_SECS") {
            if let Ok(n) = val.parse() {
                self.retry.max_backoff_secs = n;
            }
        }

        if let Ok(val) = env::var("BEADS_BACKOFF_MULTIPLIER") {
            if let Ok(n) = val.parse() {
                self.retry.backoff_multiplier = n;
            }
        }

        // Logging settings
        if let Ok(val) = env::var("BEADS_LOG_LEVEL") {
            self.logging.level = val;
        }

        if let Ok(val) = env::var("BEADS_LOG_FORMAT") {
            self.logging.format = val;
        }

        if let Ok(val) = env::var("BEADS_LOG_FILE") {
            self.logging.file = Some(PathBuf::from(val));
        }

        if let Ok(val) = env::var("BEADS_LOG_COLORS") {
            self.logging.colors = val.parse().unwrap_or(true);
        }

        // IPC settings
        if let Ok(val) = env::var("BEADS_IPC_SOCKET_PATH") {
            self.ipc.socket_path = Some(PathBuf::from(val));
        }

        if let Ok(val) = env::var("BEADS_IPC_ENABLED") {
            self.ipc.enabled = val.parse().unwrap_or(true);
        }

        if let Ok(val) = env::var("BEADS_IPC_TIMEOUT_SECS") {
            if let Ok(n) = val.parse() {
                self.ipc.timeout_secs = n;
            }
        }

        // Worker settings
        if let Ok(val) = env::var("BEADS_WORKER_POLL_INTERVAL_MS") {
            if let Ok(n) = val.parse() {
                self.worker.poll_interval_ms = n;
            }
        }

        if let Ok(val) = env::var("BEADS_WORKER_QUEUE_SIZE") {
            if let Ok(n) = val.parse() {
                self.worker.queue_size = n;
            }
        }

        if let Ok(val) = env::var("BEADS_WORKER_PREFETCH") {
            self.worker.prefetch = val.parse().unwrap_or(true);
        }

        if let Ok(val) = env::var("BEADS_WORKER_PREFETCH_COUNT") {
            if let Ok(n) = val.parse() {
                self.worker.prefetch_count = n;
            }
        }

        if let Ok(val) = env::var("BEADS_WORKER_SHUTDOWN_TIMEOUT_SECS") {
            if let Ok(n) = val.parse() {
                self.worker.shutdown_timeout_secs = n;
            }
        }
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate max_workers
        if self.max_workers == 0 {
            bail!("max_workers must be greater than 0");
        }

        if self.max_workers > 1024 {
            bail!("max_workers is too large (max: 1024, got: {})", self.max_workers);
        }

        // Validate retry config
        if self.retry.initial_backoff_secs == 0 {
            bail!("retry.initial_backoff_secs must be greater than 0");
        }

        if self.retry.max_backoff_secs < self.retry.initial_backoff_secs {
            bail!(
                "retry.max_backoff_secs ({}) must be >= retry.initial_backoff_secs ({})",
                self.retry.max_backoff_secs,
                self.retry.initial_backoff_secs
            );
        }

        if self.retry.backoff_multiplier < 1.0 {
            bail!("retry.backoff_multiplier must be >= 1.0");
        }

        // Validate logging config
        let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_log_levels.contains(&self.logging.level.as_str()) {
            bail!(
                "Invalid log level: {}. Valid levels: {:?}",
                self.logging.level,
                valid_log_levels
            );
        }

        let valid_log_formats = ["pretty", "json", "compact"];
        if !valid_log_formats.contains(&self.logging.format.as_str()) {
            bail!(
                "Invalid log format: {}. Valid formats: {:?}",
                self.logging.format,
                valid_log_formats
            );
        }

        // Validate IPC config
        if self.ipc.timeout_secs == 0 {
            bail!("ipc.timeout_secs must be greater than 0");
        }

        // Validate worker config
        if self.worker.poll_interval_ms == 0 {
            bail!("worker.poll_interval_ms must be greater than 0");
        }

        if self.worker.queue_size == 0 {
            bail!("worker.queue_size must be greater than 0");
        }

        if self.worker.prefetch && self.worker.prefetch_count == 0 {
            bail!("worker.prefetch_count must be greater than 0 when prefetch is enabled");
        }

        if self.worker.shutdown_timeout_secs == 0 {
            bail!("worker.shutdown_timeout_secs must be greater than 0");
        }

        Ok(())
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

    #[test]
    fn test_validate_success() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_zero_workers() {
        let mut config = Config::default();
        config.max_workers = 0;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be greater than 0"));
    }

    #[test]
    fn test_validate_too_many_workers() {
        let mut config = Config::default();
        config.max_workers = 2000;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too large"));
    }

    #[test]
    fn test_validate_invalid_log_level() {
        let mut config = Config::default();
        config.logging.level = "invalid".to_string();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid log level"));
    }

    #[test]
    fn test_validate_invalid_backoff() {
        let mut config = Config::default();
        config.retry.max_backoff_secs = 1;
        config.retry.initial_backoff_secs = 10;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_backoff_secs"));
    }

    #[test]
    fn test_validate_invalid_backoff_multiplier() {
        let mut config = Config::default();
        config.retry.backoff_multiplier = 0.5;
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("backoff_multiplier"));
    }

    #[test]
    fn test_env_override() {
        use std::sync::Mutex;

        // Use a mutex to prevent test parallelism issues with env vars
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap();

        env::set_var("BEADS_MAX_WORKERS", "42");
        env::set_var("BEADS_LOG_LEVEL", "debug");

        let mut config = Config::default();
        config.apply_env_overrides();

        assert_eq!(config.max_workers, 42);
        assert_eq!(config.logging.level, "debug");

        // Clean up
        env::remove_var("BEADS_MAX_WORKERS");
        env::remove_var("BEADS_LOG_LEVEL");
    }

    #[test]
    fn test_env_override_paths() {
        use std::sync::Mutex;

        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap();

        env::set_var("BEADS_DB_PATH", "/custom/path/db");
        env::set_var("BEADS_JSONL_PATH", "/custom/path/jsonl");

        let mut config = Config::default();
        config.apply_env_overrides();

        assert_eq!(config.db_path, Some(PathBuf::from("/custom/path/db")));
        assert_eq!(config.jsonl_path, Some(PathBuf::from("/custom/path/jsonl")));

        // Clean up
        env::remove_var("BEADS_DB_PATH");
        env::remove_var("BEADS_JSONL_PATH");
    }

    #[test]
    fn test_priority_cli_over_env() {
        use crate::cli::Cli;
        use clap::Parser;
        use std::sync::Mutex;

        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _guard = ENV_LOCK.lock().unwrap();

        // Set env var
        env::set_var("BEADS_MAX_WORKERS", "42");

        let mut config = Config::default();
        let cli = Cli::parse_from(["beads-workers", "-j", "16", "status"]);

        config.merge_with_cli(&cli);

        // CLI should override env
        assert_eq!(config.max_workers, 16);

        // Clean up
        env::remove_var("BEADS_MAX_WORKERS");
    }

    #[test]
    fn test_validate_zero_initial_backoff() {
        let mut config = Config::default();
        config.retry.initial_backoff_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_invalid_log_format() {
        let mut config = Config::default();
        config.logging.format = "xml".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_ipc_timeout() {
        let mut config = Config::default();
        config.ipc.timeout_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_poll_interval() {
        let mut config = Config::default();
        config.worker.poll_interval_ms = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_queue_size() {
        let mut config = Config::default();
        config.worker.queue_size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_zero_shutdown_timeout() {
        let mut config = Config::default();
        config.worker.shutdown_timeout_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_prefetch_with_zero_count() {
        let mut config = Config::default();
        config.worker.prefetch = true;
        config.worker.prefetch_count = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_get_db_path_default() {
        let config = Config::default();
        let beads_dir = PathBuf::from("/test/beads");
        assert_eq!(config.get_db_path(&beads_dir), PathBuf::from("/test/beads/beads.db"));
    }

    #[test]
    fn test_get_db_path_override() {
        let mut config = Config::default();
        config.db_path = Some(PathBuf::from("/custom/db.sqlite"));
        let beads_dir = PathBuf::from("/test/beads");
        assert_eq!(config.get_db_path(&beads_dir), PathBuf::from("/custom/db.sqlite"));
    }

    #[test]
    fn test_get_jsonl_path_default() {
        let config = Config::default();
        let beads_dir = PathBuf::from("/test/beads");
        assert_eq!(config.get_jsonl_path(&beads_dir), PathBuf::from("/test/beads/issues.jsonl"));
    }

    #[test]
    fn test_get_jsonl_path_override() {
        let mut config = Config::default();
        config.jsonl_path = Some(PathBuf::from("/custom/issues.jsonl"));
        let beads_dir = PathBuf::from("/test/beads");
        assert_eq!(config.get_jsonl_path(&beads_dir), PathBuf::from("/custom/issues.jsonl"));
    }

    #[test]
    fn test_get_socket_path_default() {
        let config = Config::default();
        let beads_dir = PathBuf::from("/test/beads");
        assert_eq!(config.get_socket_path(&beads_dir), PathBuf::from("/test/beads/beads-workers.sock"));
    }

    #[test]
    fn test_get_socket_path_override() {
        let mut config = Config::default();
        config.ipc.socket_path = Some(PathBuf::from("/custom/socket.sock"));
        let beads_dir = PathBuf::from("/test/beads");
        assert_eq!(config.get_socket_path(&beads_dir), PathBuf::from("/custom/socket.sock"));
    }

    #[test]
    fn test_num_cpus_sanity() {
        let cpus = num_cpus();
        assert!(cpus > 0, "CPU count must be positive");
        assert!(cpus <= 2048, "CPU count seems unreasonably high");
    }

    #[test]
    fn test_retry_config_serialization() {
        let config = RetryConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: RetryConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.max_retries, deserialized.max_retries);
        assert_eq!(config.initial_backoff_secs, deserialized.initial_backoff_secs);
    }

    #[test]
    fn test_logging_config_serialization() {
        let config = LoggingConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: LoggingConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.level, deserialized.level);
        assert_eq!(config.format, deserialized.format);
        assert_eq!(config.colors, deserialized.colors);
    }

    #[test]
    fn test_ipc_config_serialization() {
        let config = IpcConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: IpcConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.enabled, deserialized.enabled);
        assert_eq!(config.timeout_secs, deserialized.timeout_secs);
    }

    #[test]
    fn test_worker_config_serialization() {
        let config = WorkerConfig::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: WorkerConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(config.poll_interval_ms, deserialized.poll_interval_ms);
        assert_eq!(config.queue_size, deserialized.queue_size);
        assert_eq!(config.prefetch, deserialized.prefetch);
    }

    #[test]
    fn test_config_full_serialization_round_trip() {
        let mut config = Config::default();
        config.max_workers = 8;
        config.db_path = Some(PathBuf::from("/custom/db"));
        config.jsonl_path = Some(PathBuf::from("/custom/jsonl"));
        config.retry.max_retries = 5;
        config.logging.level = "debug".to_string();

        let yaml = serde_yaml::to_string(&config).unwrap();
        let deserialized: Config = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(config.max_workers, deserialized.max_workers);
        assert_eq!(config.db_path, deserialized.db_path);
        assert_eq!(config.jsonl_path, deserialized.jsonl_path);
        assert_eq!(config.retry.max_retries, deserialized.retry.max_retries);
        assert_eq!(config.logging.level, deserialized.logging.level);
    }

    #[test]
    fn test_validate_all_log_levels() {
        let valid_levels = ["trace", "debug", "info", "warn", "error"];

        for level in valid_levels {
            let mut config = Config::default();
            config.logging.level = level.to_string();
            assert!(config.validate().is_ok(), "Level {} should be valid", level);
        }
    }

    #[test]
    fn test_validate_all_log_formats() {
        let valid_formats = ["pretty", "json", "compact"];

        for format in valid_formats {
            let mut config = Config::default();
            config.logging.format = format.to_string();
            assert!(config.validate().is_ok(), "Format {} should be valid", format);
        }
    }

    #[test]
    fn test_config_defaults_are_valid() {
        let config = Config::default();
        assert!(config.validate().is_ok());

        let retry_config = RetryConfig::default();
        let mut config = Config::default();
        config.retry = retry_config;
        assert!(config.validate().is_ok());

        let logging_config = LoggingConfig::default();
        let mut config = Config::default();
        config.logging = logging_config;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_retry_config_boundary_values() {
        let mut config = Config::default();

        // Test minimum values
        config.retry.initial_backoff_secs = 1;
        config.retry.max_backoff_secs = 1;
        config.retry.backoff_multiplier = 1.0;
        assert!(config.validate().is_ok());

        // Test large but reasonable values
        config.retry.max_backoff_secs = 3600; // 1 hour
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_worker_config_prefetch_disabled() {
        let mut config = Config::default();
        config.worker.prefetch = false;
        config.worker.prefetch_count = 0; // Should be OK when prefetch is disabled
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_path_helpers() {
        let config = Config::default();
        let beads_dir = PathBuf::from("/test/beads");

        // Test all path helpers
        let db_path = config.get_db_path(&beads_dir);
        let jsonl_path = config.get_jsonl_path(&beads_dir);
        let socket_path = config.get_socket_path(&beads_dir);

        assert!(db_path.starts_with("/test/beads"));
        assert!(jsonl_path.starts_with("/test/beads"));
        assert!(socket_path.starts_with("/test/beads"));
    }
}
