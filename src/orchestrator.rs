// Orchestrator main loop and event handling

use crate::server::{ConnectionHandler, UnixSocketServer};
use crate::types::{
    OrchestratorConfig, OrchestratorMessage, ShutdownReason, Task, TaskQueue, WorkerMessage,
    WorkerRegistry, WorkerState,
};
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

/// The main orchestrator that manages workers and tasks
pub struct Orchestrator {
    /// Configuration
    config: OrchestratorConfig,
    /// Unix socket server
    server: UnixSocketServer,
    /// Worker registry
    registry: WorkerRegistry,
    /// Task queue
    task_queue: TaskQueue,
    /// Channel senders for each worker connection
    worker_channels: HashMap<String, mpsc::UnboundedSender<OrchestratorMessage>>,
    /// Shutdown signal
    shutdown_rx: mpsc::Receiver<()>,
    shutdown_tx: mpsc::Sender<()>,
}

impl Orchestrator {
    /// Create a new orchestrator
    pub async fn new(config: OrchestratorConfig) -> std::io::Result<Self> {
        let server = UnixSocketServer::bind(&config.socket_path).await?;
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);

        Ok(Orchestrator {
            config,
            server,
            registry: WorkerRegistry::new(),
            task_queue: TaskQueue::new(),
            worker_channels: HashMap::new(),
            shutdown_rx,
            shutdown_tx,
        })
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &Path {
        self.server.socket_path()
    }

    /// Get a shutdown trigger
    pub fn shutdown_handle(&self) -> mpsc::Sender<()> {
        self.shutdown_tx.clone()
    }

    /// Add a task to the queue
    pub fn add_task(&mut self, task: Task) {
        info!(
            "Adding task to queue: {} (priority {})",
            task.issue_id, task.priority
        );
        self.task_queue.push(task);
    }

    /// Run the orchestrator main loop
    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!(
            "Orchestrator starting on socket: {:?}",
            self.server.socket_path()
        );
        info!("Max workers: {}", self.config.max_workers);

        // Channel for receiving worker messages from all connections
        let (worker_msg_tx, mut worker_msg_rx) = mpsc::unbounded_channel::<WorkerMessageEvent>();

        loop {
            tokio::select! {
                // Accept new connections
                result = self.server.accept() => {
                    match result {
                        Ok(handler) => {
                            if self.registry.count() >= self.config.max_workers {
                                warn!("Maximum number of workers reached, rejecting connection");
                                // TODO: Send rejection message and close connection
                                continue;
                            }

                            let worker_id = self.registry.register();
                            info!("New worker connected: {}", worker_id);

                            // Spawn handler for this connection
                            self.spawn_connection_handler(worker_id.clone(), handler, worker_msg_tx.clone());
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }

                // Handle worker messages
                Some(event) = worker_msg_rx.recv() => {
                    self.handle_worker_message(event).await;
                }

                // Handle shutdown signal
                _ = self.shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    break;
                }
            }
        }

        // Graceful shutdown
        self.shutdown().await?;

        Ok(())
    }

    /// Spawn a connection handler task
    fn spawn_connection_handler(
        &mut self,
        worker_id: String,
        mut handler: ConnectionHandler,
        worker_msg_tx: mpsc::UnboundedSender<WorkerMessageEvent>,
    ) {
        // Create channel for sending messages to this worker
        let (orch_msg_tx, mut orch_msg_rx) = mpsc::unbounded_channel::<OrchestratorMessage>();
        self.worker_channels.insert(worker_id.clone(), orch_msg_tx);

        let worker_id_clone = worker_id.clone();

        tokio::spawn(async move {
            debug!("Connection handler started for worker {}", worker_id);

            loop {
                tokio::select! {
                    // Read from worker
                    result = handler.read_worker_message() => {
                        match result {
                            Ok(Some(msg)) => {
                                debug!("Received message from worker {}: {:?}", worker_id, msg);
                                if let Err(e) = worker_msg_tx.send(WorkerMessageEvent {
                                    worker_id: worker_id.clone(),
                                    message: msg,
                                }) {
                                    error!("Failed to forward worker message: {}", e);
                                    break;
                                }
                            }
                            Ok(None) => {
                                info!("Worker {} disconnected", worker_id);
                                // Notify about disconnection
                                let _ = worker_msg_tx.send(WorkerMessageEvent {
                                    worker_id: worker_id.clone(),
                                    message: WorkerMessage::Ready {
                                        worker_id: worker_id.clone(),
                                        timestamp: 0.0, // Special marker for disconnection
                                    },
                                });
                                break;
                            }
                            Err(e) => {
                                error!("Error reading message from worker {}: {}", worker_id, e);
                                break;
                            }
                        }
                    }

                    // Write to worker
                    Some(msg) = orch_msg_rx.recv() => {
                        debug!("Sending message to worker {}: {:?}", worker_id, msg);
                        if let Err(e) = handler.write_orchestrator_message(&msg).await {
                            error!("Failed to write message to worker {}: {}", worker_id, e);
                            break;
                        }
                    }
                }
            }

            // Cleanup
            if let Err(e) = handler.shutdown().await {
                warn!("Error during connection shutdown for worker {}: {}", worker_id, e);
            }

            debug!("Connection handler terminated for worker {}", worker_id_clone);
        });
    }

    /// Handle a message from a worker
    async fn handle_worker_message(&mut self, event: WorkerMessageEvent) {
        let worker_id = &event.worker_id;

        // Check for disconnection marker
        if matches!(&event.message, WorkerMessage::Ready { timestamp, .. } if *timestamp == 0.0) {
            self.handle_worker_disconnect(worker_id).await;
            return;
        }

        // Update worker's last seen time
        if let Some(worker) = self.registry.get_mut(worker_id) {
            worker.update_last_seen();
        }

        match event.message {
            WorkerMessage::Ready { .. } => {
                self.handle_worker_ready(worker_id).await;
            }
            WorkerMessage::Done {
                issue_id,
                duration_ms,
                ..
            } => {
                self.handle_worker_done(worker_id, &issue_id, duration_ms)
                    .await;
            }
            WorkerMessage::Failed {
                issue_id,
                error,
                duration_ms,
                ..
            } => {
                self.handle_worker_failed(worker_id, &issue_id, &error, duration_ms)
                    .await;
            }
        }
    }

    /// Handle worker ready message
    async fn handle_worker_ready(&mut self, worker_id: &str) {
        info!("Worker {} is ready", worker_id);

        if let Some(worker) = self.registry.get_mut(worker_id) {
            worker.state = WorkerState::Idle;
        }

        // Try to assign a task
        if let Some(task) = self.task_queue.pop() {
            self.assign_task_to_worker(worker_id, task).await;
        } else {
            // No tasks available, send WAIT message
            self.send_wait_to_worker(worker_id).await;
        }
    }

    /// Handle worker done message
    async fn handle_worker_done(&mut self, worker_id: &str, issue_id: &str, duration_ms: u64) {
        info!(
            "Worker {} completed task {} in {}ms",
            worker_id, issue_id, duration_ms
        );

        if let Some(worker) = self.registry.get_mut(worker_id) {
            worker.complete_task();
        }

        // Try to assign the next task
        if let Some(task) = self.task_queue.pop() {
            self.assign_task_to_worker(worker_id, task).await;
        } else {
            // No tasks available, send WAIT message
            self.send_wait_to_worker(worker_id).await;
        }
    }

    /// Handle worker failed message
    async fn handle_worker_failed(
        &mut self,
        worker_id: &str,
        issue_id: &str,
        error: &str,
        duration_ms: u64,
    ) {
        warn!(
            "Worker {} failed task {} after {}ms: {}",
            worker_id, issue_id, duration_ms, error
        );

        if let Some(worker) = self.registry.get_mut(worker_id) {
            worker.fail_task();
        }

        // Try to assign the next task
        if let Some(task) = self.task_queue.pop() {
            self.assign_task_to_worker(worker_id, task).await;
        } else {
            // No tasks available, send WAIT message
            self.send_wait_to_worker(worker_id).await;
        }
    }

    /// Handle worker disconnect
    async fn handle_worker_disconnect(&mut self, worker_id: &str) {
        info!("Worker {} disconnected", worker_id);

        // Mark worker as disconnected and re-queue any assigned task
        if let Some(worker) = self.registry.get_mut(worker_id) {
            if let Some(task_id) = worker.current_task.take() {
                warn!("Re-queuing task {} from disconnected worker", task_id);
                // TODO: Re-queue the task (would need to store task details)
            }
            worker.state = WorkerState::Disconnected;
        }

        // Remove the worker's channel
        self.worker_channels.remove(worker_id);

        // Optionally unregister the worker
        self.registry.unregister(worker_id);
    }

    /// Assign a task to a worker
    async fn assign_task_to_worker(&mut self, worker_id: &str, task: Task) {
        info!(
            "Assigning task {} to worker {} (priority {})",
            task.issue_id, worker_id, task.priority
        );

        // Update worker state
        if let Some(worker) = self.registry.get_mut(worker_id) {
            worker.assign_task(task.issue_id.clone());
        }

        // Send TASK message
        let msg = OrchestratorMessage::task(
            worker_id.to_string(),
            task.issue_id,
            task.priority,
            task.title,
        );

        if let Some(tx) = self.worker_channels.get(worker_id) {
            if let Err(e) = tx.send(msg) {
                error!("Failed to send task to worker {}: {}", worker_id, e);
            }
        }
    }

    /// Send WAIT message to worker
    async fn send_wait_to_worker(&mut self, worker_id: &str) {
        debug!("Sending WAIT to worker {}", worker_id);

        if let Some(worker) = self.registry.get_mut(worker_id) {
            worker.state = WorkerState::Waiting;
        }

        let msg = OrchestratorMessage::wait(
            worker_id.to_string(),
            self.config.default_wait_seconds,
        );

        if let Some(tx) = self.worker_channels.get(worker_id) {
            if let Err(e) = tx.send(msg) {
                error!("Failed to send WAIT to worker {}: {}", worker_id, e);
            }
        }
    }

    /// Gracefully shutdown the orchestrator
    async fn shutdown(&mut self) -> anyhow::Result<()> {
        info!("Starting graceful shutdown");

        // Send shutdown message to all workers
        let worker_ids: Vec<String> = self.worker_channels.keys().cloned().collect();

        for worker_id in &worker_ids {
            info!("Sending shutdown to worker {}", worker_id);
            let msg = OrchestratorMessage::shutdown(
                worker_id.clone(),
                ShutdownReason::UserRequested,
            );

            if let Some(tx) = self.worker_channels.get(worker_id) {
                let _ = tx.send(msg);
            }
        }

        // Wait for workers to shutdown (with timeout)
        let shutdown_timeout = Duration::from_secs(self.config.shutdown_timeout);
        sleep(shutdown_timeout).await;

        // Clean up
        self.worker_channels.clear();

        info!("Orchestrator shutdown complete");
        Ok(())
    }

    /// Get orchestrator statistics
    pub fn stats(&self) -> OrchestratorStats {
        OrchestratorStats {
            total_workers: self.registry.count(),
            idle_workers: self.registry.idle_workers().count(),
            working_workers: self.registry.working_workers().count(),
            queued_tasks: self.task_queue.len(),
            total_completed: self
                .registry
                .all()
                .map(|w| w.tasks_completed)
                .sum(),
            total_failed: self
                .registry
                .all()
                .map(|w| w.tasks_failed)
                .sum(),
        }
    }
}

/// Worker message event (includes worker ID context)
#[derive(Debug)]
struct WorkerMessageEvent {
    worker_id: String,
    message: WorkerMessage,
}

/// Orchestrator statistics
#[derive(Debug, Clone)]
pub struct OrchestratorStats {
    pub total_workers: usize,
    pub idle_workers: usize,
    pub working_workers: usize,
    pub queued_tasks: usize,
    pub total_completed: u64,
    pub total_failed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let config = OrchestratorConfig {
            socket_path: socket_path.to_string_lossy().to_string(),
            max_workers: 5,
            default_wait_seconds: 30,
            worker_idle_timeout: 0,
            shutdown_timeout: 5,
        };

        let orch = Orchestrator::new(config).await.unwrap();
        assert_eq!(orch.socket_path(), socket_path.as_path());
        assert_eq!(orch.registry.count(), 0);
        assert!(orch.task_queue.is_empty());
    }

    #[tokio::test]
    async fn test_add_task() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let config = OrchestratorConfig {
            socket_path: socket_path.to_string_lossy().to_string(),
            ..Default::default()
        };

        let mut orch = Orchestrator::new(config).await.unwrap();
        let task = Task::new("issue-123".to_string(), 1, "Test task".to_string());

        orch.add_task(task);
        assert_eq!(orch.task_queue.len(), 1);
    }

    #[tokio::test]
    async fn test_orchestrator_stats() {
        let temp_dir = TempDir::new().unwrap();
        let socket_path = temp_dir.path().join("test.sock");

        let config = OrchestratorConfig {
            socket_path: socket_path.to_string_lossy().to_string(),
            ..Default::default()
        };

        let orch = Orchestrator::new(config).await.unwrap();
        let stats = orch.stats();

        assert_eq!(stats.total_workers, 0);
        assert_eq!(stats.idle_workers, 0);
        assert_eq!(stats.queued_tasks, 0);
    }
}
