# Live Dashboard (Watch Command)

The beads-workers system includes a real-time live dashboard for monitoring worker status, system statistics, and task queue information.

## Features

The live dashboard provides:

### Real-Time System Overview
- Total number of active workers
- Worker state breakdown (idle, working, waiting, disconnected)
- Tasks completed and failed counters
- Current task queue size

### Per-Worker Status
- Worker ID and current state
- Current task being executed
- Individual task completion and failure counts
- Color-coded status indicators:
  - **Green**: Idle (ready for tasks)
  - **Blue**: Working (executing a task)
  - **Yellow**: Waiting (between tasks)
  - **Magenta**: Shutting down
  - **Red**: Disconnected

### Flicker-Free Rendering
- Uses crossterm for smooth terminal UI
- Alternate screen buffer for clean display
- Efficient screen updates without flicker

## Usage

### Basic Usage

Start the live dashboard with default 1-second refresh interval:

```bash
beads-workers watch
```

### Custom Refresh Interval

Set a custom refresh interval (in seconds):

```bash
# Refresh every 5 seconds
beads-workers watch --interval 5

# Or using short flag
beads-workers watch -i 5
```

### With Custom Beads Directory

```bash
beads-workers --beads-dir /path/to/.beads watch
```

## Keyboard Controls

- **q** or **Q**: Exit dashboard
- **Ctrl+C**: Exit dashboard
- **Esc**: Exit dashboard

## Dashboard Layout

```
═══════════════════════════════════════════════════════════════════════════════
                        BEADS WORKERS - LIVE DASHBOARD
═══════════════════════════════════════════════════════════════════════════════

SYSTEM OVERVIEW
───────────────────────────────────────────────────────────────────────────────
  Total Workers: 5  |  Idle: 2  |  Working: 2  |  Waiting: 1  |  Disconnected: 0
  Tasks Completed: 42  |  Tasks Failed: 3  |  Queue: 7

WORKER STATUS
───────────────────────────────────────────────────────────────────────────────
Worker ID    State           Current Task              Completed  Failed
─────────────────────────────────────────────────────────────────────────────────
W1           Working         beads-workers-abc-1       10         1
W2           Idle            -                         20         2
W3           Working         beads-workers-def-2       30         3
W4           Waiting         -                         40         4
W5           Idle            -                         50         5

───────────────────────────────────────────────────────────────────────────────
Press 'q' or Ctrl+C to quit  |  Refreshing every 1s
```

## Data Sources

The dashboard fetches data from:

1. **System Status**: Reads from the status module which checks:
   - Orchestrator process status (PID file)
   - Worker count and PIDs
   - Socket connection status
   - Project statistics from beads issues.jsonl

2. **Metrics** (future): Will integrate with the metrics system for:
   - Historical task completion rates
   - Performance statistics
   - Error tracking

3. **IPC** (future): Will support real-time queries to the orchestrator for:
   - Live worker registry state
   - Current task assignments
   - Worker heartbeat status

## Implementation Details

### Architecture

The dashboard is implemented in `src/ui.rs` with the following components:

- **Dashboard struct**: Main dashboard controller with event loop
- **DashboardStats struct**: Aggregated system statistics
- **Rendering functions**: Modular UI components for header, stats, workers, footer
- **Input handling**: Non-blocking keyboard event processing

### Technical Features

1. **Alternate Screen Buffer**: Uses crossterm's alternate screen to avoid polluting the terminal history
2. **Raw Mode Terminal**: Enables character-by-character input processing
3. **Non-Blocking Input**: Checks for user input without blocking the refresh loop
4. **Async/Await**: Built on tokio for async operations
5. **Error Recovery**: Gracefully handles missing or unavailable data sources

### Code Example

```rust
use beads_workers::ui::Dashboard;
use beads_workers::config::Config;
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let beads_dir = Path::new(".beads");
    let config = Config::default();

    let mut dashboard = Dashboard::new(
        1,              // 1-second refresh interval
        beads_dir,
        config,
    );

    dashboard.run().await?;
    Ok(())
}
```

## Testing

Run the dashboard demo:

```bash
cargo run --example dashboard_demo
```

Run unit tests:

```bash
cargo test ui::tests
```

## Future Enhancements

Planned improvements for the dashboard:

1. **Real-Time IPC Integration**: Query orchestrator directly for live worker state
2. **Metrics Integration**: Display historical statistics and trends
3. **Interactive Controls**: Pause/resume workers, cancel tasks, etc.
4. **Multiple Views**: Switch between summary, detailed, and per-worker views
5. **Filtering**: Filter workers by state, task type, or performance
6. **Export**: Save dashboard snapshots to file
7. **Alerts**: Highlight workers with issues or degraded performance
8. **Resource Monitoring**: CPU and memory usage per worker

## Troubleshooting

### Dashboard shows no workers

If the dashboard shows 0 workers:
- Check that the orchestrator is running: `beads-workers status`
- Verify the beads directory path is correct
- Check the socket file exists: `ls -la /tmp/beads-workers-*.sock`

### Dashboard exits immediately

If the dashboard exits right after starting:
- Check for terminal compatibility issues
- Ensure your terminal supports ANSI escape codes
- Try running with `TERM=xterm-256color`

### Terminal garbled after exit

If your terminal is corrupted after exiting:
- The dashboard should clean up automatically
- If not, run: `reset` or `stty sane`

## Performance Considerations

- The dashboard uses minimal CPU when idle
- Refresh interval can be increased for lower resource usage
- Network/IPC calls are async and non-blocking
- Terminal rendering is optimized for minimal updates

## Related Commands

- `beads-workers status`: One-time status snapshot (non-live)
- `beads-workers status --json`: JSON format status output
- `beads-workers logs --follow`: Follow worker logs in real-time
- `beads-workers metrics`: View performance metrics
