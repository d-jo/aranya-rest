# Aranya Visualization Coordinator

An interactive web-based visualization tool for Aranya daemon networks. This tool allows you to create, manage, and visualize networks of Aranya daemons in real-time.

## Features

- **Interactive Canvas**: Click to place nodes, drag to move them around
- **Real Daemon Instances**: Each node represents a real `aranya-daemon` process with its own REST API
- **Sync Visualization**: See sync relationships between daemons with directional arrows
- **Real-time Updates**: WebSocket-based communication for live status updates
- **Node Management**: Start, stop, and monitor daemon processes
- **Team Operations**: Create teams and manage device relationships (planned)

## Quick Start

1. **Build the Required Components**:
   ```bash
   # Build the REST server (required dependency)
   cargo build --package aranya-rest --bin aranya-rest
   
   # Build the visualization coordinator
   cargo build --package aranya-viz-coordinator
   ```

2. **Run the Coordinator**:
   ```bash
   cargo run --package aranya-viz-coordinator -- --bind-addr 127.0.0.1:3000
   ```

3. **Open Your Browser**:
   Navigate to http://127.0.0.1:3000

4. **Start Creating Nodes**:
   - Click on the canvas to place a new node
   - Enter a name for the node (or leave blank for auto-naming)
   - The node will automatically start its daemon and REST API
   - Drag nodes around to reposition them

## Architecture

```
┌─────────────────┐    WebSocket    ┌──────────────────────┐
│   Web Browser   │ ◄──────────────► │   Coordinator        │
│                 │     HTTP        │   (Port 3000)        │
└─────────────────┘                 └──────────────────────┘
                                              │
                                              │ Manages
                                              ▼
                                    ┌──────────────────┐
                                    │   Node 1         │
                                    │ ┌─────────────┐  │
                                    │ │ Daemon      │  │
                                    │ │ (Port 8000) │  │
                                    │ └─────────────┘  │
                                    │ ┌─────────────┐  │
                                    │ │ REST API    │  │
                                    │ │ (Port 9000) │  │
                                    │ └─────────────┘  │
                                    └──────────────────┘
```

### Components

- **Coordinator**: Central service that manages the web interface and daemon lifecycle
- **Node**: Each canvas node represents:
  - An `aranya-daemon` process with unique configuration
  - An `aranya-rest` server providing REST API access
  - Temporary storage directory for daemon state
- **Web Interface**: Interactive HTML5 canvas with WebSocket communication

## Port Allocation

- **Coordinator**: Default port 3000 (configurable)
- **Daemons**: Ports 8000-8999 (auto-allocated)
- **REST APIs**: Ports 9000-9999 (auto-allocated)

## Usage Examples

### Creating a Simple Network

1. Start the coordinator
2. Click to place 3 nodes: "Alice", "Bob", "Charlie"
3. Wait for all nodes to show green (running status)
4. Each node now has its own daemon and REST API running

### Adding Sync Relationships (Planned)

- Right-click on a node to see connection options
- Select "Add Sync Peer" and choose target node
- Arrow will appear showing sync relationship
- Real sync configuration will be applied to daemons

## Command Line Options

```bash
cargo run --package aranya-viz-coordinator -- [OPTIONS]
```

### Options

- `--bind-addr <ADDR>`: Address to bind the coordinator server (default: 127.0.0.1:3000)
- `--static-dir <DIR>`: Directory containing static web assets (optional)
- `--verbose`: Enable verbose logging

### Examples

```bash
# Run on custom port with verbose logging
cargo run --package aranya-viz-coordinator -- --bind-addr 0.0.0.0:8080 --verbose

# Use custom static files directory
cargo run --package aranya-viz-coordinator -- --static-dir ./web-assets
```

## Development Status

### ✅ Completed
- Interactive canvas with node placement
- Real daemon and REST server spawning
- WebSocket communication
- Node lifecycle management
- Basic visualization interface

### 🚧 In Progress
- Sync relationship configuration
- Team and device management
- Error handling and recovery

### 📋 Planned
- Network simulation (latency, packet loss)
- Advanced team operations
- Performance metrics visualization
- Saved network configurations
- Multi-coordinator clustering

## Technical Details

### Node States

- **Stopped**: Grey circle, daemon not running
- **Starting**: Orange circle, daemon is starting up
- **Running**: Green circle, daemon and REST API active
- **Error**: Red circle, startup or runtime error occurred

### WebSocket Messages

The coordinator uses JSON messages over WebSocket for real-time communication:

```json
// Create a new node
{
  "type": "CreateNode",
  "name": "Alice",
  "position": { "x": 100, "y": 200 }
}

// Node status update
{
  "type": "NodeStatusChanged",
  "node_id": "uuid-here",
  "status": "Running"
}
```

### Storage

Each daemon uses temporary directories that are automatically cleaned up:
- `/tmp/aranya-viz-{uuid}/state/` - Daemon state
- `/tmp/aranya-viz-{uuid}/config/` - Configuration files
- `/tmp/aranya-viz-{uuid}/logs/` - Log files

## Troubleshooting

### "Port already in use" errors
- Stop any existing aranya processes
- Use `--bind-addr` with a different port
- Check that ports 8000-9999 range is available

### Nodes stuck in "Starting" state
- Check coordinator logs with `--verbose`
- Ensure `aranya-rest` binary is built and available
- Verify no firewall blocking local connections

### WebSocket connection failures
- Ensure no proxy or firewall blocking WebSocket connections
- Try different browser or disable browser extensions
- Check browser console for JavaScript errors

## Contributing

This tool is part of the Aranya project and follows the same contribution guidelines. See the main project README for details.

## Future Enhancements

- **Network Simulation**: Add configurable latency and packet loss between nodes
- **Visual Enhancements**: Better graphics, animations, status indicators
- **Saved Topologies**: Save and load network configurations
- **Performance Dashboard**: Real-time metrics and monitoring
- **Multi-Team Support**: Visualize complex team hierarchies
- **Export/Import**: Network topology export for documentation