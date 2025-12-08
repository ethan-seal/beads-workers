// Dynamic worker scaling logic

use crate::types::{WorkerRegistry, WorkerScalingConfig};
use std::collections::HashMap;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Worker scaler that manages dynamic scaling of worker processes
pub struct WorkerScaler {
    /// Scaling configuration
    config: WorkerScalingConfig,
    /// Tracked spawned worker processes
    spawned_workers: HashMap<String, SpawnedWorker>,
    /// Last time we attempted to spawn a worker
    last_spawn_attempt: Option<Instant>,
    /// Current spawn backoff duration
    current_spawn_backoff: Duration,
    /// Socket path for workers to connect to
    socket_path: String,
}

/// Information about a spawned worker process
struct SpawnedWorker {
    /// Process handle
    process: Child,
    /// When the worker was spawned
    _spawned_at: Instant,
    /// Last time we checked if it was idle
    _last_idle_check: Instant,
}

impl WorkerScaler {
    /// Create a new worker scaler
    pub fn new(config: WorkerScalingConfig, socket_path: String) -> Self {
        let initial_backoff = Duration::from_secs(config.spawn_backoff_initial);

        WorkerScaler {
            config,
            spawned_workers: HashMap::new(),
            last_spawn_attempt: None,
            current_spawn_backoff: initial_backoff,
            socket_path,
        }
    }

    /// Check if scaling is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Evaluate current state and determine if scaling action is needed
    pub fn should_scale(
        &self,
        queue_size: usize,
        registry: &WorkerRegistry,
        max_workers: usize,
    ) -> ScalingAction {
        if !self.config.enabled {
            return ScalingAction::None;
        }

        let current_workers = registry.count();
        let idle_workers = registry.idle_workers().count();
        let working_workers = registry.working_workers().count();

        // Respect the absolute maximum from orchestrator config
        let effective_max = std::cmp::min(self.config.max_workers, max_workers);

        // Scale up if queue is large relative to workers
        if queue_size > 0 && working_workers < effective_max {
            let target_workers = self.calculate_target_workers(queue_size, current_workers);

            if target_workers > current_workers && target_workers <= effective_max {
                // Check if we're in backoff period
                if let Some(last_attempt) = self.last_spawn_attempt {
                    if last_attempt.elapsed() < self.current_spawn_backoff {
                        debug!(
                            "In spawn backoff period, waiting {}s more",
                            (self.current_spawn_backoff - last_attempt.elapsed()).as_secs()
                        );
                        return ScalingAction::None;
                    }
                }

                let workers_to_spawn = target_workers - current_workers;
                return ScalingAction::ScaleUp { count: workers_to_spawn };
            }
        }

        // Scale down if we have idle workers above minimum
        if idle_workers > 0 && current_workers > self.config.min_workers {
            // Check which workers have been idle for too long
            let idle_timeout = Duration::from_secs(self.config.idle_shutdown_timeout);
            let excess_idle_workers = self.find_excess_idle_workers(registry, idle_timeout);

            if !excess_idle_workers.is_empty() {
                return ScalingAction::ScaleDown {
                    worker_ids: excess_idle_workers,
                };
            }
        }

        ScalingAction::None
    }

    /// Calculate target number of workers based on queue size
    fn calculate_target_workers(&self, queue_size: usize, current_workers: usize) -> usize {
        // Target: enough workers to handle queue with target_queue_size_per_worker each
        let target_from_queue = (queue_size + self.config.target_queue_size_per_worker - 1)
            / self.config.target_queue_size_per_worker;

        // At least min_workers, but grow gradually from current
        let target = std::cmp::max(
            self.config.min_workers,
            std::cmp::max(current_workers + 1, target_from_queue),
        );

        // Cap at max_workers
        std::cmp::min(target, self.config.max_workers)
    }

    /// Find workers that have been idle for longer than the timeout
    fn find_excess_idle_workers(
        &self,
        registry: &WorkerRegistry,
        idle_timeout: Duration,
    ) -> Vec<String> {
        let mut excess_idle = Vec::new();
        let min_to_keep = self.config.min_workers;
        let current_count = registry.count();

        // Never go below min_workers
        if current_count <= min_to_keep {
            return excess_idle;
        }

        let max_to_remove = current_count - min_to_keep;

        for worker in registry.idle_workers() {
            if excess_idle.len() >= max_to_remove {
                break;
            }

            // Check if worker has been idle long enough
            if worker.last_activity_elapsed() >= idle_timeout {
                excess_idle.push(worker.worker_id.clone());
            }
        }

        excess_idle
    }

    /// Spawn new worker processes
    pub fn spawn_workers(&mut self, count: usize) -> Result<usize, ScalingError> {
        let mut spawned = 0;

        for _ in 0..count {
            match self.spawn_worker() {
                Ok(worker_id) => {
                    info!("Spawned worker process: {}", worker_id);
                    spawned += 1;
                }
                Err(e) => {
                    error!("Failed to spawn worker: {}", e);
                    self.increase_backoff();
                    break;
                }
            }
        }

        // Update spawn attempt timestamp
        self.last_spawn_attempt = Some(Instant::now());

        // Reset backoff on successful spawn
        if spawned > 0 {
            self.reset_backoff();
        }

        Ok(spawned)
    }

    /// Spawn a single worker process
    fn spawn_worker(&mut self) -> Result<String, ScalingError> {
        // TODO: Get actual worker binary path from config or environment
        let worker_binary = std::env::current_exe()
            .map_err(|e| ScalingError::SpawnFailed(format!("Failed to get binary path: {}", e)))?;

        debug!("Spawning worker with socket: {}", self.socket_path);

        // Spawn worker process with socket path
        let child = Command::new(worker_binary)
            .arg("worker")
            .arg("--socket")
            .arg(&self.socket_path)
            .spawn()
            .map_err(|e| ScalingError::SpawnFailed(format!("Failed to spawn process: {}", e)))?;

        let worker_id = format!("spawned-{}", child.id());
        let spawned_worker = SpawnedWorker {
            process: child,
            _spawned_at: Instant::now(),
            _last_idle_check: Instant::now(),
        };

        self.spawned_workers.insert(worker_id.clone(), spawned_worker);

        Ok(worker_id)
    }

    /// Gracefully shutdown specified workers
    pub fn shutdown_workers(&mut self, worker_ids: &[String]) -> usize {
        let mut shutdown_count = 0;

        for worker_id in worker_ids {
            // Check if we spawned this worker
            if let Some(mut spawned_worker) = self.spawned_workers.remove(worker_id) {
                info!("Shutting down spawned worker: {}", worker_id);

                // Try graceful shutdown first
                if let Err(e) = spawned_worker.process.kill() {
                    warn!("Failed to kill worker process {}: {}", worker_id, e);
                } else {
                    // Wait for process to exit
                    match spawned_worker.process.wait() {
                        Ok(status) => {
                            debug!("Worker {} exited with status: {:?}", worker_id, status);
                            shutdown_count += 1;
                        }
                        Err(e) => {
                            warn!("Failed to wait for worker {} exit: {}", worker_id, e);
                        }
                    }
                }
            } else {
                debug!(
                    "Worker {} not spawned by scaler, will be shut down by orchestrator",
                    worker_id
                );
                // The orchestrator will send SHUTDOWN message to these workers
                shutdown_count += 1;
            }
        }

        shutdown_count
    }

    /// Clean up any terminated worker processes
    pub fn cleanup_terminated(&mut self) {
        let mut terminated = Vec::new();

        for (worker_id, spawned_worker) in &mut self.spawned_workers {
            match spawned_worker.process.try_wait() {
                Ok(Some(status)) => {
                    debug!("Worker {} terminated with status: {:?}", worker_id, status);
                    terminated.push(worker_id.clone());
                }
                Ok(None) => {
                    // Still running
                }
                Err(e) => {
                    warn!("Error checking worker {} status: {}", worker_id, e);
                    terminated.push(worker_id.clone());
                }
            }
        }

        for worker_id in terminated {
            self.spawned_workers.remove(&worker_id);
        }
    }

    /// Increase spawn backoff duration (exponential backoff)
    fn increase_backoff(&mut self) {
        let new_backoff_secs = (self.current_spawn_backoff.as_secs() as f64
            * self.config.spawn_backoff_multiplier) as u64;

        let capped_backoff = std::cmp::min(new_backoff_secs, self.config.spawn_backoff_max);
        self.current_spawn_backoff = Duration::from_secs(capped_backoff);

        debug!("Increased spawn backoff to {}s", capped_backoff);
    }

    /// Reset spawn backoff to initial value
    fn reset_backoff(&mut self) {
        self.current_spawn_backoff = Duration::from_secs(self.config.spawn_backoff_initial);
        debug!("Reset spawn backoff to {}s", self.config.spawn_backoff_initial);
    }

    /// Get the number of spawned workers being tracked
    pub fn spawned_count(&self) -> usize {
        self.spawned_workers.len()
    }
}

/// Scaling action to take
#[derive(Debug, Clone, PartialEq)]
pub enum ScalingAction {
    /// No scaling action needed
    None,
    /// Scale up by spawning workers
    ScaleUp { count: usize },
    /// Scale down by shutting down workers
    ScaleDown { worker_ids: Vec<String> },
}

/// Errors that can occur during scaling operations
#[derive(Debug, thiserror::Error)]
pub enum ScalingError {
    #[error("Failed to spawn worker: {0}")]
    SpawnFailed(String),
    #[error("Failed to shutdown worker: {0}")]
    ShutdownFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> WorkerScalingConfig {
        WorkerScalingConfig {
            enabled: true,
            min_workers: 2,
            max_workers: 10,
            target_queue_size_per_worker: 3,
            idle_shutdown_timeout: 60,
            scale_check_interval: 10,
            spawn_backoff_initial: 1,
            spawn_backoff_max: 60,
            spawn_backoff_multiplier: 2.0,
        }
    }

    #[test]
    fn test_scaler_creation() {
        let config = test_config();
        let scaler = WorkerScaler::new(config.clone(), "/tmp/test.sock".to_string());

        assert!(scaler.is_enabled());
        assert_eq!(scaler.spawned_count(), 0);
    }

    #[test]
    fn test_calculate_target_workers() {
        let config = test_config();
        let scaler = WorkerScaler::new(config, "/tmp/test.sock".to_string());

        // Queue of 10, target 3 per worker = need 4 workers
        assert_eq!(scaler.calculate_target_workers(10, 2), 4);

        // Queue of 30, target 3 per worker = need 10 workers (capped at max)
        assert_eq!(scaler.calculate_target_workers(30, 2), 10);

        // Small queue should still maintain minimum
        assert_eq!(scaler.calculate_target_workers(1, 1), 2);
    }

    #[test]
    fn test_should_scale_up() {
        let config = test_config();
        let scaler = WorkerScaler::new(config, "/tmp/test.sock".to_string());
        let mut registry = WorkerRegistry::new();

        // Register 2 workers
        registry.register();
        registry.register();

        // Large queue should trigger scale up
        let action = scaler.should_scale(15, &registry, 10);
        match action {
            ScalingAction::ScaleUp { count } => {
                assert!(count > 0);
            }
            _ => panic!("Expected ScaleUp action"),
        }
    }

    #[test]
    fn test_should_not_scale_above_max() {
        let config = test_config();
        let scaler = WorkerScaler::new(config, "/tmp/test.sock".to_string());
        let mut registry = WorkerRegistry::new();

        // Register max workers
        for _ in 0..10 {
            registry.register();
        }

        // Should not scale up even with large queue
        let action = scaler.should_scale(100, &registry, 10);
        assert_eq!(action, ScalingAction::None);
    }

    #[test]
    fn test_backoff_mechanism() {
        let config = test_config();
        let mut scaler = WorkerScaler::new(config, "/tmp/test.sock".to_string());

        // Initial backoff
        assert_eq!(scaler.current_spawn_backoff.as_secs(), 1);

        // Increase backoff
        scaler.increase_backoff();
        assert_eq!(scaler.current_spawn_backoff.as_secs(), 2);

        scaler.increase_backoff();
        assert_eq!(scaler.current_spawn_backoff.as_secs(), 4);

        // Reset backoff
        scaler.reset_backoff();
        assert_eq!(scaler.current_spawn_backoff.as_secs(), 1);
    }

    #[test]
    fn test_backoff_caps_at_max() {
        let config = test_config();
        let mut scaler = WorkerScaler::new(config, "/tmp/test.sock".to_string());

        // Keep increasing until we hit the cap
        for _ in 0..10 {
            scaler.increase_backoff();
        }

        assert_eq!(scaler.current_spawn_backoff.as_secs(), 60);
    }

    #[test]
    fn test_disabled_scaling() {
        let mut config = test_config();
        config.enabled = false;

        let scaler = WorkerScaler::new(config, "/tmp/test.sock".to_string());
        let registry = WorkerRegistry::new();

        assert!(!scaler.is_enabled());
        assert_eq!(scaler.should_scale(100, &registry, 10), ScalingAction::None);
    }
}
