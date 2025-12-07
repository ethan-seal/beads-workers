// Terminal UI implementation

use crate::types::{WorkerInfo, WorkerState};
use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
use std::time::Duration;

/// Dashboard statistics
#[derive(Debug, Clone, Default)]
pub struct DashboardStats {
    pub total_workers: usize,
    pub idle_workers: usize,
    pub working_workers: usize,
    pub waiting_workers: usize,
    pub disconnected_workers: usize,
    pub total_tasks_completed: u64,
    pub total_tasks_failed: u64,
    pub tasks_in_queue: usize,
}

/// Live dashboard for monitoring worker status in real-time
pub struct Dashboard {
    /// Refresh interval in seconds
    refresh_interval: Duration,
    /// Whether the dashboard is running
    running: bool,
}

impl Dashboard {
    /// Create a new dashboard with the specified refresh interval
    pub fn new(refresh_interval: u64) -> Self {
        Dashboard {
            refresh_interval: Duration::from_secs(refresh_interval),
            running: false,
        }
    }

    /// Start the dashboard and run the main loop
    pub async fn run(&mut self) -> Result<()> {
        self.running = true;

        // Enter alternate screen and enable raw mode
        execute!(io::stdout(), EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;

        // Hide cursor for cleaner UI
        execute!(io::stdout(), cursor::Hide)?;

        let result = self.main_loop().await;

        // Clean up: restore terminal state
        execute!(io::stdout(), cursor::Show)?;
        terminal::disable_raw_mode()?;
        execute!(io::stdout(), LeaveAlternateScreen)?;

        result
    }

    /// Main event loop for the dashboard
    async fn main_loop(&mut self) -> Result<()> {
        while self.running {
            // Clear screen for flicker-free rendering
            self.clear_screen()?;

            // Fetch current stats (in a real implementation, this would query the orchestrator)
            let stats = self.fetch_stats().await?;
            let workers = self.fetch_workers().await?;

            // Render the dashboard
            self.render(&stats, &workers)?;

            // Flush output to ensure everything is displayed
            io::stdout().flush()?;

            // Check for user input (non-blocking)
            if self.check_input()? {
                break;
            }

            // Wait for next refresh or user input
            tokio::time::sleep(self.refresh_interval).await;
        }

        Ok(())
    }

    /// Clear the screen without flicker
    fn clear_screen(&self) -> Result<()> {
        execute!(
            io::stdout(),
            cursor::MoveTo(0, 0),
            Clear(ClearType::All)
        )?;
        Ok(())
    }

    /// Render the dashboard UI
    fn render(&self, stats: &DashboardStats, workers: &[WorkerInfo]) -> Result<()> {
        let mut stdout = io::stdout();

        // Header
        self.render_header(&mut stdout)?;

        // Overall statistics
        self.render_stats(&mut stdout, stats)?;

        // Worker status table
        self.render_workers(&mut stdout, workers)?;

        // Footer with help text
        self.render_footer(&mut stdout)?;

        Ok(())
    }

    /// Render the header section
    fn render_header(&self, stdout: &mut impl Write) -> Result<()> {
        execute!(
            stdout,
            SetForegroundColor(Color::Cyan),
            Print("═══════════════════════════════════════════════════════════════════════════════\n"),
            Print("                        BEADS WORKERS - LIVE DASHBOARD                         \n"),
            Print("═══════════════════════════════════════════════════════════════════════════════\n"),
            ResetColor,
            Print("\n")
        )?;
        Ok(())
    }

    /// Render overall statistics
    fn render_stats(&self, stdout: &mut impl Write, stats: &DashboardStats) -> Result<()> {
        execute!(
            stdout,
            SetForegroundColor(Color::Yellow),
            Print("SYSTEM OVERVIEW\n"),
            ResetColor,
            Print("───────────────────────────────────────────────────────────────────────────────\n")
        )?;

        writeln!(
            stdout,
            "  Total Workers: {}  |  Idle: {}  |  Working: {}  |  Waiting: {}  |  Disconnected: {}",
            stats.total_workers,
            stats.idle_workers,
            stats.working_workers,
            stats.waiting_workers,
            stats.disconnected_workers
        )?;

        writeln!(
            stdout,
            "  Tasks Completed: {}  |  Tasks Failed: {}  |  Queue: {}",
            stats.total_tasks_completed, stats.total_tasks_failed, stats.tasks_in_queue
        )?;

        writeln!(stdout, "\n")?;

        Ok(())
    }

    /// Render worker status table
    fn render_workers(&self, stdout: &mut impl Write, workers: &[WorkerInfo]) -> Result<()> {
        execute!(
            stdout,
            SetForegroundColor(Color::Yellow),
            Print("WORKER STATUS\n"),
            ResetColor,
            Print("───────────────────────────────────────────────────────────────────────────────\n")
        )?;

        // Table header
        writeln!(
            stdout,
            "{:<12} {:<15} {:<25} {:<10} {:<10}",
            "Worker ID", "State", "Current Task", "Completed", "Failed"
        )?;
        writeln!(
            stdout,
            "─────────────────────────────────────────────────────────────────────────────────"
        )?;

        if workers.is_empty() {
            writeln!(stdout, "\n  No workers connected.\n")?;
        } else {
            for worker in workers {
                let state_str = self.format_worker_state(worker.state);
                let task_str = worker
                    .current_task
                    .as_ref()
                    .map(|t| {
                        if t.len() > 22 {
                            format!("{}...", &t[..22])
                        } else {
                            t.clone()
                        }
                    })
                    .unwrap_or_else(|| "-".to_string());

                // Color code by state
                let state_color = match worker.state {
                    WorkerState::Idle => Color::Green,
                    WorkerState::Working => Color::Blue,
                    WorkerState::Waiting => Color::Yellow,
                    WorkerState::ShuttingDown => Color::Magenta,
                    WorkerState::Disconnected => Color::Red,
                };

                execute!(stdout, Print(format!("{:<12} ", worker.worker_id)))?;
                execute!(
                    stdout,
                    SetForegroundColor(state_color),
                    Print(format!("{:<15}", state_str)),
                    ResetColor
                )?;
                execute!(
                    stdout,
                    Print(format!(
                        " {:<25} {:<10} {:<10}\n",
                        task_str, worker.tasks_completed, worker.tasks_failed
                    ))
                )?;
            }
        }

        writeln!(stdout, "\n")?;

        Ok(())
    }

    /// Render footer with help text
    fn render_footer(&self, stdout: &mut impl Write) -> Result<()> {
        execute!(
            stdout,
            Print("───────────────────────────────────────────────────────────────────────────────\n"),
            SetForegroundColor(Color::DarkGrey),
            Print("Press 'q' or Ctrl+C to quit  |  Refreshing every "),
            Print(format!("{}", self.refresh_interval.as_secs())),
            Print("s\n"),
            ResetColor
        )?;
        Ok(())
    }

    /// Format worker state as a string
    fn format_worker_state(&self, state: WorkerState) -> String {
        match state {
            WorkerState::Idle => "Idle".to_string(),
            WorkerState::Working => "Working".to_string(),
            WorkerState::Waiting => "Waiting".to_string(),
            WorkerState::ShuttingDown => "Shutting Down".to_string(),
            WorkerState::Disconnected => "Disconnected".to_string(),
        }
    }

    /// Check for user input (non-blocking)
    fn check_input(&mut self) -> Result<bool> {
        // Check if there's an event available (non-blocking)
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key_event) = event::read()? {
                return Ok(self.handle_key_event(key_event));
            }
        }
        Ok(false)
    }

    /// Handle keyboard events
    fn handle_key_event(&mut self, event: KeyEvent) -> bool {
        match event.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.running = false;
                true
            }
            KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
                true
            }
            KeyCode::Esc => {
                self.running = false;
                true
            }
            _ => false,
        }
    }

    /// Fetch current statistics (mock implementation)
    async fn fetch_stats(&self) -> Result<DashboardStats> {
        // TODO: In a real implementation, this would query the orchestrator via IPC
        // For now, return mock data
        Ok(DashboardStats {
            total_workers: 5,
            idle_workers: 2,
            working_workers: 2,
            waiting_workers: 1,
            disconnected_workers: 0,
            total_tasks_completed: 42,
            total_tasks_failed: 3,
            tasks_in_queue: 7,
        })
    }

    /// Fetch current worker information (mock implementation)
    async fn fetch_workers(&self) -> Result<Vec<WorkerInfo>> {
        // TODO: In a real implementation, this would query the orchestrator via IPC
        // For now, return mock data
        let mut workers = Vec::new();

        for i in 1..=5 {
            let mut worker = WorkerInfo::new(format!("W{}", i));
            worker.state = match i % 4 {
                0 => WorkerState::Idle,
                1 => WorkerState::Working,
                2 => WorkerState::Waiting,
                _ => WorkerState::Idle,
            };

            if worker.state == WorkerState::Working {
                worker.current_task = Some(format!("beads-workers-abc-{}", i));
            }

            worker.tasks_completed = (i as u64) * 10;
            worker.tasks_failed = i as u64;

            workers.push(worker);
        }

        Ok(workers)
    }
}

/// Simple status display (non-live)
pub fn display_status(stats: &DashboardStats, workers: &[WorkerInfo], detailed: bool) -> Result<()> {
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("                         BEADS WORKERS - STATUS                                ");
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!();

    println!("SYSTEM OVERVIEW");
    println!("───────────────────────────────────────────────────────────────────────────────");
    println!(
        "  Total Workers: {}  |  Idle: {}  |  Working: {}  |  Waiting: {}  |  Disconnected: {}",
        stats.total_workers,
        stats.idle_workers,
        stats.working_workers,
        stats.waiting_workers,
        stats.disconnected_workers
    );
    println!(
        "  Tasks Completed: {}  |  Tasks Failed: {}  |  Queue: {}",
        stats.total_tasks_completed, stats.total_tasks_failed, stats.tasks_in_queue
    );
    println!();

    if detailed {
        println!("WORKER STATUS");
        println!("───────────────────────────────────────────────────────────────────────────────");
        println!(
            "{:<12} {:<15} {:<25} {:<10} {:<10}",
            "Worker ID", "State", "Current Task", "Completed", "Failed"
        );
        println!("───────────────────────────────────────────────────────────────────────────────");

        if workers.is_empty() {
            println!("  No workers connected.");
        } else {
            for worker in workers {
                let state_str = format!("{:?}", worker.state);
                let task_str = worker
                    .current_task
                    .as_ref()
                    .map(|t| {
                        if t.len() > 22 {
                            format!("{}...", &t[..22])
                        } else {
                            t.clone()
                        }
                    })
                    .unwrap_or_else(|| "-".to_string());

                println!(
                    "{:<12} {:<15} {:<25} {:<10} {:<10}",
                    worker.worker_id, state_str, task_str, worker.tasks_completed, worker.tasks_failed
                );
            }
        }
        println!();
    }

    println!("───────────────────────────────────────────────────────────────────────────────");

    Ok(())
}

/// Display status in JSON format
pub fn display_status_json(stats: &DashboardStats, workers: &[WorkerInfo]) -> Result<()> {
    use serde_json::json;

    let worker_data: Vec<_> = workers
        .iter()
        .map(|w| {
            json!({
                "worker_id": w.worker_id,
                "state": format!("{:?}", w.state),
                "current_task": w.current_task,
                "tasks_completed": w.tasks_completed,
                "tasks_failed": w.tasks_failed,
                "registered_at": w.registered_at,
                "last_seen": w.last_seen,
            })
        })
        .collect();

    let output = json!({
        "stats": {
            "total_workers": stats.total_workers,
            "idle_workers": stats.idle_workers,
            "working_workers": stats.working_workers,
            "waiting_workers": stats.waiting_workers,
            "disconnected_workers": stats.disconnected_workers,
            "total_tasks_completed": stats.total_tasks_completed,
            "total_tasks_failed": stats.total_tasks_failed,
            "tasks_in_queue": stats.tasks_in_queue,
        },
        "workers": worker_data,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_creation() {
        let dashboard = Dashboard::new(2);
        assert_eq!(dashboard.refresh_interval, Duration::from_secs(2));
        assert!(!dashboard.running);
    }

    #[test]
    fn test_format_worker_state() {
        let dashboard = Dashboard::new(1);
        assert_eq!(dashboard.format_worker_state(WorkerState::Idle), "Idle");
        assert_eq!(dashboard.format_worker_state(WorkerState::Working), "Working");
        assert_eq!(dashboard.format_worker_state(WorkerState::Waiting), "Waiting");
    }

    #[test]
    fn test_dashboard_stats_default() {
        let stats = DashboardStats::default();
        assert_eq!(stats.total_workers, 0);
        assert_eq!(stats.total_tasks_completed, 0);
    }

    #[tokio::test]
    async fn test_fetch_mock_data() {
        let dashboard = Dashboard::new(1);
        let stats = dashboard.fetch_stats().await.unwrap();
        assert!(stats.total_workers > 0);

        let workers = dashboard.fetch_workers().await.unwrap();
        assert!(!workers.is_empty());
    }

    #[test]
    fn test_display_status_empty() {
        let stats = DashboardStats::default();
        let workers = Vec::new();
        let result = display_status(&stats, &workers, true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_display_status_json() {
        let stats = DashboardStats {
            total_workers: 2,
            idle_workers: 1,
            working_workers: 1,
            waiting_workers: 0,
            disconnected_workers: 0,
            total_tasks_completed: 10,
            total_tasks_failed: 1,
            tasks_in_queue: 5,
        };

        let mut worker = WorkerInfo::new("W1".to_string());
        worker.state = WorkerState::Working;
        worker.current_task = Some("test-task".to_string());

        let workers = vec![worker];
        let result = display_status_json(&stats, &workers);
        assert!(result.is_ok());
    }
}
