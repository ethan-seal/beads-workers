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

    /// Worker heartbeat to indicate it's still alive
    #[serde(rename = "HEARTBEAT")]
    Heartbeat {
        worker_id: String,
        timestamp: f64,
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

    /// Create a new HEARTBEAT message
    pub fn heartbeat(worker_id: String) -> Self {
        WorkerMessage::Heartbeat {
            worker_id,
            timestamp: get_timestamp(),
        }
    }

    /// Get the worker_id from any message variant
    pub fn worker_id(&self) -> &str {
        match self {
            WorkerMessage::Ready { worker_id, .. } => worker_id,
            WorkerMessage::Done { worker_id, .. } => worker_id,
            WorkerMessage::Failed { worker_id, .. } => worker_id,
            WorkerMessage::Heartbeat { worker_id, .. } => worker_id,
        }
    }

    /// Get the timestamp from any message variant
    pub fn timestamp(&self) -> f64 {
        match self {
            WorkerMessage::Ready { timestamp, .. } => *timestamp,
            WorkerMessage::Done { timestamp, .. } => *timestamp,
            WorkerMessage::Failed { timestamp, .. } => *timestamp,
            WorkerMessage::Heartbeat { timestamp, .. } => *timestamp,
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
    /// When the last heartbeat was received
    pub last_heartbeat: u64,
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
            last_heartbeat: now,
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

    /// Update the last heartbeat timestamp
    pub fn update_heartbeat(&mut self) {
        self.last_heartbeat = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.update_last_seen();
    }

    /// Check if worker is stale (hasn't sent heartbeat in given seconds)
    pub fn is_stale(&self, timeout_seconds: u64) -> bool {
        if timeout_seconds == 0 {
            return false;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now.saturating_sub(self.last_heartbeat) > timeout_seconds
    }

    /// Get seconds since last heartbeat
    pub fn seconds_since_heartbeat(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now.saturating_sub(self.last_heartbeat)
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

    /// Find stale workers (haven't sent heartbeat in timeout seconds)
    pub fn find_stale_workers(&self, timeout_seconds: u64) -> Vec<String> {
        self.workers
            .values()
            .filter(|w| w.is_stale(timeout_seconds))
            .map(|w| w.worker_id.clone())
            .collect()
    }

    /// Get disconnected workers
    pub fn disconnected_workers(&self) -> impl Iterator<Item = &WorkerInfo> {
        self.workers.values().filter(|w| w.state == WorkerState::Disconnected)
    }

    /// Remove all disconnected workers
    pub fn cleanup_disconnected(&mut self) -> Vec<String> {
        let disconnected: Vec<String> = self
            .disconnected_workers()
            .map(|w| w.worker_id.clone())
            .collect();

        for worker_id in &disconnected {
            self.workers.remove(worker_id);
        }

        disconnected
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
    /// Worker stale timeout in seconds (0 = disabled)
    /// Workers that haven't sent a message in this time are considered stale
    pub worker_stale_timeout: u64,
    /// How often to check for stale workers in seconds
    pub stale_check_interval: u64,
    /// Task polling interval in seconds (how often to check bd ready)
    pub poll_interval_seconds: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        OrchestratorConfig {
            socket_path: format!("/tmp/beads-workers-{}.sock", std::process::id()),
            max_workers: 10,
            default_wait_seconds: 30,
            worker_idle_timeout: 0,
            shutdown_timeout: 10,
            worker_stale_timeout: 300, // 5 minutes default
            stale_check_interval: 30,   // Check every 30 seconds
            poll_interval_seconds: 30,  // Poll bd ready every 30 seconds
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

    #[test]
    fn test_worker_message_failed() {
        let msg = WorkerMessage::failed(
            "W1".to_string(),
            "issue-123".to_string(),
            "Connection timeout".to_string(),
            3000,
        );
        assert_eq!(msg.worker_id(), "W1");
        match msg {
            WorkerMessage::Failed { issue_id, error, duration_ms, .. } => {
                assert_eq!(issue_id, "issue-123");
                assert_eq!(error, "Connection timeout");
                assert_eq!(duration_ms, 3000);
            }
            _ => panic!("Expected Failed variant"),
        }
    }

    #[test]
    fn test_orchestrator_message_wait() {
        let msg = OrchestratorMessage::wait("W1".to_string(), 60);
        assert_eq!(msg.worker_id(), "W1");
        match msg {
            OrchestratorMessage::Wait { wait_seconds, .. } => {
                assert_eq!(wait_seconds, 60);
            }
            _ => panic!("Expected Wait variant"),
        }
    }

    #[test]
    fn test_orchestrator_message_shutdown() {
        let msg = OrchestratorMessage::shutdown("W1".to_string(), ShutdownReason::UserRequested);
        assert_eq!(msg.worker_id(), "W1");
        match msg {
            OrchestratorMessage::Shutdown { reason, .. } => {
                assert_eq!(reason, ShutdownReason::UserRequested);
            }
            _ => panic!("Expected Shutdown variant"),
        }
    }

    #[test]
    fn test_shutdown_reasons() {
        let reasons = vec![
            ShutdownReason::UserRequested,
            ShutdownReason::Error,
            ShutdownReason::IdleTimeout,
        ];
        for reason in reasons {
            let msg = OrchestratorMessage::shutdown("W1".to_string(), reason);
            match msg {
                OrchestratorMessage::Shutdown { reason: r, .. } => assert_eq!(r, reason),
                _ => panic!("Expected Shutdown variant"),
            }
        }
    }

    #[test]
    fn test_worker_state_transitions() {
        let states = vec![
            WorkerState::Idle,
            WorkerState::Working,
            WorkerState::Waiting,
            WorkerState::ShuttingDown,
            WorkerState::Disconnected,
        ];
        for state in states {
            let mut info = WorkerInfo::new("W1".to_string());
            info.state = state;
            assert_eq!(info.state, state);
        }
    }

    #[test]
    fn test_task_new() {
        let task = Task::new("issue-123".to_string(), 2, "Test task".to_string());
        assert_eq!(task.issue_id, "issue-123");
        assert_eq!(task.priority, 2);
        assert_eq!(task.title, "Test task");
        assert!(task.queued_at > 0);
    }

    #[test]
    fn test_task_queue_fifo_within_priority() {
        let mut queue = TaskQueue::new();

        // Add multiple tasks with same priority
        queue.push(Task::new("first".to_string(), 1, "First".to_string()));
        queue.push(Task::new("second".to_string(), 1, "Second".to_string()));
        queue.push(Task::new("third".to_string(), 1, "Third".to_string()));

        // Should pop in FIFO order within same priority
        assert_eq!(queue.pop().unwrap().issue_id, "first");
        assert_eq!(queue.pop().unwrap().issue_id, "second");
        assert_eq!(queue.pop().unwrap().issue_id, "third");
    }

    #[test]
    fn test_task_queue_priority_clamping() {
        let mut queue = TaskQueue::new();

        // Priority > 4 should be clamped to 4
        queue.push(Task::new("task1".to_string(), 10, "High priority".to_string()));
        queue.push(Task::new("task2".to_string(), 0, "Low priority".to_string()));

        // Task with priority 0 should come first
        assert_eq!(queue.pop().unwrap().issue_id, "task2");
        assert_eq!(queue.pop().unwrap().issue_id, "task1");
    }

    #[test]
    fn test_task_queue_empty_operations() {
        let mut queue = TaskQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.pop().is_none());
    }

    #[test]
    fn test_task_queue_tasks_at_priority() {
        let mut queue = TaskQueue::new();

        queue.push(Task::new("p0-1".to_string(), 0, "P0 task 1".to_string()));
        queue.push(Task::new("p0-2".to_string(), 0, "P0 task 2".to_string()));
        queue.push(Task::new("p2-1".to_string(), 2, "P2 task 1".to_string()));

        let p0_tasks: Vec<_> = queue.tasks_at_priority(0).collect();
        assert_eq!(p0_tasks.len(), 2);

        let p2_tasks: Vec<_> = queue.tasks_at_priority(2).collect();
        assert_eq!(p2_tasks.len(), 1);

        let p1_tasks: Vec<_> = queue.tasks_at_priority(1).collect();
        assert_eq!(p1_tasks.len(), 0);
    }

    #[test]
    fn test_worker_registry_multiple_workers() {
        let mut registry = WorkerRegistry::new();

        let ids: Vec<_> = (0..5).map(|_| registry.register()).collect();

        assert_eq!(registry.count(), 5);
        assert_eq!(ids, vec!["W1", "W2", "W3", "W4", "W5"]);

        for id in &ids {
            assert!(registry.get(id).is_some());
        }
    }

    #[test]
    fn test_worker_registry_idle_and_working() {
        let mut registry = WorkerRegistry::new();

        let w1 = registry.register();
        let w2 = registry.register();
        let _w3 = registry.register();

        // Initially all idle
        assert_eq!(registry.idle_workers().count(), 3);
        assert_eq!(registry.working_workers().count(), 0);

        // Assign task to w1
        registry.get_mut(&w1).unwrap().assign_task("task-1".to_string());

        assert_eq!(registry.idle_workers().count(), 2);
        assert_eq!(registry.working_workers().count(), 1);

        // Assign task to w2
        registry.get_mut(&w2).unwrap().assign_task("task-2".to_string());

        assert_eq!(registry.idle_workers().count(), 1);
        assert_eq!(registry.working_workers().count(), 2);
    }

    #[test]
    fn test_worker_registry_unregister() {
        let mut registry = WorkerRegistry::new();

        let w1 = registry.register();
        let w2 = registry.register();

        assert_eq!(registry.count(), 2);

        let removed = registry.unregister(&w1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().worker_id, w1);
        assert_eq!(registry.count(), 1);

        assert!(registry.get(&w1).is_none());
        assert!(registry.get(&w2).is_some());
    }

    #[test]
    fn test_worker_info_update_last_seen() {
        let mut info = WorkerInfo::new("W1".to_string());
        let initial_time = info.last_seen;

        std::thread::sleep(std::time::Duration::from_secs(1));
        info.update_last_seen();

        assert!(info.last_seen > initial_time);
    }

    #[test]
    fn test_worker_info_task_counters() {
        let mut info = WorkerInfo::new("W1".to_string());

        assert_eq!(info.tasks_completed, 0);
        assert_eq!(info.tasks_failed, 0);

        info.assign_task("task-1".to_string());
        info.complete_task();
        assert_eq!(info.tasks_completed, 1);

        info.assign_task("task-2".to_string());
        info.fail_task();
        assert_eq!(info.tasks_failed, 1);

        info.assign_task("task-3".to_string());
        info.complete_task();
        assert_eq!(info.tasks_completed, 2);
        assert_eq!(info.tasks_failed, 1);
    }

    #[test]
    fn test_worker_config_default() {
        let config = WorkerConfig::default();
        assert!(config.socket_path.is_empty());
        assert_eq!(config.retry_attempts, 3);
        assert_eq!(config.retry_delay_seconds, 1);
    }

    #[test]
    fn test_orchestrator_config_fields() {
        let config = OrchestratorConfig {
            socket_path: "/tmp/test.sock".to_string(),
            max_workers: 5,
            default_wait_seconds: 15,
            worker_idle_timeout: 120,
            shutdown_timeout: 30,
            worker_stale_timeout: 300,
            stale_check_interval: 30,
            poll_interval_seconds: 60,
        };

        assert_eq!(config.socket_path, "/tmp/test.sock");
        assert_eq!(config.max_workers, 5);
        assert_eq!(config.default_wait_seconds, 15);
        assert_eq!(config.worker_idle_timeout, 120);
        assert_eq!(config.shutdown_timeout, 30);
        assert_eq!(config.worker_stale_timeout, 300);
    }

    #[test]
    fn test_get_timestamp() {
        let ts1 = get_timestamp();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ts2 = get_timestamp();

        assert!(ts2 > ts1);
        assert!(ts2 - ts1 < 1.0); // Should be less than 1 second
    }

    #[test]
    fn test_message_json_format() {
        let msg = WorkerMessage::ready("W1".to_string());
        let json = serde_json::to_string(&msg).unwrap();

        // Check JSON structure
        assert!(json.contains("\"type\":\"READY\""));
        assert!(json.contains("\"worker_id\":\"W1\""));
        assert!(json.contains("\"timestamp\""));
    }

    #[test]
    fn test_orchestrator_message_json_format() {
        let msg = OrchestratorMessage::task(
            "W1".to_string(),
            "issue-123".to_string(),
            2,
            "Test".to_string(),
        );
        let json = serde_json::to_string(&msg).unwrap();

        assert!(json.contains("\"type\":\"TASK\""));
        assert!(json.contains("\"worker_id\":\"W1\""));
        assert!(json.contains("\"issue_id\":\"issue-123\""));
        assert!(json.contains("\"priority\":2"));
    }

    #[test]
    fn test_shutdown_reason_serialization() {
        let reasons = vec![
            (ShutdownReason::UserRequested, "user_requested"),
            (ShutdownReason::Error, "error"),
            (ShutdownReason::IdleTimeout, "idle_timeout"),
        ];

        for (reason, expected_str) in reasons {
            let json = serde_json::to_string(&reason).unwrap();
            assert!(json.contains(expected_str));
        }
    }

    #[test]
    fn test_worker_heartbeat_update() {
        let mut info = WorkerInfo::new("W1".to_string());
        let initial_heartbeat = info.last_heartbeat;

        std::thread::sleep(std::time::Duration::from_secs(1));
        info.update_heartbeat();

        assert!(info.last_heartbeat > initial_heartbeat);
        assert!(info.last_seen > initial_heartbeat);
    }

    #[test]
    fn test_task_queue_multiple_priorities() {
        let mut queue = TaskQueue::new();

        // Add tasks at all priority levels
        for priority in 0..=4 {
            queue.push(Task::new(
                format!("task-p{}", priority),
                priority,
                format!("Priority {} task", priority),
            ));
        }

        assert_eq!(queue.len(), 5);

        // Should pop in priority order (0 is highest)
        for priority in 0..=4 {
            let task = queue.pop().unwrap();
            assert_eq!(task.issue_id, format!("task-p{}", priority));
        }

        assert!(queue.is_empty());
    }

    #[test]
    fn test_worker_registry_all_states() {
        let mut registry = WorkerRegistry::new();

        let w1 = registry.register();
        let w2 = registry.register();
        let w3 = registry.register();
        let w4 = registry.register();

        // Set different states
        registry.get_mut(&w1).unwrap().state = WorkerState::Idle;
        registry.get_mut(&w2).unwrap().state = WorkerState::Working;
        registry.get_mut(&w3).unwrap().state = WorkerState::Waiting;
        registry.get_mut(&w4).unwrap().state = WorkerState::Disconnected;

        assert_eq!(registry.idle_workers().count(), 1);
        assert_eq!(registry.working_workers().count(), 1);
        assert_eq!(registry.disconnected_workers().count(), 1);
    }

    #[test]
    fn test_worker_message_all_types() {
        let messages = vec![
            WorkerMessage::ready("W1".to_string()),
            WorkerMessage::done("W1".to_string(), "issue-123".to_string(), 1000),
            WorkerMessage::failed("W1".to_string(), "issue-456".to_string(), "error".to_string(), 2000),
            WorkerMessage::heartbeat("W1".to_string()),
        ];

        for msg in messages {
            assert_eq!(msg.worker_id(), "W1");
            assert!(msg.timestamp() > 0.0);
        }
    }

    #[test]
    fn test_orchestrator_message_all_types() {
        let messages = vec![
            OrchestratorMessage::task("W1".to_string(), "issue".to_string(), 0, "title".to_string()),
            OrchestratorMessage::wait("W1".to_string(), 30),
            OrchestratorMessage::shutdown("W1".to_string(), ShutdownReason::UserRequested),
        ];

        for msg in messages {
            assert_eq!(msg.worker_id(), "W1");
            assert!(msg.timestamp() > 0.0);
        }
    }

    #[test]
    fn test_task_queue_clamping_edge_cases() {
        let mut queue = TaskQueue::new();

        // Test priority clamping with extreme values
        queue.push(Task::new("task1".to_string(), 255, "Max u8".to_string()));
        queue.push(Task::new("task2".to_string(), 100, "Large value".to_string()));

        // Both should be clamped to priority 4
        let task1 = queue.pop().unwrap();
        let task2 = queue.pop().unwrap();

        // Order within same priority is FIFO
        assert_eq!(task1.issue_id, "task1");
        assert_eq!(task2.issue_id, "task2");
    }

    #[test]
    fn test_worker_registry_get_nonexistent() {
        let registry = WorkerRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_worker_registry_unregister_nonexistent() {
        let mut registry = WorkerRegistry::new();
        let result = registry.unregister("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_worker_info_consecutive_operations() {
        let mut info = WorkerInfo::new("W1".to_string());

        // Multiple task completions
        for i in 0..5 {
            info.assign_task(format!("task-{}", i));
            assert_eq!(info.state, WorkerState::Working);
            info.complete_task();
            assert_eq!(info.state, WorkerState::Idle);
        }

        assert_eq!(info.tasks_completed, 5);
        assert_eq!(info.tasks_failed, 0);

        // Multiple task failures
        for i in 0..3 {
            info.assign_task(format!("task-{}", i));
            info.fail_task();
        }

        assert_eq!(info.tasks_completed, 5);
        assert_eq!(info.tasks_failed, 3);
    }

    #[test]
    fn test_worker_is_stale() {
        let mut info = WorkerInfo::new("W1".to_string());

        // Not stale immediately after creation
        assert!(!info.is_stale(10));

        // Manually set last_heartbeat to be old
        info.last_heartbeat = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 100; // 100 seconds ago

        // Should be stale with 50 second timeout
        assert!(info.is_stale(50));

        // Should not be stale with 200 second timeout
        assert!(!info.is_stale(200));

        // Timeout of 0 means disabled
        assert!(!info.is_stale(0));
    }

    #[test]
    fn test_worker_seconds_since_heartbeat() {
        let mut info = WorkerInfo::new("W1".to_string());

        // Should be close to 0 immediately
        assert!(info.seconds_since_heartbeat() < 2);

        // Manually set old heartbeat
        info.last_heartbeat = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 50; // 50 seconds ago

        let elapsed = info.seconds_since_heartbeat();
        assert!(elapsed >= 50 && elapsed <= 52);
    }

    #[test]
    fn test_worker_registry_find_stale_workers() {
        let mut registry = WorkerRegistry::new();

        let w1 = registry.register();
        let w2 = registry.register();
        let w3 = registry.register();

        // Make w2 stale by setting old heartbeat
        if let Some(worker) = registry.get_mut(&w2) {
            worker.last_heartbeat = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                - 100; // 100 seconds ago
        }

        let stale = registry.find_stale_workers(50);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0], w2);

        // With higher timeout, none should be stale
        let stale = registry.find_stale_workers(200);
        assert_eq!(stale.len(), 0);

        // With disabled timeout (0), none should be stale
        let stale = registry.find_stale_workers(0);
        assert_eq!(stale.len(), 0);
    }

    #[test]
    fn test_worker_registry_disconnected_workers() {
        let mut registry = WorkerRegistry::new();

        let w1 = registry.register();
        let w2 = registry.register();
        let w3 = registry.register();

        // Mark w2 as disconnected
        if let Some(worker) = registry.get_mut(&w2) {
            worker.state = WorkerState::Disconnected;
        }

        let disconnected: Vec<_> = registry.disconnected_workers().collect();
        assert_eq!(disconnected.len(), 1);
        assert_eq!(disconnected[0].worker_id, w2);
    }

    #[test]
    fn test_worker_registry_cleanup_disconnected() {
        let mut registry = WorkerRegistry::new();

        let w1 = registry.register();
        let w2 = registry.register();
        let w3 = registry.register();

        // Mark w1 and w3 as disconnected
        if let Some(worker) = registry.get_mut(&w1) {
            worker.state = WorkerState::Disconnected;
        }
        if let Some(worker) = registry.get_mut(&w3) {
            worker.state = WorkerState::Disconnected;
        }

        assert_eq!(registry.count(), 3);

        let removed = registry.cleanup_disconnected();
        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&w1));
        assert!(removed.contains(&w3));
        assert_eq!(registry.count(), 1);

        // Only w2 should remain
        assert!(registry.get(&w2).is_some());
        assert!(registry.get(&w1).is_none());
        assert!(registry.get(&w3).is_none());
    }

    #[test]
    fn test_worker_message_heartbeat() {
        let msg = WorkerMessage::heartbeat("W1".to_string());
        assert_eq!(msg.worker_id(), "W1");
        assert!(msg.timestamp() > 0.0);

        match msg {
            WorkerMessage::Heartbeat { worker_id, .. } => {
                assert_eq!(worker_id, "W1");
            }
            _ => panic!("Expected Heartbeat variant"),
        }
    }
}
