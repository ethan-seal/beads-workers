// Metrics and monitoring module for beads-workers

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Comprehensive metrics for the worker system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMetrics {
    /// Global system metrics
    pub system: SystemMetrics,
    /// Per-worker metrics
    pub workers: HashMap<String, WorkerPerformanceMetrics>,
    /// Task completion history
    pub task_history: Vec<TaskCompletionRecord>,
    /// Error tracking
    pub errors: ErrorMetrics,
    /// Timestamp of last metrics update
    pub last_updated: u64,
}

/// System-level metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// Total tasks completed across all workers
    pub total_tasks_completed: u64,
    /// Total tasks failed across all workers
    pub total_tasks_failed: u64,
    /// Total tasks in progress
    pub total_tasks_in_progress: u64,
    /// Average task duration in milliseconds
    pub avg_task_duration_ms: f64,
    /// Median task duration in milliseconds
    pub median_task_duration_ms: f64,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Overall success rate (0.0 - 1.0)
    pub success_rate: f64,
    /// Tasks per hour
    pub tasks_per_hour: f64,
    /// Peak concurrent tasks
    pub peak_concurrent_tasks: u64,
}

/// Per-worker performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerPerformanceMetrics {
    /// Worker ID
    pub worker_id: String,
    /// Total tasks completed by this worker
    pub tasks_completed: u64,
    /// Total tasks failed by this worker
    pub tasks_failed: u64,
    /// Success rate for this worker (0.0 - 1.0)
    pub success_rate: f64,
    /// Average task duration in milliseconds
    pub avg_duration_ms: f64,
    /// Minimum task duration in milliseconds
    pub min_duration_ms: u64,
    /// Maximum task duration in milliseconds
    pub max_duration_ms: u64,
    /// Total time spent working in milliseconds
    pub total_work_time_ms: u64,
    /// Total idle time in milliseconds
    pub total_idle_time_ms: u64,
    /// Worker efficiency (work time / total time)
    pub efficiency: f64,
    /// Last task timestamp
    pub last_task_timestamp: Option<u64>,
    /// Worker registered at
    pub registered_at: u64,
}

/// Record of a completed or failed task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletionRecord {
    /// Task issue ID
    pub issue_id: String,
    /// Worker that handled the task
    pub worker_id: String,
    /// Whether task succeeded
    pub success: bool,
    /// Task duration in milliseconds
    pub duration_ms: u64,
    /// Completion timestamp
    pub completed_at: u64,
    /// Error message if failed
    pub error: Option<String>,
    /// Task priority
    pub priority: Option<u8>,
}

/// Error tracking metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMetrics {
    /// Total number of errors
    pub total_errors: u64,
    /// Errors by type
    pub errors_by_type: HashMap<String, u64>,
    /// Recent errors (last 100)
    pub recent_errors: Vec<ErrorRecord>,
}

/// Record of an error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    /// Error message
    pub message: String,
    /// Error type/category
    pub error_type: String,
    /// Worker ID if applicable
    pub worker_id: Option<String>,
    /// Task ID if applicable
    pub issue_id: Option<String>,
    /// Timestamp
    pub timestamp: u64,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        SystemMetrics {
            total_tasks_completed: 0,
            total_tasks_failed: 0,
            total_tasks_in_progress: 0,
            avg_task_duration_ms: 0.0,
            median_task_duration_ms: 0.0,
            uptime_seconds: 0,
            success_rate: 0.0,
            tasks_per_hour: 0.0,
            peak_concurrent_tasks: 0,
        }
    }
}

impl Default for ErrorMetrics {
    fn default() -> Self {
        ErrorMetrics {
            total_errors: 0,
            errors_by_type: HashMap::new(),
            recent_errors: Vec::new(),
        }
    }
}

impl WorkerMetrics {
    /// Create new empty metrics
    pub fn new() -> Self {
        WorkerMetrics {
            system: SystemMetrics::default(),
            workers: HashMap::new(),
            task_history: Vec::new(),
            errors: ErrorMetrics::default(),
            last_updated: get_timestamp(),
        }
    }

    /// Register a new worker
    pub fn register_worker(&mut self, worker_id: String) {
        let now = get_timestamp();
        let metrics = WorkerPerformanceMetrics {
            worker_id: worker_id.clone(),
            tasks_completed: 0,
            tasks_failed: 0,
            success_rate: 0.0,
            avg_duration_ms: 0.0,
            min_duration_ms: 0,
            max_duration_ms: 0,
            total_work_time_ms: 0,
            total_idle_time_ms: 0,
            efficiency: 0.0,
            last_task_timestamp: None,
            registered_at: now,
        };
        self.workers.insert(worker_id, metrics);
        self.last_updated = now;
    }

    /// Record a completed task
    pub fn record_task_completion(
        &mut self,
        worker_id: String,
        issue_id: String,
        duration_ms: u64,
        priority: Option<u8>,
    ) {
        let now = get_timestamp();

        // Update worker metrics
        if let Some(worker) = self.workers.get_mut(&worker_id) {
            worker.tasks_completed += 1;
            worker.total_work_time_ms += duration_ms;
            worker.last_task_timestamp = Some(now);

            // Update min/max durations
            if worker.min_duration_ms == 0 || duration_ms < worker.min_duration_ms {
                worker.min_duration_ms = duration_ms;
            }
            if duration_ms > worker.max_duration_ms {
                worker.max_duration_ms = duration_ms;
            }

            // Update average duration
            let total_tasks = worker.tasks_completed + worker.tasks_failed;
            if total_tasks > 0 {
                worker.avg_duration_ms = worker.total_work_time_ms as f64 / total_tasks as f64;
                worker.success_rate =
                    worker.tasks_completed as f64 / total_tasks as f64;
            }

            // Update efficiency
            let total_time = (now - worker.registered_at) * 1000; // Convert to ms
            if total_time > 0 {
                worker.efficiency = worker.total_work_time_ms as f64 / total_time as f64;
            }
        }

        // Update system metrics
        self.system.total_tasks_completed += 1;
        self.recalculate_system_metrics();

        // Add to task history
        let record = TaskCompletionRecord {
            issue_id,
            worker_id,
            success: true,
            duration_ms,
            completed_at: now,
            error: None,
            priority,
        };
        self.task_history.push(record);

        // Keep only last 1000 task records in memory
        if self.task_history.len() > 1000 {
            self.task_history.drain(0..100);
        }

        self.last_updated = now;
    }

    /// Record a failed task
    pub fn record_task_failure(
        &mut self,
        worker_id: String,
        issue_id: String,
        error: String,
        duration_ms: u64,
        priority: Option<u8>,
    ) {
        let now = get_timestamp();

        // Update worker metrics
        if let Some(worker) = self.workers.get_mut(&worker_id) {
            worker.tasks_failed += 1;
            worker.total_work_time_ms += duration_ms;
            worker.last_task_timestamp = Some(now);

            // Update average duration
            let total_tasks = worker.tasks_completed + worker.tasks_failed;
            if total_tasks > 0 {
                worker.avg_duration_ms = worker.total_work_time_ms as f64 / total_tasks as f64;
                worker.success_rate =
                    worker.tasks_completed as f64 / total_tasks as f64;
            }

            // Update efficiency
            let total_time = (now - worker.registered_at) * 1000; // Convert to ms
            if total_time > 0 {
                worker.efficiency = worker.total_work_time_ms as f64 / total_time as f64;
            }
        }

        // Update system metrics
        self.system.total_tasks_failed += 1;
        self.recalculate_system_metrics();

        // Add to task history
        let record = TaskCompletionRecord {
            issue_id: issue_id.clone(),
            worker_id: worker_id.clone(),
            success: false,
            duration_ms,
            completed_at: now,
            error: Some(error.clone()),
            priority,
        };
        self.task_history.push(record);

        // Keep only last 1000 task records
        if self.task_history.len() > 1000 {
            self.task_history.drain(0..100);
        }

        // Record error
        self.record_error(
            error,
            "task_failure".to_string(),
            Some(worker_id),
            Some(issue_id),
        );

        self.last_updated = now;
    }

    /// Record an error
    pub fn record_error(
        &mut self,
        message: String,
        error_type: String,
        worker_id: Option<String>,
        issue_id: Option<String>,
    ) {
        let now = get_timestamp();

        self.errors.total_errors += 1;
        *self.errors.errors_by_type.entry(error_type.clone()).or_insert(0) += 1;

        let error_record = ErrorRecord {
            message,
            error_type,
            worker_id,
            issue_id,
            timestamp: now,
        };

        self.errors.recent_errors.push(error_record);

        // Keep only last 100 errors
        if self.errors.recent_errors.len() > 100 {
            self.errors.recent_errors.drain(0..10);
        }

        self.last_updated = now;
    }

    /// Recalculate system-level metrics
    fn recalculate_system_metrics(&mut self) {
        let total_completed = self.system.total_tasks_completed;
        let total_failed = self.system.total_tasks_failed;
        let total_tasks = total_completed + total_failed;

        // Calculate success rate
        if total_tasks > 0 {
            self.system.success_rate = total_completed as f64 / total_tasks as f64;
        }

        // Calculate average duration from recent task history
        if !self.task_history.is_empty() {
            let sum: u64 = self.task_history.iter().map(|t| t.duration_ms).sum();
            self.system.avg_task_duration_ms = sum as f64 / self.task_history.len() as f64;

            // Calculate median duration
            let mut durations: Vec<u64> = self.task_history.iter().map(|t| t.duration_ms).collect();
            durations.sort_unstable();
            let mid = durations.len() / 2;
            self.system.median_task_duration_ms = if durations.len() % 2 == 0 {
                (durations[mid - 1] + durations[mid]) as f64 / 2.0
            } else {
                durations[mid] as f64
            };
        }

        // Calculate tasks per hour
        if self.system.uptime_seconds > 0 {
            let hours = self.system.uptime_seconds as f64 / 3600.0;
            self.system.tasks_per_hour = total_tasks as f64 / hours;
        }
    }

    /// Update system uptime
    pub fn update_uptime(&mut self, start_time: u64) {
        let now = get_timestamp();
        self.system.uptime_seconds = now - start_time;
        self.recalculate_system_metrics();
        self.last_updated = now;
    }

    /// Update concurrent task count
    pub fn update_concurrent_tasks(&mut self, count: u64) {
        self.system.total_tasks_in_progress = count;
        if count > self.system.peak_concurrent_tasks {
            self.system.peak_concurrent_tasks = count;
        }
        self.last_updated = get_timestamp();
    }

    /// Get metrics for a specific worker
    pub fn get_worker_metrics(&self, worker_id: &str) -> Option<&WorkerPerformanceMetrics> {
        self.workers.get(worker_id)
    }

    /// Get top performing workers by task completion
    pub fn top_workers_by_completion(&self, limit: usize) -> Vec<&WorkerPerformanceMetrics> {
        let mut workers: Vec<&WorkerPerformanceMetrics> = self.workers.values().collect();
        workers.sort_by(|a, b| b.tasks_completed.cmp(&a.tasks_completed));
        workers.into_iter().take(limit).collect()
    }

    /// Get workers with lowest success rate
    pub fn workers_by_lowest_success_rate(&self, limit: usize) -> Vec<&WorkerPerformanceMetrics> {
        let mut workers: Vec<&WorkerPerformanceMetrics> = self.workers.values()
            .filter(|w| w.tasks_completed + w.tasks_failed > 0)
            .collect();
        workers.sort_by(|a, b| a.success_rate.partial_cmp(&b.success_rate).unwrap());
        workers.into_iter().take(limit).collect()
    }

    /// Get recent failed tasks
    pub fn recent_failures(&self, limit: usize) -> Vec<&TaskCompletionRecord> {
        self.task_history
            .iter()
            .rev()
            .filter(|t| !t.success)
            .take(limit)
            .collect()
    }
}

impl Default for WorkerMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current Unix timestamp in seconds
fn get_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Metrics persistence manager
pub struct MetricsStore {
    /// Path to metrics file
    metrics_file: PathBuf,
    /// Path to metrics history directory
    history_dir: PathBuf,
}

impl MetricsStore {
    /// Create a new metrics store
    pub fn new(beads_dir: &Path) -> Self {
        let metrics_file = beads_dir.join("beads-workers-metrics.json");
        let history_dir = beads_dir.join("metrics-history");

        MetricsStore {
            metrics_file,
            history_dir,
        }
    }

    /// Load metrics from disk
    pub fn load(&self) -> Result<WorkerMetrics> {
        if !self.metrics_file.exists() {
            return Ok(WorkerMetrics::new());
        }

        let content = fs::read_to_string(&self.metrics_file)
            .context("Failed to read metrics file")?;

        let metrics: WorkerMetrics = serde_json::from_str(&content)
            .context("Failed to parse metrics JSON")?;

        Ok(metrics)
    }

    /// Save metrics to disk
    pub fn save(&self, metrics: &WorkerMetrics) -> Result<()> {
        let json = serde_json::to_string_pretty(metrics)
            .context("Failed to serialize metrics")?;

        fs::write(&self.metrics_file, json)
            .context("Failed to write metrics file")?;

        Ok(())
    }

    /// Archive current metrics to history
    pub fn archive(&self, metrics: &WorkerMetrics) -> Result<()> {
        // Create history directory if it doesn't exist
        fs::create_dir_all(&self.history_dir)
            .context("Failed to create metrics history directory")?;

        // Generate filename with timestamp
        let timestamp = get_timestamp();
        let filename = format!("metrics-{}.json", timestamp);
        let archive_path = self.history_dir.join(filename);

        let json = serde_json::to_string_pretty(metrics)
            .context("Failed to serialize metrics for archive")?;

        fs::write(&archive_path, json)
            .context("Failed to write metrics archive")?;

        // Clean up old archives (keep last 30 days)
        self.cleanup_old_archives(30)?;

        Ok(())
    }

    /// Clean up archives older than specified days
    fn cleanup_old_archives(&self, keep_days: u64) -> Result<()> {
        if !self.history_dir.exists() {
            return Ok(());
        }

        let now = get_timestamp();
        let cutoff = now - (keep_days * 24 * 60 * 60);

        let entries = fs::read_dir(&self.history_dir)
            .context("Failed to read metrics history directory")?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Extract timestamp from filename
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.starts_with("metrics-") && filename.ends_with(".json") {
                    let timestamp_str = &filename[8..filename.len() - 5];
                    if let Ok(timestamp) = timestamp_str.parse::<u64>() {
                        if timestamp < cutoff {
                            fs::remove_file(&path)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Append a task completion event to the event log
    pub fn log_task_event(&self, beads_dir: &Path, record: &TaskCompletionRecord) -> Result<()> {
        let event_log = beads_dir.join("beads-workers-events.jsonl");

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(event_log)
            .context("Failed to open events log")?;

        let json = serde_json::to_string(record)
            .context("Failed to serialize task event")?;

        writeln!(file, "{}", json)
            .context("Failed to write task event")?;

        Ok(())
    }
}

/// Format metrics for human-readable output
pub fn format_metrics(metrics: &WorkerMetrics, detailed: bool) -> String {
    let mut output = String::new();

    output.push_str("=== Worker System Metrics ===\n\n");

    // System overview
    output.push_str("System Overview:\n");
    output.push_str(&format!(
        "  Uptime: {}h {}m {}s\n",
        metrics.system.uptime_seconds / 3600,
        (metrics.system.uptime_seconds % 3600) / 60,
        metrics.system.uptime_seconds % 60
    ));
    output.push_str(&format!(
        "  Total Tasks: {} completed, {} failed\n",
        metrics.system.total_tasks_completed, metrics.system.total_tasks_failed
    ));
    output.push_str(&format!(
        "  Success Rate: {:.1}%\n",
        metrics.system.success_rate * 100.0
    ));
    output.push_str(&format!(
        "  Tasks in Progress: {}\n",
        metrics.system.total_tasks_in_progress
    ));
    output.push_str(&format!(
        "  Peak Concurrent: {}\n",
        metrics.system.peak_concurrent_tasks
    ));
    output.push_str(&format!(
        "  Tasks per Hour: {:.2}\n",
        metrics.system.tasks_per_hour
    ));
    output.push_str(&format!(
        "  Avg Duration: {:.0}ms\n",
        metrics.system.avg_task_duration_ms
    ));
    output.push_str(&format!(
        "  Median Duration: {:.0}ms\n\n",
        metrics.system.median_task_duration_ms
    ));

    // Worker performance
    output.push_str(&format!("Workers: {}\n", metrics.workers.len()));

    if detailed && !metrics.workers.is_empty() {
        output.push_str("\nTop Workers by Completion:\n");
        for worker in metrics.top_workers_by_completion(5) {
            output.push_str(&format!(
                "  {} - {} tasks ({:.1}% success, {:.0}ms avg, {:.1}% efficiency)\n",
                worker.worker_id,
                worker.tasks_completed,
                worker.success_rate * 100.0,
                worker.avg_duration_ms,
                worker.efficiency * 100.0
            ));
        }
    }

    // Error summary
    output.push_str(&format!("\nErrors: {}\n", metrics.errors.total_errors));

    if detailed && !metrics.errors.errors_by_type.is_empty() {
        output.push_str("  By Type:\n");
        for (error_type, count) in &metrics.errors.errors_by_type {
            output.push_str(&format!("    {}: {}\n", error_type, count));
        }
    }

    if detailed && !metrics.errors.recent_errors.is_empty() {
        output.push_str("\nRecent Errors:\n");
        for error in metrics.errors.recent_errors.iter().rev().take(5) {
            output.push_str(&format!(
                "  [{}s ago] {}: {}\n",
                get_timestamp() - error.timestamp,
                error.error_type,
                error.message
            ));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_metrics() {
        let metrics = WorkerMetrics::new();
        assert_eq!(metrics.system.total_tasks_completed, 0);
        assert_eq!(metrics.workers.len(), 0);
    }

    #[test]
    fn test_register_worker() {
        let mut metrics = WorkerMetrics::new();
        metrics.register_worker("W1".to_string());

        assert_eq!(metrics.workers.len(), 1);
        assert!(metrics.workers.contains_key("W1"));
    }

    #[test]
    fn test_record_task_completion() {
        let mut metrics = WorkerMetrics::new();
        metrics.register_worker("W1".to_string());

        metrics.record_task_completion("W1".to_string(), "task-1".to_string(), 5000, Some(1));

        assert_eq!(metrics.system.total_tasks_completed, 1);
        let worker = metrics.get_worker_metrics("W1").unwrap();
        assert_eq!(worker.tasks_completed, 1);
        assert_eq!(worker.avg_duration_ms, 5000.0);
    }

    #[test]
    fn test_record_task_failure() {
        let mut metrics = WorkerMetrics::new();
        metrics.register_worker("W1".to_string());

        metrics.record_task_failure(
            "W1".to_string(),
            "task-1".to_string(),
            "Test error".to_string(),
            3000,
            Some(1),
        );

        assert_eq!(metrics.system.total_tasks_failed, 1);
        assert_eq!(metrics.errors.total_errors, 1);
        let worker = metrics.get_worker_metrics("W1").unwrap();
        assert_eq!(worker.tasks_failed, 1);
    }

    #[test]
    fn test_success_rate_calculation() {
        let mut metrics = WorkerMetrics::new();
        metrics.register_worker("W1".to_string());

        metrics.record_task_completion("W1".to_string(), "task-1".to_string(), 1000, None);
        metrics.record_task_completion("W1".to_string(), "task-2".to_string(), 1000, None);
        metrics.record_task_failure("W1".to_string(), "task-3".to_string(), "error".to_string(), 1000, None);

        let worker = metrics.get_worker_metrics("W1").unwrap();
        assert!((worker.success_rate - 0.666).abs() < 0.01);
    }
}
