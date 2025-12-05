#!/bin/bash
# Test runner for beads-workers with mock agents
# Creates test issues and runs workers with mock agent simulation

set -e

TEST_DIR="/tmp/beads-workers-test-$$"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

log_test() {
    echo -e "${BLUE}[Test]${NC} $1"
}

log_success() {
    echo -e "${GREEN}✓${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

# Setup test environment
setup_test_env() {
    log_test "Setting up test environment in $TEST_DIR"

    mkdir -p "$TEST_DIR"
    cd "$TEST_DIR"

    # Initialize beads project
    bd init

    # Enable no-daemon mode
    echo "no-daemon: true" >> .beads/config.yaml

    log_success "Test environment ready"
}

# Create test issues
create_test_issues() {
    local count=${1:-10}

    log_test "Creating $count test issues..."

    for i in $(seq 1 "$count"); do
        bd create --title="Test issue $i - Mock agent task" \
                 --type=task \
                 --priority=1 \
                 --json > /dev/null
    done

    log_success "Created $count test issues"
}

# Create modified beads-workers that uses mock agents
create_mock_workers() {
    log_test "Creating mock-enabled beads-workers..."

    # Copy original script
    cp "$SCRIPT_DIR/beads-workers" "$TEST_DIR/beads-workers-test"

    # Modify worker_process to use mock agent instead of real work
    # Replace the work simulation section
    local temp_file=$(mktemp)

    awk '
    /# Simulate work/ {
        print "        # Use mock agent instead of sleep"
        print "        log \"Invoking mock agent for $issue_id...\""
        print "        "
        print "        # Get issue details for context"
        print "        issue_title=$(bd show \"$issue_id\" 2>/dev/null | grep -oP \"Title: \\K.*\" || echo \"Unknown\")"
        print "        "
        print "        # Run mock agent"
        print "        if \"'"$SCRIPT_DIR"'/test-agent-mock\" \"$issue_id\" \"$issue_title\" 2>&1; then"
        print "            log \"Mock agent completed successfully for $issue_id\""
        print "        else"
        print "            log \"Mock agent failed for $issue_id\""
        print "            # Continue anyway for testing"
        print "        fi"
        # Skip the next 2 lines (work_time and sleep)
        next
        next
        next
    }
    { print }
    ' "$TEST_DIR/beads-workers-test" > "$temp_file"

    mv "$temp_file" "$TEST_DIR/beads-workers-test"
    chmod +x "$TEST_DIR/beads-workers-test"

    log_success "Mock-enabled beads-workers created"
}

# Run test scenario
run_test() {
    local worker_count=${1:-3}
    local issue_count=${2:-5}
    local mock_mode=${3:-fast}

    log_test "Running test with $worker_count workers, $issue_count issues, mode=$mock_mode"

    export BEADS_MOCK_MODE="$mock_mode"
    export BEADS_WORKERS_COUNT="$worker_count"
    export BEADS_WORKERS_LOG_DIR="$TEST_DIR/logs"

    mkdir -p "$TEST_DIR/logs"

    # Start workers
    ./beads-workers-test start

    log_success "Workers started. Check status with:"
    echo "  cd $TEST_DIR && ./beads-workers-test status"
    echo "  cd $TEST_DIR && ./beads-workers-test watch"
    echo ""
    echo "View logs:"
    echo "  tail -f $TEST_DIR/logs/beads_worker_*.log"
    echo ""
    echo "Stop workers:"
    echo "  cd $TEST_DIR && ./beads-workers-test stop"
}

# Cleanup test environment
cleanup() {
    log_test "Cleaning up test environment..."

    if [ -f "$TEST_DIR/beads-workers-test" ]; then
        cd "$TEST_DIR"
        ./beads-workers-test stop 2>/dev/null || true
    fi

    # Optionally remove test directory
    if [ "$1" = "--remove" ]; then
        rm -rf "$TEST_DIR"
        log_success "Test environment removed"
    else
        log_warning "Test environment preserved at: $TEST_DIR"
        log_warning "Remove with: rm -rf $TEST_DIR"
    fi
}

# Show help
show_help() {
    cat << 'EOF'
Test Runner for beads-workers

Usage:
  ./test-runner.sh [command] [options]

Commands:
  setup              Create test environment and issues
  run [W] [I] [M]    Run test (W=workers, I=issues, M=mode)
  cleanup [--remove] Clean up (optionally remove test dir)
  full [W] [I] [M]   Setup + Run (default: 3 workers, 10 issues, fast)

Mock Modes:
  fast      - Quick simulation (1-2s per issue)
  realistic - Realistic timing (10-15s per issue)
  explore   - Heavy exploration workflow
  error     - Simulate failures
  timeout   - Simulate long-running agents
  mixed     - Random behavior

Examples:
  # Quick test with 3 workers, 10 issues
  ./test-runner.sh full

  # Custom configuration
  ./test-runner.sh full 5 20 realistic

  # Manual control
  ./test-runner.sh setup
  ./test-runner.sh run 3 10 fast
  # ... monitor in separate terminal ...
  ./test-runner.sh cleanup

  # Test error handling
  ./test-runner.sh full 2 5 error

Environment Variables:
  BEADS_MOCK_MODE         Override mock mode
  BEADS_WORKERS_COUNT     Override worker count
  BEADS_WORKERS_LOG_DIR   Override log directory

EOF
}

# Main
main() {
    local cmd=${1:-help}

    case "$cmd" in
        setup)
            setup_test_env
            create_test_issues "${2:-10}"
            create_mock_workers
            echo ""
            log_success "Setup complete! Now run: $0 run"
            ;;

        run)
            if [ ! -d "$TEST_DIR/.beads" ]; then
                echo "Test environment not found. Run: $0 setup"
                exit 1
            fi
            cd "$TEST_DIR"
            run_test "${2:-3}" "${3:-10}" "${4:-fast}"
            ;;

        cleanup)
            cleanup "$2"
            ;;

        full)
            setup_test_env
            create_test_issues "${3:-10}"
            create_mock_workers
            run_test "${2:-3}" "${3:-10}" "${4:-fast}"
            echo ""
            log_success "Test running! Monitor with:"
            echo "  cd $TEST_DIR && ./beads-workers-test watch"
            ;;

        help|--help|-h)
            show_help
            ;;

        *)
            echo "Unknown command: $cmd"
            echo "Run: $0 help"
            exit 1
            ;;
    esac
}

main "$@"
