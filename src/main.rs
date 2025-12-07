mod cli;
mod config;
mod logs;
mod metrics;
mod status;
mod types;
mod ui;

use anyhow::{Context, Result};
use cli::{Cli, Commands, ConfigAction};
use config::Config;
use logs::{get_log_path, LogViewer};
use std::path::Path;
use tracing::{debug, info};
use ui::Dashboard;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse_args();

    // Initialize logging based on CLI flags
    init_logging(&cli);

    debug!("Parsed CLI arguments: {:?}", cli);

    // Load configuration
    let config_path = cli.get_config_path();
    let mut config = Config::load(&config_path)
        .with_context(|| format!("Failed to load config from {:?}", config_path))?;

    // Merge CLI and environment overrides into config
    config.merge_with_cli(&cli);

    // Validate configuration
    config
        .validate()
        .with_context(|| "Configuration validation failed")?;

    debug!("Final configuration: {:?}", config);

    // Get beads directory
    let beads_dir = cli.get_beads_dir();
    info!("Using beads directory: {:?}", beads_dir);

    // Dispatch commands
    match cli.command {
        Commands::Start { daemon, force } => {
            info!("Starting worker daemon (daemon={}, force={})", daemon, force);
            // TODO: Implement start command
            println!("Start command not yet implemented");
        }

        Commands::Stop { force, wait } => {
            info!("Stopping worker daemon (force={}, wait={})", force, wait);
            // TODO: Implement stop command
            println!("Stop command not yet implemented");
        }

        Commands::Restart { daemon, force } => {
            info!("Restarting worker daemon (daemon={}, force={})", daemon, force);
            // TODO: Implement restart command
            println!("Restart command not yet implemented");
        }

        Commands::Status { json, detailed } => {
            info!("Checking worker status (json={}, detailed={})", json, detailed);
            handle_status_command(&beads_dir, &config, json, detailed)?;
        }

        Commands::Watch { interval } => {
            info!("Starting live dashboard (interval={}s)", interval);
            let mut dashboard = Dashboard::new(interval, &beads_dir, config.clone());
            dashboard.run().await?;
        }

        Commands::Logs {
            lines,
            follow,
            level,
            worker,
        } => {
            info!(
                "Viewing logs (lines={}, follow={}, level={:?}, worker={:?})",
                lines, follow, level, worker
            );
            handle_logs_command(&beads_dir, lines, follow, level, worker).await?;
        }

        Commands::Exec {
            command,
            timeout,
            dir,
        } => {
            info!(
                "Executing command: {:?} (timeout={:?}, dir={:?})",
                command, timeout, dir
            );
            // TODO: Implement exec command
            println!("Exec command not yet implemented");
        }

        Commands::Config { action } => {
            handle_config_command(action, &config, &config_path)?;
        }

        Commands::Metrics { json, detailed, reset, archive } => {
            info!("Viewing metrics (json={}, detailed={}, reset={}, archive={})",
                json, detailed, reset, archive);
            handle_metrics_command(&beads_dir, json, detailed, reset, archive)?;
        }
    }

    Ok(())
}

fn init_logging(cli: &Cli) {
    let log_level = cli.get_log_level();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&log_level)),
        )
        .with_target(cli.debug)
        .with_thread_ids(cli.debug)
        .with_file(cli.debug)
        .with_line_number(cli.debug)
        .init();
}

fn handle_config_command(
    action: ConfigAction,
    config: &Config,
    config_path: &std::path::Path,
) -> Result<()> {
    match action {
        ConfigAction::Show { json } => {
            if json {
                let json = serde_json::to_string_pretty(config)
                    .context("Failed to serialize config to JSON")?;
                println!("{}", json);
            } else {
                let yaml = serde_yaml::to_string(config)
                    .context("Failed to serialize config to YAML")?;
                println!("{}", yaml);
            }
        }

        ConfigAction::Get { key } => {
            // Convert config to JSON for easy access
            let json = serde_json::to_value(config)
                .context("Failed to convert config to JSON")?;

            // Navigate to the key using dot notation
            let parts: Vec<&str> = key.split('.').collect();
            let mut current = &json;

            for part in &parts {
                current = current
                    .get(part)
                    .with_context(|| format!("Key not found: {}", key))?;
            }

            println!("{}", current);
        }

        ConfigAction::Set { key, value } => {
            println!("Setting {} = {}", key, value);
            println!("Note: Config modification not yet fully implemented");
            println!("Please edit {:?} directly", config_path);
        }

        ConfigAction::Reset { yes } => {
            if !yes {
                println!("This will reset the configuration to defaults.");
                println!("Run with --yes to confirm.");
                return Ok(());
            }

            let default_config = Config::default();
            default_config.save(config_path)?;
            println!("Configuration reset to defaults");
        }
    }

    Ok(())
}

async fn handle_logs_command(
    beads_dir: &Path,
    lines: usize,
    follow: bool,
    level: Option<String>,
    worker: Option<String>,
) -> Result<()> {
    let log_path = get_log_path(beads_dir);

    // Check if log file exists
    if !log_path.exists() {
        anyhow::bail!(
            "Log file not found at {:?}. Is the worker daemon running?",
            log_path
        );
    }

    let viewer = LogViewer::new(log_path)
        .with_level_filter(level)
        .with_worker_filter(worker);

    if follow {
        viewer.follow(lines).await?;
    } else {
        viewer.tail(lines)?;
    }

    Ok(())
}

fn handle_status_command(
    beads_dir: &Path,
    config: &Config,
    json: bool,
    detailed: bool,
) -> Result<()> {
    match status::check_status(beads_dir, config) {
        Ok(status_info) => {
            if json {
                // Output as JSON
                let json_str = serde_json::to_string_pretty(&status_info)
                    .context("Failed to serialize status to JSON")?;
                println!("{}", json_str);
            } else {
                // Output as human-readable format
                let formatted = status::format_status(&status_info, detailed);
                println!("{}", formatted);
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error checking status: {}", e);
            Err(e)
        }
    }
}

fn handle_metrics_command(
    beads_dir: &Path,
    json: bool,
    detailed: bool,
    reset: bool,
    archive: bool,
) -> Result<()> {
    let store = metrics::MetricsStore::new(beads_dir);

    if reset {
        // Reset metrics
        let new_metrics = metrics::WorkerMetrics::new();
        store.save(&new_metrics)
            .context("Failed to reset metrics")?;
        println!("Metrics reset successfully");
        return Ok(());
    }

    if archive {
        // Archive current metrics
        let metrics = store.load()
            .context("Failed to load metrics")?;
        store.archive(&metrics)
            .context("Failed to archive metrics")?;
        println!("Metrics archived successfully");
        return Ok(());
    }

    // Load and display metrics
    match store.load() {
        Ok(metrics) => {
            if json {
                let json_str = serde_json::to_string_pretty(&metrics)
                    .context("Failed to serialize metrics to JSON")?;
                println!("{}", json_str);
            } else {
                let formatted = metrics::format_metrics(&metrics, detailed);
                println!("{}", formatted);
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Error loading metrics: {}", e);
            Err(e)
        }
    }
}
