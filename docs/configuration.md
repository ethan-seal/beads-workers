# Configuration Guide

This document describes the configuration system for beads-workers.

## Configuration File

By default, beads-workers looks for a configuration file at `.beads-workers.yaml` in the current directory. You can specify a custom config file path using the `--config` flag:

```bash
beads-workers --config /path/to/config.yaml status
```

Or via environment variable:

```bash
export BEADS_CONFIG=/path/to/config.yaml
beads-workers status
```

## Configuration Priority

Settings are applied in the following order (highest to lowest priority):

1. **CLI flags** - Command-line arguments
2. **Environment variables** - Shell environment variables
3. **Configuration file** - YAML config file
4. **Defaults** - Built-in default values

This means CLI flags override environment variables, which override config file values, which override defaults.

## Configuration Options

### Core Settings

```yaml
# Maximum number of concurrent workers (default: number of CPUs)
max_workers: 4

# Path to beads database (default: .beads/beads.db)
db_path: /path/to/beads.db

# Path to beads JSONL file (default: .beads/issues.jsonl)
jsonl_path: /path/to/issues.jsonl

# Default timeout for tasks in seconds (default: 300)
default_timeout: 300
```

### Retry Configuration

```yaml
retry:
  # Maximum number of retries for failed tasks (default: 3)
  max_retries: 3

  # Initial backoff duration in seconds (default: 1)
  initial_backoff_secs: 1

  # Maximum backoff duration in seconds (default: 60)
  max_backoff_secs: 60

  # Backoff multiplier for exponential backoff (default: 2.0)
  backoff_multiplier: 2.0
```

### Logging Configuration

```yaml
logging:
  # Log level: trace, debug, info, warn, error (default: info)
  level: info

  # Log format: pretty, json, compact (default: pretty)
  format: pretty

  # Log file path (optional, no default)
  file: /var/log/beads-workers.log

  # Enable ANSI colors (default: true)
  colors: true
```

### IPC Configuration

```yaml
ipc:
  # Socket path for IPC communication (default: .beads/beads-workers.sock)
  socket_path: /tmp/beads-workers.sock

  # Enable IPC server (default: true)
  enabled: true

  # Connection timeout in seconds (default: 30)
  timeout_secs: 30
```

### Worker Configuration

```yaml
worker:
  # Poll interval in milliseconds (default: 100)
  poll_interval_ms: 100

  # Maximum task queue size per worker (default: 1000)
  queue_size: 1000

  # Enable task prefetching (default: true)
  prefetch: true

  # Number of tasks to prefetch (default: 10)
  prefetch_count: 10

  # Graceful shutdown timeout in seconds (default: 30)
  shutdown_timeout_secs: 30
```

## Environment Variables

All configuration options can be overridden using environment variables with the `BEADS_` prefix:

### Core Settings

```bash
export BEADS_MAX_WORKERS=8
export BEADS_DB_PATH=/path/to/db
export BEADS_JSONL_PATH=/path/to/jsonl
export BEADS_DEFAULT_TIMEOUT=600
```

### Retry Settings

```bash
export BEADS_MAX_RETRIES=5
export BEADS_INITIAL_BACKOFF_SECS=2
export BEADS_MAX_BACKOFF_SECS=120
export BEADS_BACKOFF_MULTIPLIER=2.5
```

### Logging Settings

```bash
export BEADS_LOG_LEVEL=debug
export BEADS_LOG_FORMAT=json
export BEADS_LOG_FILE=/var/log/beads.log
export BEADS_LOG_COLORS=false
```

### IPC Settings

```bash
export BEADS_IPC_SOCKET_PATH=/tmp/custom.sock
export BEADS_IPC_ENABLED=true
export BEADS_IPC_TIMEOUT_SECS=60
```

### Worker Settings

```bash
export BEADS_WORKER_POLL_INTERVAL_MS=200
export BEADS_WORKER_QUEUE_SIZE=2000
export BEADS_WORKER_PREFETCH=true
export BEADS_WORKER_PREFETCH_COUNT=20
export BEADS_WORKER_SHUTDOWN_TIMEOUT_SECS=60
```

## CLI Flags

CLI flags have the highest priority and override both environment variables and config file settings:

```bash
# Custom config file
beads-workers --config /path/to/config.yaml status

# Beads directory
beads-workers --beads-dir /path/to/.beads status

# Maximum concurrent workers
beads-workers -j 16 status
beads-workers --max-workers 16 status

# Logging flags
beads-workers -v status              # Verbose (trace level)
beads-workers -d status              # Debug level
beads-workers --log-level warn status  # Explicit log level
```

## Configuration Commands

### Show Current Configuration

Display the current configuration (after applying all overrides):

```bash
# Show as YAML (default)
beads-workers config show

# Show as JSON
beads-workers config show --json
```

### Get a Specific Value

Retrieve a specific configuration value using dot notation:

```bash
# Get top-level value
beads-workers config get max_workers

# Get nested value
beads-workers config get retry.max_retries
beads-workers config get logging.level
beads-workers config get worker.poll_interval_ms
```

### Reset Configuration

Reset the configuration file to defaults:

```bash
# Preview what will happen
beads-workers config reset

# Actually reset (requires confirmation)
beads-workers config reset --yes
```

## Validation

Configuration is validated at startup. Invalid configurations will result in an error:

### Valid Ranges

- `max_workers`: 1-1024
- `retry.initial_backoff_secs`: > 0
- `retry.max_backoff_secs`: >= `initial_backoff_secs`
- `retry.backoff_multiplier`: >= 1.0
- `logging.level`: one of: `trace`, `debug`, `info`, `warn`, `error`
- `logging.format`: one of: `pretty`, `json`, `compact`
- `ipc.timeout_secs`: > 0
- `worker.poll_interval_ms`: > 0
- `worker.queue_size`: > 0
- `worker.prefetch_count`: > 0 (when `prefetch` is enabled)
- `worker.shutdown_timeout_secs`: > 0

### Example Validation Errors

```bash
# Invalid: max_workers too low
$ beads-workers -j 0 status
Error: Configuration validation failed
Caused by: max_workers must be greater than 0

# Invalid: unknown log level
$ BEADS_LOG_LEVEL=invalid beads-workers status
Error: Configuration validation failed
Caused by: Invalid log level: invalid. Valid levels: ["trace", "debug", "info", "warn", "error"]

# Invalid: backoff configuration
$ cat > invalid.yaml << EOF
retry:
  initial_backoff_secs: 10
  max_backoff_secs: 5
EOF
$ beads-workers --config invalid.yaml status
Error: Configuration validation failed
Caused by: retry.max_backoff_secs (5) must be >= retry.initial_backoff_secs (10)
```

## Examples

### Development Configuration

For local development with verbose logging:

```yaml
# .beads-workers.yaml
max_workers: 2
default_timeout: 60

logging:
  level: debug
  format: pretty
  colors: true

worker:
  poll_interval_ms: 200
```

### Production Configuration

For production with JSON logging and higher concurrency:

```yaml
# /etc/beads-workers/config.yaml
max_workers: 16
default_timeout: 600

logging:
  level: info
  format: json
  file: /var/log/beads-workers/beads-workers.log
  colors: false

retry:
  max_retries: 5
  initial_backoff_secs: 2
  max_backoff_secs: 300

worker:
  poll_interval_ms: 100
  queue_size: 5000
  prefetch: true
  prefetch_count: 20
```

### CI/CD Configuration

For CI environments with minimal output:

```yaml
# ci-config.yaml
max_workers: 4
default_timeout: 300

logging:
  level: warn
  format: compact
  colors: false

retry:
  max_retries: 2
  initial_backoff_secs: 1
```

Use with:

```bash
beads-workers --config ci-config.yaml start
```

## Partial Configuration

You don't need to specify all options - any omitted values will use defaults:

```yaml
# Only override what you need
max_workers: 8
logging:
  level: debug
```

This configuration only changes `max_workers` and `logging.level`, while all other options use their default values.

## Troubleshooting

### Configuration Not Loading

1. Check the file path:
   ```bash
   beads-workers config show --debug
   ```

2. Verify YAML syntax:
   ```bash
   cat .beads-workers.yaml | python -c 'import sys, yaml; yaml.safe_load(sys.stdin)'
   ```

3. Check for validation errors:
   ```bash
   beads-workers --config .beads-workers.yaml config show
   ```

### Finding Current Configuration

To see what configuration is actually being used:

```bash
# Show full config with all applied overrides
beads-workers config show

# Show as JSON for programmatic access
beads-workers config show --json

# Check specific values
beads-workers config get max_workers
beads-workers config get logging.level
```

### Priority Debugging

To understand which source set a value, test incrementally:

```bash
# 1. Default values
beads-workers --config /nonexistent config show

# 2. With config file
beads-workers config show

# 3. With environment variable
BEADS_MAX_WORKERS=8 beads-workers config show

# 4. With CLI flag (highest priority)
BEADS_MAX_WORKERS=8 beads-workers -j 16 config show
```
