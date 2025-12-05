# Beads Workers

Concurrent task processing system for [beads](https://github.com/cablehead/beads) - a Git-backed issue tracker.

## Overview

`beads-workers` is a single command-line tool that spawns concurrent workers to automatically process beads tasks in parallel, achieving 80-90% worker efficiency.

## Features

- 🚀 **Concurrent Processing** - 10 workers by default (configurable)
- 📊 **Live Dashboard** - Flicker-free real-time monitoring
- ⚡ **High Efficiency** - 80-90% of workers actively working
- 🎯 **Smart Task Claiming** - Random selection reduces collisions
- 📝 **Individual Logs** - Separate log file per worker
- 🔄 **Auto-Retry** - Waits 30s when no tasks available
- 🔧 **Auto-Configuration** - Enables `no-daemon` mode for 192x performance boost

## Quick Start

```bash
# Start workers
./beads-workers start

# Watch live dashboard
./beads-workers watch

# Check status
./beads-workers status

# Stop workers
./beads-workers stop
```

## Installation

See [INSTALL.md](INSTALL.md) for detailed installation instructions.

### Quick Install (Global)

```bash
sudo cp beads-workers /usr/local/bin/
```

Now you can run `beads-workers` from any beads project:

```bash
cd ~/my-beads-project
beads-workers start
beads-workers watch
```

## Commands

```bash
beads-workers start           # Start workers
beads-workers stop            # Stop all workers
beads-workers restart         # Restart workers
beads-workers status          # Show snapshot
beads-workers watch           # Live dashboard (Ctrl+C to exit)
beads-workers logs [ID]       # Follow worker logs
beads-workers clean           # Remove log files
beads-workers help            # Show help
beads-workers version         # Show version
```

## Configuration

Use environment variables to customize behavior:

```bash
# Start 5 workers instead of 10
BEADS_WORKERS_COUNT=5 beads-workers start

# Use custom log directory
BEADS_WORKERS_LOG_DIR=~/logs beads-workers start

# Combine both
BEADS_WORKERS_COUNT=20 BEADS_WORKERS_LOG_DIR=~/logs beads-workers start
```

## Performance

With `no-daemon: true` (automatically enabled):
- **Database operations**: ~26ms per command
- **Worker efficiency**: 80-90%
- **Throughput**: ~29 tasks/minute with 10 workers

Without `no-daemon` (legacy):
- **Database operations**: ~5000ms per command
- **Worker efficiency**: 14%
- **Throughput**: ~3 tasks/minute (database bottleneck)

The tool automatically enables `no-daemon: true` in `.beads/config.yaml` for optimal performance.

## How It Works

1. Each worker queries `bd ready` for available tasks
2. Picks a random task to reduce collisions
3. Claims it with `bd update --status=in_progress`
4. Works on the task (simulated 10-30s by default)
5. Closes with `bd close`
6. Random delay 2-5s to desynchronize workers
7. Repeats from step 1

Workers automatically wait 30s and retry when no tasks are available.

## Customization

The default worker behavior simulates work for 10-30 seconds. To customize, edit the `worker_process` function in the `beads-workers` script:

```bash
# Find this section:
work_time=$((10 + RANDOM % 20))
log "Working on $issue_id for ${work_time}s..."
sleep "$work_time"

# Replace with actual work, for example:
log "Processing $issue_id..."
claude code process-issue "$issue_id"
```

## Live Dashboard

The `watch` command provides a real-time dashboard showing:

```
╔════════════════════════════════════════════════════════════════════════════╗
║                    BEADS WORKER LIVE DASHBOARD                             ║
╚════════════════════════════════════════════════════════════════════════════╝

🕐 Last update: 2025-12-04 20:45:30

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 PROJECT STATS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Total: 78   │ Open: 56   │ Closed: 21   │ Ready: 24   │ Blocked: 32

🔥 IN PROGRESS: 9 workers actively working on tasks

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
👷 WORKERS (last 2 lines of output)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
[W1  PID:12345  🔨 WORKING]
  Working on beads-workers-abc for 15s...

[W2  PID:12346  ⏸️  WAITING]
  Waiting 3s before next cycle...
```

Features:
- Updates every 2 seconds
- No screen flicker (buffered output)
- Shows each worker's PID and status
- Real-time task count (not snapshot)
- Status indicators: 🔨 WORKING, ⏸️ WAITING, 🚀 STARTING, ✅ CLOSING, 🎯 CLAIMING

## Requirements

- Bash 4.0+
- A beads project (directory with `.beads/`)
- `bd` command available in PATH

## Logs

Workers log to `/tmp/beads_worker_N.log` by default (where N is 1-10).

```bash
# Follow all logs
beads-workers logs

# Follow specific worker
beads-workers logs 3

# View with tail
tail -f /tmp/beads_worker_1.log

# Clean up
beads-workers clean
```

## Troubleshooting

### "Not in a beads project directory"

Make sure you're in the root of a beads project (directory containing `.beads/`).

### Workers not starting

```bash
# Check logs for errors
beads-workers logs

# Verify bd is available
which bd

# Check beads project
ls -la .beads/
```

### Low efficiency

The tool automatically enables `no-daemon: true` in `.beads/config.yaml`. If efficiency is still low:

```bash
# Verify no-daemon is enabled
grep "no-daemon" .beads/config.yaml
# Should show: no-daemon: true

# Reduce number of workers if database is still bottlenecked
BEADS_WORKERS_COUNT=5 beads-workers start
```

## Contributing

Contributions welcome! This project uses beads for issue tracking:

```bash
# Check for ready work
bd ready

# Create an issue
bd create "Your feature idea" --type feature

# See AGENTS.md for full workflow
```

## License

MIT License - see LICENSE file for details.

## Links

- [beads](https://github.com/cablehead/beads) - Git-backed issue tracker
- [INSTALL.md](INSTALL.md) - Detailed installation guide

## Credits

Created for efficient concurrent processing of beads tasks.
