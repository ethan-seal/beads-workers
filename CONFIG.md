# Configuration

beads-workers supports flexible configuration through multiple sources with a clear priority order:

**Priority (highest to lowest):**
1. CLI flags
2. Environment variables
3. Configuration file
4. Default values

## Configuration File

By default, beads-workers looks for `.beads-workers.yaml` in the current directory. You can specify a custom config file path using:

```bash
beads-workers --config /path/to/config.yaml status
```

Or via environment variable:

```bash
export BEADS_CONFIG=/path/to/config.yaml
beads-workers status
```

See `.beads-workers.yaml.example` for a complete example configuration file.

## Environment Variables

All configuration options can be overridden using environment variables:

### Core Settings
- `BEADS_DIR` - Beads directory path
- `BEADS_CONFIG` - Configuration file path
- `BEADS_MAX_WORKERS` - Maximum number of concurrent workers
- `BEADS_DB_PATH` - Path to beads database
- `BEADS_JSONL_PATH` - Path to beads JSONL file
- `BEADS_DEFAULT_TIMEOUT` - Default timeout for tasks in seconds

### Retry Settings
- `BEADS_MAX_RETRIES` - Maximum number of retries for failed tasks
- `BEADS_INITIAL_BACKOFF_SECS` - Initial backoff duration in seconds
- `BEADS_MAX_BACKOFF_SECS` - Maximum backoff duration in seconds
- `BEADS_BACKOFF_MULTIPLIER` - Backoff multiplier for exponential backoff

### Logging Settings
- `BEADS_LOG_LEVEL` - Log level (trace, debug, info, warn, error)
- `BEADS_LOG_FORMAT` - Log format (pretty, json, compact)
- `BEADS_LOG_FILE` - Log file path
- `BEADS_LOG_COLORS` - Enable ANSI colors (true/false)
- `BEADS_VERBOSE` - Enable verbose logging (equivalent to trace level)
- `BEADS_DEBUG` - Enable debug logging

### IPC Settings
- `BEADS_IPC_SOCKET_PATH` - Socket path for IPC communication
- `BEADS_IPC_ENABLED` - Enable IPC server (true/false)
- `BEADS_IPC_TIMEOUT_SECS` - Connection timeout in seconds

### Worker Settings
- `BEADS_WORKER_POLL_INTERVAL_MS` - Poll interval in milliseconds
- `BEADS_WORKER_QUEUE_SIZE` - Maximum task queue size per worker
- `BEADS_WORKER_PREFETCH` - Enable task prefetching (true/false)
- `BEADS_WORKER_PREFETCH_COUNT` - Number of tasks to prefetch
- `BEADS_WORKER_SHUTDOWN_TIMEOUT_SECS` - Graceful shutdown timeout in seconds

## CLI Flags

Common CLI flags that override all other configuration:

```bash
beads-workers [OPTIONS] <COMMAND>

Options:
  --beads-dir <PATH>        Path to beads directory
  --config <PATH>           Path to config file
  -j, --max-workers <NUM>   Maximum number of concurrent workers
  -v, --verbose             Enable verbose logging
  -d, --debug               Enable debug logging
  --log-level <LEVEL>       Log level (overrides verbose/debug)
```

## Configuration Validation

beads-workers validates all configuration values on startup. Common validation rules:

- `max_workers` must be between 1 and 1024
- Retry settings must have valid backoff configuration
- Log level must be one of: trace, debug, info, warn, error
- Log format must be one of: pretty, json, compact
- All timeout and interval values must be greater than 0

If validation fails, beads-workers will exit with a descriptive error message.

## Examples

### Using Configuration File

```yaml
# .beads-workers.yaml
max_workers: 16
logging:
  level: debug
  format: json
```

### Overriding with Environment Variables

```bash
export BEADS_MAX_WORKERS=32
export BEADS_LOG_LEVEL=trace
beads-workers start
```

### Overriding with CLI Flags

```bash
beads-workers -j 64 --log-level debug start
```

### Combination

```bash
# Config file sets max_workers=8
# Environment variable sets BEADS_MAX_WORKERS=16
# CLI flag sets -j 32
# Result: max_workers=32 (CLI flag wins)
```

## Viewing Configuration

To see the current effective configuration:

```bash
# Show configuration in YAML format
beads-workers config show

# Show configuration in JSON format
beads-workers config show --json

# Get a specific configuration value
beads-workers config get max_workers
beads-workers config get retry.max_retries
beads-workers config get logging.level
```

## Resetting Configuration

To reset configuration to defaults:

```bash
beads-workers config reset --yes
```
