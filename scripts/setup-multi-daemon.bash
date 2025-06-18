#!/usr/bin/env bash

# Setup script for multiple Aranya daemons with REST servers
# Usage: ./setup-multi-daemon.bash [count] [base_port]

set -euo pipefail

COUNT=${1:-2}
BASE_PORT=${2:-8080}

echo "🚀 Setting up $COUNT Aranya daemon instances with REST servers"
echo "   Base port: $BASE_PORT (each daemon gets port + index)"
echo ""

# Find binaries
SCRIPT_DIR="$(dirname "$0")"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DAEMON_BIN=""
REST_BIN=""

# Try to find daemon binary
for candidate in "$ROOT_DIR/target/release/aranya-daemon" "$ROOT_DIR/target/debug/aranya-daemon"; do
    if [[ -x "$candidate" ]]; then
        DAEMON_BIN="$candidate"
        break
    fi
done

if [[ -z "$DAEMON_BIN" ]]; then
    echo "❌ Could not find aranya-daemon binary"
    echo "   Please build it first: cargo build --bin aranya-daemon"
    exit 1
fi

# Try to find REST binary
for candidate in "$ROOT_DIR/target/release/aranya-rest" "$ROOT_DIR/target/debug/aranya-rest"; do
    if [[ -x "$candidate" ]]; then
        REST_BIN="$candidate"
        break
    fi
done

if [[ -z "$REST_BIN" ]]; then
    echo "❌ Could not find aranya-rest binary"
    echo "   Please build it first: cargo build --bin aranya-rest"
    exit 1
fi

echo "✅ Found binaries:"
echo "   Daemon: $DAEMON_BIN"
echo "   REST:   $REST_BIN"
echo ""

# Create temp directory for all instances
WORK_DIR=$(mktemp -d -t aranya-multi-XXXXXX)
echo "📁 Working directory: $WORK_DIR"

# Arrays to track processes and info
declare -a DAEMON_PIDS=()
declare -a REST_PIDS=()
declare -a PORTS=()
declare -a UDS_PATHS=()

# Cleanup function
cleanup() {
    echo ""
    echo "🛑 Cleaning up..."
    
    # Kill REST servers
    for pid in "${REST_PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            echo "   Stopping REST server (PID: $pid)"
            kill "$pid" 2>/dev/null || true
        fi
    done
    
    # Kill daemons
    for pid in "${DAEMON_PIDS[@]}"; do
        if kill -0 "$pid" 2>/dev/null; then
            echo "   Stopping daemon (PID: $pid)"
            kill "$pid" 2>/dev/null || true
        fi
    done
    
    # Remove temp directory
    if [[ -n "${WORK_DIR:-}" ]]; then
        echo "   Removing $WORK_DIR"
        rm -rf "$WORK_DIR"
    fi
    
    echo "✅ Cleanup complete"
}

# Set up signal handlers
trap cleanup EXIT INT TERM

# Create daemon instances
for ((i=0; i<COUNT; i++)); do
    INSTANCE_NAME="daemon-$i"
    PORT=$((BASE_PORT + i))
    INSTANCE_DIR="$WORK_DIR/$INSTANCE_NAME"
    
    echo "🔧 Creating $INSTANCE_NAME (port $PORT)..."
    
    # Create directory structure
    mkdir -p "$INSTANCE_DIR"/{run,state,cache,logs,config}
    
    # Create daemon config
    cat > "$INSTANCE_DIR/config.json" << EOF
{
    "name": "$INSTANCE_NAME",
    "runtime_dir": "$INSTANCE_DIR/run",
    "state_dir": "$INSTANCE_DIR/state", 
    "cache_dir": "$INSTANCE_DIR/cache",
    "logs_dir": "$INSTANCE_DIR/logs",
    "config_dir": "$INSTANCE_DIR/config",
    "sync_addr": "localhost:0"
}
EOF
    
    # Start daemon
    echo "   Starting daemon..."
    cd "$INSTANCE_DIR"
    "$DAEMON_BIN" --config "$INSTANCE_DIR/config.json" &
    DAEMON_PID=$!
    DAEMON_PIDS+=("$DAEMON_PID")
    
    # Wait for daemon to create UDS socket
    UDS_PATH="$INSTANCE_DIR/run/uds.sock"
    UDS_PATHS+=("$UDS_PATH")
    
    echo "   Waiting for daemon to start..."
    for attempt in {1..50}; do
        if [[ -S "$UDS_PATH" ]]; then
            break
        fi
        sleep 0.1
        if ((attempt == 50)); then
            echo "❌ Daemon failed to start (no UDS socket after 5 seconds)"
            exit 1
        fi
    done
    
    # Wait a bit more for API key
    sleep 0.2
    
    # Start REST server
    echo "   Starting REST server on port $PORT..."
    "$REST_BIN" --daemon-socket "$UDS_PATH" --bind-addr "127.0.0.1:$PORT" &
    REST_PID=$!
    REST_PIDS+=("$REST_PID")
    PORTS+=("$PORT")
    
    # Wait for REST server to start
    sleep 0.5
    
    echo "   ✅ $INSTANCE_NAME ready!"
done

echo ""
echo "🎉 All daemon instances are running!"
echo ""
echo "┌─────────────┬──────────────┬─────────────────────────────────────────┐"
echo "│ Instance    │ REST Port    │ UDS Socket                              │"
echo "├─────────────┼──────────────┼─────────────────────────────────────────┤"

for ((i=0; i<COUNT; i++)); do
    printf "│ %-11s │ %-12s │ %-39s │\n" \
        "daemon-$i" \
        "${PORTS[i]}" \
        "${UDS_PATHS[i]}"
done

echo "└─────────────┴──────────────┴─────────────────────────────────────────┘"
echo ""
echo "📋 REST API endpoints:"
for ((i=0; i<COUNT; i++)); do
    echo "  • daemon-$i: http://127.0.0.1:${PORTS[i]}/api/v1/"
done

echo ""
echo "🔧 Example curl commands:"
if ((COUNT > 0)); then
    PORT="${PORTS[0]}"
    echo "  curl http://127.0.0.1:$PORT/api/v1/version"
    echo "  curl http://127.0.0.1:$PORT/api/v1/device-id"
    echo "  curl -X POST http://127.0.0.1:$PORT/api/v1/teams -H 'Content-Type: application/json' -d '{\"config\": {}}'"
fi

echo ""
echo "⏳ Daemons are running... Press Ctrl+C to stop all instances."

# Wait for interrupt
wait