# Aranya Visualization Demo

This guide walks you through a basic demonstration of the Aranya Visualization Tool.

## Prerequisites

Make sure you have built the required components:

```bash
# From the project root directory
cargo build --package aranya-rest --bin aranya-rest
cargo build --package aranya-viz-coordinator
```

## Demo Steps

### Step 1: Start the Coordinator

Open a terminal and start the visualization coordinator:

```bash
cargo run --package aranya-viz-coordinator -- --verbose
```

You should see output like:
```
INFO aranya_viz_coordinator::coordinator: Starting Aranya Visualization Coordinator on 127.0.0.1:3000
```

### Step 2: Open the Web Interface

1. Open your web browser
2. Navigate to: http://127.0.0.1:3000
3. You should see the Aranya Visualization interface with:
   - A large canvas area
   - Control buttons at the top
   - Status information at the bottom

### Step 3: Create Your First Node

1. **Click anywhere on the canvas** to place a node
2. **Enter a name** (e.g., "Alice") in the text box or leave blank for auto-naming
3. **Watch the node status**:
   - Initially: Grey (stopped)
   - Then: Orange (starting)
   - Finally: Green (running)

The status message will show: "Connected. Click to place nodes!"

### Step 4: Create Multiple Nodes

1. Click in different areas to create more nodes:
   - Name them "Bob", "Charlie", etc.
   - Space them out across the canvas
2. **Observe the port allocation**:
   - Each node gets unique daemon and REST ports
   - Check the coordinator logs to see port assignments

### Step 5: Move Nodes Around

1. **Drag nodes** by clicking and holding on them
2. **Reposition** them to create your desired layout
3. The WebSocket connection updates positions in real-time

### Step 6: Monitor Node Status

Watch the coordinator terminal for logs showing:
- Node creation
- Daemon startup process
- REST server initialization
- Port allocations

Example log output:
```
INFO aranya_viz_coordinator::daemon_manager: Starting node <uuid>
INFO aranya_viz_coordinator::daemon_manager: Node <uuid> started successfully
```

### Step 7: Test Node Management

1. **Clear all nodes**: Click the "Clear All" button
2. **Observe cleanup**: Watch logs as daemons are terminated
3. **Create new nodes**: Add fresh nodes with different names

### Step 8: Inspect Running Processes

While nodes are running, you can verify the actual daemon processes:

```bash
# Check for aranya-daemon processes
ps aux | grep aranya-daemon

# Check for aranya-rest processes  
ps aux | grep aranya-rest

# Check listening ports
netstat -tlnp | grep -E "(800[0-9]|900[0-9])"
```

## Understanding the Interface

### Node Colors
- **Grey**: Stopped/not running
- **Orange**: Starting up
- **Green**: Running successfully  
- **Red**: Error state

### Status Messages
- "Connecting to WebSocket...": Initial connection
- "Connected. Click to place nodes!": Ready to use
- "Disconnected. Attempting to reconnect...": Connection lost

### Controls
- **Clear All**: Remove all nodes and stop their daemons
- **Toggle Connections**: Show/hide sync relationships (future feature)
- **Node name input**: Set custom names for new nodes

## What's Happening Behind the Scenes

When you create a node, the coordinator:

1. **Allocates ports**: Assigns unique daemon and REST ports
2. **Creates temp directory**: Sets up isolated storage for the daemon
3. **Starts daemon**: Launches `aranya-daemon` with custom config
4. **Starts REST server**: Launches `aranya-rest` connected to the daemon
5. **Updates status**: Reports back to the web interface via WebSocket

Each node represents a fully functional Aranya daemon that could:
- Join teams with other nodes
- Sync with peer daemons
- Handle encrypted communications
- Manage roles and permissions

## Next Steps

This basic demo shows the foundation. Future enhancements will include:
- **Sync connections**: Visualize peer relationships with arrows  
- **Team management**: Create teams and assign roles
- **Network simulation**: Add latency and packet loss
- **Performance monitoring**: View real-time metrics

## Troubleshooting

**If nodes stay orange (starting):**
- Check that `aranya-rest` binary is available in target/debug/ or target/release/
- Verify ports 8000-9999 are not blocked by firewall
- Look at coordinator logs for error messages

**If WebSocket disconnects:**
- Refresh the browser page
- Check network connectivity
- Try a different browser

**If "Clear All" doesn't work:**
- Manually kill processes: `pkill -f aranya-daemon; pkill -f aranya-rest`
- Restart the coordinator

## Demonstration Script

For presentations, here's a suggested flow:

1. **Introduction** (2 min): Explain what Aranya is and the visualization goals
2. **Start coordinator** (1 min): Show the startup process
3. **Open browser** (1 min): Show the clean interface
4. **Create first node** (2 min): Demonstrate node creation and status changes
5. **Add more nodes** (2 min): Show multiple daemons running
6. **Move nodes around** (1 min): Demonstrate interactivity
7. **Show processes** (2 min): Terminal view of actual running daemons
8. **Future features** (2 min): Discuss planned sync visualization and network simulation

Total demo time: ~10-12 minutes