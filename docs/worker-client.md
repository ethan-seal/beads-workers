# Worker Client - Unix Socket IPC

The `WorkerClient` provides a robust Unix socket client implementation for worker processes to communicate with the orchestrator.

## Features

- **Exponential Backoff Retry**: Automatic reconnection with configurable exponential backoff
- **Connection Management**: Automatic detection and handling of connection drops
- **Message Protocol**: Type-safe message serialization/deserialization with validation
- **Timeout Handling**: Configurable timeouts for connect and read operations
- **Connection State Tracking**: Easy checks for connection status

## Basic Usage

```rust
use beads_workers::ipc::{WorkerClient, RetryConfig};
use beads_workers::types::WorkerMessage;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client
    let mut client = WorkerClient::new("/tmp/beads-workers.sock");

    // Connect with retry logic
    client.connect().await?;

    // Send a READY message
    let msg = WorkerMessage::ready("W1".to_string());
    client.send_message(msg).await?;

    // Receive response
    let response = client.receive_message().await?;
    println!("Received: {:?}", response);

    // Disconnect cleanly
    client.disconnect().await;
    Ok(())
}
```

## Custom Retry Configuration

```rust
use std::time::Duration;

let retry_config = RetryConfig {
    max_attempts: 5,           // Try 5 times before giving up
    initial_backoff: Duration::from_secs(1),  // Start with 1 second
    max_backoff: Duration::from_secs(60),     // Cap at 60 seconds
    multiplier: 2.0,           // Double the wait time each retry
};

let mut client = WorkerClient::with_retry_config(
    "/tmp/beads-workers.sock",
    retry_config
);
```

## Exponential Backoff

The backoff calculation follows this pattern:

- Attempt 1: 1 second
- Attempt 2: 2 seconds  (1 × 2^1)
- Attempt 3: 4 seconds  (1 × 2^2)
- Attempt 4: 8 seconds  (1 × 2^3)
- Attempt 5: 16 seconds (1 × 2^4)
- ...continues until `max_backoff` is reached

## Handling Connection Drops

The client automatically marks connections as dead when errors occur:

```rust
// If connection drops during operation, it will return an error
match client.send_message(msg).await {
    Ok(()) => println!("Message sent"),
    Err(e) => {
        // Connection is now marked as dead
        eprintln!("Send failed: {}", e);

        // Reconnect
        client.reconnect().await?;
    }
}
```

## Worker Lifecycle Example

```rust
let mut client = WorkerClient::new(socket_path);

// Initial connection
client.connect().await?;

loop {
    // Send READY
    let ready = WorkerMessage::ready(worker_id.clone());
    if let Err(e) = client.send_message(ready).await {
        eprintln!("Connection lost: {}", e);

        // Attempt to reconnect
        match client.reconnect().await {
            Ok(()) => continue,
            Err(e) => {
                eprintln!("Reconnection failed: {}", e);
                break;
            }
        }
    }

    // Receive task or wait instruction
    match client.receive_message().await {
        Ok(OrchestratorMessage::Task { issue_id, .. }) => {
            // Execute task
            execute_task(&issue_id).await?;

            // Report completion
            let done = WorkerMessage::done(
                worker_id.clone(),
                issue_id,
                duration_ms
            );
            client.send_message(done).await?;
        }
        Ok(OrchestratorMessage::Wait { wait_seconds, .. }) => {
            tokio::time::sleep(Duration::from_secs(wait_seconds as u64)).await;
        }
        Ok(OrchestratorMessage::Shutdown { .. }) => {
            break;
        }
        Err(e) => {
            eprintln!("Receive error: {}", e);
            client.reconnect().await?;
        }
    }
}

client.disconnect().await;
```

## Error Handling

The client returns `ProtocolResult<T>` which can contain these errors:

- `ProtocolError::ConnectionClosed` - No active connection
- `ProtocolError::Timeout` - Operation timed out
- `ProtocolError::IoError` - I/O error during communication
- `ProtocolError::MessageTooLarge` - Message exceeds size limit
- `ProtocolError::ValidationError` - Message validation failed

## Configuration Integration

The client can be configured from the application config:

```rust
use beads_workers::config::Config;
use beads_workers::ipc::{RetryConfig, WorkerClient};

let config = Config::load("config.yaml")?;

let retry_config = RetryConfig {
    max_attempts: config.retry.max_retries as u32,
    initial_backoff: Duration::from_secs(config.retry.initial_backoff_secs),
    max_backoff: Duration::from_secs(config.retry.max_backoff_secs),
    multiplier: config.retry.backoff_multiplier,
};

let socket_path = config.get_socket_path(&beads_dir);
let mut client = WorkerClient::with_retry_config(socket_path, retry_config);
```

## Testing

Run the IPC tests:

```bash
cargo test --lib ipc
```

Run the example:

```bash
# Start orchestrator first (in another terminal)
cargo run -- orchestrator

# Run worker client example
RUST_LOG=debug cargo run --example worker_client
```

## Implementation Details

### Message Protocol

Messages are sent using a length-prefixed protocol:
1. Write message length as `u32` (big-endian)
2. Write message payload (JSON)

The same format is used for reading:
1. Read `u32` length
2. Read payload bytes
3. Deserialize and validate

### Connection State

The client maintains connection state internally:
- `stream: Option<UnixStream>` - Active connection or None
- Connection is set to None on any I/O error
- Use `is_connected()` to check current state

### Timeouts

- Connect timeout: 5 seconds (per attempt)
- Read timeout: 30 seconds
- These are currently hardcoded but could be made configurable

## See Also

- `src/types.rs` - Message type definitions
- `src/protocol.rs` - Serialization and validation
- `examples/worker_client.rs` - Complete working example
