// Demo of the live dashboard (watch command)
//
// This example demonstrates how the live dashboard works.
// Run with: cargo run --example dashboard_demo
//
// The dashboard displays:
// - Real-time system statistics
// - Worker status and current tasks
// - Task queue information
//
// Press 'q' or Ctrl+C to exit

use beads_workers::ui::Dashboard;
use beads_workers::config::Config;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    println!("Starting live dashboard demo...");
    println!("This will show real-time worker status.");
    println!("Press 'q' or Ctrl+C to exit.\n");

    // Use current directory's .beads or default
    let beads_dir = std::env::var("BEADS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".beads"));

    // Load or use default config
    let config = Config::default();

    // Create and run the dashboard with 2-second refresh interval
    let mut dashboard = Dashboard::new(2, &beads_dir, config);

    if let Err(e) = dashboard.run().await {
        eprintln!("Dashboard error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
