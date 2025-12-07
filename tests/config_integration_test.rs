use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_config_file_loading() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("test-config.yaml");

    // Create a test config file
    let config_content = r#"
max_workers: 8
default_timeout: 600
retry:
  max_retries: 5
  initial_backoff_secs: 2
logging:
  level: debug
  format: json
worker:
  poll_interval_ms: 200
"#;

    fs::write(&config_path, config_content)?;

    // Load the config
    let config = beads_workers::config::Config::load(&config_path)?;

    // Verify values
    assert_eq!(config.max_workers, 8);
    assert_eq!(config.default_timeout, Some(600));
    assert_eq!(config.retry.max_retries, 5);
    assert_eq!(config.retry.initial_backoff_secs, 2);
    assert_eq!(config.logging.level, "debug");
    assert_eq!(config.logging.format, "json");
    assert_eq!(config.worker.poll_interval_ms, 200);

    Ok(())
}

#[test]
fn test_config_validation_errors() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("invalid-config.yaml");

    // Test invalid max_workers (zero)
    let invalid_config = r#"
max_workers: 0
"#;
    fs::write(&config_path, invalid_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("must be greater than 0"));

    // Test invalid max_workers (too large)
    let invalid_config = r#"
max_workers: 2000
"#;
    fs::write(&config_path, invalid_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("too large"));

    // Test invalid log level
    let invalid_config = r#"
max_workers: 4
logging:
  level: invalid_level
"#;
    fs::write(&config_path, invalid_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid log level"));

    // Test invalid log format
    let invalid_config = r#"
max_workers: 4
logging:
  format: xml
"#;
    fs::write(&config_path, invalid_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid log format"));

    // Test invalid backoff configuration
    let invalid_config = r#"
max_workers: 4
retry:
  initial_backoff_secs: 10
  max_backoff_secs: 5
"#;
    fs::write(&config_path, invalid_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("max_backoff_secs"));

    // Test invalid backoff multiplier
    let invalid_config = r#"
max_workers: 4
retry:
  backoff_multiplier: 0.5
"#;
    fs::write(&config_path, invalid_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("backoff_multiplier"));

    Ok(())
}

#[test]
fn test_config_default_fallback() -> Result<()> {
    // Loading a non-existent config should return defaults without error
    let config = beads_workers::config::Config::load("/nonexistent/config.yaml")?;

    // Should have default values
    assert!(config.max_workers > 0);
    assert_eq!(config.default_timeout, Some(300));
    assert_eq!(config.retry.max_retries, 3);
    assert_eq!(config.logging.level, "info");
    assert_eq!(config.logging.format, "pretty");
    assert!(config.logging.colors);
    assert!(config.ipc.enabled);
    assert_eq!(config.ipc.timeout_secs, 30);
    assert_eq!(config.worker.poll_interval_ms, 100);
    assert_eq!(config.worker.queue_size, 1000);
    assert!(config.worker.prefetch);
    assert_eq!(config.worker.prefetch_count, 10);

    // Validate should succeed on defaults
    assert!(config.validate().is_ok());

    Ok(())
}

#[test]
fn test_config_partial_override() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("partial-config.yaml");

    // Only override a few fields, rest should use defaults
    let partial_config = r#"
max_workers: 6
logging:
  level: warn
"#;

    fs::write(&config_path, partial_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;

    // Overridden values
    assert_eq!(config.max_workers, 6);
    assert_eq!(config.logging.level, "warn");

    // Default values should still be present
    assert_eq!(config.retry.max_retries, 3);
    assert_eq!(config.logging.format, "pretty");
    assert!(config.worker.prefetch);

    Ok(())
}

#[test]
fn test_config_save_and_reload() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("save-config.yaml");

    // Create a custom config
    let mut config = beads_workers::config::Config::default();
    config.max_workers = 12;
    config.logging.level = "trace".to_string();
    config.retry.max_retries = 10;

    // Save it
    config.save(&config_path)?;

    // Reload it
    let loaded_config = beads_workers::config::Config::load(&config_path)?;

    // Verify values match
    assert_eq!(loaded_config.max_workers, 12);
    assert_eq!(loaded_config.logging.level, "trace");
    assert_eq!(loaded_config.retry.max_retries, 10);

    Ok(())
}

#[test]
fn test_config_paths_resolution() -> Result<()> {
    let beads_dir = PathBuf::from("/test/beads");

    // Test default path resolution
    let config = beads_workers::config::Config::default();
    assert_eq!(config.get_db_path(&beads_dir), PathBuf::from("/test/beads/beads.db"));
    assert_eq!(config.get_jsonl_path(&beads_dir), PathBuf::from("/test/beads/issues.jsonl"));
    assert_eq!(config.get_socket_path(&beads_dir), PathBuf::from("/test/beads/beads-workers.sock"));

    // Test custom path override
    let mut config = beads_workers::config::Config::default();
    config.db_path = Some(PathBuf::from("/custom/db.sqlite"));
    config.jsonl_path = Some(PathBuf::from("/custom/issues.jsonl"));
    config.ipc.socket_path = Some(PathBuf::from("/custom/workers.sock"));

    assert_eq!(config.get_db_path(&beads_dir), PathBuf::from("/custom/db.sqlite"));
    assert_eq!(config.get_jsonl_path(&beads_dir), PathBuf::from("/custom/issues.jsonl"));
    assert_eq!(config.get_socket_path(&beads_dir), PathBuf::from("/custom/workers.sock"));

    Ok(())
}

#[test]
fn test_config_all_valid_log_levels() -> Result<()> {
    let valid_levels = ["trace", "debug", "info", "warn", "error"];

    for level in &valid_levels {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.yaml");
        let config_content = format!("max_workers: 4\nlogging:\n  level: {}", level);
        fs::write(&config_path, config_content)?;

        let config = beads_workers::config::Config::load(&config_path)?;
        assert!(config.validate().is_ok(), "Log level {} should be valid", level);
        assert_eq!(config.logging.level, *level);
    }

    Ok(())
}

#[test]
fn test_config_all_valid_log_formats() -> Result<()> {
    let valid_formats = ["pretty", "json", "compact"];

    for format in &valid_formats {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join("config.yaml");
        let config_content = format!("max_workers: 4\nlogging:\n  format: {}", format);
        fs::write(&config_path, config_content)?;

        let config = beads_workers::config::Config::load(&config_path)?;
        assert!(config.validate().is_ok(), "Log format {} should be valid", format);
        assert_eq!(config.logging.format, *format);
    }

    Ok(())
}

#[test]
fn test_config_worker_validation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.yaml");

    // Test zero poll interval
    let invalid_config = r#"
max_workers: 4
worker:
  poll_interval_ms: 0
"#;
    fs::write(&config_path, invalid_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;
    assert!(config.validate().is_err());

    // Test zero queue size
    let invalid_config = r#"
max_workers: 4
worker:
  queue_size: 0
"#;
    fs::write(&config_path, invalid_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;
    assert!(config.validate().is_err());

    // Test prefetch enabled with zero count
    let invalid_config = r#"
max_workers: 4
worker:
  prefetch: true
  prefetch_count: 0
"#;
    fs::write(&config_path, invalid_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;
    assert!(config.validate().is_err());

    // Test zero shutdown timeout
    let invalid_config = r#"
max_workers: 4
worker:
  shutdown_timeout_secs: 0
"#;
    fs::write(&config_path, invalid_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;
    assert!(config.validate().is_err());

    Ok(())
}

#[test]
fn test_config_ipc_validation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.yaml");

    // Test zero timeout
    let invalid_config = r#"
max_workers: 4
ipc:
  timeout_secs: 0
"#;
    fs::write(&config_path, invalid_config)?;
    let config = beads_workers::config::Config::load(&config_path)?;
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("timeout_secs"));

    Ok(())
}
