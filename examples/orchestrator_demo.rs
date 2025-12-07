// Example demonstrating the orchestrator main loop

use beads_workers::orchestrator::Orchestrator;
use beads_workers::types::{OrchestratorConfig, Task};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    println!("=== Beads Workers Orchestrator Demo ===\n");

    // Create orchestrator configuration
    let config = OrchestratorConfig {
        socket_path: format!("/tmp/beads-demo-{}.sock", std::process::id()),
        max_workers: 5,
        default_wait_seconds: 30,
        worker_idle_timeout: 0,
        shutdown_timeout: 10,
        worker_stale_timeout: 300,
        stale_check_interval: 30,
        poll_interval_seconds: 30,
    };

    println!("Socket path: {}", config.socket_path);
    println!("Max workers: {}", config.max_workers);
    println!();

    // Create orchestrator (using current directory as beads_dir for demo)
    let beads_dir = std::env::current_dir()?;
    let mut orchestrator = Orchestrator::new(config, &beads_dir).await?;
    let shutdown_handle = orchestrator.shutdown_handle();

    println!("Orchestrator created successfully");
    println!("Socket listening at: {:?}\n", orchestrator.socket_path());

    // Add some sample tasks
    orchestrator.add_task(Task::new(
        "beads-workers-001".to_string(),
        1,
        "Implement feature X".to_string(),
    ));
    orchestrator.add_task(Task::new(
        "beads-workers-002".to_string(),
        0,
        "Fix critical bug Y".to_string(),
    ));
    orchestrator.add_task(Task::new(
        "beads-workers-003".to_string(),
        2,
        "Update documentation".to_string(),
    ));

    println!("Added 3 tasks to the queue\n");

    // Print initial stats
    let stats = orchestrator.stats();
    println!("Initial stats:");
    println!("  Workers: {}", stats.total_workers);
    println!("  Queued tasks: {}", stats.queued_tasks);
    println!();

    // Spawn a task to trigger shutdown after a delay
    tokio::spawn(async move {
        println!("Demo will run for 10 seconds...");
        println!("Connect workers to the socket to see them in action!\n");
        println!("Example worker connection command:");
        println!("  cargo run --example worker_client -- --socket /tmp/beads-demo-<PID>.sock\n");

        sleep(Duration::from_secs(10)).await;
        println!("\nTriggering shutdown...");
        let _ = shutdown_handle.send(()).await;
    });

    // Run the orchestrator
    println!("Starting orchestrator main loop...\n");
    orchestrator.run().await?;

    println!("\nOrchestrator shutdown complete");
    Ok(())
}
