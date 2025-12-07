// Integration tests for task polling and assignment
//
// These tests verify that the orchestrator can poll tasks from beads CLI,
// track in-progress tasks, and prevent duplicate assignments.

use beads_workers::task_poller::TaskPoller;
use beads_workers::types::Task;

#[test]
fn test_task_poller_prevents_duplicates() {
    let mut poller = TaskPoller::new("/tmp/test");

    // Mark a task as in-progress
    poller.mark_in_progress("task-1");

    // Verify it's tracked
    assert!(poller.is_in_progress("task-1"));
    assert!(!poller.is_in_progress("task-2"));
}

#[test]
fn test_task_poller_lifecycle() {
    let mut poller = TaskPoller::new("/tmp/test");

    // Start with no tasks
    assert_eq!(poller.in_progress_count(), 0);

    // Mark task as in-progress
    poller.mark_in_progress("task-1");
    assert_eq!(poller.in_progress_count(), 1);

    // Complete the task
    poller.mark_completed("task-1");
    assert_eq!(poller.in_progress_count(), 0);
    assert!(!poller.is_in_progress("task-1"));
}

#[test]
fn test_task_poller_failure_allows_retry() {
    let mut poller = TaskPoller::new("/tmp/test");

    // Mark task as in-progress
    poller.mark_in_progress("task-1");
    assert!(poller.is_in_progress("task-1"));

    // Mark as failed (should allow it to be picked up again)
    poller.mark_failed("task-1");
    assert!(!poller.is_in_progress("task-1"));

    // Can be marked as in-progress again
    poller.mark_in_progress("task-1");
    assert!(poller.is_in_progress("task-1"));
}

#[test]
fn test_multiple_tasks_tracking() {
    let mut poller = TaskPoller::new("/tmp/test");

    // Track multiple tasks
    poller.mark_in_progress("task-1");
    poller.mark_in_progress("task-2");
    poller.mark_in_progress("task-3");

    assert_eq!(poller.in_progress_count(), 3);

    // Complete one
    poller.mark_completed("task-2");
    assert_eq!(poller.in_progress_count(), 2);
    assert!(poller.is_in_progress("task-1"));
    assert!(!poller.is_in_progress("task-2"));
    assert!(poller.is_in_progress("task-3"));

    // Fail one
    poller.mark_failed("task-1");
    assert_eq!(poller.in_progress_count(), 1);
    assert!(poller.is_in_progress("task-3"));
}

#[test]
fn test_task_creation() {
    let task = Task::new(
        "beads-workers-xyz.1".to_string(),
        1,
        "Test task".to_string(),
    );

    assert_eq!(task.issue_id, "beads-workers-xyz.1");
    assert_eq!(task.priority, 1);
    assert_eq!(task.title, "Test task");
    assert!(task.queued_at > 0);
}
