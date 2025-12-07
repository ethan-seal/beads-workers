// Example demonstrating the Unix socket client (worker side)
//
// This example shows how a worker connects to the orchestrator socket,
// sends messages, receives tasks, and handles connection drops with retry logic.
//
// Usage:
//   cargo run --example worker_client_demo

use beads_workers::ipc::{RetryConfig, WorkerClient};
use beads_workers::types::WorkerMessage;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    println!("=== Worker Client Demo ===\n");

    // Configure socket path
    let socket_path = PathBuf::from("/tmp/beads-workers-demo.sock");
    println!("Socket path: {}", socket_path.display());

    // Configure retry with exponential backoff
    let retry_config = RetryConfig {
        max_attempts: 5,
        initial_backoff: Duration::from_millis(500),
        max_backoff: Duration::from_secs(30),
        multiplier: 2.0,
    };

    println!("\nRetry Configuration:");
    println!("  Max attempts: {}", retry_config.max_attempts);
    println!("  Initial backoff: {:?}", retry_config.initial_backoff);
    println!("  Max backoff: {:?}", retry_config.max_backoff);
    println!("  Multiplier: {}", retry_config.multiplier);

    // Display exponential backoff schedule
    println!("\nExponential Backoff Schedule:");
    for attempt in 0..=5 {
        let backoff = retry_config.backoff_for_attempt(attempt);
        println!("  Attempt {}: wait {:?}", attempt, backoff);
    }

    // Create worker client
    let mut client = WorkerClient::with_retry_config(&socket_path, retry_config);
    println!("\n✓ Worker client created");

    // Demonstrate connection with retry
    println!("\nAttempting to connect to orchestrator...");
    println!("(Note: This will fail unless an orchestrator is running at the socket path)");

    match client.connect().await {
        Ok(()) => {
            println!("✓ Connected to orchestrator!");

            // Send READY message
            let worker_id = format!("worker-{}", std::process::id());
            println!("\nSending READY message with worker_id: {}", worker_id);

            let ready_msg = WorkerMessage::ready(worker_id.clone());
            match client.send_message(ready_msg).await {
                Ok(()) => println!("✓ READY message sent"),
                Err(e) => println!("✗ Failed to send READY message: {}", e),
            }

            // Wait for response
            println!("\nWaiting for orchestrator response...");
            match client.receive_message().await {
                Ok(message) => {
                    println!("✓ Received message from orchestrator:");
                    println!("  {:?}", message);

                    // Set worker ID from response
                    let assigned_id = message.worker_id();
                    client.set_worker_id(assigned_id.to_string());
                    println!("  Assigned worker ID: {}", assigned_id);
                }
                Err(e) => println!("✗ Failed to receive message: {}", e),
            }

            // Demonstrate heartbeat
            println!("\nSending HEARTBEAT message...");
            let heartbeat_msg = WorkerMessage::heartbeat(worker_id.clone());
            match client.send_message(heartbeat_msg).await {
                Ok(()) => println!("✓ HEARTBEAT sent"),
                Err(e) => println!("✗ Failed to send heartbeat: {}", e),
            }

            // Demonstrate reconnection
            println!("\nTesting reconnection logic...");
            client.disconnect().await;
            println!("  Disconnected from orchestrator");

            match client.reconnect().await {
                Ok(()) => println!("✓ Successfully reconnected!"),
                Err(e) => println!("✗ Reconnection failed: {}", e),
            }

            // Clean disconnect
            client.disconnect().await;
            println!("\n✓ Disconnected gracefully");
        }
        Err(e) => {
            println!("✗ Connection failed after all retry attempts: {}", e);
            println!("\nTo test this example fully:");
            println!("1. Start an orchestrator: cargo run --bin orchestrator");
            println!("2. Run this example: cargo run --example worker_client_demo");
        }
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}
