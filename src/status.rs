// Status reporting module for beads-workers

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Status information about the worker system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatus {
    /// Whether the orchestrator is running
    pub orchestrator_running: bool,

    /// Orchestrator process ID (if running)
    pub orchestrator_pid: Option<u32>,

    /// Number of active workers
    pub worker_count: usize,

    /// Worker process IDs
    pub worker_pids: Vec<u32>,

    /// Project statistics
    pub project_stats: ProjectStats,

    /// Recent activity
    pub recent_activity: Vec<ActivityEntry>,

    /// Orchestrator health information
    pub orchestrator_health: OrchestratorHealth,

    /// Timestamp of status check
    pub timestamp: u64,
}

/// Project statistics from the beads database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStats {
    /// Total number of open issues
    pub open_issues: usize,

    /// Number of issues ready to work (no blockers)
    pub ready_issues: usize,

    /// Number of blocked issues
    pub blocked_issues: usize,

    /// Number of issues in progress
    pub in_progress_issues: usize,

    /// Total number of closed issues
    pub closed_issues: usize,
}

/// Recent activity entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    /// Timestamp of the activity
    pub timestamp: u64,

    /// Type of activity (e.g., "task_started", "task_completed", "task_failed")
    pub activity_type: String,

    /// Worker ID involved
    pub worker_id: Option<String>,

    /// Issue ID involved
    pub issue_id: Option<String>,

    /// Additional details
    pub details: Option<String>,
}

/// Orchestrator health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorHealth {
    /// Whether orchestrator is responding
    pub responsive: bool,

    /// Socket path
    pub socket_path: Option<String>,

    /// Socket exists
    pub socket_exists: bool,

    /// Uptime in seconds (if available)
    pub uptime_seconds: Option<u64>,

    /// Memory usage in bytes (if available)
    pub memory_bytes: Option<u64>,

    /// CPU usage percentage (if available)
    pub cpu_percent: Option<f64>,
}

impl Default for ProjectStats {
    fn default() -> Self {
        ProjectStats {
            open_issues: 0,
            ready_issues: 0,
            blocked_issues: 0,
            in_progress_issues: 0,
            closed_issues: 0,
        }
    }
}

impl Default for OrchestratorHealth {
    fn default() -> Self {
        OrchestratorHealth {
            responsive: false,
            socket_path: None,
            socket_exists: false,
            uptime_seconds: None,
            memory_bytes: None,
            cpu_percent: None,
        }
    }
}

impl WorkerStatus {
    /// Create a new empty status
    pub fn new() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        WorkerStatus {
            orchestrator_running: false,
            orchestrator_pid: None,
            worker_count: 0,
            worker_pids: Vec::new(),
            project_stats: ProjectStats::default(),
            recent_activity: Vec::new(),
            orchestrator_health: OrchestratorHealth::default(),
            timestamp: now,
        }
    }
}

/// Check the status of the worker system
pub fn check_status(beads_dir: &Path, config: &crate::config::Config) -> Result<WorkerStatus> {
    let mut status = WorkerStatus::new();

    // Check for PID file to see if orchestrator is running
    let pid_file = beads_dir.join("beads-workers.pid");
    if let Ok(pid_str) = fs::read_to_string(&pid_file) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            // Check if process is actually running
            if is_process_running(pid) {
                status.orchestrator_running = true;
                status.orchestrator_pid = Some(pid);
            }
        }
    }

    // Check socket path
    let socket_path = config.get_socket_path(beads_dir);
    status.orchestrator_health.socket_path = Some(socket_path.to_string_lossy().to_string());
    status.orchestrator_health.socket_exists = socket_path.exists();

    // If orchestrator is running, try to get more health info
    if status.orchestrator_running {
        if let Some(pid) = status.orchestrator_pid {
            status.orchestrator_health.responsive = true;

            // Get uptime (time since process started)
            if let Ok(uptime) = get_process_uptime(pid) {
                status.orchestrator_health.uptime_seconds = Some(uptime);
            }

            // Get memory usage
            if let Ok(memory) = get_process_memory(pid) {
                status.orchestrator_health.memory_bytes = Some(memory);
            }

            // Get CPU usage
            if let Ok(cpu) = get_process_cpu(pid) {
                status.orchestrator_health.cpu_percent = Some(cpu);
            }
        }
    }

    // Check for worker PIDs file
    let workers_file = beads_dir.join("beads-workers-pids.txt");
    if let Ok(content) = fs::read_to_string(&workers_file) {
        for line in content.lines() {
            if let Ok(pid) = line.trim().parse::<u32>() {
                if is_process_running(pid) {
                    status.worker_pids.push(pid);
                }
            }
        }
    }
    status.worker_count = status.worker_pids.len();

    // Get project statistics from beads database
    status.project_stats = get_project_stats(beads_dir)?;

    // Get recent activity
    status.recent_activity = get_recent_activity(beads_dir)?;

    Ok(status)
}

/// Check if a process is running
fn is_process_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        // On non-Unix systems, assume it's running if we have a PID
        // This is a fallback and may not be accurate
        true
    }
}

/// Get process uptime in seconds
fn get_process_uptime(pid: u32) -> Result<u64> {
    #[cfg(target_os = "linux")]
    {
        let stat_path = format!("/proc/{}/stat", pid);
        let content = fs::read_to_string(&stat_path)
            .context("Failed to read process stat")?;

        // Parse the stat file to get start time
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() > 21 {
            let start_time_ticks: u64 = parts[21].parse()
                .context("Failed to parse start time")?;

            // Get system uptime
            let uptime_content = fs::read_to_string("/proc/uptime")
                .context("Failed to read system uptime")?;
            let uptime_secs: f64 = uptime_content
                .split_whitespace()
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            // Calculate process uptime
            let ticks_per_sec = 100; // Usually 100 on Linux
            let process_start_secs = start_time_ticks / ticks_per_sec;
            let process_uptime = uptime_secs as u64 - process_start_secs;

            return Ok(process_uptime);
        }
    }

    // Fallback: return 0 if we can't determine uptime
    Ok(0)
}

/// Get process memory usage in bytes
fn get_process_memory(pid: u32) -> Result<u64> {
    #[cfg(target_os = "linux")]
    {
        let status_path = format!("/proc/{}/status", pid);
        let content = fs::read_to_string(&status_path)
            .context("Failed to read process status")?;

        // Look for VmRSS (Resident Set Size)
        for line in content.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: u64 = parts[1].parse()
                        .context("Failed to parse memory size")?;
                    return Ok(kb * 1024); // Convert KB to bytes
                }
            }
        }
    }

    // Fallback: return 0 if we can't determine memory
    Ok(0)
}

/// Get process CPU usage percentage
fn get_process_cpu(pid: u32) -> Result<f64> {
    #[cfg(target_os = "linux")]
    {
        // This is a simplified CPU calculation
        // For more accurate CPU usage, we'd need to sample over time
        let stat_path = format!("/proc/{}/stat", pid);
        let content = fs::read_to_string(&stat_path)
            .context("Failed to read process stat")?;

        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() > 14 {
            let _utime: u64 = parts[13].parse().unwrap_or(0);
            let _stime: u64 = parts[14].parse().unwrap_or(0);
            // let _total_time = utime + stime;

            // This is a rough estimate - real CPU usage requires sampling
            // For now, just return a placeholder
            return Ok(0.0);
        }
    }

    // Fallback: return 0.0 if we can't determine CPU
    Ok(0.0)
}

/// Get project statistics from beads database
fn get_project_stats(beads_dir: &Path) -> Result<ProjectStats> {
    let issues_file = beads_dir.join("issues.jsonl");

    if !issues_file.exists() {
        return Ok(ProjectStats::default());
    }

    let content = fs::read_to_string(&issues_file)
        .context("Failed to read issues.jsonl")?;

    let mut stats = ProjectStats::default();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(issue) = serde_json::from_str::<serde_json::Value>(line) {
            let status = issue.get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("");

            match status {
                "open" => {
                    stats.open_issues += 1;

                    // Check if it's ready (no blockers)
                    let blocked_by = issue.get("blocked_by")
                        .and_then(|b| b.as_array())
                        .map(|a| a.len())
                        .unwrap_or(0);

                    if blocked_by == 0 {
                        stats.ready_issues += 1;
                    } else {
                        stats.blocked_issues += 1;
                    }
                }
                "in_progress" => {
                    stats.in_progress_issues += 1;
                }
                "closed" => {
                    stats.closed_issues += 1;
                }
                _ => {}
            }
        }
    }

    Ok(stats)
}

/// Get recent activity from activity log
fn get_recent_activity(beads_dir: &Path) -> Result<Vec<ActivityEntry>> {
    let activity_file = beads_dir.join("beads-workers-activity.jsonl");

    if !activity_file.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&activity_file)
        .context("Failed to read activity log")?;

    let mut activities: Vec<ActivityEntry> = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if let Ok(activity) = serde_json::from_str::<ActivityEntry>(line) {
            activities.push(activity);
        }
    }

    // Sort by timestamp descending and take last 10
    activities.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    activities.truncate(10);

    Ok(activities)
}

/// Format status for human-readable output
pub fn format_status(status: &WorkerStatus, detailed: bool) -> String {
    let mut output = String::new();

    // Header
    output.push_str("=== Beads Workers Status ===\n\n");

    // Orchestrator status
    output.push_str("Orchestrator:\n");
    if status.orchestrator_running {
        output.push_str(&format!("  Status: Running (PID: {})\n",
            status.orchestrator_pid.unwrap()));

        if let Some(uptime) = status.orchestrator_health.uptime_seconds {
            let hours = uptime / 3600;
            let minutes = (uptime % 3600) / 60;
            let seconds = uptime % 60;
            output.push_str(&format!("  Uptime: {}h {}m {}s\n", hours, minutes, seconds));
        }

        if let Some(memory) = status.orchestrator_health.memory_bytes {
            let mb = memory as f64 / (1024.0 * 1024.0);
            output.push_str(&format!("  Memory: {:.2} MB\n", mb));
        }
    } else {
        output.push_str("  Status: Not running\n");
    }

    // Socket info
    if let Some(ref socket_path) = status.orchestrator_health.socket_path {
        output.push_str(&format!("  Socket: {} ({})\n",
            socket_path,
            if status.orchestrator_health.socket_exists { "exists" } else { "missing" }
        ));
    }

    output.push_str("\n");

    // Workers
    output.push_str(&format!("Workers: {}\n", status.worker_count));
    if !status.worker_pids.is_empty() && detailed {
        output.push_str("  PIDs: ");
        output.push_str(&status.worker_pids.iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", "));
        output.push_str("\n");
    }
    output.push_str("\n");

    // Project statistics
    output.push_str("Project Statistics:\n");
    output.push_str(&format!("  Open: {} ({} ready, {} blocked)\n",
        status.project_stats.open_issues,
        status.project_stats.ready_issues,
        status.project_stats.blocked_issues
    ));
    output.push_str(&format!("  In Progress: {}\n", status.project_stats.in_progress_issues));
    output.push_str(&format!("  Closed: {}\n", status.project_stats.closed_issues));
    output.push_str("\n");

    // Recent activity
    if !status.recent_activity.is_empty() && detailed {
        output.push_str("Recent Activity:\n");
        for activity in &status.recent_activity {
            let time = format_timestamp(activity.timestamp);
            output.push_str(&format!("  [{}] {}", time, activity.activity_type));

            if let Some(ref issue_id) = activity.issue_id {
                output.push_str(&format!(" - {}", issue_id));
            }

            if let Some(ref worker_id) = activity.worker_id {
                output.push_str(&format!(" (worker: {})", worker_id));
            }

            output.push_str("\n");
        }
    } else if status.recent_activity.is_empty() {
        output.push_str("Recent Activity: None\n");
    }

    output
}

/// Format timestamp as relative time or absolute
fn format_timestamp(timestamp: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let diff = now.saturating_sub(timestamp);

    if diff < 60 {
        format!("{}s ago", diff)
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_status_new() {
        let status = WorkerStatus::new();
        assert!(!status.orchestrator_running);
        assert_eq!(status.worker_count, 0);
        assert!(status.timestamp > 0);
    }

    #[test]
    fn test_project_stats_default() {
        let stats = ProjectStats::default();
        assert_eq!(stats.open_issues, 0);
        assert_eq!(stats.ready_issues, 0);
        assert_eq!(stats.closed_issues, 0);
    }

    #[test]
    fn test_format_timestamp() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        assert_eq!(format_timestamp(now), "0s ago");
        assert_eq!(format_timestamp(now - 30), "30s ago");
        assert_eq!(format_timestamp(now - 120), "2m ago");
        assert_eq!(format_timestamp(now - 7200), "2h ago");
    }

    #[test]
    fn test_format_status() {
        let status = WorkerStatus::new();
        let output = format_status(&status, false);

        assert!(output.contains("Beads Workers Status"));
        assert!(output.contains("Not running"));
        assert!(output.contains("Workers: 0"));
    }
}
