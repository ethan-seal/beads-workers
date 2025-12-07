// Task polling from beads CLI
//
// This module handles polling `bd ready` to fetch available tasks and
// converting them into Task objects for the orchestrator's queue.

use crate::types::Task;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use tracing::{debug, error, info, warn};

/// Beads issue representation from `bd ready --json`
#[derive(Debug, Clone, Deserialize)]
pub struct BeadsIssue {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: u8,
    #[serde(rename = "issue_type")]
    pub issue_type: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Task poller that fetches available tasks from beads CLI
pub struct TaskPoller {
    /// Directory containing .beads
    beads_dir: String,
    /// Set of task IDs currently in-progress (to prevent duplicates)
    in_progress: HashSet<String>,
}

impl TaskPoller {
    /// Create a new task poller
    pub fn new<P: AsRef<Path>>(beads_dir: P) -> Self {
        TaskPoller {
            beads_dir: beads_dir.as_ref().to_string_lossy().to_string(),
            in_progress: HashSet::new(),
        }
    }

    /// Poll `bd ready` and return list of available tasks
    ///
    /// This will filter out tasks that are already in-progress to prevent
    /// duplicate assignments.
    pub fn poll(&mut self) -> Result<Vec<Task>> {
        debug!("Polling bd ready for available tasks");

        let output = Command::new("bd")
            .arg("ready")
            .arg("--json")
            .current_dir(&self.beads_dir)
            .output()
            .context("Failed to execute 'bd ready'")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("bd ready command failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let issues: Vec<BeadsIssue> = serde_json::from_str(&stdout)
            .context("Failed to parse JSON output from 'bd ready'")?;

        info!("Polled {} available tasks from bd ready", issues.len());

        // Convert to Task objects, filtering out in-progress tasks
        let tasks: Vec<Task> = issues
            .into_iter()
            .filter(|issue| {
                if self.in_progress.contains(&issue.id) {
                    debug!("Filtering out in-progress task: {}", issue.id);
                    false
                } else {
                    true
                }
            })
            .map(|issue| {
                debug!(
                    "Converting issue to task: {} (priority {})",
                    issue.id, issue.priority
                );
                Task::new(issue.id, issue.priority, issue.title)
            })
            .collect();

        debug!("Returning {} new tasks", tasks.len());
        Ok(tasks)
    }

    /// Mark a task as in-progress
    pub fn mark_in_progress(&mut self, task_id: &str) {
        debug!("Marking task as in-progress: {}", task_id);
        self.in_progress.insert(task_id.to_string());
    }

    /// Mark a task as completed (remove from in-progress)
    pub fn mark_completed(&mut self, task_id: &str) {
        debug!("Marking task as completed: {}", task_id);
        self.in_progress.remove(task_id);
    }

    /// Mark a task as failed (remove from in-progress so it can be retried)
    pub fn mark_failed(&mut self, task_id: &str) {
        warn!("Marking task as failed: {}", task_id);
        self.in_progress.remove(task_id);
    }

    /// Get the number of tasks currently in-progress
    pub fn in_progress_count(&self) -> usize {
        self.in_progress.len()
    }

    /// Check if a task is currently in-progress
    pub fn is_in_progress(&self, task_id: &str) -> bool {
        self.in_progress.contains(task_id)
    }

    /// Clear all in-progress tasks (for cleanup/reset)
    pub fn clear_in_progress(&mut self) {
        info!("Clearing all in-progress tasks");
        self.in_progress.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_poller_creation() {
        let poller = TaskPoller::new("/tmp/test");
        assert_eq!(poller.in_progress_count(), 0);
    }

    #[test]
    fn test_task_poller_mark_in_progress() {
        let mut poller = TaskPoller::new("/tmp/test");

        poller.mark_in_progress("task-1");
        assert_eq!(poller.in_progress_count(), 1);
        assert!(poller.is_in_progress("task-1"));
        assert!(!poller.is_in_progress("task-2"));
    }

    #[test]
    fn test_task_poller_mark_completed() {
        let mut poller = TaskPoller::new("/tmp/test");

        poller.mark_in_progress("task-1");
        poller.mark_in_progress("task-2");
        assert_eq!(poller.in_progress_count(), 2);

        poller.mark_completed("task-1");
        assert_eq!(poller.in_progress_count(), 1);
        assert!(!poller.is_in_progress("task-1"));
        assert!(poller.is_in_progress("task-2"));
    }

    #[test]
    fn test_task_poller_mark_failed() {
        let mut poller = TaskPoller::new("/tmp/test");

        poller.mark_in_progress("task-1");
        assert!(poller.is_in_progress("task-1"));

        poller.mark_failed("task-1");
        assert!(!poller.is_in_progress("task-1"));
    }

    #[test]
    fn test_task_poller_clear() {
        let mut poller = TaskPoller::new("/tmp/test");

        poller.mark_in_progress("task-1");
        poller.mark_in_progress("task-2");
        poller.mark_in_progress("task-3");
        assert_eq!(poller.in_progress_count(), 3);

        poller.clear_in_progress();
        assert_eq!(poller.in_progress_count(), 0);
    }

    #[test]
    fn test_beads_issue_deserialization() {
        let json = r#"{
            "id": "beads-workers-xyz.1",
            "title": "Test task",
            "description": "A test task",
            "status": "open",
            "priority": 1,
            "issue_type": "task",
            "created_at": "2025-12-06T12:00:00Z",
            "updated_at": "2025-12-06T12:00:00Z"
        }"#;

        let issue: BeadsIssue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.id, "beads-workers-xyz.1");
        assert_eq!(issue.title, "Test task");
        assert_eq!(issue.priority, 1);
    }
}
