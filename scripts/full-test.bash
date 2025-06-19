#!/usr/bin/env bash

# Full integration test: Start daemons, test adding device, cleanup
# This script demonstrates the complete workflow in one go

set -euo pipefail

echo "🚀 Aranya REST API Full Integration Test"
echo "========================================"
echo ""

# Configuration - can be overridden by command line arguments
COUNT=${1:-2}
BASE_PORT=${2:-8080}
KEEP_RUNNING=${3:-false}

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
declare -a DEVICE_IDS=()
declare -a LOCAL_ADDRS=()

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
echo "   Using ports $BASE_PORT-$((BASE_PORT + COUNT - 1))"
if [[ "$KEEP_RUNNING" == "true" ]]; then
    echo "   Will keep daemons running after test completion"
fi

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
for ((i=0; i<COUNT; i++)); do
    PORT=$((BASE_PORT + i))
    echo "   Testing daemon-$i version..."
    if VERSION=$(make_request "http://127.0.0.1:$PORT/api/v1/version"); then
        echo "   daemon-$i: $(echo "$VERSION" | jq -r .version)"
    else
        echo "❌ Step 1 failed for daemon-$i!"
        exit 1
    fi
done
echo "✅ Step 1 completed"
echo ""

# Step 2: Get device IDs and local addresses
echo "📋 Step 2: Getting device IDs and local addresses"
for ((i=0; i<COUNT; i++)); do
    PORT=$((BASE_PORT + i))
    
    # Get device ID
    if DEVICE_RESP=$(make_request "http://127.0.0.1:$PORT/api/v1/device-id"); then
        DEVICE_ID=$(echo "$DEVICE_RESP" | jq -r .device_id)
        DEVICE_IDS+=("$DEVICE_ID")
        echo "   Device $i: $DEVICE_ID"
    else
        echo "❌ Step 2 failed for daemon-$i device ID!"
        exit 1
    fi
    
    # Get local address for sync peers
    if ADDR_RESP=$(make_request "http://127.0.0.1:$PORT/api/v1/local-addr"); then
        LOCAL_ADDR=$(echo "$ADDR_RESP" | jq -r .address)
        LOCAL_ADDRS+=("$LOCAL_ADDR")
        echo "   Address $i: $LOCAL_ADDR"
    else
        echo "❌ Step 2 failed for daemon-$i local address!"
        exit 1
    fi
done
echo "✅ Step 2 completed"
echo ""

# Step 3: Create team on daemon-0
echo "📋 Step 3: Creating team on daemon-0"
if TEAM_RESPONSE=$(make_request "http://127.0.0.1:$BASE_PORT/api/v1/teams" "POST" '{"config":{}}'); then
    TEAM_ID=$(echo "$TEAM_RESPONSE" | jq -r .team_id)
    echo "   Team ID: $TEAM_ID"
else
    echo "❌ Step 3 failed!"
    exit 1
fi
echo "✅ Step 3 completed"
echo ""

# Step 4: Add all other devices to the team
echo "📋 Step 4: Adding devices to team"
for ((i=1; i<COUNT; i++)); do
    PORT=$((BASE_PORT + i))
    echo "   Getting daemon-$i key bundle..."
    
    if KEYS=$(make_request "http://127.0.0.1:$PORT/api/v1/key-bundle"); then
        echo "   Got key bundle for daemon-$i ($(echo "$KEYS" | jq -r ".identity | length") bytes identity key)"
        
        # Create the request payload
        REQUEST_PAYLOAD=$(jq -n --argjson keys "$KEYS" '{"keys": $keys}')
        
        echo "   Adding daemon-$i to team..."
        if ADD_RESPONSE=$(make_request "http://127.0.0.1:$BASE_PORT/api/v1/teams/$TEAM_ID/devices" "POST" "$REQUEST_PAYLOAD"); then
            echo "   Successfully added daemon-$i to team"
        else
            echo "❌ Step 4 failed adding daemon-$i!"
            exit 1
        fi
    else
        echo "❌ Step 4 failed getting keys for daemon-$i!"
        exit 1
    fi
done
echo "✅ Step 4 completed"
echo ""

# Step 5: Verify devices on team
echo "📋 Step 5: Verifying devices on team"
if DEVICES=$(make_request "http://127.0.0.1:$BASE_PORT/api/v1/teams/$TEAM_ID/devices"); then
    DEVICE_COUNT=$(echo "$DEVICES" | jq "length")
    
    echo "   Devices on team: $DEVICES"
    echo "   Device count: $DEVICE_COUNT"
    
    if [[ "$DEVICE_COUNT" != "$COUNT" ]]; then
        echo "❌ Expected $COUNT devices, got $DEVICE_COUNT"
        exit 1
    fi
    
    echo "   ✅ Correct number of devices on team!"
else
    echo "❌ Step 5 failed!"
    exit 1
fi
echo "✅ Step 5 completed"
echo ""

# Step 6: Test querying device roles
echo "📋 Step 6: Testing device role queries"

# Query device roles
for ((i=0; i<COUNT; i++)); do
    DEVICE_ID="${DEVICE_IDS[$i]}"
    
    if ROLE=$(make_request "http://127.0.0.1:$BASE_PORT/api/v1/teams/$TEAM_ID/devices/$DEVICE_ID/role"); then
        ROLE_VALUE=$(echo "$ROLE" | jq -r .)
        echo "   Device $i role: $ROLE_VALUE"
        
        # Device 0 should be Owner, others should be Member
        if [[ $i -eq 0 ]]; then
            if [[ "$ROLE_VALUE" != "Owner" ]]; then
                echo "❌ Device 0 should be Owner, got: $ROLE_VALUE"
                exit 1
            fi
        else
            if [[ "$ROLE_VALUE" != "Member" ]]; then
                echo "❌ Device $i should be Member, got: $ROLE_VALUE"
                exit 1
            fi
        fi
    else
        echo "❌ Failed to query device $i role"
        exit 1
    fi
done

echo "   ✅ Device roles are correct!"
echo "✅ Step 6 completed"
echo ""

# Step 7: Setup sync peers
echo "📋 Step 7: Setting up sync peers"
echo "   Setting up sync relationships between all daemons..."

# Add each daemon as a sync peer to all other daemons
for ((i=0; i<COUNT; i++)); do
    PORT_I=$((BASE_PORT + i))
    
    for ((j=0; j<COUNT; j++)); do
        if [[ $i != $j ]]; then
            LOCAL_ADDR_J="${LOCAL_ADDRS[$j]}"
            
            echo "   Adding daemon-$j as sync peer to daemon-$i..."
            
            # Create sync peer request
            SYNC_PAYLOAD=$(jq -n \
                --arg addr "$LOCAL_ADDR_J" \
                --arg team_id "$TEAM_ID" \
                '{
                    "addr": $addr,
                    "team_id": $team_id,
                    "config": {
                        "interval_secs": 5,
                        "sync_now": false
                    }
                }')
            
            if SYNC_RESPONSE=$(make_request "http://127.0.0.1:$PORT_I/api/v1/sync/peers" "POST" "$SYNC_PAYLOAD"); then
                echo "   ✅ daemon-$i ← daemon-$j"
            else
                echo "❌ Failed to add daemon-$j as sync peer to daemon-$i"
                exit 1
            fi
        fi
    done
done

echo "   ✅ All sync peer relationships established!"
echo "✅ Step 7 completed"
echo ""

echo "🎉 All tests passed!"
echo ""
echo "📊 Test Summary:"
echo "   • Created team: $TEAM_ID"
echo "   • Total devices: $COUNT"
for ((i=0; i<COUNT; i++)); do
    ROLE="Owner"
    if [[ $i -gt 0 ]]; then
        ROLE="Member"
    fi
    echo "   • Device $i ($ROLE): ${DEVICE_IDS[$i]}"
done
echo "   • Sync peers: $((COUNT * (COUNT - 1))) relationships established"
echo "   • All REST endpoints working correctly"
echo "   • Device addition workflow successful"
echo "   • Multi-daemon sync setup completed"
echo ""
echo "✅ Integration test completed successfully!"
echo ""

if [[ "$KEEP_RUNNING" == "true" ]]; then
    echo "🔄 Keeping daemons running for interactive use..."
    echo ""
    echo "┌─────────────────────────────────────────────────────────────────┐"
    echo "│                        DAEMON INFORMATION                       │"
    echo "└─────────────────────────────────────────────────────────────────┘"
    echo ""
    
    # Display daemon information in a nice table
    printf "%-12s %-12s %-15s %-s\n" "Instance" "REST Port" "Local Address" "Device ID"
    printf "%-12s %-12s %-15s %-s\n" "--------" "---------" "-------------" "---------"
    for ((i=0; i<COUNT; i++)); do
        PORT=$((BASE_PORT + i))
        printf "%-12s %-12s %-15s %-s\n" "daemon-$i" "$PORT" "${LOCAL_ADDRS[$i]}" "${DEVICE_IDS[$i]}"
    done
    echo ""
    
    echo "🌐 REST API Base URLs:"
    for ((i=0; i<COUNT; i++)); do
        PORT=$((BASE_PORT + i))
        echo "   • daemon-$i: http://127.0.0.1:$PORT/api/v1/"
    done
    echo ""
    
    echo "📋 Useful curl commands:"
    echo "   # Get daemon version"
    echo "   curl http://127.0.0.1:$BASE_PORT/api/v1/version"
    echo ""
    echo "   # List devices on the team"
    echo "   curl http://127.0.0.1:$BASE_PORT/api/v1/teams/$TEAM_ID/devices"
    echo ""
    echo "   # Get device role"
    echo "   curl http://127.0.0.1:$BASE_PORT/api/v1/teams/$TEAM_ID/devices/${DEVICE_IDS[0]}/role"
    echo ""
    echo "   # Trigger sync now"
    echo "   curl -X POST http://127.0.0.1:$BASE_PORT/api/v1/sync/now \\"
    echo "     -H 'Content-Type: application/json' \\"
    echo "     -d '{\"addr\": \"${LOCAL_ADDRS[1]}\", \"team_id\": \"$TEAM_ID\"}'"
    echo ""
    
    echo "📁 Working directory: $WORK_DIR"
    echo "   (Will be cleaned up when you exit)"
    echo ""
    echo "🛑 Press Ctrl+C to stop all daemons and clean up"
    echo ""
    
    # Disable automatic cleanup on exit for interactive mode
    trap 'echo ""; echo "🛑 Stopping daemons and cleaning up..."; cleanup; exit 0' INT TERM
    
    # Wait indefinitely
    echo "⏳ Daemons are running... (waiting for Ctrl+C)"
    while true; do
        sleep 1
    done
else
    echo "💡 Usage: $0 [daemon_count] [base_port] [keep_running]"
    echo "   Examples:"
    echo "     $0                    # 2 daemons on ports 8080-8081"
    echo "     $0 3                  # 3 daemons on ports 8080-8082"
    echo "     $0 4 9000             # 4 daemons on ports 9000-9003"
    echo "     $0 2 8080 true        # 2 daemons, keep running for interaction"
fi

# Cleanup will happen automatically via trap