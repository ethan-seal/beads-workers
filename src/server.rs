// Unix socket server for orchestrator-worker communication

use crate::protocol::{
    deserialize_and_validate_worker_message, serialize_orchestrator_message, ProtocolError,
    ProtocolResult,
};
use crate::types::{OrchestratorMessage, WorkerMessage};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Maximum message size for reading (1 MB)
const MAX_MESSAGE_SIZE: usize = 1_048_576;

/// Unix socket server for orchestrator-worker communication
pub struct UnixSocketServer {
    /// Path to the Unix socket
    socket_path: PathBuf,
    /// Listener for incoming connections
    listener: UnixListener,
}

impl UnixSocketServer {
    /// Create a new Unix socket server
    ///
    /// This will bind to the specified socket path. If a socket already exists
    /// at that path, it will be removed first.
    pub async fn bind<P: AsRef<Path>>(socket_path: P) -> std::io::Result<Self> {
        let socket_path = socket_path.as_ref().to_path_buf();

        // Remove existing socket if it exists
        if socket_path.exists() {
            info!("Removing existing socket at {:?}", socket_path);
            std::fs::remove_file(&socket_path)?;
        }

        // Bind to the socket path
        let listener = UnixListener::bind(&socket_path)?;
        info!("Unix socket server bound to {:?}", socket_path);

        Ok(UnixSocketServer {
            socket_path,
            listener,
        })
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Accept a new connection
    ///
    /// Returns a ConnectionHandler for the new connection
    pub async fn accept(&self) -> std::io::Result<ConnectionHandler> {
        let (stream, addr) = self.listener.accept().await?;
        debug!("Accepted connection from {:?}", addr);
        Ok(ConnectionHandler::new(stream))
    }

    /// Clean up the socket file
    ///
    /// This should be called on shutdown to remove the socket file
    pub fn cleanup(&self) -> std::io::Result<()> {
        if self.socket_path.exists() {
            info!("Cleaning up socket at {:?}", self.socket_path);
            std::fs::remove_file(&self.socket_path)?;
        }
        Ok(())
    }
}

impl Drop for UnixSocketServer {
    fn drop(&mut self) {
        // Best effort cleanup on drop
        if let Err(e) = self.cleanup() {
            error!("Failed to cleanup socket on drop: {}", e);
        }
    }
}

/// Handle a single connection from a worker
pub struct ConnectionHandler {
    /// The underlying Unix stream
    stream: UnixStream,
}

impl ConnectionHandler {
    /// Create a new connection handler
    pub fn new(stream: UnixStream) -> Self {
        ConnectionHandler { stream }
    }

    /// Read a worker message from the connection
    ///
    /// This reads a length-prefixed message from the socket and deserializes it as a WorkerMessage.
    /// Returns None if the connection is closed.
    pub async fn read_worker_message(&mut self) -> ProtocolResult<Option<WorkerMessage>> {
        // Read message length (u32, big-endian)
        let mut length_bytes = [0u8; 4];
        match self.stream.read_exact(&mut length_bytes).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                debug!("Connection closed by worker");
                return Ok(None);
            }
            Err(e) => return Err(ProtocolError::IoError(e)),
        }

        let length = u32::from_be_bytes(length_bytes) as usize;

        // Validate message size
        if length > MAX_MESSAGE_SIZE {
            return Err(ProtocolError::MessageTooLarge {
                size: length,
                max: MAX_MESSAGE_SIZE,
            });
        }

        // Read message payload
        let mut payload = vec![0u8; length];
        self.stream
            .read_exact(&mut payload)
            .await
            .map_err(ProtocolError::IoError)?;

        debug!("Received worker message ({} bytes)", length);

        // Deserialize and validate the message
        let message = deserialize_and_validate_worker_message(&payload)?;
        Ok(Some(message))
    }

    /// Write an orchestrator message to the connection
    ///
    /// This serializes the message and writes it with a length prefix to the socket.
    pub async fn write_orchestrator_message(
        &mut self,
        message: &OrchestratorMessage,
    ) -> ProtocolResult<()> {
        let payload = serialize_orchestrator_message(message)?;

        // Write message length as u32 (big-endian)
        let length = payload.len() as u32;
        self.stream
            .write_all(&length.to_be_bytes())
            .await
            .map_err(ProtocolError::IoError)?;

        // Write the message payload
        self.stream
            .write_all(&payload)
            .await
            .map_err(ProtocolError::IoError)?;

        debug!("Sent orchestrator message ({} bytes)", length);

        Ok(())
    }

    /// Gracefully shut down the connection
    pub async fn shutdown(&mut self) -> std::io::Result<()> {
        debug!("Shutting down connection");
        self.stream.shutdown().await
    }
}

/// Message routing for bidirectional communication
///
/// This provides channels for sending and receiving messages between
/// the orchestrator and worker connections.
pub struct MessageRouter {
    /// Sender for orchestrator messages (orchestrator → worker)
    orchestrator_tx: mpsc::UnboundedSender<OrchestratorMessage>,
    /// Receiver for orchestrator messages
    orchestrator_rx: mpsc::UnboundedReceiver<OrchestratorMessage>,
    /// Sender for worker messages (worker → orchestrator)
    worker_tx: mpsc::UnboundedSender<WorkerMessage>,
    /// Receiver for worker messages
    worker_rx: mpsc::UnboundedReceiver<WorkerMessage>,
}

impl MessageRouter {
    /// Create a new message router
    pub fn new() -> Self {
        let (orchestrator_tx, orchestrator_rx) = mpsc::unbounded_channel();
        let (worker_tx, worker_rx) = mpsc::unbounded_channel();

        MessageRouter {
            orchestrator_tx,
            orchestrator_rx,
            worker_tx,
            worker_rx,
        }
    }

    /// Get a sender for orchestrator messages
    pub fn orchestrator_sender(&self) -> mpsc::UnboundedSender<OrchestratorMessage> {
        self.orchestrator_tx.clone()
    }

    /// Get a sender for worker messages
    pub fn worker_sender(&self) -> mpsc::UnboundedSender<WorkerMessage> {
        self.worker_tx.clone()
    }

    /// Receive the next orchestrator message
    pub async fn recv_orchestrator_message(&mut self) -> Option<OrchestratorMessage> {
        self.orchestrator_rx.recv().await
    }

    /// Receive the next worker message
    pub async fn recv_worker_message(&mut self) -> Option<WorkerMessage> {
        self.worker_rx.recv().await
    }
}

impl Default for MessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Spawn a task to handle a worker connection
///
/// This spawns a tokio task that:
/// - Reads worker messages and forwards them to the worker_tx channel
/// - Reads orchestrator messages from the orchestrator_rx channel and sends them to the worker
/// - Handles disconnections gracefully
pub async fn handle_connection(
    mut handler: ConnectionHandler,
    worker_tx: mpsc::UnboundedSender<WorkerMessage>,
    mut orchestrator_rx: mpsc::UnboundedReceiver<OrchestratorMessage>,
) {
    debug!("Starting connection handler");

    loop {
        tokio::select! {
            // Read from worker
            result = handler.read_worker_message() => {
                match result {
                    Ok(Some(msg)) => {
                        debug!("Received worker message: {:?}", msg);
                        if let Err(e) = worker_tx.send(msg) {
                            error!("Failed to forward worker message: {}", e);
                            break;
                        }
                    }
                    Ok(None) => {
                        info!("Worker disconnected");
                        break;
                    }
                    Err(e) => {
                        error!("Error reading worker message: {}", e);
                        break;
                    }
                }
            }

            // Write to worker
            msg = orchestrator_rx.recv() => {
                match msg {
                    Some(msg) => {
                        debug!("Sending orchestrator message: {:?}", msg);
                        if let Err(e) = handler.write_orchestrator_message(&msg).await {
                            error!("Failed to write orchestrator message: {}", e);
                            break;
                        }
                    }
                    None => {
                        info!("Orchestrator channel closed");
                        break;
                    }
                }
            }
        }
    }

    // Graceful shutdown
    if let Err(e) = handler.shutdown().await {
        warn!("Error during connection shutdown: {}", e);
    }

    debug!("Connection handler terminated");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ShutdownReason;
    use tempfile::TempDir;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_bind_and_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let server = UnixSocketServer::bind(&socket_path).await.unwrap();
        assert!(socket_path.exists());

        server.cleanup().unwrap();
        assert!(!socket_path.exists());
    }

    #[tokio::test]
    async fn test_bind_removes_existing_socket() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        // Create first server
        let server1 = UnixSocketServer::bind(&socket_path).await.unwrap();
        assert!(socket_path.exists());
        drop(server1);

        // Socket should be cleaned up on drop
        // But let's manually create one to test removal
        std::fs::File::create(&socket_path).unwrap();
        assert!(socket_path.exists());

        // Create second server - should remove existing socket
        let server2 = UnixSocketServer::bind(&socket_path).await.unwrap();
        assert!(socket_path.exists());

        server2.cleanup().unwrap();
    }

    #[tokio::test]
    async fn test_connection_handler_detects_closed_connection() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let server = UnixSocketServer::bind(&socket_path).await.unwrap();

        let accept_task = tokio::spawn(async move { server.accept().await.unwrap() });

        sleep(Duration::from_millis(50)).await;

        let client_stream = UnixStream::connect(&socket_path).await.unwrap();
        drop(client_stream); // Close connection immediately

        let mut server_handler = accept_task.await.unwrap();

        // Should return None when connection is closed
        let result = server_handler.read_worker_message().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_message_router() {
        let mut router = MessageRouter::new();

        // Send worker message
        let worker_msg = WorkerMessage::ready("W1".to_string());
        router.worker_sender().send(worker_msg.clone()).unwrap();

        // Receive worker message
        let received = router.recv_worker_message().await.unwrap();
        assert_eq!(received.worker_id(), worker_msg.worker_id());

        // Send orchestrator message
        let orch_msg =
            OrchestratorMessage::shutdown("W1".to_string(), ShutdownReason::UserRequested);
        router
            .orchestrator_sender()
            .send(orch_msg.clone())
            .unwrap();

        // Receive orchestrator message
        let received = router.recv_orchestrator_message().await.unwrap();
        assert_eq!(received.worker_id(), orch_msg.worker_id());
    }

    #[tokio::test]
    async fn test_handle_connection_bidirectional() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let server = UnixSocketServer::bind(&socket_path).await.unwrap();

        // Create channels for routing
        let (worker_tx, mut worker_rx) = mpsc::unbounded_channel();
        let (orchestrator_tx, orchestrator_rx) = mpsc::unbounded_channel();

        // Spawn server accept task
        let accept_task = tokio::spawn(async move {
            let handler = server.accept().await.unwrap();
            tokio::spawn(handle_connection(handler, worker_tx, orchestrator_rx));
        });

        sleep(Duration::from_millis(50)).await;

        // Connect client
        let mut client_stream = UnixStream::connect(&socket_path).await.unwrap();

        accept_task.await.unwrap();

        // Client sends READY
        use crate::protocol::serialize_worker_message;
        let ready_msg = WorkerMessage::ready("W1".to_string());
        let payload = serialize_worker_message(&ready_msg).unwrap();
        let length = payload.len() as u32;
        client_stream.write_all(&length.to_be_bytes()).await.unwrap();
        client_stream.write_all(&payload).await.unwrap();

        // Orchestrator receives READY
        let received = worker_rx.recv().await.unwrap();
        assert_eq!(received.worker_id(), "W1");

        // Orchestrator sends TASK
        let task_msg = OrchestratorMessage::task(
            "W1".to_string(),
            "issue-123".to_string(),
            1,
            "Test task".to_string(),
        );
        orchestrator_tx.send(task_msg.clone()).unwrap();

        // Client receives TASK
        let mut length_bytes = [0u8; 4];
        client_stream.read_exact(&mut length_bytes).await.unwrap();
        let length = u32::from_be_bytes(length_bytes) as usize;
        let mut payload = vec![0u8; length];
        client_stream.read_exact(&mut payload).await.unwrap();

        use crate::protocol::deserialize_and_validate_orchestrator_message;
        let received_msg = deserialize_and_validate_orchestrator_message(&payload).unwrap();
        assert_eq!(received_msg.worker_id(), "W1");
        match received_msg {
            OrchestratorMessage::Task { issue_id, .. } => {
                assert_eq!(issue_id, "issue-123");
            }
            _ => panic!("Expected Task message"),
        }
    }
}
