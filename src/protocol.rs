// Message serialization, protocol validation, and error types

use crate::types::{OrchestratorMessage, WorkerMessage};
use serde::{Deserialize, Serialize};
use std::io;
use thiserror::Error;

/// Protocol version for wire format compatibility
pub const PROTOCOL_VERSION: u8 = 1;

/// Maximum message size (1 MB) to prevent memory exhaustion attacks
pub const MAX_MESSAGE_SIZE: usize = 1_048_576;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during protocol operations
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// IO error during message transmission
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    /// Message exceeds maximum allowed size
    #[error("Message size {size} exceeds maximum {max}")]
    MessageTooLarge { size: usize, max: usize },

    /// Invalid protocol version
    #[error("Invalid protocol version: expected {expected}, got {actual}")]
    VersionMismatch { expected: u8, actual: u8 },

    /// Message validation failed
    #[error("Validation error: {0}")]
    ValidationError(String),

    /// Unexpected message type in current context
    #[error("Unexpected message type: {0}")]
    UnexpectedMessageType(String),

    /// Worker ID mismatch
    #[error("Worker ID mismatch: expected {expected}, got {actual}")]
    WorkerIdMismatch { expected: String, actual: String },

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Timeout waiting for message
    #[error("Timeout waiting for message")]
    Timeout,
}

pub type ProtocolResult<T> = Result<T, ProtocolError>;

// ============================================================================
// Wire Format
// ============================================================================

/// Wire format envelope for protocol messages
///
/// Format: [version:u8][length:u32][payload:json]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope<T> {
    /// Protocol version
    pub version: u8,
    /// Message payload
    pub payload: T,
}

impl<T> MessageEnvelope<T> {
    /// Create a new message envelope with current protocol version
    pub fn new(payload: T) -> Self {
        MessageEnvelope {
            version: PROTOCOL_VERSION,
            payload,
        }
    }

    /// Get the protocol version
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Validate protocol version
    pub fn validate_version(&self) -> ProtocolResult<()> {
        if self.version != PROTOCOL_VERSION {
            return Err(ProtocolError::VersionMismatch {
                expected: PROTOCOL_VERSION,
                actual: self.version,
            });
        }
        Ok(())
    }
}

// ============================================================================
// Serialization
// ============================================================================

/// Serialize a message to JSON bytes
pub fn serialize_message<T: Serialize>(message: &T) -> ProtocolResult<Vec<u8>> {
    let envelope = MessageEnvelope::new(message);
    let json = serde_json::to_vec(&envelope)?;

    // Check message size
    if json.len() > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::MessageTooLarge {
            size: json.len(),
            max: MAX_MESSAGE_SIZE,
        });
    }

    Ok(json)
}

/// Deserialize a message from JSON bytes
pub fn deserialize_message<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> ProtocolResult<T> {
    // Check message size
    if bytes.len() > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::MessageTooLarge {
            size: bytes.len(),
            max: MAX_MESSAGE_SIZE,
        });
    }

    let envelope: MessageEnvelope<T> = serde_json::from_slice(bytes)?;
    envelope.validate_version()?;

    Ok(envelope.payload)
}

/// Serialize a worker message
pub fn serialize_worker_message(message: &WorkerMessage) -> ProtocolResult<Vec<u8>> {
    serialize_message(message)
}

/// Deserialize a worker message
pub fn deserialize_worker_message(bytes: &[u8]) -> ProtocolResult<WorkerMessage> {
    deserialize_message(bytes)
}

/// Serialize an orchestrator message
pub fn serialize_orchestrator_message(message: &OrchestratorMessage) -> ProtocolResult<Vec<u8>> {
    serialize_message(message)
}

/// Deserialize an orchestrator message
pub fn deserialize_orchestrator_message(bytes: &[u8]) -> ProtocolResult<OrchestratorMessage> {
    deserialize_message(bytes)
}

// ============================================================================
// Message Validation
// ============================================================================

/// Validate a worker message
pub fn validate_worker_message(message: &WorkerMessage) -> ProtocolResult<()> {
    // Check worker_id is not empty
    if message.worker_id().is_empty() {
        return Err(ProtocolError::ValidationError(
            "worker_id cannot be empty".to_string(),
        ));
    }

    // Check timestamp is reasonable (not in the future by more than 60s)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    if message.timestamp() > now + 60.0 {
        return Err(ProtocolError::ValidationError(format!(
            "timestamp {} is too far in the future (now: {})",
            message.timestamp(),
            now
        )));
    }

    // Check timestamp is not too old (more than 1 hour)
    if message.timestamp() < now - 3600.0 {
        return Err(ProtocolError::ValidationError(format!(
            "timestamp {} is too old (now: {})",
            message.timestamp(),
            now
        )));
    }

    // Validate specific message types
    match message {
        WorkerMessage::Done { issue_id, duration_ms, .. } => {
            if issue_id.is_empty() {
                return Err(ProtocolError::ValidationError(
                    "issue_id cannot be empty in DONE message".to_string(),
                ));
            }
            // Duration should be reasonable (less than 1 hour = 3600000 ms)
            if *duration_ms > 3_600_000 {
                return Err(ProtocolError::ValidationError(format!(
                    "duration_ms {} is unreasonably large",
                    duration_ms
                )));
            }
        }
        WorkerMessage::Failed { issue_id, error, duration_ms, .. } => {
            if issue_id.is_empty() {
                return Err(ProtocolError::ValidationError(
                    "issue_id cannot be empty in FAILED message".to_string(),
                ));
            }
            if error.is_empty() {
                return Err(ProtocolError::ValidationError(
                    "error cannot be empty in FAILED message".to_string(),
                ));
            }
            if *duration_ms > 3_600_000 {
                return Err(ProtocolError::ValidationError(format!(
                    "duration_ms {} is unreasonably large",
                    duration_ms
                )));
            }
        }
        WorkerMessage::Ready { .. } => {
            // READY messages are simple, no additional validation needed
        }
    }

    Ok(())
}

/// Validate an orchestrator message
pub fn validate_orchestrator_message(message: &OrchestratorMessage) -> ProtocolResult<()> {
    // Check worker_id is not empty
    if message.worker_id().is_empty() {
        return Err(ProtocolError::ValidationError(
            "worker_id cannot be empty".to_string(),
        ));
    }

    // Check timestamp is reasonable
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();

    if message.timestamp() > now + 60.0 {
        return Err(ProtocolError::ValidationError(format!(
            "timestamp {} is too far in the future (now: {})",
            message.timestamp(),
            now
        )));
    }

    // Validate specific message types
    match message {
        OrchestratorMessage::Task { issue_id, priority, title, .. } => {
            if issue_id.is_empty() {
                return Err(ProtocolError::ValidationError(
                    "issue_id cannot be empty in TASK message".to_string(),
                ));
            }
            if title.is_empty() {
                return Err(ProtocolError::ValidationError(
                    "title cannot be empty in TASK message".to_string(),
                ));
            }
            if *priority > 4 {
                return Err(ProtocolError::ValidationError(format!(
                    "priority {} is out of range (0-4)",
                    priority
                )));
            }
        }
        OrchestratorMessage::Wait { wait_seconds, .. } => {
            // Wait time should be reasonable (less than 1 hour)
            if *wait_seconds > 3600 {
                return Err(ProtocolError::ValidationError(format!(
                    "wait_seconds {} is unreasonably large",
                    wait_seconds
                )));
            }
        }
        OrchestratorMessage::Shutdown { .. } => {
            // SHUTDOWN messages are simple, no additional validation needed
        }
    }

    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a validated worker message and serialize it
pub fn create_and_serialize_worker_message(
    message: WorkerMessage,
) -> ProtocolResult<Vec<u8>> {
    validate_worker_message(&message)?;
    serialize_worker_message(&message)
}

/// Deserialize and validate a worker message
pub fn deserialize_and_validate_worker_message(
    bytes: &[u8],
) -> ProtocolResult<WorkerMessage> {
    let message = deserialize_worker_message(bytes)?;
    validate_worker_message(&message)?;
    Ok(message)
}

/// Create a validated orchestrator message and serialize it
pub fn create_and_serialize_orchestrator_message(
    message: OrchestratorMessage,
) -> ProtocolResult<Vec<u8>> {
    validate_orchestrator_message(&message)?;
    serialize_orchestrator_message(&message)
}

/// Deserialize and validate an orchestrator message
pub fn deserialize_and_validate_orchestrator_message(
    bytes: &[u8],
) -> ProtocolResult<OrchestratorMessage> {
    let message = deserialize_orchestrator_message(bytes)?;
    validate_orchestrator_message(&message)?;
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ShutdownReason, WorkerMessage};

    #[test]
    fn test_protocol_version() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }

    #[test]
    fn test_serialize_deserialize_worker_ready() {
        let msg = WorkerMessage::ready("W1".to_string());
        let bytes = serialize_worker_message(&msg).unwrap();
        let deserialized = deserialize_worker_message(&bytes).unwrap();

        assert_eq!(msg.worker_id(), deserialized.worker_id());
        match deserialized {
            WorkerMessage::Ready { .. } => {}
            _ => panic!("Expected Ready variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_worker_done() {
        let msg = WorkerMessage::done("W1".to_string(), "issue-123".to_string(), 5000);
        let bytes = serialize_worker_message(&msg).unwrap();
        let deserialized = deserialize_worker_message(&bytes).unwrap();

        match deserialized {
            WorkerMessage::Done { worker_id, issue_id, duration_ms, .. } => {
                assert_eq!(worker_id, "W1");
                assert_eq!(issue_id, "issue-123");
                assert_eq!(duration_ms, 5000);
            }
            _ => panic!("Expected Done variant"),
        }
    }

    #[test]
    fn test_serialize_deserialize_orchestrator_task() {
        let msg = OrchestratorMessage::task(
            "W1".to_string(),
            "issue-123".to_string(),
            2,
            "Test task".to_string(),
        );
        let bytes = serialize_orchestrator_message(&msg).unwrap();
        let deserialized = deserialize_orchestrator_message(&bytes).unwrap();

        match deserialized {
            OrchestratorMessage::Task { worker_id, issue_id, priority, title, .. } => {
                assert_eq!(worker_id, "W1");
                assert_eq!(issue_id, "issue-123");
                assert_eq!(priority, 2);
                assert_eq!(title, "Test task");
            }
            _ => panic!("Expected Task variant"),
        }
    }

    #[test]
    fn test_validate_worker_message_empty_worker_id() {
        let msg = WorkerMessage::ready("".to_string());
        let result = validate_worker_message(&msg);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProtocolError::ValidationError(_)));
    }

    #[test]
    fn test_validate_worker_message_empty_issue_id() {
        let msg = WorkerMessage::done("W1".to_string(), "".to_string(), 1000);
        let result = validate_worker_message(&msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_worker_message_excessive_duration() {
        let msg = WorkerMessage::done("W1".to_string(), "issue-123".to_string(), 4_000_000);
        let result = validate_worker_message(&msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_orchestrator_message_empty_worker_id() {
        let msg = OrchestratorMessage::wait("".to_string(), 30);
        let result = validate_orchestrator_message(&msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_orchestrator_message_invalid_priority() {
        let msg = OrchestratorMessage::task(
            "W1".to_string(),
            "issue-123".to_string(),
            5, // Invalid priority (max is 4)
            "Test".to_string(),
        );
        let result = validate_orchestrator_message(&msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_orchestrator_message_excessive_wait() {
        let msg = OrchestratorMessage::wait("W1".to_string(), 5000);
        let result = validate_orchestrator_message(&msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_message_too_large() {
        // Create a message with a very large error string
        let large_error = "x".repeat(MAX_MESSAGE_SIZE);
        let msg = WorkerMessage::failed(
            "W1".to_string(),
            "issue-123".to_string(),
            large_error,
            1000,
        );

        let result = serialize_worker_message(&msg);
        assert!(result.is_err());
        match result.unwrap_err() {
            ProtocolError::MessageTooLarge { .. } => {}
            e => panic!("Expected MessageTooLarge, got {:?}", e),
        }
    }

    #[test]
    fn test_version_mismatch() {
        let msg = WorkerMessage::ready("W1".to_string());
        let bytes = serialize_worker_message(&msg).unwrap();

        // Manually create an envelope with wrong version
        let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        json["version"] = serde_json::json!(99);
        let bad_bytes = serde_json::to_vec(&json).unwrap();

        let result = deserialize_worker_message(&bad_bytes);
        assert!(result.is_err());
        match result.unwrap_err() {
            ProtocolError::VersionMismatch { expected, actual } => {
                assert_eq!(expected, PROTOCOL_VERSION);
                assert_eq!(actual, 99);
            }
            e => panic!("Expected VersionMismatch, got {:?}", e),
        }
    }

    #[test]
    fn test_create_and_serialize_valid_message() {
        let msg = WorkerMessage::ready("W1".to_string());
        let result = create_and_serialize_worker_message(msg);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_and_serialize_invalid_message() {
        let msg = WorkerMessage::ready("".to_string()); // Invalid: empty worker_id
        let result = create_and_serialize_worker_message(msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_and_validate_valid_message() {
        let msg = WorkerMessage::done("W1".to_string(), "issue-123".to_string(), 1000);
        let bytes = serialize_worker_message(&msg).unwrap();
        let result = deserialize_and_validate_worker_message(&bytes);
        assert!(result.is_ok());
    }

    #[test]
    fn test_round_trip_all_worker_messages() {
        let messages = vec![
            WorkerMessage::ready("W1".to_string()),
            WorkerMessage::done("W1".to_string(), "issue-123".to_string(), 5000),
            WorkerMessage::failed(
                "W1".to_string(),
                "issue-456".to_string(),
                "Test error".to_string(),
                2000,
            ),
        ];

        for msg in messages {
            let bytes = create_and_serialize_worker_message(msg.clone()).unwrap();
            let deserialized = deserialize_and_validate_worker_message(&bytes).unwrap();
            assert_eq!(msg.worker_id(), deserialized.worker_id());
        }
    }

    #[test]
    fn test_round_trip_all_orchestrator_messages() {
        let messages = vec![
            OrchestratorMessage::task(
                "W1".to_string(),
                "issue-123".to_string(),
                2,
                "Test task".to_string(),
            ),
            OrchestratorMessage::wait("W1".to_string(), 30),
            OrchestratorMessage::shutdown("W1".to_string(), ShutdownReason::UserRequested),
        ];

        for msg in messages {
            let bytes = create_and_serialize_orchestrator_message(msg.clone()).unwrap();
            let deserialized =
                deserialize_and_validate_orchestrator_message(&bytes).unwrap();
            assert_eq!(msg.worker_id(), deserialized.worker_id());
        }
    }

    #[test]
    fn test_envelope_version_validation() {
        let envelope = MessageEnvelope::new(WorkerMessage::ready("W1".to_string()));
        assert_eq!(envelope.version(), PROTOCOL_VERSION);
        assert!(envelope.validate_version().is_ok());
    }

    #[test]
    fn test_validate_empty_error_in_failed_message() {
        let msg = WorkerMessage::failed(
            "W1".to_string(),
            "issue-123".to_string(),
            "".to_string(),
            1000,
        );
        let result = validate_worker_message(&msg);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("error cannot be empty"));
    }

    #[test]
    fn test_validate_empty_title_in_task_message() {
        let msg = OrchestratorMessage::task(
            "W1".to_string(),
            "issue-123".to_string(),
            2,
            "".to_string(),
        );
        let result = validate_orchestrator_message(&msg);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("title cannot be empty"));
    }

    #[test]
    fn test_validate_empty_issue_id_in_task_message() {
        let msg = OrchestratorMessage::task(
            "W1".to_string(),
            "".to_string(),
            2,
            "Test".to_string(),
        );
        let result = validate_orchestrator_message(&msg);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("issue_id cannot be empty"));
    }

    #[test]
    fn test_serialize_all_orchestrator_message_types() {
        let messages = vec![
            OrchestratorMessage::task(
                "W1".to_string(),
                "issue-123".to_string(),
                3,
                "Task".to_string(),
            ),
            OrchestratorMessage::wait("W1".to_string(), 45),
            OrchestratorMessage::shutdown("W1".to_string(), ShutdownReason::IdleTimeout),
        ];

        for msg in messages {
            let bytes = serialize_orchestrator_message(&msg).unwrap();
            assert!(bytes.len() > 0);
            let deserialized = deserialize_orchestrator_message(&bytes).unwrap();
            assert_eq!(msg.worker_id(), deserialized.worker_id());
        }
    }

    #[test]
    fn test_protocol_error_display() {
        let errors = vec![
            ProtocolError::MessageTooLarge { size: 2000, max: 1000 },
            ProtocolError::VersionMismatch { expected: 1, actual: 2 },
            ProtocolError::ValidationError("test error".to_string()),
            ProtocolError::UnexpectedMessageType("UNKNOWN".to_string()),
            ProtocolError::WorkerIdMismatch {
                expected: "W1".to_string(),
                actual: "W2".to_string(),
            },
            ProtocolError::ConnectionClosed,
            ProtocolError::Timeout,
        ];

        for error in errors {
            let display = format!("{}", error);
            assert!(display.len() > 0);
        }
    }

    #[test]
    fn test_max_message_size_constant() {
        assert_eq!(MAX_MESSAGE_SIZE, 1_048_576);
    }

    #[test]
    fn test_protocol_version_constant() {
        assert_eq!(PROTOCOL_VERSION, 1);
    }

    #[test]
    fn test_deserialize_invalid_json() {
        let invalid_json = b"{ invalid json }";
        let result = deserialize_worker_message(invalid_json);
        assert!(result.is_err());
        match result.unwrap_err() {
            ProtocolError::JsonError(_) => {}
            e => panic!("Expected JsonError, got {:?}", e),
        }
    }

    #[test]
    fn test_validate_future_timestamp() {
        use std::time::{SystemTime, UNIX_EPOCH};

        // Create a message with a timestamp far in the future
        let future_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            + 120.0; // 2 minutes in the future

        let msg = WorkerMessage::Ready {
            worker_id: "W1".to_string(),
            timestamp: future_timestamp,
        };

        let result = validate_worker_message(&msg);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too far in the future"));
    }

    #[test]
    fn test_validate_old_timestamp() {
        use std::time::{SystemTime, UNIX_EPOCH};

        // Create a message with a timestamp from 2 hours ago
        let old_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            - 7200.0;

        let msg = WorkerMessage::Ready {
            worker_id: "W1".to_string(),
            timestamp: old_timestamp,
        };

        let result = validate_worker_message(&msg);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too old"));
    }

    #[test]
    fn test_create_and_serialize_all_message_types() {
        // Test worker messages
        let worker_messages = vec![
            WorkerMessage::ready("W1".to_string()),
            WorkerMessage::done("W1".to_string(), "issue-123".to_string(), 1500),
            WorkerMessage::failed(
                "W1".to_string(),
                "issue-456".to_string(),
                "Error occurred".to_string(),
                2500,
            ),
        ];

        for msg in worker_messages {
            let result = create_and_serialize_worker_message(msg);
            assert!(result.is_ok());
        }

        // Test orchestrator messages
        let orchestrator_messages = vec![
            OrchestratorMessage::task(
                "W1".to_string(),
                "issue-123".to_string(),
                1,
                "Task title".to_string(),
            ),
            OrchestratorMessage::wait("W1".to_string(), 60),
            OrchestratorMessage::shutdown("W1".to_string(), ShutdownReason::Error),
        ];

        for msg in orchestrator_messages {
            let result = create_and_serialize_orchestrator_message(msg);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_message_size_boundary() {
        // Create a message that's close to but under the limit
        let large_but_valid_error = "x".repeat(MAX_MESSAGE_SIZE / 2);
        let msg = WorkerMessage::failed(
            "W1".to_string(),
            "issue-123".to_string(),
            large_but_valid_error,
            1000,
        );

        let result = serialize_worker_message(&msg);
        // This should succeed if the overall message is under the limit
        match result {
            Ok(bytes) => assert!(bytes.len() <= MAX_MESSAGE_SIZE),
            Err(ProtocolError::MessageTooLarge { .. }) => {
                // Also acceptable if the serialized form exceeds the limit
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_envelope_new() {
        let payload = WorkerMessage::ready("W1".to_string());
        let envelope = MessageEnvelope::new(payload);
        assert_eq!(envelope.version, PROTOCOL_VERSION);
    }

    #[test]
    fn test_round_trip_with_validation() {
        let msg = WorkerMessage::done("W1".to_string(), "issue-789".to_string(), 2000);

        // Serialize with validation
        let bytes = create_and_serialize_worker_message(msg.clone()).unwrap();

        // Deserialize with validation
        let deserialized = deserialize_and_validate_worker_message(&bytes).unwrap();

        assert_eq!(msg.worker_id(), deserialized.worker_id());
    }

    #[test]
    fn test_validate_all_priority_values() {
        for priority in 0..=4 {
            let msg = OrchestratorMessage::task(
                "W1".to_string(),
                "issue-123".to_string(),
                priority,
                "Test".to_string(),
            );
            assert!(validate_orchestrator_message(&msg).is_ok());
        }

        // Priority 5 should fail
        let msg = OrchestratorMessage::task(
            "W1".to_string(),
            "issue-123".to_string(),
            5,
            "Test".to_string(),
        );
        assert!(validate_orchestrator_message(&msg).is_err());
    }

    #[test]
    fn test_validate_boundary_wait_seconds() {
        // Valid wait seconds
        let msg = OrchestratorMessage::wait("W1".to_string(), 3600);
        assert!(validate_orchestrator_message(&msg).is_ok());

        // Invalid wait seconds (too large)
        let msg = OrchestratorMessage::wait("W1".to_string(), 3601);
        assert!(validate_orchestrator_message(&msg).is_err());
    }

    #[test]
    fn test_validate_boundary_duration_ms() {
        // Valid duration
        let msg = WorkerMessage::done("W1".to_string(), "issue-123".to_string(), 3_600_000);
        assert!(validate_worker_message(&msg).is_ok());

        // Invalid duration (too large)
        let msg = WorkerMessage::done("W1".to_string(), "issue-123".to_string(), 3_600_001);
        assert!(validate_worker_message(&msg).is_err());
    }

    #[test]
    fn test_all_shutdown_reasons() {
        for reason in [
            ShutdownReason::UserRequested,
            ShutdownReason::Error,
            ShutdownReason::IdleTimeout,
        ] {
            let msg = OrchestratorMessage::shutdown("W1".to_string(), reason);
            let bytes = create_and_serialize_orchestrator_message(msg.clone()).unwrap();
            let deserialized = deserialize_and_validate_orchestrator_message(&bytes).unwrap();

            match (msg, deserialized) {
                (
                    OrchestratorMessage::Shutdown { reason: r1, .. },
                    OrchestratorMessage::Shutdown { reason: r2, .. },
                ) => assert_eq!(r1, r2),
                _ => panic!("Expected Shutdown messages"),
            }
        }
    }
}
