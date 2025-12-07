// Inter-process communication

use crate::protocol::{
    deserialize_and_validate_orchestrator_message, serialize_worker_message, ProtocolError,
    ProtocolResult,
};
use crate::types::{OrchestratorMessage, WorkerMessage};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::{sleep, timeout};

/// Maximum message size for reading (1 MB)
const MAX_MESSAGE_SIZE: usize = 1_048_576;

/// Default connection timeout
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Default read timeout
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);

/// Unix socket client for worker-side communication
pub struct WorkerClient {
    /// Path to the Unix socket
    socket_path: PathBuf,
    /// Connection to the orchestrator
    stream: Option<UnixStream>,
    /// Retry configuration
    retry_config: RetryConfig,
    /// Worker ID (assigned after first READY message)
    worker_id: Option<String>,
}

/// Retry configuration for connection attempts
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of connection attempts
    pub max_attempts: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        RetryConfig {
            max_attempts: 5,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Calculate backoff duration for attempt number
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }

        let backoff_secs = self.initial_backoff.as_secs_f64()
            * self.multiplier.powi((attempt - 1) as i32);

        let backoff = Duration::from_secs_f64(backoff_secs.min(self.max_backoff.as_secs_f64()));

        backoff
    }
}

impl WorkerClient {
    /// Create a new worker client
    pub fn new<P: AsRef<Path>>(socket_path: P) -> Self {
        WorkerClient {
            socket_path: socket_path.as_ref().to_path_buf(),
            stream: None,
            retry_config: RetryConfig::default(),
            worker_id: None,
        }
    }

    /// Create a new worker client with custom retry configuration
    pub fn with_retry_config<P: AsRef<Path>>(socket_path: P, retry_config: RetryConfig) -> Self {
        WorkerClient {
            socket_path: socket_path.as_ref().to_path_buf(),
            stream: None,
            retry_config,
            worker_id: None,
        }
    }

    /// Connect to the orchestrator socket with retry logic
    pub async fn connect(&mut self) -> ProtocolResult<()> {
        let mut last_error = None;

        for attempt in 0..self.retry_config.max_attempts {
            // Calculate backoff for this attempt
            if attempt > 0 {
                let backoff = self.retry_config.backoff_for_attempt(attempt);
                tracing::debug!(
                    "Connection attempt {} failed, retrying in {:?}",
                    attempt,
                    backoff
                );
                sleep(backoff).await;
            }

            tracing::debug!(
                "Attempting to connect to socket: {} (attempt {}/{})",
                self.socket_path.display(),
                attempt + 1,
                self.retry_config.max_attempts
            );

            // Try to connect with timeout
            match timeout(DEFAULT_CONNECT_TIMEOUT, UnixStream::connect(&self.socket_path)).await {
                Ok(Ok(stream)) => {
                    tracing::info!("Connected to orchestrator at {}", self.socket_path.display());
                    self.stream = Some(stream);
                    return Ok(());
                }
                Ok(Err(e)) => {
                    tracing::warn!("Connection failed: {}", e);
                    last_error = Some(ProtocolError::IoError(e));
                }
                Err(_) => {
                    tracing::warn!("Connection timed out");
                    last_error = Some(ProtocolError::Timeout);
                }
            }
        }

        // All attempts failed
        Err(last_error.unwrap_or_else(|| {
            ProtocolError::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to connect after all retry attempts",
            ))
        }))
    }

    /// Send a worker message to the orchestrator
    pub async fn send_message(&mut self, message: WorkerMessage) -> ProtocolResult<()> {
        let stream = self
            .stream
            .as_mut()
            .ok_or(ProtocolError::ConnectionClosed)?;

        // Serialize the message
        let payload = serialize_worker_message(&message)?;

        // Write message length as u32 (big-endian)
        let length = payload.len() as u32;
        if let Err(e) = stream.write_all(&length.to_be_bytes()).await {
            self.stream = None; // Mark connection as dead
            return Err(ProtocolError::IoError(e));
        }

        // Write the message payload
        if let Err(e) = stream.write_all(&payload).await {
            self.stream = None; // Mark connection as dead
            return Err(ProtocolError::IoError(e));
        }

        tracing::debug!("Sent message: {:?}", message);
        Ok(())
    }

    /// Receive an orchestrator message
    pub async fn receive_message(&mut self) -> ProtocolResult<OrchestratorMessage> {
        let stream = self
            .stream
            .as_mut()
            .ok_or(ProtocolError::ConnectionClosed)?;

        // Read message with timeout
        let result = timeout(DEFAULT_READ_TIMEOUT, async {
            // Read message length (u32, big-endian)
            let mut length_bytes = [0u8; 4];
            stream.read_exact(&mut length_bytes).await?;
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
            stream.read_exact(&mut payload).await?;

            Ok::<Vec<u8>, ProtocolError>(payload)
        })
        .await;

        match result {
            Ok(Ok(payload)) => {
                let message = deserialize_and_validate_orchestrator_message(&payload)?;
                tracing::debug!("Received message: {:?}", message);
                Ok(message)
            }
            Ok(Err(e)) => {
                self.stream = None; // Mark connection as dead
                Err(e)
            }
            Err(_) => {
                self.stream = None; // Mark connection as dead
                Err(ProtocolError::Timeout)
            }
        }
    }

    /// Send a message and wait for a response
    pub async fn send_and_receive(
        &mut self,
        message: WorkerMessage,
    ) -> ProtocolResult<OrchestratorMessage> {
        self.send_message(message).await?;
        self.receive_message().await
    }

    /// Check if the client is connected
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// Disconnect from the orchestrator
    pub async fn disconnect(&mut self) {
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.shutdown().await;
            tracing::info!("Disconnected from orchestrator");
        }
    }

    /// Get the assigned worker ID
    pub fn worker_id(&self) -> Option<&str> {
        self.worker_id.as_deref()
    }

    /// Set the worker ID (called after receiving first task assignment)
    pub fn set_worker_id(&mut self, worker_id: String) {
        self.worker_id = Some(worker_id);
    }

    /// Reconnect to the orchestrator after connection drop
    pub async fn reconnect(&mut self) -> ProtocolResult<()> {
        tracing::info!("Attempting to reconnect to orchestrator");
        self.stream = None;
        self.connect().await
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

/// Helper function to create a worker client from config
pub fn create_worker_client(socket_path: PathBuf, retry_config: RetryConfig) -> WorkerClient {
    WorkerClient::with_retry_config(socket_path, retry_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.initial_backoff, Duration::from_secs(1));
        assert_eq!(config.max_backoff, Duration::from_secs(60));
        assert_eq!(config.multiplier, 2.0);
    }

    #[test]
    fn test_retry_config_backoff_calculation() {
        let config = RetryConfig::default();

        // Attempt 0 should have no backoff
        assert_eq!(config.backoff_for_attempt(0), Duration::from_millis(0));

        // Attempt 1: 1 second
        assert_eq!(config.backoff_for_attempt(1), Duration::from_secs(1));

        // Attempt 2: 2 seconds
        assert_eq!(config.backoff_for_attempt(2), Duration::from_secs(2));

        // Attempt 3: 4 seconds
        assert_eq!(config.backoff_for_attempt(3), Duration::from_secs(4));

        // Attempt 4: 8 seconds
        assert_eq!(config.backoff_for_attempt(4), Duration::from_secs(8));

        // Attempt 5: 16 seconds
        assert_eq!(config.backoff_for_attempt(5), Duration::from_secs(16));
    }

    #[test]
    fn test_retry_config_max_backoff() {
        let config = RetryConfig {
            max_attempts: 10,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(10),
            multiplier: 2.0,
        };

        // Should cap at max_backoff
        assert_eq!(config.backoff_for_attempt(10), Duration::from_secs(10));
        assert_eq!(config.backoff_for_attempt(20), Duration::from_secs(10));
    }

    #[test]
    fn test_worker_client_creation() {
        let client = WorkerClient::new("/tmp/test.sock");
        assert_eq!(client.socket_path(), Path::new("/tmp/test.sock"));
        assert!(!client.is_connected());
        assert_eq!(client.worker_id(), None);
    }

    #[test]
    fn test_worker_client_with_retry_config() {
        let retry_config = RetryConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(30),
            multiplier: 1.5,
        };

        let client = WorkerClient::with_retry_config("/tmp/test.sock", retry_config.clone());
        assert_eq!(client.socket_path(), Path::new("/tmp/test.sock"));
        assert_eq!(client.retry_config.max_attempts, 3);
        assert_eq!(
            client.retry_config.initial_backoff,
            Duration::from_millis(500)
        );
    }

    #[test]
    fn test_set_worker_id() {
        let mut client = WorkerClient::new("/tmp/test.sock");
        assert_eq!(client.worker_id(), None);

        client.set_worker_id("W1".to_string());
        assert_eq!(client.worker_id(), Some("W1"));
    }
}
