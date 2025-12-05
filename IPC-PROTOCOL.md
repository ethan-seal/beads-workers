# IPC Protocol Specification

## Overview

This document specifies the Inter-Process Communication (IPC) protocol for `beads-workers` orchestrator-worker architecture. The protocol enables efficient, bidirectional communication between the orchestrator process and worker processes.

## IPC Mechanism: Unix Domain Sockets

**Decision**: Use Unix domain sockets over FIFO pipes.

**Rationale**:
- **Bidirectional**: Single socket handles both directions, simpler than managing two FIFO pipes
- **Connection-oriented**: Automatic client detection when workers connect/disconnect
- **Performance**: Comparable to pipes for local IPC
- **Flexibility**: Easier to extend protocol with complex message patterns
- **Error handling**: Built-in connection state management

**Socket Location**: `/tmp/beads-workers-{PID}.sock` where PID is the orchestrator's process ID.

## Message Format

All messages use JSON Lines format (one JSON object per line, newline-delimited).

### Base Message Structure

```json
{
  "type": "MESSAGE_TYPE",
  "worker_id": "W1",
  "timestamp": 1733423456.789
}
```

All messages include:
- `type`: Message type identifier (string)
- `worker_id`: Worker identifier (string, format "W{N}")
- `timestamp`: Unix timestamp with milliseconds (float)

## Worker → Orchestrator Messages

### READY

Worker is idle and ready to receive a task.

```json
{
  "type": "READY",
  "worker_id": "W1",
  "timestamp": 1733423456.789
}
```

**When sent**:
- On worker startup (after initialization)
- After completing a task (DONE/FAILED)
- After receiving a WAIT message and waiting period expires

### DONE

Worker successfully completed an assigned task.

```json
{
  "type": "DONE",
  "worker_id": "W1",
  "timestamp": 1733423456.789,
  "issue_id": "beads-workers-abc",
  "duration_ms": 15234
}
```

**Additional fields**:
- `issue_id`: The beads issue ID that was completed (string)
- `duration_ms`: Time taken to complete the task in milliseconds (integer)

### FAILED

Worker failed to complete an assigned task.

```json
{
  "type": "FAILED",
  "worker_id": "W1",
  "timestamp": 1733423456.789,
  "issue_id": "beads-workers-abc",
  "error": "Task execution failed: command not found",
  "duration_ms": 1234
}
```

**Additional fields**:
- `issue_id`: The beads issue ID that failed (string)
- `error`: Human-readable error message (string)
- `duration_ms`: Time spent attempting the task in milliseconds (integer)

## Orchestrator → Worker Messages

### TASK

Assign a task to the worker.

```json
{
  "type": "TASK",
  "worker_id": "W1",
  "timestamp": 1733423456.789,
  "issue_id": "beads-workers-abc",
  "priority": 2,
  "title": "Implement feature X"
}
```

**Additional fields**:
- `issue_id`: The beads issue ID to work on (string)
- `priority`: Task priority level 0-4 (integer)
- `title`: Brief task description (string)

**Worker behavior**:
1. Claim the task: `bd update {issue_id} --status=in_progress`
2. Execute the task
3. Close the task: `bd close {issue_id}`
4. Send DONE or FAILED message
5. Send READY message

### WAIT

No tasks currently available, worker should wait before asking again.

```json
{
  "type": "WAIT",
  "worker_id": "W1",
  "timestamp": 1733423456.789,
  "wait_seconds": 30
}
```

**Additional fields**:
- `wait_seconds`: Suggested wait time before sending next READY (integer)

**Worker behavior**:
1. Sleep for `wait_seconds`
2. Send READY message

### SHUTDOWN

Gracefully shut down the worker.

```json
{
  "type": "SHUTDOWN",
  "worker_id": "W1",
  "timestamp": 1733423456.789,
  "reason": "user_requested"
}
```

**Additional fields**:
- `reason`: Shutdown reason: `user_requested`, `error`, `idle_timeout` (string)

**Worker behavior**:
1. If currently working on a task, update status back to `open`
2. Clean up resources
3. Exit with code 0

## Protocol Flows

### Normal Task Processing

```
Worker                          Orchestrator
  |                                   |
  |-------- READY ------------------>|
  |                                   |
  |<------- TASK ---------------------|
  |                                   |
  | (work on task)                    |
  |                                   |
  |-------- DONE ------------------->|
  |                                   |
  |-------- READY ------------------>|
  |                                   |
```

### No Tasks Available

```
Worker                          Orchestrator
  |                                   |
  |-------- READY ------------------>|
  |                                   |
  |<------- WAIT (30s) ---------------|
  |                                   |
  | (sleep 30s)                       |
  |                                   |
  |-------- READY ------------------>|
  |                                   |
```

### Task Failure

```
Worker                          Orchestrator
  |                                   |
  |-------- READY ------------------>|
  |                                   |
  |<------- TASK ---------------------|
  |                                   |
  | (task fails)                      |
  |                                   |
  |-------- FAILED ----------------->|
  |                                   |
  |-------- READY ------------------>|
  |                                   |
```

### Graceful Shutdown

```
Worker                          Orchestrator
  |                                   |
  |-------- READY ------------------>|
  |                                   |
  |<------- SHUTDOWN -----------------|
  |                                   |
  | (cleanup)                         |
  |                                   |
  | (exit 0)                          |
  X                                   |
```

## Connection Management

### Worker Startup

1. Worker connects to orchestrator socket
2. Worker sends initial READY message
3. Orchestrator registers worker

### Worker Disconnect

- **Graceful**: Worker receives SHUTDOWN, cleans up, closes connection
- **Ungraceful**: Connection drops unexpectedly
  - Orchestrator detects broken socket
  - If worker had a task, reset task status to `open`
  - Remove worker from active worker list

### Orchestrator Startup

1. Create Unix domain socket at `/tmp/beads-workers-{PID}.sock`
2. Listen for worker connections
3. Accept connections as workers start

### Orchestrator Shutdown

1. Send SHUTDOWN to all connected workers
2. Wait up to 10 seconds for workers to disconnect
3. Force kill remaining workers
4. Close and remove socket file

## Error Handling

### Invalid Message Format

If a message cannot be parsed as JSON or is missing required fields:
- Log warning with raw message
- Continue listening for next message
- Do not crash the process

### Unknown Message Type

If a message has an unrecognized `type`:
- Log warning with message details
- Ignore the message
- Continue normal operation

### Unexpected Message

If a message is received in wrong state (e.g., TASK when not READY):
- Log error
- Send appropriate response to resync state
- Example: If worker sends DONE without being assigned a task, respond with WAIT

### Connection Failures

**Worker cannot connect to orchestrator**:
- Retry connection 3 times with exponential backoff (1s, 2s, 4s)
- Log error and exit if all retries fail

**Orchestrator socket read/write fails**:
- Treat as worker disconnect
- Clean up worker state
- If worker had active task, reset task to `open`

## Implementation Notes

### Bash Implementation

For Bash implementation using `socat` or native `/dev/tcp`:

**Orchestrator (server)**:
```bash
socat UNIX-LISTEN:/tmp/beads-workers-${ORCHESTRATOR_PID}.sock,fork SYSTEM:'handle_worker_connection'
```

**Worker (client)**:
```bash
exec 3<>/dev/tcp/unix:/tmp/beads-workers-${ORCHESTRATOR_PID}.sock
echo '{"type":"READY","worker_id":"W1","timestamp":'$(date +%s.%N)'}' >&3
read -u 3 response
```

### Message Parsing in Bash

Use `jq` for JSON parsing:
```bash
message_type=$(echo "$line" | jq -r '.type')
worker_id=$(echo "$line" | jq -r '.worker_id')
issue_id=$(echo "$line" | jq -r '.issue_id // empty')
```

### Timestamp Generation

Unix timestamp with milliseconds:
```bash
timestamp=$(date +%s.%N)
# Or for better precision:
timestamp=$(python3 -c 'import time; print(time.time())')
```

### Concurrency

- Orchestrator must handle multiple concurrent worker connections
- Use process-per-connection model with `fork` in socat
- Or use a single orchestrator process with non-blocking I/O and multiplexing

## Performance Considerations

- **Latency**: Socket communication < 1ms on same machine
- **Throughput**: Can handle 10-100 workers easily
- **Buffering**: Use line-buffered I/O to ensure complete messages
- **Keep-alive**: No keep-alive needed; workers only send when they have something to report

## Security

- Socket permissions: 0600 (owner read/write only)
- Socket location in `/tmp`: Ensure proper cleanup on orchestrator exit
- Message validation: Validate all fields before acting on messages
- Worker identity: Trust `worker_id` only for logging; orchestrator assigns workers

## Testing Strategy

### Unit Tests

Test message parsing and generation:
- Valid messages parse correctly
- Invalid JSON is handled gracefully
- Missing required fields are detected

### Integration Tests

Test protocol flows:
- Worker connects and receives task
- Worker completes task and gets new one
- Worker fails task and reports error
- Orchestrator sends WAIT when no tasks
- Graceful shutdown works

### Stress Tests

- 100 workers connecting simultaneously
- Rapid task completion (messages every 100ms)
- Worker disconnect during task execution
- Orchestrator restart with active workers

## Future Extensions

Possible protocol extensions (not in v1):

- **HEARTBEAT**: Periodic keep-alive messages
- **STATUS**: Worker reports current status without state change
- **PAUSE/RESUME**: Orchestrator pauses worker temporarily
- **PRIORITY**: Worker requests high-priority tasks
- **BATCH**: Multiple tasks in single message
- **PROGRESS**: Worker reports task progress (0-100%)

These would be added in backwards-compatible way using message versioning:

```json
{
  "version": "1.1",
  "type": "HEARTBEAT",
  ...
}
```
