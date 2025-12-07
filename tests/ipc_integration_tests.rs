// Integration tests for IPC communication between orchestrator and workers
//
// These tests verify the full IPC protocol implementation including:
// - Socket connection establishment
// - Message exchange (bidirectional)
// - Protocol violations and error handling
// - Concurrent workers
// - Connection lifecycle

use beads_workers::ipc::{RetryConfig, WorkerClient};
use beads_workers::protocol::{
    deserialize_and_validate_orchestrator_message, serialize_worker_message, ProtocolError,
};
use beads_workers::server::UnixSocketServer;
use beads_workers::types::{OrchestratorMessage, ShutdownReason, WorkerMessage};
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};

// Helper to create a test socket path
fn test_socket_path(temp_dir: &TempDir, name: &str) -> std::path::PathBuf {
    temp_dir.path().join(format!("{}.sock", name))
}

// Helper to read a message from a raw UnixStream
async fn read_orchestrator_message(
    stream: &mut UnixStream,
) -> Result<OrchestratorMessage, ProtocolError> {
    let mut length_bytes = [0u8; 4];
    stream
        .read_exact(&mut length_bytes)
        .await
        .map_err(ProtocolError::IoError)?;
    let length = u32::from_be_bytes(length_bytes) as usize;

    let mut payload = vec![0u8; length];
    stream
        .read_exact(&mut payload)
        .await
        .map_err(ProtocolError::IoError)?;

    deserialize_and_validate_orchestrator_message(&payload)
}

// Helper to write a worker message to a raw UnixStream
async fn write_worker_message(
    stream: &mut UnixStream,
    message: &WorkerMessage,
) -> Result<(), ProtocolError> {
    let payload = serialize_worker_message(message)?;
    let length = payload.len() as u32;

    stream
        .write_all(&length.to_be_bytes())
        .await
        .map_err(ProtocolError::IoError)?;
    stream
        .write_all(&payload)
        .await
        .map_err(ProtocolError::IoError)?;

    Ok(())
}

#[tokio::test]
async fn test_basic_socket_connection() {
    // Test that a worker can successfully connect to the orchestrator socket
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "basic_connection");

    // Start server
    let server = UnixSocketServer::bind(&socket_path).await.unwrap();
    assert!(socket_path.exists());

    // Spawn server accept task
    let accept_task = tokio::spawn(async move { server.accept().await });

    // Give server time to start listening
    sleep(Duration::from_millis(50)).await;

    // Connect client
    let client_result = UnixStream::connect(&socket_path).await;
    assert!(client_result.is_ok());

    // Server should accept the connection
    let handler_result = accept_task.await;
    assert!(handler_result.is_ok());
}

#[tokio::test]
async fn test_basic_message_exchange() {
    // Test basic READY -> TASK message exchange
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "basic_exchange");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    // Spawn server task
    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();

        // Read READY from worker
        let msg = handler.read_worker_message().await.unwrap().unwrap();
        assert!(matches!(msg, WorkerMessage::Ready { .. }));
        assert_eq!(msg.worker_id(), "W1");

        // Send TASK to worker
        let task = OrchestratorMessage::task(
            "W1".to_string(),
            "issue-123".to_string(),
            1,
            "Test task".to_string(),
        );
        handler.write_orchestrator_message(&task).await.unwrap();
    });

    sleep(Duration::from_millis(50)).await;

    // Connect client and send READY
    let mut client = UnixStream::connect(&socket_path).await.unwrap();
    let ready_msg = WorkerMessage::ready("W1".to_string());
    write_worker_message(&mut client, &ready_msg)
        .await
        .unwrap();

    // Receive TASK
    let task_msg = read_orchestrator_message(&mut client).await.unwrap();
    match task_msg {
        OrchestratorMessage::Task {
            worker_id,
            issue_id,
            priority,
            title,
            ..
        } => {
            assert_eq!(worker_id, "W1");
            assert_eq!(issue_id, "issue-123");
            assert_eq!(priority, 1);
            assert_eq!(title, "Test task");
        }
        _ => panic!("Expected Task message"),
    }
}

#[tokio::test]
async fn test_complete_task_workflow() {
    // Test complete workflow: READY -> TASK -> DONE -> READY
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "complete_workflow");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();

        // First READY
        let msg = handler.read_worker_message().await.unwrap().unwrap();
        assert!(matches!(msg, WorkerMessage::Ready { .. }));

        // Send TASK
        let task = OrchestratorMessage::task(
            "W1".to_string(),
            "issue-123".to_string(),
            1,
            "Test task".to_string(),
        );
        handler.write_orchestrator_message(&task).await.unwrap();

        // Receive DONE
        let msg = handler.read_worker_message().await.unwrap().unwrap();
        match msg {
            WorkerMessage::Done {
                issue_id,
                duration_ms,
                ..
            } => {
                assert_eq!(issue_id, "issue-123");
                assert!(duration_ms > 0);
            }
            _ => panic!("Expected Done message"),
        }

        // Second READY
        let msg = handler.read_worker_message().await.unwrap().unwrap();
        assert!(matches!(msg, WorkerMessage::Ready { .. }));
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = UnixStream::connect(&socket_path).await.unwrap();

    // Send first READY
    let ready_msg = WorkerMessage::ready("W1".to_string());
    write_worker_message(&mut client, &ready_msg)
        .await
        .unwrap();

    // Receive TASK
    let _task_msg = read_orchestrator_message(&mut client).await.unwrap();

    // Send DONE
    let done_msg = WorkerMessage::done("W1".to_string(), "issue-123".to_string(), 1500);
    write_worker_message(&mut client, &done_msg).await.unwrap();

    // Send second READY
    write_worker_message(&mut client, &ready_msg)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_task_failure_workflow() {
    // Test failure workflow: READY -> TASK -> FAILED -> READY
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "failure_workflow");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();

        // READY
        let _msg = handler.read_worker_message().await.unwrap().unwrap();

        // Send TASK
        let task = OrchestratorMessage::task(
            "W1".to_string(),
            "issue-456".to_string(),
            2,
            "Failing task".to_string(),
        );
        handler.write_orchestrator_message(&task).await.unwrap();

        // Receive FAILED
        let msg = handler.read_worker_message().await.unwrap().unwrap();
        match msg {
            WorkerMessage::Failed {
                issue_id,
                error,
                duration_ms,
                ..
            } => {
                assert_eq!(issue_id, "issue-456");
                assert!(!error.is_empty());
                assert!(duration_ms > 0);
            }
            _ => panic!("Expected Failed message"),
        }

        // READY again
        let _msg = handler.read_worker_message().await.unwrap().unwrap();
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = UnixStream::connect(&socket_path).await.unwrap();

    // READY
    let ready_msg = WorkerMessage::ready("W1".to_string());
    write_worker_message(&mut client, &ready_msg)
        .await
        .unwrap();

    // Receive TASK
    let _task_msg = read_orchestrator_message(&mut client).await.unwrap();

    // Send FAILED
    let failed_msg = WorkerMessage::failed(
        "W1".to_string(),
        "issue-456".to_string(),
        "Task execution failed".to_string(),
        2000,
    );
    write_worker_message(&mut client, &failed_msg)
        .await
        .unwrap();

    // READY again
    write_worker_message(&mut client, &ready_msg)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_wait_message_flow() {
    // Test WAIT message when no tasks available
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "wait_flow");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();

        // READY
        let _msg = handler.read_worker_message().await.unwrap().unwrap();

        // Send WAIT
        let wait_msg = OrchestratorMessage::wait("W1".to_string(), 5);
        handler
            .write_orchestrator_message(&wait_msg)
            .await
            .unwrap();

        // Should receive another READY after wait
        let msg = handler.read_worker_message().await.unwrap().unwrap();
        assert!(matches!(msg, WorkerMessage::Ready { .. }));
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = UnixStream::connect(&socket_path).await.unwrap();

    // Send READY
    let ready_msg = WorkerMessage::ready("W1".to_string());
    write_worker_message(&mut client, &ready_msg)
        .await
        .unwrap();

    // Receive WAIT
    let wait_msg = read_orchestrator_message(&mut client).await.unwrap();
    match wait_msg {
        OrchestratorMessage::Wait { wait_seconds, .. } => {
            assert_eq!(wait_seconds, 5);
        }
        _ => panic!("Expected Wait message"),
    }

    // Send READY again (in real scenario, would wait first)
    write_worker_message(&mut client, &ready_msg)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_graceful_shutdown() {
    // Test graceful shutdown flow
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "shutdown");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();

        // READY
        let _msg = handler.read_worker_message().await.unwrap().unwrap();

        // Send SHUTDOWN
        let shutdown_msg =
            OrchestratorMessage::shutdown("W1".to_string(), ShutdownReason::UserRequested);
        handler
            .write_orchestrator_message(&shutdown_msg)
            .await
            .unwrap();
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = UnixStream::connect(&socket_path).await.unwrap();

    // Send READY
    let ready_msg = WorkerMessage::ready("W1".to_string());
    write_worker_message(&mut client, &ready_msg)
        .await
        .unwrap();

    // Receive SHUTDOWN
    let shutdown_msg = read_orchestrator_message(&mut client).await.unwrap();
    match shutdown_msg {
        OrchestratorMessage::Shutdown { reason, .. } => {
            assert_eq!(reason, ShutdownReason::UserRequested);
        }
        _ => panic!("Expected Shutdown message"),
    }
}

#[tokio::test]
async fn test_protocol_violation_invalid_json() {
    // Test handling of invalid JSON
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "invalid_json");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();

        // Try to read message - should fail with protocol error
        let result = handler.read_worker_message().await;
        tx.send(result).await.unwrap();
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = UnixStream::connect(&socket_path).await.unwrap();

    // Send invalid JSON
    let invalid_json = b"{invalid json}";
    let length = invalid_json.len() as u32;
    client.write_all(&length.to_be_bytes()).await.unwrap();
    client.write_all(invalid_json).await.unwrap();

    // Server should get protocol error
    let result = rx.recv().await.unwrap();
    assert!(result.is_err());
    match result {
        Err(ProtocolError::JsonError(_)) => {}
        _ => panic!("Expected JsonError"),
    }
}

#[tokio::test]
async fn test_protocol_violation_message_too_large() {
    // Test handling of oversized messages
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "too_large");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();

        // Try to read message - should fail with MessageTooLarge
        let result = handler.read_worker_message().await;
        tx.send(result).await.unwrap();
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = UnixStream::connect(&socket_path).await.unwrap();

    // Send message with size exceeding limit
    let oversized_length = 2_000_000u32; // 2 MB, exceeds 1 MB limit
    client
        .write_all(&oversized_length.to_be_bytes())
        .await
        .unwrap();

    // Server should get MessageTooLarge error
    let result = rx.recv().await.unwrap();
    assert!(result.is_err());
    match result {
        Err(ProtocolError::MessageTooLarge { .. }) => {}
        _ => panic!("Expected MessageTooLarge error"),
    }
}

#[tokio::test]
async fn test_protocol_violation_wrong_version() {
    // Test handling of protocol version mismatch
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "wrong_version");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();
        let result = handler.read_worker_message().await;
        tx.send(result).await.unwrap();
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = UnixStream::connect(&socket_path).await.unwrap();

    // Create message with wrong version
    let msg_json = r#"{"version":99,"payload":{"type":"READY","worker_id":"W1","timestamp":1234567890.0}}"#;
    let length = msg_json.len() as u32;
    client.write_all(&length.to_be_bytes()).await.unwrap();
    client.write_all(msg_json.as_bytes()).await.unwrap();

    // Server should get VersionMismatch error
    let result = rx.recv().await.unwrap();
    assert!(result.is_err());
    match result {
        Err(ProtocolError::VersionMismatch { .. }) => {}
        _ => panic!("Expected VersionMismatch error"),
    }
}

#[tokio::test]
async fn test_concurrent_workers() {
    // Test multiple workers connecting simultaneously
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "concurrent");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();
    let socket_path_clone = socket_path.clone();

    // Spawn server to handle multiple connections
    let (tx, mut rx) = mpsc::channel(10);
    tokio::spawn(async move {
        for i in 1..=5 {
            let mut handler = server.accept().await.unwrap();
            let tx = tx.clone();

            tokio::spawn(async move {
                // Read READY
                let msg = handler.read_worker_message().await.unwrap().unwrap();
                let worker_id = msg.worker_id().to_string();

                // Send TASK
                let task = OrchestratorMessage::task(
                    worker_id.clone(),
                    format!("issue-{}", i),
                    1,
                    format!("Task {}", i),
                );
                handler.write_orchestrator_message(&task).await.unwrap();

                tx.send(worker_id).await.unwrap();
            });
        }
    });

    sleep(Duration::from_millis(50)).await;

    // Spawn 5 concurrent worker clients
    let mut client_tasks = vec![];
    for i in 1..=5 {
        let socket_path = socket_path_clone.clone();
        let task = tokio::spawn(async move {
            let mut client = UnixStream::connect(&socket_path).await.unwrap();

            // Send READY
            let ready_msg = WorkerMessage::ready(format!("W{}", i));
            write_worker_message(&mut client, &ready_msg)
                .await
                .unwrap();

            // Receive TASK
            let task_msg = read_orchestrator_message(&mut client).await.unwrap();
            match task_msg {
                OrchestratorMessage::Task { issue_id, .. } => issue_id,
                _ => panic!("Expected Task message"),
            }
        });
        client_tasks.push(task);
    }

    // Wait for all clients to complete
    let mut received_tasks = vec![];
    for task in client_tasks {
        let issue_id = task.await.unwrap();
        received_tasks.push(issue_id);
    }

    // Verify all tasks were received
    assert_eq!(received_tasks.len(), 5);
    for i in 1..=5 {
        assert!(received_tasks.contains(&format!("issue-{}", i)));
    }

    // Verify server received all worker IDs
    let mut received_workers = vec![];
    for _ in 0..5 {
        let worker_id = timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        received_workers.push(worker_id);
    }
    assert_eq!(received_workers.len(), 5);
}

#[tokio::test]
async fn test_worker_client_connection() {
    // Test WorkerClient connect functionality
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "worker_client");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    // Spawn server
    tokio::spawn(async move {
        let _handler = server.accept().await.unwrap();
        // Just accept, don't do anything
    });

    sleep(Duration::from_millis(50)).await;

    // Connect using WorkerClient
    let mut client = WorkerClient::new(&socket_path);
    let result = client.connect().await;
    assert!(result.is_ok());
    assert!(client.is_connected());
}

#[tokio::test]
async fn test_worker_client_send_receive() {
    // Test WorkerClient send and receive
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "worker_client_send");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    // Spawn server
    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();

        // Read READY
        let _msg = handler.read_worker_message().await.unwrap().unwrap();

        // Send TASK
        let task = OrchestratorMessage::task(
            "W1".to_string(),
            "issue-789".to_string(),
            2,
            "Test task".to_string(),
        );
        handler.write_orchestrator_message(&task).await.unwrap();
    });

    sleep(Duration::from_millis(50)).await;

    // Connect and send/receive using WorkerClient
    let mut client = WorkerClient::new(&socket_path);
    client.connect().await.unwrap();

    // Send READY
    let ready_msg = WorkerMessage::ready("W1".to_string());
    client.send_message(ready_msg).await.unwrap();

    // Receive TASK
    let task_msg = client.receive_message().await.unwrap();
    match task_msg {
        OrchestratorMessage::Task { issue_id, .. } => {
            assert_eq!(issue_id, "issue-789");
        }
        _ => panic!("Expected Task message"),
    }
}

#[tokio::test]
async fn test_worker_client_retry_on_failure() {
    // Test WorkerClient retry logic when connection fails
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "retry");

    // Create client with fast retry config - only 2 attempts with short timeout
    let retry_config = RetryConfig {
        max_attempts: 2,
        initial_backoff: Duration::from_millis(10),
        max_backoff: Duration::from_millis(100),
        multiplier: 2.0,
    };
    let mut client = WorkerClient::with_retry_config(&socket_path, retry_config);

    // Don't create server - client should fail after retries
    let result = client.connect().await;

    // Should fail since server doesn't exist
    assert!(result.is_err());
}

#[tokio::test]
async fn test_connection_closed_detection() {
    // Test detection of closed connection
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "closed");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();

        // Read one message then close
        let _msg = handler.read_worker_message().await.unwrap();

        // Read should return None when connection closes
        let msg = handler.read_worker_message().await.unwrap();
        assert!(msg.is_none());
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = UnixStream::connect(&socket_path).await.unwrap();

    // Send READY
    let ready_msg = WorkerMessage::ready("W1".to_string());
    write_worker_message(&mut client, &ready_msg)
        .await
        .unwrap();

    // Close connection
    drop(client);

    sleep(Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_multiple_messages_rapid_fire() {
    // Test rapid message exchange
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "rapid");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    let (tx, mut rx) = mpsc::channel(100);

    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();

        // Read multiple READY messages rapidly
        for _ in 0..10 {
            let msg = handler.read_worker_message().await.unwrap().unwrap();
            assert!(matches!(msg, WorkerMessage::Ready { .. }));
            tx.send(()).await.unwrap();
        }
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = UnixStream::connect(&socket_path).await.unwrap();

    // Send 10 READY messages rapidly
    for _ in 0..10 {
        let ready_msg = WorkerMessage::ready("W1".to_string());
        write_worker_message(&mut client, &ready_msg)
            .await
            .unwrap();
    }

    // Verify all received
    for _ in 0..10 {
        timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn test_worker_client_disconnect() {
    // Test WorkerClient disconnect
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "disconnect");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    tokio::spawn(async move {
        let _handler = server.accept().await.unwrap();
        sleep(Duration::from_secs(1)).await;
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = WorkerClient::new(&socket_path);
    client.connect().await.unwrap();
    assert!(client.is_connected());

    client.disconnect().await;
    assert!(!client.is_connected());
}

#[tokio::test]
async fn test_protocol_validation_empty_worker_id() {
    // Test that empty worker_id is rejected
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "empty_worker_id");

    let server = UnixSocketServer::bind(&socket_path).await.unwrap();

    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut handler = server.accept().await.unwrap();
        let result = handler.read_worker_message().await;
        tx.send(result).await.unwrap();
    });

    sleep(Duration::from_millis(50)).await;

    let mut client = UnixStream::connect(&socket_path).await.unwrap();

    // Create message with empty worker_id (bypassing validation)
    let msg_json =
        r#"{"version":1,"payload":{"type":"READY","worker_id":"","timestamp":1234567890.0}}"#;
    let length = msg_json.len() as u32;
    client.write_all(&length.to_be_bytes()).await.unwrap();
    client.write_all(msg_json.as_bytes()).await.unwrap();

    // Server should get validation error
    let result = rx.recv().await.unwrap();
    assert!(result.is_err());
    match result {
        Err(ProtocolError::ValidationError(_)) => {}
        _ => panic!("Expected ValidationError"),
    }
}

#[tokio::test]
async fn test_socket_cleanup_on_drop() {
    // Test that socket file is cleaned up when server is dropped
    let temp_dir = TempDir::new().unwrap();
    let socket_path = test_socket_path(&temp_dir, "cleanup");

    {
        let server = UnixSocketServer::bind(&socket_path).await.unwrap();
        assert!(socket_path.exists());
        drop(server);
    }

    // Socket should be cleaned up
    sleep(Duration::from_millis(50)).await;
    assert!(!socket_path.exists());
}
