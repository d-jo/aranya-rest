#!/usr/bin/env bash

# Full integration test: Start daemons, test adding device, cleanup
# This script demonstrates the complete workflow in one go

set -euo pipefail

echo "🚀 Aranya REST API Full Integration Test"
echo "========================================"
echo ""

# Configuration
COUNT=2
BASE_PORT=8080

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
WORK_DIR=$(mktemp -d -t aranya-full-test-XXXXXX)
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

echo "🔧 Setting up $COUNT daemon instances..."

# Create daemon instances
for ((i=0; i<COUNT; i++)); do
    INSTANCE_NAME="daemon-$i"
    PORT=$((BASE_PORT + i))
    INSTANCE_DIR="$WORK_DIR/$INSTANCE_NAME"
    
    echo "   Creating $INSTANCE_NAME (port $PORT)..."
    
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
    cd "$INSTANCE_DIR"
    "$DAEMON_BIN" --config "$INSTANCE_DIR/config.json" &
    DAEMON_PID=$!
    DAEMON_PIDS+=("$DAEMON_PID")
    
    # Wait for daemon to create UDS socket
    UDS_PATH="$INSTANCE_DIR/run/uds.sock"
    UDS_PATHS+=("$UDS_PATH")
    
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
    "$REST_BIN" --daemon-socket "$UDS_PATH" --bind-addr "127.0.0.1:$PORT" > /dev/null 2>&1 &
    REST_PID=$!
    REST_PIDS+=("$REST_PID")
    PORTS+=("$PORT")
    
    # Wait for REST server to start
    sleep 0.5
done

echo "✅ All daemon instances ready!"
echo ""

# Wait a bit for everything to settle
sleep 1

echo "🧪 Running Integration Test"
echo "============================"
echo ""

# Test function with error handling
run_test() {
    local step="$1"
    local description="$2"
    shift 2
    
    echo "📋 Step $step: $description"
    
    if ! "$@"; then
        echo "❌ Step $step failed!"
        return 1
    fi
    
    echo "✅ Step $step completed"
    echo ""
}

# Helper function to make HTTP requests with error checking
make_request() {
    local url="$1"
    local method="${2:-GET}"
    local data="${3:-}"
    local response
    local http_code
    local body
    
    if [[ "$method" == "GET" ]]; then
        if ! response=$(curl -s -w "%{http_code}" "$url"); then
            echo "❌ Failed to connect to $url" >&2
            return 1
        fi
    else
        if ! response=$(curl -s -w "%{http_code}" -X "$method" -H "Content-Type: application/json" -d "$data" "$url"); then
            echo "❌ Failed to connect to $url" >&2
            return 1
        fi
    fi
    
    http_code="${response: -3}"
    body="${response%???}"
    
    if [[ "$http_code" != "200" ]]; then
        echo "❌ HTTP $http_code from $url" >&2
        echo "Response: $body" >&2
        return 1
    fi
    
    echo "$body"
}

# Step 1: Test basic connectivity
echo "📋 Step 1: Testing basic connectivity"
echo "   Testing daemon-0 version..."
if VERSION_0=$(make_request "http://127.0.0.1:8080/api/v1/version"); then
    echo "   daemon-0: $(echo "$VERSION_0" | jq -r .version)"
else
    echo "❌ Step 1 failed!"
    exit 1
fi

echo "   Testing daemon-1 version..."
if VERSION_1=$(make_request "http://127.0.0.1:8081/api/v1/version"); then
    echo "   daemon-1: $(echo "$VERSION_1" | jq -r .version)"
else
    echo "❌ Step 1 failed!"
    exit 1
fi
echo "✅ Step 1 completed"
echo ""

# Step 2: Get device IDs
echo "📋 Step 2: Getting device IDs"
if DEVICE_0_RESP=$(make_request "http://127.0.0.1:8080/api/v1/device-id"); then
    DEVICE_0=$(echo "$DEVICE_0_RESP" | jq -r .device_id)
    echo "   Device 0: $DEVICE_0"
else
    echo "❌ Step 2 failed!"
    exit 1
fi

if DEVICE_1_RESP=$(make_request "http://127.0.0.1:8081/api/v1/device-id"); then
    DEVICE_1=$(echo "$DEVICE_1_RESP" | jq -r .device_id)
    echo "   Device 1: $DEVICE_1"
else
    echo "❌ Step 2 failed!"
    exit 1
fi
echo "✅ Step 2 completed"
echo ""

# Step 3: Create team on daemon-0
echo "📋 Step 3: Creating team on daemon-0"
if TEAM_RESPONSE=$(make_request "http://127.0.0.1:8080/api/v1/teams" "POST" '{"config":{}}'); then
    TEAM_ID=$(echo "$TEAM_RESPONSE" | jq -r .team_id)
    echo "   Team ID: $TEAM_ID"
else
    echo "❌ Step 3 failed!"
    exit 1
fi
echo "✅ Step 3 completed"
echo ""

# Step 4: Get daemon-1's key bundle
echo "📋 Step 4: Getting daemon-1 key bundle"
if KEYS=$(make_request "http://127.0.0.1:8081/api/v1/key-bundle"); then
    echo "   Got key bundle ($(echo "$KEYS" | jq -r ".identity | length") bytes identity key)"
    # Save for next step
    echo "$KEYS" > "$WORK_DIR/keys.json"
else
    echo "❌ Step 4 failed!"
    exit 1
fi
echo "✅ Step 4 completed"
echo ""

# Step 5: Add daemon-1 to team
echo "📋 Step 5: Adding daemon-1 to team"
KEYS=$(cat "$WORK_DIR/keys.json")

# Create the request payload
REQUEST_PAYLOAD=$(jq -n --argjson keys "$KEYS" '{"keys": $keys}')
echo "   Payload prepared"

if ADD_RESPONSE=$(make_request "http://127.0.0.1:8080/api/v1/teams/$TEAM_ID/devices" "POST" "$REQUEST_PAYLOAD"); then
    echo "   Add device response: $ADD_RESPONSE"
else
    echo "❌ Step 5 failed!"
    exit 1
fi
echo "✅ Step 5 completed"
echo ""

# Step 6: Verify devices on team
echo "📋 Step 6: Verifying devices on team"
if DEVICES=$(make_request "http://127.0.0.1:8080/api/v1/teams/$TEAM_ID/devices"); then
    DEVICE_COUNT=$(echo "$DEVICES" | jq "length")
    
    echo "   Devices on team: $DEVICES"
    echo "   Device count: $DEVICE_COUNT"
    
    if [[ "$DEVICE_COUNT" != "2" ]]; then
        echo "❌ Expected 2 devices, got $DEVICE_COUNT"
        exit 1
    fi
    
    echo "   ✅ Correct number of devices on team!"
else
    echo "❌ Step 6 failed!"
    exit 1
fi
echo "✅ Step 6 completed"
echo ""

# Step 7: Test querying device role
echo "📋 Step 7: Testing device role queries"

# Query device 0 role (should be Owner)
if ROLE_0=$(make_request "http://127.0.0.1:8080/api/v1/teams/$TEAM_ID/devices/$DEVICE_0/role"); then
    echo "   Device 0 role: $ROLE_0"
else
    echo "❌ Failed to query device 0 role"
    exit 1
fi

# Query device 1 role (should be Member)
if ROLE_1=$(make_request "http://127.0.0.1:8080/api/v1/teams/$TEAM_ID/devices/$DEVICE_1/role"); then
    echo "   Device 1 role: $ROLE_1"
else
    echo "❌ Failed to query device 1 role"
    exit 1
fi

if [[ "$(echo "$ROLE_0" | jq -r .)" != "Owner" ]]; then
    echo "❌ Device 0 should be Owner, got: $ROLE_0"
    exit 1
fi

if [[ "$(echo "$ROLE_1" | jq -r .)" != "Member" ]]; then
    echo "❌ Device 1 should be Member, got: $ROLE_1"
    exit 1
fi

echo "   ✅ Device roles are correct!"
echo "✅ Step 7 completed"
echo ""

echo "🎉 All tests passed!"
echo ""
echo "📊 Test Summary:"
echo "   • Created team: $TEAM_ID"
echo "   • Device 0 (Owner): $DEVICE_0"
echo "   • Device 1 (Member): $DEVICE_1" 
echo "   • All REST endpoints working correctly"
echo "   • Device addition workflow successful"
echo ""
echo "✅ Integration test completed successfully!"

# Cleanup will happen automatically via trap