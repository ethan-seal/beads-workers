// Example demonstrating the Unix socket worker client
//
// This example shows how to:
// 1. Create a worker client
// 2. Connect to the orchestrator with retry logic
// 3. Send and receive messages
// 4. Handle connection drops and reconnection

use beads_workers::ipc::{RetryConfig, WorkerClient};
use beads_workers::types::WorkerMessage;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Socket path (typically provided by orchestrator)
    let socket_path = std::env::var("BEADS_SOCKET_PATH")
        .unwrap_or_else(|_| "/tmp/beads-workers.sock".to_string());

    println!("Worker client example");
    println!("Socket path: {}", socket_path);
    println!();

    // Create a worker client with custom retry configuration
    let retry_config = RetryConfig {
        max_attempts: 5,
        initial_backoff: Duration::from_secs(1),
        max_backoff: Duration::from_secs(30),
        multiplier: 2.0,
    };

    let mut client = WorkerClient::with_retry_config(&socket_path, retry_config);

    // Attempt to connect with exponential backoff
    println!("Connecting to orchestrator...");
    match client.connect().await {
        Ok(()) => {
            println!("✓ Connected successfully!");
            println!();
        }
        Err(e) => {
            eprintln!("✗ Failed to connect: {}", e);
            eprintln!();
            eprintln!("Make sure the orchestrator is running:");
            eprintln!("  cargo run -- orchestrator --socket {}", socket_path);
            return Err(e.into());
        }
    }

    // Send READY message
    println!("Sending READY message...");
    let ready_msg = WorkerMessage::ready("example-worker".to_string());

    match client.send_message(ready_msg).await {
        Ok(()) => println!("✓ READY message sent"),
        Err(e) => {
            eprintln!("✗ Failed to send READY: {}", e);
            return Err(e.into());
        }
    }

    // Receive response from orchestrator
    println!("Waiting for orchestrator response...");
    match client.receive_message().await {
        Ok(msg) => {
            println!("✓ Received message: {:?}", msg);

            // If we got a task assignment, store the worker ID
            if let Some(worker_id) = msg.worker_id() {
                client.set_worker_id(worker_id.to_string());
                println!("Worker ID: {}", worker_id);
            }
        }
        Err(e) => {
            eprintln!("✗ Failed to receive message: {}", e);
            return Err(e.into());
        }
    }

    // Demonstrate reconnection after simulated connection drop
    println!();
    println!("Demonstrating reconnection...");

    // Disconnect
    client.disconnect().await;
    println!("Disconnected from orchestrator");

    // Wait a bit
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Reconnect
    println!("Reconnecting...");
    match client.reconnect().await {
        Ok(()) => println!("✓ Reconnected successfully!"),
        Err(e) => {
            eprintln!("✗ Reconnection failed: {}", e);
            return Err(e.into());
        }
    }

    // Clean disconnect
    println!();
    println!("Disconnecting...");
    client.disconnect().await;
    println!("✓ Worker client shut down cleanly");

    Ok(())
}
