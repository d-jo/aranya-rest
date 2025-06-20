use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use axum::{
    response::Html,
    routing::get,
    Router,
};
use tokio::sync::{broadcast, RwLock};
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::info;

use crate::{
    daemon_manager::DaemonManager,
    websocket::websocket_handler,
    AppState, AppStateInner, WsMessage,
};

/// Main coordinator service
pub struct Coordinator {
    bind_addr: SocketAddr,
    static_dir: Option<std::path::PathBuf>,
}

impl Coordinator {
    pub fn new(bind_addr: SocketAddr, static_dir: Option<std::path::PathBuf>) -> Self {
        Self { bind_addr, static_dir }
    }

    pub async fn run(self) -> Result<()> {
        // Initialize shared state
        let state: AppState = Arc::new(RwLock::new(AppStateInner::new()));
        
        // Initialize daemon manager
        let daemon_manager = Arc::new(DaemonManager::new(state.clone()));
        
        // Create broadcast channel for WebSocket messages
        let (tx, _rx) = broadcast::channel::<WsMessage>(1000);

        // Build router
        let mut router = Router::new()
            .route("/ws", get(websocket_handler))
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive())
            .with_state((state, daemon_manager, tx));

        // Add static file serving if directory is provided
        if let Some(static_dir) = &self.static_dir {
            info!("Serving static files from: {}", static_dir.display());
            router = router.nest_service("/", ServeDir::new(static_dir));
        } else {
            // Serve basic HTML if no static directory provided
            router = router.route("/", get(serve_basic_html));
        }

        info!("Starting Aranya Visualization Coordinator on {}", self.bind_addr);

        let listener = tokio::net::TcpListener::bind(self.bind_addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }
}

async fn serve_basic_html() -> Html<&'static str> {
    Html(r#"<!DOCTYPE html>
<html>
<head>
    <title>Aranya Visualization</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 0; padding: 20px; }
        .container { max-width: 1200px; margin: 0 auto; }
        .header { text-align: center; margin-bottom: 20px; }
        .canvas-container { border: 2px solid #ccc; position: relative; margin: 20px 0; }
        #canvas { display: block; cursor: crosshair; }
        .controls { margin: 20px 0; }
        .controls button { margin: 5px; padding: 10px 15px; }
        .status { background: #f5f5f5; padding: 10px; border-radius: 5px; }
        .node { position: absolute; width: 60px; height: 60px; border-radius: 50%; 
                background: #4CAF50; border: 3px solid #333; cursor: move; 
                display: flex; align-items: center; justify-content: center; 
                color: white; font-weight: bold; }
        .node.starting { background: #FF9800; }
        .node.error { background: #F44336; }
        .node.stopped { background: #9E9E9E; }
        .context-menu {
            position: absolute;
            background: white;
            border: 1px solid #ccc;
            border-radius: 4px;
            padding: 0;
            display: none;
            z-index: 1000;
            box-shadow: 0 2px 5px rgba(0,0,0,0.2);
        }
        .context-menu-item {
            padding: 10px 20px;
            cursor: pointer;
            border-bottom: 1px solid #eee;
        }
        .context-menu-item:last-child {
            border-bottom: none;
        }
        .context-menu-item:hover {
            background: #f5f5f5;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Aranya Visualization</h1>
            <p>Click to place nodes. Drag to move them. Right-click for options.</p>
        </div>
        
        <div class="controls">
            <button onclick="clearAll()">Clear All</button>
            <button onclick="toggleConnections()">Toggle Connections</button>
            <input type="text" id="nodeNameInput" placeholder="Node name..." value="">
        </div>
        
        <div class="canvas-container">
            <canvas id="canvas" width="1000" height="600"></canvas>
        </div>
        
        <div class="status" id="status">
            Connecting to WebSocket...
        </div>
    </div>

    <div class="context-menu" id="contextMenu">
        <div class="context-menu-item" onclick="deleteSelectedNode()">Delete Node</div>
        <div class="context-menu-item" onclick="addSyncPeer()">Add Sync Peer</div>
        <div class="context-menu-item" onclick="viewNodeInfo()">View Info</div>
    </div>

    <script>
        let ws = null;
        let nodes = new Map();
        let connections = new Map();
        let canvas = document.getElementById('canvas');
        let ctx = canvas.getContext('2d');
        let dragNode = null;
        let showConnections = true;
        let contextMenuNode = null;

        function connectWebSocket() {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            ws = new WebSocket(`${protocol}//${window.location.host}/ws`);
            
            ws.onopen = function() {
                document.getElementById('status').textContent = 'Connected. Click to place nodes!';
                ws.send(JSON.stringify({ type: 'GetState' }));
            };
            
            ws.onmessage = function(event) {
                const message = JSON.parse(event.data);
                handleWebSocketMessage(message);
            };
            
            ws.onclose = function() {
                document.getElementById('status').textContent = 'Disconnected. Attempting to reconnect...';
                setTimeout(connectWebSocket, 1000);
            };
            
            ws.onerror = function(error) {
                console.error('WebSocket error:', error);
            };
        }

        function handleWebSocketMessage(message) {
            switch (message.type) {
                case 'State':
                    nodes.clear();
                    connections.clear();
                    message.nodes.forEach(node => nodes.set(node.id, node));
                    message.connections.forEach(conn => connections.set(`${conn.from}-${conn.to}`, conn));
                    draw();
                    break;
                case 'NodeCreated':
                    nodes.set(message.node.id, message.node);
                    draw();
                    break;
                case 'NodeDeleted':
                    nodes.delete(message.node_id);
                    draw();
                    break;
                case 'NodeMoved':
                    const node = nodes.get(message.node_id);
                    if (node) {
                        node.position = message.position;
                        draw();
                    }
                    break;
                case 'NodeStatusChanged':
                    const statusNode = nodes.get(message.node_id);
                    if (statusNode) {
                        statusNode.status = message.status;
                        draw();
                    }
                    break;
                case 'SyncConnectionAdded':
                    connections.set(`${message.connection.from}-${message.connection.to}`, message.connection);
                    draw();
                    break;
                case 'SyncConnectionRemoved':
                    connections.delete(`${message.from}-${message.to}`);
                    draw();
                    break;
            }
        }

        function draw() {
            ctx.clearRect(0, 0, canvas.width, canvas.height);
            
            // Draw connections
            if (showConnections) {
                connections.forEach(conn => {
                    const fromNode = nodes.get(conn.from);
                    const toNode = nodes.get(conn.to);
                    if (fromNode && toNode) {
                        drawArrow(fromNode.position, toNode.position);
                    }
                });
            }
            
            // Draw nodes
            nodes.forEach(node => {
                drawNode(node);
            });
        }

        function drawNode(node) {
            const x = node.position.x;
            const y = node.position.y;
            const radius = 30;
            
            // Set color based on status
            let color = '#9E9E9E'; // stopped
            if (node.status === 'Running') color = '#4CAF50';
            else if (node.status === 'Starting') color = '#FF9800';
            else if (typeof node.status === 'object' && node.status.Error) color = '#F44336';
            
            ctx.fillStyle = color;
            ctx.strokeStyle = '#333';
            ctx.lineWidth = 3;
            
            ctx.beginPath();
            ctx.arc(x, y, radius, 0, 2 * Math.PI);
            ctx.fill();
            ctx.stroke();
            
            // Draw name
            ctx.fillStyle = 'white';
            ctx.font = '12px Arial';
            ctx.textAlign = 'center';
            ctx.fillText(node.name, x, y + 4);
        }

        function drawArrow(from, to) {
            const headlen = 10;
            const dx = to.x - from.x;
            const dy = to.y - from.y;
            const angle = Math.atan2(dy, dx);
            
            // Adjust start and end points to node edges
            const nodeRadius = 30;
            const dist = Math.sqrt(dx * dx + dy * dy);
            const startX = from.x + (nodeRadius * dx) / dist;
            const startY = from.y + (nodeRadius * dy) / dist;
            const endX = to.x - (nodeRadius * dx) / dist;
            const endY = to.y - (nodeRadius * dy) / dist;
            
            ctx.strokeStyle = '#666';
            ctx.lineWidth = 2;
            ctx.beginPath();
            ctx.moveTo(startX, startY);
            ctx.lineTo(endX, endY);
            ctx.stroke();
            
            // Draw arrowhead
            ctx.beginPath();
            ctx.moveTo(endX, endY);
            ctx.lineTo(endX - headlen * Math.cos(angle - Math.PI / 6), endY - headlen * Math.sin(angle - Math.PI / 6));
            ctx.moveTo(endX, endY);
            ctx.lineTo(endX - headlen * Math.cos(angle + Math.PI / 6), endY - headlen * Math.sin(angle + Math.PI / 6));
            ctx.stroke();
        }

        function getNodeAt(x, y) {
            for (let [id, node] of nodes) {
                const dx = x - node.position.x;
                const dy = y - node.position.y;
                if (dx * dx + dy * dy <= 30 * 30) {
                    return id;
                }
            }
            return null;
        }

        canvas.addEventListener('click', function(e) {
            const rect = canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            
            const nodeId = getNodeAt(x, y);
            if (!nodeId) {
                const nameInput = document.getElementById('nodeNameInput');
                const name = nameInput.value.trim() || `Node${nodes.size + 1}`;
                nameInput.value = '';
                
                ws.send(JSON.stringify({
                    type: 'CreateNode',
                    name: name,
                    position: { x: x, y: y }
                }));
            }
        });

        canvas.addEventListener('mousedown', function(e) {
            const rect = canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            
            dragNode = getNodeAt(x, y);
        });

        canvas.addEventListener('mousemove', function(e) {
            if (dragNode) {
                const rect = canvas.getBoundingClientRect();
                const x = e.clientX - rect.left;
                const y = e.clientY - rect.top;
                
                ws.send(JSON.stringify({
                    type: 'MoveNode',
                    node_id: dragNode,
                    position: { x: x, y: y }
                }));
            }
        });

        canvas.addEventListener('mouseup', function() {
            dragNode = null;
        });

        // Prevent default right-click menu
        canvas.addEventListener('contextmenu', function(e) {
            e.preventDefault();
            const rect = canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            
            const nodeId = getNodeAt(x, y);
            if (nodeId) {
                contextMenuNode = nodeId;
                const menu = document.getElementById('contextMenu');
                menu.style.display = 'block';
                menu.style.left = e.clientX + 'px';
                menu.style.top = e.clientY + 'px';
            }
        });

        // Hide context menu on click elsewhere
        document.addEventListener('click', function(e) {
            const menu = document.getElementById('contextMenu');
            if (!menu.contains(e.target)) {
                menu.style.display = 'none';
                contextMenuNode = null;
            }
        });

        function clearAll() {
            nodes.forEach((node, id) => {
                ws.send(JSON.stringify({ type: 'DeleteNode', node_id: id }));
            });
        }

        function toggleConnections() {
            showConnections = !showConnections;
            draw();
        }

        function deleteSelectedNode() {
            if (contextMenuNode) {
                ws.send(JSON.stringify({ type: 'DeleteNode', node_id: contextMenuNode }));
                document.getElementById('contextMenu').style.display = 'none';
                contextMenuNode = null;
            }
        }

        function addSyncPeer() {
            if (contextMenuNode) {
                // TODO: Implement sync peer selection UI
                alert('Sync peer functionality coming soon!');
                document.getElementById('contextMenu').style.display = 'none';
            }
        }

        function viewNodeInfo() {
            if (contextMenuNode) {
                const node = nodes.get(contextMenuNode);
                if (node) {
                    alert(`Node Info:\nName: ${node.name}\nStatus: ${JSON.stringify(node.status)}\nDaemon Port: ${node.daemon_port}\nREST Port: ${node.rest_port}`);
                }
                document.getElementById('contextMenu').style.display = 'none';
            }
        }

        // Initialize
        connectWebSocket();
    </script>
</body>
</html>"#)
}