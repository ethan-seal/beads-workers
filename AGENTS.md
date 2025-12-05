# Agent Instructions for Beads Workers

## Project Overview

**beads-workers** is a single-file CLI tool for running concurrent workers that process beads tasks automatically.

## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Quick Start

**Check for ready work:**
```bash
bd ready --json
```

**Create new issues:**
```bash
bd create "Issue title" -t bug|feature|task -p 0-4 --json
bd create "Issue title" -p 1 --deps discovered-from:beads-workers-123 --json
bd create "Subtask" --parent <epic-id> --json
```

**Claim and update:**
```bash
bd update beads-workers-42 --status in_progress --json
bd close beads-workers-42 --reason "Completed" --json
```

### Workflow for AI Agents

1. **Check ready work**: `bd ready`
2. **Claim your task**: `bd update <id> --status in_progress`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue: `bd create "Found bug" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`
6. **Commit together**: Always commit `.beads/issues.jsonl` with code changes

## Tech Stack

- **Language**: Pure Bash (4.0+)
- **Dependencies**: None (just `bd` command)
- **Target**: Any Unix-like system
- **Testing**: Manual testing with real beads projects

## Project Structure

```
beads-workers/
├── beads-workers        # Main executable (all code in one file)
├── README.md            # User documentation
├── INSTALL.md           # Installation guide
├── AGENTS.md            # This file (agent instructions)
├── LICENSE              # MIT license
└── .beads/              # beads issue tracking
    ├── beads.db         # SQLite database (DO NOT COMMIT)
    ├── issues.jsonl     # Git-synced issues (COMMIT THIS)
    └── config.yaml      # beads configuration
```

## Code Structure

The `beads-workers` file contains:

1. **Main dispatch** - `main()` function routes to subcommands
2. **Commands** - Each `cmd_*()` function implements a subcommand
3. **Worker process** - `worker_process()` function is the worker loop
4. **Dashboard** - `show_dashboard()` in `cmd_watch()` for live UI
5. **Helpers** - Logging, PID management, etc.

## Development Guidelines

### Adding New Commands

To add a new command:

1. Create a `cmd_yourcommand()` function
2. Add case to `main()` dispatch
3. Add to help text in `cmd_help()`
4. Test in a beads project

Example:
```bash
cmd_yourcommand() {
    check_beads_project
    # Your code here
}

# In main():
yourcommand)
    cmd_yourcommand "$@"
    ;;
```

### Code Style

- Use lowercase variable names with underscores
- Use `local` for function-scoped variables
- Quote all variable expansions: `"$var"` not `$var`
- Use `set -e` for error handling
- Use helper functions: `log_info`, `log_success`, `log_error`, `log_warning`
- Check beads project with `check_beads_project` before bd commands

### Testing

Test manually in real beads projects:

```bash
# Create test project
mkdir test-project && cd test-project
bd init

# Test commands
../beads-workers help
../beads-workers status
../beads-workers start
../beads-workers watch
../beads-workers stop
```

### Modifying Worker Behavior

The worker loop is in `worker_process()`. Key sections:

1. **Startup** - Staggered delays to reduce contention
2. **Task claiming** - Random selection from `bd ready`
3. **Work** - Currently simulates with `sleep`, customize here
4. **Cleanup** - Close task and delay before next cycle

### Dashboard Customization

The live dashboard is in `show_dashboard()` inside `cmd_watch()`:

- Uses terminal control sequences (`\033[H`, etc.)
- Buffers all output before printing (reduces flicker)
- Updates every 2 seconds
- Shows: stats, workers, PIDs, status, recent logs

## Release Process

1. Update version in script: `VERSION="1.0.1"`
2. Create git tag: `git tag v1.0.1`
3. Push tag: `git push origin v1.0.1`
4. Create GitHub release with `beads-workers` file attached

## Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic bd use
- ✅ Test changes in real beads projects
- ✅ Keep everything in one file (portability)
- ✅ Use portable bash (avoid bashisms that require 5.0+)
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT add external dependencies
- ❌ Do NOT split into multiple files
- ❌ Do NOT commit `.beads/beads.db`

## Common Tasks

### Add a new environment variable

1. Add default at top: `NEW_VAR="${BEADS_WORKERS_NEW_VAR:-default}"`
2. Document in `cmd_help()`
3. Use in relevant command

### Change default worker count

Edit: `NUM_WORKERS="${BEADS_WORKERS_COUNT:-10}"`

### Add new worker status

1. Add check in `show_dashboard()` worker status section
2. Add emoji and label
3. Document in README.md

### Improve performance

- Profile with `time beads-workers status`
- Check for unnecessary subprocess calls
- Buffer output before printing
- Use `--json` flag on bd commands (faster parsing)

## Notes

- The script uses `pgrep -f "beads-workers worker-process"` to find worker PIDs
- Workers run as background processes via `&`
- Each worker is a separate invocation: `beads-workers worker-process <id>`
- Logs go to `$BEADS_WORKERS_LOG_DIR/beads_worker_N.log`
- The `no-daemon` mode is critical for performance (192x faster)

## Resources

- [beads documentation](https://github.com/cablehead/beads)
- [Advanced Bash-Scripting Guide](https://tldp.org/LDP/abs/html/)
- [Terminal control sequences](https://en.wikipedia.org/wiki/ANSI_escape_code)
