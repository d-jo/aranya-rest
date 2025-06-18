# Multi-Daemon Testing Scripts

This directory contains tools to easily spin up multiple Aranya daemon instances with REST servers for testing and development.

## Quick Start

### Full Integration Test (Easiest)

Run the complete test including daemon setup, device addition, and cleanup:

```bash
# Run full integration test - starts daemons, tests functionality, cleans up
./scripts/full-test.bash
```

### Manual Daemon Setup

For manual testing or development:

```bash
# Setup 2 daemons (default) with REST servers on ports 8080-8081
./scripts/setup-multi-daemon.bash

# Setup 3 daemons with REST servers on ports 9000-9002
./scripts/setup-multi-daemon.bash 3 9000

# Setup 5 daemons with default ports
./scripts/setup-multi-daemon.bash 5
```

### Using the Rust Binary

```bash
# Build the multi-daemon tool
cargo build --bin multi-daemon

# Setup 2 daemons (default)
./target/debug/multi-daemon

# Setup 3 daemons starting from port 9000
./target/debug/multi-daemon --count 3 --port 9000

# Keep the tool running (prevents auto-cleanup)
./target/debug/multi-daemon --keep-running
```

## Prerequisites

Before running these scripts, make sure you have built the required binaries:

```bash
# Build the daemon
cargo build --bin aranya-daemon

# Build the REST server
cargo build --bin aranya-rest

# Optional: Build in release mode for better performance
cargo build --release --bin aranya-daemon --bin aranya-rest
```

## Available Scripts

### `full-test.bash` - Complete Integration Test
- Automatically starts 2 daemon instances
- Tests all REST API functionality
- Demonstrates device addition workflow
- Verifies team management and device roles
- Automatically cleans up when finished
- **Best for**: Quick validation and CI/CD

### `setup-multi-daemon.bash` - Manual Daemon Setup
- Starts multiple daemon instances with REST servers
- Keeps them running for manual testing
- Provides connection information and examples
- Requires manual cleanup (Ctrl+C)
- **Best for**: Development and interactive testing

### `demo-simple.bash` / `demo-fixed.bash` - Simple Examples
- Basic device addition examples
- Requires daemons to already be running
- **Best for**: Learning the API calls

## What the Scripts Do

1. **Create temporary directories** for each daemon instance
2. **Generate daemon configurations** with unique directories and ports
3. **Start daemon processes** with proper isolation
4. **Wait for daemons to initialize** and create their UDS sockets and API keys
5. **Start REST servers** that connect to each daemon
6. **Display connection information** in a nice table format
7. **Provide example commands** to test the setup
8. **Handle cleanup** when interrupted (Ctrl+C)

## Example Output

```
🚀 Setting up 2 Aranya daemon instances with REST servers
   Base port: 8080 (each daemon gets port + index)

🎉 All daemon instances are running!

┌─────────────┬──────────────┬─────────────────────────────────────────┐
│ Instance    │ REST Port    │ UDS Socket                              │
├─────────────┼──────────────┼─────────────────────────────────────────┤
│ daemon-0    │ 8080         │ /tmp/aranya-multi-abc123/daemon-0/...   │
│ daemon-1    │ 8081         │ /tmp/aranya-multi-abc123/daemon-1/...   │
└─────────────┴──────────────┴─────────────────────────────────────────┘

📋 REST API endpoints:
  • daemon-0: http://127.0.0.1:8080/api/v1/
  • daemon-1: http://127.0.0.1:8081/api/v1/

🔧 Example curl commands:
  curl http://127.0.0.1:8080/api/v1/version
  curl http://127.0.0.1:8080/api/v1/device-id
  curl -X POST http://127.0.0.1:8080/api/v1/teams \
    -H 'Content-Type: application/json' -d '{"config": {}}'
```

## Testing Multi-Device Scenarios

Once you have multiple daemons running, you can test multi-device scenarios:

### 1. Create a team on one daemon
```bash
TEAM_ID=$(curl -s -X POST http://127.0.0.1:8080/api/v1/teams \
  -H 'Content-Type: application/json' \
  -d '{"config": {}}' | jq -r '.team_id')
echo "Created team: $TEAM_ID"
```

### 2. Get device info from both daemons
```bash
# Device 0 (team owner)
DEVICE_0=$(curl -s http://127.0.0.1:8080/api/v1/device-id | jq -r '.device_id')
KEYS_0=$(curl -s http://127.0.0.1:8080/api/v1/key-bundle)

# Device 1 (to be added to team)
DEVICE_1=$(curl -s http://127.0.0.1:8081/api/v1/device-id | jq -r '.device_id')
KEYS_1=$(curl -s http://127.0.0.1:8081/api/v1/key-bundle)
```

### 3. Add device 1 to the team
```bash
curl -X POST "http://127.0.0.1:8080/api/v1/teams/$TEAM_ID/devices" \
  -H 'Content-Type: application/json' \
  -d "{\"keys\": $KEYS_1}"
```

### 4. Setup sync between devices
```bash
# Get daemon addresses for syncing
ADDR_0=$(curl -s http://127.0.0.1:8080/api/v1/local-addr | jq -r '.address')
ADDR_1=$(curl -s http://127.0.0.1:8081/api/v1/local-addr | jq -r '.address')

# Add sync peers
curl -X POST http://127.0.0.1:8080/api/v1/sync/peers \
  -H 'Content-Type: application/json' \
  -d "{\"addr\": \"$ADDR_1\", \"team_id\": \"$TEAM_ID\", \"config\": {\"interval_secs\": 1, \"sync_now\": true}}"

curl -X POST http://127.0.0.1:8081/api/v1/sync/peers \
  -H 'Content-Type: application/json' \
  -d "{\"addr\": \"$ADDR_0\", \"team_id\": \"$TEAM_ID\", \"config\": {\"interval_secs\": 1, \"sync_now\": true}}"
```

## Script Options

### Bash Script (`setup-multi-daemon.bash`)

- **Argument 1**: Number of daemons (default: 2)
- **Argument 2**: Base port (default: 8080)
- **Auto-cleanup**: Always cleans up on exit/interrupt
- **Auto-discovery**: Finds binaries in target/debug or target/release

### Rust Binary (`multi-daemon`)

- `--count, -c`: Number of daemon instances (default: 2)
- `--port, -p`: Base port for REST servers (default: 8080)
- `--daemon-path`: Path to aranya-daemon binary (auto-detected if not specified)
- `--rest-path`: Path to aranya-rest binary (auto-detected if not specified)
- `--keep-running, -k`: Keep running after setup (default: false)

## Troubleshooting

### "Could not find binary" errors
Make sure you've built the required binaries:
```bash
cargo build --bin aranya-daemon --bin aranya-rest
```

### Port already in use
Try a different base port:
```bash
./scripts/setup-multi-daemon.bash 2 9000
```

### Daemon fails to start
Check that you have proper permissions and disk space. The script creates temporary directories in `/tmp`.

### Connection refused
Wait a moment after the script reports "ready" - the REST servers may need a few seconds to fully start up.

## Cleanup

- **Bash script**: Automatically cleans up when you press Ctrl+C
- **Rust binary**: 
  - With `--keep-running`: Cleans up on Ctrl+C
  - Without `--keep-running`: You need to manually kill the processes

To manually find and kill processes:
```bash
# Find aranya processes
ps aux | grep aranya

# Kill all aranya processes (be careful!)
pkill -f aranya-daemon
pkill -f aranya-rest
```