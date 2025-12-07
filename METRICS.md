# Worker Metrics System

## Overview

The metrics system provides comprehensive monitoring and performance tracking for the beads-workers orchestrator and worker pool.

## Features

### System-Level Metrics
- Total tasks completed/failed
- Average and median task duration
- System uptime
- Overall success rate
- Tasks per hour
- Peak concurrent task count

### Per-Worker Metrics
- Tasks completed/failed per worker
- Worker success rate
- Average/min/max task duration per worker
- Total work time and idle time
- Worker efficiency (work time / total time)
- Last task timestamp

### Task History
- Completion records (last 1000 in memory)
- Task duration tracking
- Success/failure status
- Error messages for failed tasks
- Priority level tracking

### Error Tracking
- Total error count
- Errors categorized by type
- Recent error log (last 100 errors)
- Error timestamp and context

## Usage

### CLI Commands

```bash
# View current metrics
beads-workers metrics

# View detailed metrics
beads-workers metrics --detailed

# View metrics as JSON
beads-workers metrics --json

# Reset metrics
beads-workers metrics --reset

# Archive current metrics to history
beads-workers metrics --archive
```

### Data Persistence

Metrics are automatically persisted to:
- `.beads/beads-workers-metrics.json` - Current metrics
- `.beads/metrics-history/metrics-{timestamp}.json` - Archived metrics
- `.beads/beads-workers-events.jsonl` - Task event log

Old archives are automatically cleaned up after 30 days.

## API

### WorkerMetrics

Main metrics container with methods for:
- `register_worker(worker_id)` - Register a new worker
- `record_task_completion(worker_id, issue_id, duration_ms, priority)` - Record successful task
- `record_task_failure(worker_id, issue_id, error, duration_ms, priority)` - Record failed task
- `record_error(message, error_type, worker_id, issue_id)` - Record an error
- `update_uptime(start_time)` - Update system uptime
- `update_concurrent_tasks(count)` - Update concurrent task count

### MetricsStore

Handles persistence:
- `load()` - Load metrics from disk
- `save(metrics)` - Save metrics to disk
- `archive(metrics)` - Archive metrics to history
- `log_task_event(beads_dir, record)` - Log task event to JSONL

## Implementation Details

The metrics system:
1. Tracks all task completions and failures with timestamps
2. Calculates real-time statistics (averages, medians, rates)
3. Maintains per-worker performance metrics
4. Automatically manages memory (keeps last 1000 tasks, 100 errors)
5. Persists data to JSON for analysis and reporting
6. Archives historical metrics for long-term trend analysis

## Integration

The metrics system integrates with the orchestrator to automatically track:
- Worker registration
- Task assignments
- Task completions
- Task failures
- Worker disconnections
- System events

Metrics are updated in real-time as events occur in the worker pool.
