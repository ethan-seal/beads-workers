// Example demonstrating the worker main loop
//
// This example shows how to:
// 1. Create a worker loop configuration
// 2. Initialize the worker (connect + send READY)
// 3. Run the main loop (receive tasks, execute, send results)
// 4. Handle reconnection and graceful shutdown

use beads_workers::ipc::RetryConfig;
use beads_workers::worker::{WorkerLoop, WorkerLoopConfig};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    println!("Worker Main Loop Example");
    println!("========================\n");

    // Get socket path from environment or use default
    let socket_path = std::env::var("BEADS_SOCKET_PATH")
        .unwrap_or_else(|_| "/tmp/beads-workers.sock".to_string());

    println!("Socket path: {}", socket_path);
    println!();

    // Configure retry behavior
    let retry_config = RetryConfig {
        max_attempts: 5,
        initial_backoff: Duration::from_secs(1),
        max_backoff: Duration::from_secs(30),
        multiplier: 2.0,
    };

    // Create worker loop configuration
    let config = WorkerLoopConfig::new(PathBuf::from(&socket_path))
        .with_retry_config(retry_config)
        .with_heartbeat_interval(Duration::from_secs(30))
        .with_auto_reconnect(true);

    // Create worker loop
    let mut worker = WorkerLoop::new(config);

    // Initialize worker
    println!("Initializing worker...");
    match worker.initialize().await {
        Ok(worker_id) => {
            println!("✓ Worker initialized with ID: {}", worker_id);
            println!();
        }
        Err(e) => {
            eprintln!("✗ Failed to initialize worker: {}", e);
            eprintln!();
            eprintln!("Make sure the orchestrator is running:");
            eprintln!("  cargo run --example orchestrator_demo");
            return Err(e.into());
        }
    }

    // Set up graceful shutdown handler
    let shutdown_handle = tokio::spawn(async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        println!("\n\nReceived Ctrl+C, shutting down...");
    });

    // Run the main loop
    println!("Starting worker main loop...");
    println!("Press Ctrl+C to stop\n");

    let result = tokio::select! {
        res = worker.run() => {
            match res {
                Ok(()) => {
                    println!("✓ Worker shut down gracefully");
                    Ok(())
                }
                Err(e) => {
                    eprintln!("✗ Worker error: {}", e);
                    Err(e)
                }
            }
        }
        _ = shutdown_handle => {
            println!("Shutting down worker...");
            worker.disconnect().await;
            Ok(())
        }
    };

    println!("\n✓ Worker process terminated cleanly");
    result.map_err(|e| e.into())
}
