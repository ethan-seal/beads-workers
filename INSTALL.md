# Beads Workers Installation Guide

## What is Beads Workers?

A single command-line tool for running concurrent workers that automatically process beads tasks with 80-90% efficiency.

## Quick Start (No Installation)

```bash
# Just run it from any beads project
./beads-workers start
./beads-workers watch
```

## Global Installation

### Option 1: Install to /usr/local/bin (Recommended)

```bash
sudo cp beads-workers /usr/local/bin/
sudo chmod +x /usr/local/bin/beads-workers
```

Now you can run `beads-workers` from anywhere:

```bash
cd ~/my-beads-project
beads-workers start
beads-workers watch
```

### Option 2: Install to ~/bin (User-only)

```bash
mkdir -p ~/bin
cp beads-workers ~/bin/
chmod +x ~/bin/beads-workers

# Add to PATH if not already (add to ~/.bashrc or ~/.zshrc)
export PATH="$HOME/bin:$PATH"
```

### Option 3: Create Alias

```bash
# Add to ~/.bashrc or ~/.zshrc
alias beads-workers='/path/to/beads-workers'
```

## Usage

### Basic Commands

```bash
# Start workers
beads-workers start

# Watch live dashboard
beads-workers watch

# Check status
beads-workers status

# View logs
beads-workers logs      # All workers
beads-workers logs 3    # Worker 3 only

# Stop workers
beads-workers stop

# Restart workers
beads-workers restart

# Clean up old logs
beads-workers clean
```

### Advanced Usage

```bash
# Start 5 workers instead of 10
BEADS_WORKERS_COUNT=5 beads-workers start

# Use custom log directory
BEADS_WORKERS_LOG_DIR=~/logs beads-workers start

# Combine both
BEADS_WORKERS_COUNT=20 BEADS_WORKERS_LOG_DIR=~/logs beads-workers start
```

## Features

✅ **Single command** - All functionality in one executable
✅ **Auto-detects** beads project directory
✅ **No dependencies** - Pure bash, works everywhere
✅ **Performance optimized** - Automatically enables `no-daemon` mode
✅ **Live dashboard** - Flicker-free real-time updates
✅ **Concurrent processing** - 10 workers by default (configurable)
✅ **Smart task claiming** - Random selection reduces collisions
✅ **80-90% efficiency** - Workers actively working most of the time

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `BEADS_WORKERS_COUNT` | 10 | Number of concurrent workers |
| `BEADS_WORKERS_LOG_DIR` | /tmp | Directory for worker logs |

## Workflow Examples

### Daily Development

```bash
cd ~/projects/my-app

# Start workers in background
beads-workers start

# Monitor progress
beads-workers watch

# When done
beads-workers stop
```

### High-Throughput Processing

```bash
# Crank up to 20 workers for big projects
BEADS_WORKERS_COUNT=20 beads-workers start

# Watch the work fly by
beads-workers watch
```

### Debugging Issues

```bash
# Start workers
beads-workers start

# Follow logs for worker 5
beads-workers logs 5

# Or follow all workers
beads-workers logs
```

## Uninstallation

```bash
# If installed to /usr/local/bin
sudo rm /usr/local/bin/beads-workers

# If installed to ~/bin
rm ~/bin/beads-workers

# Clean up logs
beads-workers clean  # or manually: rm /tmp/beads_worker_*.log
```

## Requirements

- Bash 4.0+
- A beads project (directory with `.beads/`)
- `bd` command available in PATH

## Performance

With `no-daemon: true` (automatically enabled):
- Database operations: ~26ms
- Worker efficiency: 80-90%
- Throughput: ~29 tasks/minute with 10 workers

## Troubleshooting

### "Not in a beads project directory"

Make sure you're in the root of a beads project (directory containing `.beads/`).

### Workers not starting

```bash
# Check for errors
beads-workers logs

# Make sure bd is available
which bd

# Verify beads project
ls -la .beads/
```

### Low efficiency

The tool automatically enables `no-daemon: true` in `.beads/config.yaml` for optimal performance.

## Distribution

To share with others, just copy the single `beads-workers` file:

```bash
# Copy to another machine
scp beads-workers user@remote:/usr/local/bin/

# Or commit to your project
git add beads-workers
git commit -m "Add beads-workers tool"
```

## License

Free to use, modify, and distribute.
