// Core data structures and message types

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

/// Get current Unix timestamp with millisecond precision
pub fn get_timestamp() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

// ============================================================================
// Message Types (Worker → Orchestrator)
// ============================================================================

/// Messages sent from worker to orchestrator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum WorkerMessage {
    /// Worker is idle and ready to receive a task
    #[serde(rename = "READY")]
    Ready {
        worker_id: String,
        timestamp: f64,
    },

    /// Worker successfully completed an assigned task
    #[serde(rename = "DONE")]
    Done {
        worker_id: String,
        timestamp: f64,
        issue_id: String,
        duration_ms: u64,
    },

    /// Worker failed to complete an assigned task
    #[serde(rename = "FAILED")]
    Failed {
        worker_id: String,
        timestamp: f64,
        issue_id: String,
        error: String,
        duration_ms: u64,
    },
}

impl WorkerMessage {
    /// Create a new READY message
    pub fn ready(worker_id: String) -> Self {
        WorkerMessage::Ready {
            worker_id,
            timestamp: get_timestamp(),
        }
    }

    /// Create a new DONE message
    pub fn done(worker_id: String, issue_id: String, duration_ms: u64) -> Self {
        WorkerMessage::Done {
            worker_id,
            timestamp: get_timestamp(),
            issue_id,
            duration_ms,
        }
    }

    /// Create a new FAILED message
    pub fn failed(worker_id: String, issue_id: String, error: String, duration_ms: u64) -> Self {
        WorkerMessage::Failed {
            worker_id,
            timestamp: get_timestamp(),
            issue_id,
            error,
            duration_ms,
        }
    }

    /// Get the worker_id from any message variant
    pub fn worker_id(&self) -> &str {
        match self {
            WorkerMessage::Ready { worker_id, .. } => worker_id,
            WorkerMessage::Done { worker_id, .. } => worker_id,
            WorkerMessage::Failed { worker_id, .. } => worker_id,
        }
    }

    /// Get the timestamp from any message variant
    pub fn timestamp(&self) -> f64 {
        match self {
            WorkerMessage::Ready { timestamp, .. } => *timestamp,
            WorkerMessage::Done { timestamp, .. } => *timestamp,
            WorkerMessage::Failed { timestamp, .. } => *timestamp,
        }
    }
}

// ============================================================================
// Message Types (Orchestrator → Worker)
// ============================================================================

/// Messages sent from orchestrator to worker
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum OrchestratorMessage {
    /// Assign a task to the worker
    #[serde(rename = "TASK")]
    Task {
        worker_id: String,
        timestamp: f64,
        issue_id: String,
        priority: u8,
        title: String,
    },

    /// No tasks currently available, worker should wait
    #[serde(rename = "WAIT")]
    Wait {
        worker_id: String,
        timestamp: f64,
        wait_seconds: u32,
    },

    /// Gracefully shut down the worker
    #[serde(rename = "SHUTDOWN")]
    Shutdown {
        worker_id: String,
        timestamp: f64,
        reason: ShutdownReason,
    },
}

impl OrchestratorMessage {
    /// Create a new TASK message
    pub fn task(worker_id: String, issue_id: String, priority: u8, title: String) -> Self {
        OrchestratorMessage::Task {
            worker_id,
            timestamp: get_timestamp(),
            issue_id,
            priority,
            title,
        }
    }

    /// Create a new WAIT message
    pub fn wait(worker_id: String, wait_seconds: u32) -> Self {
        OrchestratorMessage::Wait {
            worker_id,
            timestamp: get_timestamp(),
            wait_seconds,
        }
    }

    /// Create a new SHUTDOWN message
    pub fn shutdown(worker_id: String, reason: ShutdownReason) -> Self {
        OrchestratorMessage::Shutdown {
            worker_id,
            timestamp: get_timestamp(),
            reason,
        }
    }

    /// Get the worker_id from any message variant
    pub fn worker_id(&self) -> &str {
        match self {
            OrchestratorMessage::Task { worker_id, .. } => worker_id,
            OrchestratorMessage::Wait { worker_id, .. } => worker_id,
            OrchestratorMessage::Shutdown { worker_id, .. } => worker_id,
        }
    }

    /// Get the timestamp from any message variant
    pub fn timestamp(&self) -> f64 {
        match self {
            OrchestratorMessage::Task { timestamp, .. } => *timestamp,
            OrchestratorMessage::Wait { timestamp, .. } => *timestamp,
            OrchestratorMessage::Shutdown { timestamp, .. } => *timestamp,
        }
    }
}

/// Shutdown reason for graceful worker termination
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownReason {
    /// User requested shutdown
    UserRequested,
    /// Error condition requiring shutdown
    Error,
    /// Worker idle timeout
    IdleTimeout,
}

// ============================================================================
// Worker State
// ============================================================================

/// Current state of a worker
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerState {
    /// Worker is idle and ready to receive tasks
    Idle,
    /// Worker is currently executing a task
    Working,
    /// Worker is waiting before sending next READY
    Waiting,
    /// Worker is shutting down
    ShuttingDown,
    /// Worker has disconnected
    Disconnected,
}

// ============================================================================
// Task Queue
// ============================================================================

/// A task in the queue waiting to be assigned
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    /// Beads issue ID (e.g., "beads-workers-abc")
    pub issue_id: String,
    /// Task priority (0 = highest, 4 = lowest)
    pub priority: u8,
    /// Brief task description
    pub title: String,
    /// When the task was added to the queue
    pub queued_at: u64,
}

impl Task {
    /// Create a new task
    pub fn new(issue_id: String, priority: u8, title: String) -> Self {
        Task {
            issue_id,
            priority,
            title,
            queued_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

/// Priority-based task queue
#[derive(Debug, Default)]
pub struct TaskQueue {
    /// Tasks organized by priority (0 = highest, 4 = lowest)
    queues: [VecDeque<Task>; 5],
}

impl TaskQueue {
    /// Create a new empty task queue
    pub fn new() -> Self {
        TaskQueue {
            queues: [
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
                VecDeque::new(),
            ],
        }
    }

    /// Add a task to the queue
    pub fn push(&mut self, task: Task) {
        let priority = task.priority.min(4) as usize;
        self.queues[priority].push_back(task);
    }

    /// Get the next highest priority task
    pub fn pop(&mut self) -> Option<Task> {
        for queue in &mut self.queues {
            if let Some(task) = queue.pop_front() {
                return Some(task);
            }
        }
        None
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.queues.iter().all(|q| q.is_empty())
    }

    /// Get the total number of tasks in the queue
    pub fn len(&self) -> usize {
        self.queues.iter().map(|q| q.len()).sum()
    }

    /// Get tasks by priority level
    pub fn tasks_at_priority(&self, priority: u8) -> impl Iterator<Item = &Task> {
        let priority = priority.min(4) as usize;
        self.queues[priority].iter()
    }
}

// ============================================================================
// Worker Registry
// ============================================================================

/// Information about a registered worker
#[derive(Debug, Clone)]
pub struct WorkerInfo {
    /// Worker ID (e.g., "W1")
    pub worker_id: String,
    /// Current state of the worker
    pub state: WorkerState,
    /// Currently assigned task (if any)
    pub current_task: Option<String>,
    /// When the worker was registered
    pub registered_at: u64,
    /// When the worker last sent a message
    pub last_seen: u64,
    /// Total tasks completed by this worker
    pub tasks_completed: u64,
    /// Total tasks failed by this worker
    pub tasks_failed: u64,
}

impl WorkerInfo {
    /// Create a new worker info
    pub fn new(worker_id: String) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        WorkerInfo {
            worker_id,
            state: WorkerState::Idle,
            current_task: None,
            registered_at: now,
            last_seen: now,
            tasks_completed: 0,
            tasks_failed: 0,
        }
    }

    /// Update the last seen timestamp
    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    /// Assign a task to this worker
    pub fn assign_task(&mut self, issue_id: String) {
        self.state = WorkerState::Working;
        self.current_task = Some(issue_id);
        self.update_last_seen();
    }

    /// Mark a task as completed
    pub fn complete_task(&mut self) {
        self.tasks_completed += 1;
        self.current_task = None;
        self.state = WorkerState::Idle;
        self.update_last_seen();
    }

    /// Mark a task as failed
    pub fn fail_task(&mut self) {
        self.tasks_failed += 1;
        self.current_task = None;
        self.state = WorkerState::Idle;
        self.update_last_seen();
    }
}

/// Registry of all connected workers
#[derive(Debug, Default)]
pub struct WorkerRegistry {
    /// Map of worker_id to worker info
    workers: HashMap<String, WorkerInfo>,
    /// Counter for generating new worker IDs
    next_worker_id: usize,
}

impl WorkerRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        WorkerRegistry {
            workers: HashMap::new(),
            next_worker_id: 1,
        }
    }

    /// Register a new worker and return its assigned ID
    pub fn register(&mut self) -> String {
        let worker_id = format!("W{}", self.next_worker_id);
        self.next_worker_id += 1;

        let info = WorkerInfo::new(worker_id.clone());
        self.workers.insert(worker_id.clone(), info);

        worker_id
    }

    /// Get worker info by ID
    pub fn get(&self, worker_id: &str) -> Option<&WorkerInfo> {
        self.workers.get(worker_id)
    }

    /// Get mutable worker info by ID
    pub fn get_mut(&mut self, worker_id: &str) -> Option<&mut WorkerInfo> {
        self.workers.get_mut(worker_id)
    }

    /// Remove a worker from the registry
    pub fn unregister(&mut self, worker_id: &str) -> Option<WorkerInfo> {
        self.workers.remove(worker_id)
    }

    /// Get all workers
    pub fn all(&self) -> impl Iterator<Item = &WorkerInfo> {
        self.workers.values()
    }

    /// Get idle workers (ready to receive tasks)
    pub fn idle_workers(&self) -> impl Iterator<Item = &WorkerInfo> {
        self.workers.values().filter(|w| w.state == WorkerState::Idle)
    }

    /// Get working workers
    pub fn working_workers(&self) -> impl Iterator<Item = &WorkerInfo> {
        self.workers.values().filter(|w| w.state == WorkerState::Working)
    }

    /// Get total number of registered workers
    pub fn count(&self) -> usize {
        self.workers.len()
    }
}

// ============================================================================
// Configuration
// ============================================================================

/// Orchestrator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Socket path (e.g., "/tmp/beads-workers-12345.sock")
    pub socket_path: String,
    /// Maximum number of concurrent workers
    pub max_workers: usize,
    /// Default wait time in seconds when no tasks available
    pub default_wait_seconds: u32,
    /// Worker idle timeout in seconds (0 = no timeout)
    pub worker_idle_timeout: u64,
    /// Graceful shutdown timeout in seconds
    pub shutdown_timeout: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        OrchestratorConfig {
            socket_path: format!("/tmp/beads-workers-{}.sock", std::process::id()),
            max_workers: 10,
            default_wait_seconds: 30,
            worker_idle_timeout: 0,
            shutdown_timeout: 10,
        }
    }
}

/// Worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Socket path to connect to
    pub socket_path: String,
    /// Connection retry attempts
    pub retry_attempts: u32,
    /// Connection retry delay in seconds (exponential backoff base)
    pub retry_delay_seconds: u64,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        WorkerConfig {
            socket_path: String::new(),
            retry_attempts: 3,
            retry_delay_seconds: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_message_ready() {
        let msg = WorkerMessage::ready("W1".to_string());
        assert_eq!(msg.worker_id(), "W1");
        assert!(msg.timestamp() > 0.0);
    }

    #[test]
    fn test_worker_message_done() {
        let msg = WorkerMessage::done("W1".to_string(), "issue-123".to_string(), 5000);
        assert_eq!(msg.worker_id(), "W1");
        match msg {
            WorkerMessage::Done { issue_id, duration_ms, .. } => {
                assert_eq!(issue_id, "issue-123");
                assert_eq!(duration_ms, 5000);
            }
            _ => panic!("Expected Done variant"),
        }
    }

    #[test]
    fn test_orchestrator_message_task() {
        let msg = OrchestratorMessage::task(
            "W1".to_string(),
            "issue-123".to_string(),
            2,
            "Test task".to_string(),
        );
        assert_eq!(msg.worker_id(), "W1");
        match msg {
            OrchestratorMessage::Task { issue_id, priority, title, .. } => {
                assert_eq!(issue_id, "issue-123");
                assert_eq!(priority, 2);
                assert_eq!(title, "Test task");
            }
            _ => panic!("Expected Task variant"),
        }
    }

    #[test]
    fn test_task_queue_priority() {
        let mut queue = TaskQueue::new();

        queue.push(Task::new("low".to_string(), 3, "Low priority".to_string()));
        queue.push(Task::new("high".to_string(), 0, "High priority".to_string()));
        queue.push(Task::new("medium".to_string(), 1, "Medium priority".to_string()));

        assert_eq!(queue.len(), 3);

        // Should pop in priority order
        let task1 = queue.pop().unwrap();
        assert_eq!(task1.issue_id, "high");

        let task2 = queue.pop().unwrap();
        assert_eq!(task2.issue_id, "medium");

        let task3 = queue.pop().unwrap();
        assert_eq!(task3.issue_id, "low");

        assert!(queue.is_empty());
    }

    #[test]
    fn test_worker_registry() {
        let mut registry = WorkerRegistry::new();

        let w1 = registry.register();
        let w2 = registry.register();

        assert_eq!(w1, "W1");
        assert_eq!(w2, "W2");
        assert_eq!(registry.count(), 2);

        let info = registry.get(&w1).unwrap();
        assert_eq!(info.state, WorkerState::Idle);

        registry.unregister(&w1);
        assert_eq!(registry.count(), 1);
    }

    #[test]
    fn test_worker_info_lifecycle() {
        let mut info = WorkerInfo::new("W1".to_string());

        assert_eq!(info.state, WorkerState::Idle);
        assert_eq!(info.tasks_completed, 0);

        info.assign_task("task-1".to_string());
        assert_eq!(info.state, WorkerState::Working);
        assert_eq!(info.current_task, Some("task-1".to_string()));

        info.complete_task();
        assert_eq!(info.state, WorkerState::Idle);
        assert_eq!(info.current_task, None);
        assert_eq!(info.tasks_completed, 1);

        info.assign_task("task-2".to_string());
        info.fail_task();
        assert_eq!(info.state, WorkerState::Idle);
        assert_eq!(info.tasks_failed, 1);
    }

    #[test]
    fn test_message_serialization() {
        let msg = WorkerMessage::ready("W1".to_string());
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: WorkerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.worker_id(), deserialized.worker_id());
    }

    #[test]
    fn test_orchestrator_config_default() {
        let config = OrchestratorConfig::default();
        assert!(config.socket_path.contains("/tmp/beads-workers-"));
        assert_eq!(config.max_workers, 10);
        assert_eq!(config.default_wait_seconds, 30);
    }
}
