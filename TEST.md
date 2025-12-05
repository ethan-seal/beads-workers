# Testing Guide for beads-workers

## Quick Start

```bash
# Run a quick test with mock agents (no real Claude Code)
./test-runner.sh full

# This will:
# 1. Create a test environment in /tmp/beads-workers-test-<pid>
# 2. Initialize a beads project
# 3. Create 10 test issues
# 4. Start 3 workers with mock agents
# 5. Show you how to monitor progress
```

## Mock Agent Modes

The `test-agent-mock` script simulates Claude Code agents without actually running them:

- **fast** - Quick simulation (1-2s per task) - best for testing dashboard
- **realistic** - Realistic timing (10-15s per task) - tests actual workflow
- **explore** - Heavy exploration pattern - tests exploration-heavy work
- **error** - Simulates agent failures - tests error handling
- **timeout** - Long-running agents (60s+) - tests timeout handling
- **mixed** - Random behavior - tests variety

## Usage Examples

### Quick Dashboard Test
```bash
# Fast mode with few workers to see dashboard updates quickly
./test-runner.sh full 3 5 fast

# In another terminal:
cd /tmp/beads-workers-test-<pid>
./beads-workers-test watch
```

### Realistic Workflow Test
```bash
# More workers, more issues, realistic timing
./test-runner.sh full 5 20 realistic
```

### Error Handling Test
```bash
# Test how workers handle agent failures
./test-runner.sh full 2 5 error
```

### Manual Control
```bash
# Setup test environment
./test-runner.sh setup

# Start workers in background
cd /tmp/beads-workers-test-<pid>
./beads-workers-test start

# Watch dashboard in one terminal
./beads-workers-test watch

# Tail logs in another terminal
tail -f logs/beads_worker_*.log

# Stop when done
./beads-workers-test stop

# Cleanup
cd -
./test-runner.sh cleanup --remove
```

## What Gets Tested

### Mock Agent Behavior
The mock agent simulates:
- Reading files (Grep, Glob, Read tools)
- Editing files (Edit, Write tools)
- Running commands (Bash tool)
- Spawning sub-agents (Task tool)
- Realistic thinking/processing time
- Success and failure scenarios

### Worker Behavior
Tests the complete worker lifecycle:
1. Startup with staggered delays
2. Querying for ready tasks
3. Random task selection (collision avoidance)
4. Task claiming
5. Agent execution
6. Task completion
7. Cycle delays
8. Retry logic when no tasks available

## Monitoring

### Live Dashboard
```bash
cd /tmp/beads-workers-test-<pid>
./beads-workers-test watch
```

Shows:
- Real-time worker count
- Active tasks being worked on
- Worker status (WORKING, WAITING, etc.)
- Last 2 lines of each worker's output
- Project statistics

### Individual Logs
```bash
# Follow specific worker
tail -f /tmp/beads-workers-test-<pid>/logs/beads_worker_1.log

# Follow all workers
tail -f /tmp/beads-workers-test-<pid>/logs/beads_worker_*.log
```

### Status Snapshot
```bash
cd /tmp/beads-workers-test-<pid>
./beads-workers-test status
```

## Customization

### Environment Variables
```bash
# Change mock mode
BEADS_MOCK_MODE=realistic ./test-runner.sh full

# More workers
BEADS_WORKERS_COUNT=10 ./test-runner.sh run

# Custom log location
BEADS_WORKERS_LOG_DIR=/tmp/my-logs ./test-runner.sh run
```

### Creating Custom Test Scenarios
Edit `test-agent-mock` to add new simulation modes:

```bash
# Add to the case statement in simulate_agent()
your-scenario)
    log_thinking "Custom behavior..."
    mock_read "important-file.sh"
    mock_edit "another-file.sh"
    log_agent "$agent_type" "✓ Custom work complete"
    ;;
```

## Cleanup

### Keep Environment for Inspection
```bash
./test-runner.sh cleanup
# Preserves /tmp/beads-workers-test-<pid> for inspection
```

### Remove Everything
```bash
./test-runner.sh cleanup --remove
# Deletes the entire test environment
```

## Troubleshooting

### Workers Not Starting
```bash
# Check if bd is available
which bd

# Check test environment
ls -la /tmp/beads-workers-test-*/

# Check logs for errors
tail /tmp/beads-workers-test-*/logs/beads_worker_*.log
```

### Mock Agent Not Found
```bash
# Ensure test-agent-mock is executable
chmod +x test-agent-mock

# Check path in test-runner.sh points to correct location
```

### Dashboard Not Updating
```bash
# Verify workers are running
cd /tmp/beads-workers-test-<pid>
./beads-workers-test status

# Check if bd daemon is working
bd stats
```

## Next Steps

Once testing shows the basic workflow works:
1. Modify `worker_process()` in `beads-workers` to call real Claude Code
2. Replace the mock agent invocation with actual Task tool calls
3. Test with real issues in your project
4. Iterate on worker behavior based on real-world usage

## Files

- `test-agent-mock` - Mock Claude Code agent simulator
- `test-runner.sh` - Test environment setup and runner
- `TEST.md` - This file
