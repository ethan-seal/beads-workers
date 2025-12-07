mod cli;
mod config;

use anyhow::{Context, Result};
use cli::{Cli, Commands, ConfigAction};
use config::Config;
use tracing::{debug, info};

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

    // Merge CLI overrides into config
    config.merge_with_cli(&cli);

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
            // TODO: Implement status command
            println!("Status command not yet implemented");
        }

        Commands::Watch { interval } => {
            info!("Watching worker status (interval={}s)", interval);
            // TODO: Implement watch command
            println!("Watch command not yet implemented");
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
            // TODO: Implement logs command
            println!("Logs command not yet implemented");
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
