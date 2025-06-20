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
        .message-bubble {
            position: absolute;
            background: #2196F3;
            color: white;
            padding: 10px 15px;
            border-radius: 18px;
            font-size: 13px;
            font-weight: 500;
            max-width: 250px;
            word-wrap: break-word;
            pointer-events: none;
            opacity: 0;
            transform: translateY(-10px) scale(0.8);
            transition: all 0.4s cubic-bezier(0.175, 0.885, 0.32, 1.275);
            z-index: 1001;
            box-shadow: 0 4px 15px rgba(0,0,0,0.3);
            border: 2px solid rgba(255,255,255,0.3);
        }
        .message-bubble.show {
            opacity: 1;
            transform: translateY(-50px) scale(1);
        }
        .message-bubble::after {
            content: '';
            position: absolute;
            bottom: -8px;
            left: 50%;
            transform: translateX(-50%);
            width: 0;
            height: 0;
            border-left: 8px solid transparent;
            border-right: 8px solid transparent;
            border-top: 8px solid #2196F3;
        }
        .message-panel {
            position: absolute;
            left: 20px;
            top: 20px;
            width: 300px;
            background: white;
            border: 1px solid #ccc;
            border-radius: 5px;
            padding: 15px;
            box-shadow: 0 2px 5px rgba(0,0,0,0.1);
        }
        .message-input {
            width: 100%;
            padding: 8px;
            border: 1px solid #ccc;
            border-radius: 4px;
            resize: vertical;
            min-height: 60px;
        }
        .message-send-btn {
            margin-top: 10px;
            padding: 8px 15px;
            background: #4CAF50;
            color: white;
            border: none;
            border-radius: 4px;
            cursor: pointer;
        }
        .message-send-btn:hover {
            background: #45a049;
        }
        .message-send-btn:disabled {
            background: #ccc;
            cursor: not-allowed;
        }
        .tool-selector {
            margin: 10px 0;
            padding: 10px;
            background: #f5f5f5;
            border-radius: 5px;
        }
        .tool-button {
            margin: 5px;
            padding: 8px 15px;
            border: 2px solid #ccc;
            background: white;
            cursor: pointer;
            border-radius: 5px;
        }
        .tool-button.active {
            background: #4CAF50;
            color: white;
            border-color: #4CAF50;
        }
        .connection-preview {
            stroke: #666;
            stroke-width: 2;
            stroke-dasharray: 5,5;
            fill: none;
        }
        .teams-panel {
            position: absolute;
            right: 20px;
            top: 20px;
            width: 300px;
            background: white;
            border: 1px solid #ccc;
            border-radius: 5px;
            padding: 15px;
            box-shadow: 0 2px 5px rgba(0,0,0,0.1);
        }
        .team-section {
            margin-bottom: 20px;
        }
        .team-item {
            padding: 8px;
            margin: 5px 0;
            background: #f5f5f5;
            border-radius: 4px;
            cursor: pointer;
        }
        .team-item.selected {
            background: #e3f2fd;
            border: 1px solid #2196F3;
        }
        .team-members {
            font-size: 12px;
            color: #666;
            margin-top: 5px;
        }
        .no-team-selected {
            color: #999;
            font-style: italic;
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
        
        <div class="tool-selector">
            <span>Tool: </span>
            <button class="tool-button active" onclick="selectTool('select')" id="selectTool">Select/Move</button>
            <button class="tool-button" onclick="selectTool('place')" id="placeTool">Place Node</button>
            <button class="tool-button" onclick="selectTool('connect')" id="connectTool">Connect Nodes</button>
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
        <div class="context-menu-item" onclick="createTeamForNode()">Create Team</div>
        <div class="context-menu-item" onclick="joinTeamDialog()">Join Team</div>
        <div class="context-menu-item" onclick="removeSyncPeers()">Remove Sync Peers</div>
        <div class="context-menu-item" onclick="sendMessageToNode()">Send Message</div>
        <div class="context-menu-item" onclick="viewNodeInfo()">View Info</div>
    </div>
    
    <div class="message-panel">
        <h3>Send Message</h3>
        <div style="margin-bottom: 10px;">
            <label for="messageSenderSelector">Select Sender Node:</label>
            <select id="messageSenderSelector" style="width: 100%; padding: 5px; margin-top: 5px;">
                <option value="">-- Select Sender Node --</option>
            </select>
        </div>
        <div style="margin-bottom: 10px;">
            <label for="messageTeamSelector">Select Team:</label>
            <select id="messageTeamSelector" style="width: 100%; padding: 5px; margin-top: 5px;">
                <option value="">-- Select Team to Send Message --</option>
            </select>
        </div>
        <div style="margin-bottom: 10px;">
            <label for="messageInput">Message:</label>
            <textarea id="messageInput" class="message-input" placeholder="Type your message here..."></textarea>
        </div>
        <button class="message-send-btn" onclick="sendBroadcastMessage()" id="sendMessageBtn" disabled>Send Message</button>
        
        <div style="margin-top: 20px; border-top: 1px solid #eee; padding-top: 15px;">
            <h4>Recent Messages</h4>
            <div id="recentMessages" style="max-height: 200px; overflow-y: auto; font-size: 12px;">
                <p style="color: #999; font-style: italic;">No messages yet</p>
            </div>
        </div>
    </div>
    
    <div class="teams-panel">
        <h3>Team Management</h3>
        
        <div class="team-section">
            <h4>Create Team</h4>
            <div style="display: flex; gap: 10px; margin-bottom: 10px;">
                <select id="nodeSelector" style="flex: 1; padding: 5px;">
                    <option value="">-- Select Node --</option>
                </select>
                <button onclick="createTeamFromUI()" style="padding: 5px 10px;">Create Team</button>
            </div>
            <input type="text" id="teamNameInput" placeholder="Team name..." style="width: 100%; padding: 5px; margin-bottom: 10px;">
        </div>
        
        <div class="team-section">
            <h4>Join Team</h4>
            <div style="display: flex; gap: 10px; margin-bottom: 10px;">
                <select id="joinNodeSelector" style="flex: 1; padding: 5px;">
                    <option value="">-- Select Node --</option>
                </select>
                <button onclick="joinTeamFromUI()" style="padding: 5px 10px;">Join Team</button>
            </div>
            <select id="availableTeamsSelector" style="width: 100%; padding: 5px; margin-bottom: 10px;">
                <option value="">-- Select Team to Join --</option>
            </select>
        </div>
        
        <div class="team-section">
            <h4>Sync Operations</h4>
            <p class="no-team-selected" id="selectedTeamInfo">No team selected for sync operations</p>
            <select id="teamSelector" onchange="selectTeam()" style="width: 100%; padding: 5px; margin: 10px 0;">
                <option value="">-- Select Team for Sync --</option>
            </select>
        </div>
        
        <div class="team-section">
            <h4>Team Actions</h4>
            <div style="display: flex; flex-direction: column; gap: 5px; margin-bottom: 15px;">
                <button onclick="addAllNodesToSelectedTeam()" style="padding: 8px; background: #4CAF50; color: white; border: none; border-radius: 4px;">
                    Add All Nodes to Selected Team
                </button>
                <button onclick="autoSyncTeamMembers()" style="padding: 8px; background: #2196F3; color: white; border: none; border-radius: 4px;">
                    Auto-Sync All Team Members
                </button>
                <button onclick="quickTeamSetup()" style="padding: 8px; background: #FF9800; color: white; border: none; border-radius: 4px;">
                    Quick Team Setup (All Nodes)
                </button>
            </div>
        </div>
        
        <div class="team-section">
            <h4>All Teams</h4>
            <div id="teamsList"></div>
        </div>
    </div>

    <script>
        let ws = null;
        let nodes = new Map();
        let connections = new Map();
        let teams = new Map();
        let canvas = document.getElementById('canvas');
        let ctx = canvas.getContext('2d');
        let dragNode = null;
        let showConnections = true;
        let contextMenuNode = null;
        let currentTool = 'select';
        let connectStart = null;
        let mousePos = { x: 0, y: 0 };
        let selectedTeamId = null;
        let recentMessages = [];
        let messageBubbles = new Map();
        let messagePollingInterval = null;
        let lastKnownMessages = new Map(); // nodeId-teamId -> Set of messageIds

        function connectWebSocket() {
            const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            ws = new WebSocket(`${protocol}//${window.location.host}/ws`);
            
            ws.onopen = function() {
                document.getElementById('status').textContent = 'Connected. Click to place nodes!';
                ws.send(JSON.stringify({ type: 'GetState' }));
                // Start message polling after connection
                setTimeout(startMessagePolling, 2000);
            };
            
            ws.onmessage = function(event) {
                const message = JSON.parse(event.data);
                handleWebSocketMessage(message);
            };
            
            ws.onclose = function() {
                document.getElementById('status').textContent = 'Disconnected. Attempting to reconnect...';
                stopMessagePolling();
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
                    teams.clear();
                    message.nodes.forEach(node => nodes.set(node.id, node));
                    message.connections.forEach(conn => connections.set(`${conn.from}-${conn.to}-${conn.team_id}`, conn));
                    message.teams.forEach(team => teams.set(team.id, team));
                    updateTeamsUI();
                    draw();
                    break;
                case 'NodeCreated':
                    nodes.set(message.node.id, message.node);
                    updateNodeSelectors();
                    draw();
                    break;
                case 'NodeDeleted':
                    nodes.delete(message.node_id);
                    updateTeamsUI();
                    updateNodeSelectors();
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
                        updateNodeSelectors(); // Update since only running nodes are shown
                        draw();
                    }
                    break;
                case 'NodeTeamsChanged':
                    const teamsNode = nodes.get(message.node_id);
                    if (teamsNode) {
                        teamsNode.teams = message.teams;
                        updateTeamsUI();
                        draw();
                    }
                    break;
                case 'SyncConnectionAdded':
                    connections.set(`${message.connection.from}-${message.connection.to}-${message.connection.team_id}`, message.connection);
                    draw();
                    break;
                case 'SyncConnectionRemoved':
                    connections.delete(`${message.from}-${message.to}-${message.team_id}`);
                    draw();
                    break;
                case 'TeamCreated':
                    const team = message.team;
                    const creatorNode = nodes.get(message.node_id);
                    if (creatorNode) {
                        creatorNode.teams = creatorNode.teams || [];
                        creatorNode.teams.push(team);
                        teams.set(team.id, {
                            id: team.id,
                            name: team.name,
                            owner_node_id: message.node_id,
                            members: [{ node_id: message.node_id, role: 'Owner' }]
                        });
                        updateTeamsUI();
                        draw();
                        
                        // Check if this is part of a quick setup
                        if (window.pendingQuickSetup) {
                            completeQuickSetup(team.id);
                        }
                    }
                    break;
                case 'TeamJoined':
                    const joinedNode = nodes.get(message.node_id);
                    if (joinedNode) {
                        joinedNode.teams = joinedNode.teams || [];
                        joinedNode.teams.push(message.team);
                        const existingTeam = teams.get(message.team.id);
                        if (existingTeam) {
                            existingTeam.members.push({ node_id: message.node_id, role: message.team.role || 'Member' });
                        }
                        updateTeamsUI();
                        draw();
                    }
                    break;
                case 'MessageSent':
                    // Add to recent messages
                    const sentMessage = {
                        id: message.message_id,
                        author_id: message.author_id,
                        author_name: getNodeName(message.author_id),
                        text: message.text,
                        timestamp: message.timestamp,
                        team_id: message.team_id
                    };
                    recentMessages.push(sentMessage);
                    updateRecentMessages();
                    console.log(`📤 Message sent by ${sentMessage.author_name}: "${sentMessage.text}"`);
                    break;
                case 'MessageReceived':
                    // Track messages per node-team to avoid duplicates
                    const nodeTeamKey = `${message.node_id}-${message.team_id}`;
                    if (!lastKnownMessages.has(nodeTeamKey)) {
                        lastKnownMessages.set(nodeTeamKey, new Set());
                    }
                    const knownMessageIds = lastKnownMessages.get(nodeTeamKey);
                    
                    // Only process if this is a new message for this node-team
                    if (!knownMessageIds.has(message.message_id)) {
                        knownMessageIds.add(message.message_id);
                        
                        // Get author name from node ID
                        const authorName = getNodeName(message.author_id);
                        
                        const receivedMessage = {
                            id: message.message_id,
                            author_id: message.author_id,
                            author_name: authorName,
                            text: message.text,
                            timestamp: message.timestamp,
                            team_id: message.team_id
                        };
                        
                        // Add to recent messages if not already there globally
                        if (!recentMessages.some(m => m.id === receivedMessage.id)) {
                            recentMessages.push(receivedMessage);
                            updateRecentMessages();
                        }
                        
                        // Show bubble on receiving node 
                        const receivingNode = nodes.get(message.node_id);
                        if (receivingNode) {
                            console.log(`📨 Node ${receivingNode.name} received message: "${receivedMessage.text}" from ${receivedMessage.author_name}`);
                            showMessageBubble(message.node_id, receivedMessage.text, receivedMessage.author_name);
                        }
                    }
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
                        const color = conn.status === 'Connected' ? '#4CAF50' : 
                                     conn.status === 'Failed' ? '#F44336' : '#FF9800';
                        drawArrow(fromNode.position, toNode.position, color);
                    }
                });
            }
            
            // Draw connection preview when in connect mode
            if (currentTool === 'connect' && connectStart) {
                const startNode = nodes.get(connectStart);
                if (startNode) {
                    ctx.save();
                    ctx.strokeStyle = '#666';
                    ctx.lineWidth = 2;
                    ctx.setLineDash([5, 5]);
                    ctx.beginPath();
                    ctx.moveTo(startNode.position.x, startNode.position.y);
                    ctx.lineTo(mousePos.x, mousePos.y);
                    ctx.stroke();
                    ctx.restore();
                }
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
            
            // Draw team indicators (small colored dots around the node)
            if (node.teams && node.teams.length > 0) {
                const dotRadius = 4;
                const dotDistance = radius + 8;
                node.teams.forEach((team, index) => {
                    const angle = (index / node.teams.length) * 2 * Math.PI;
                    const dotX = x + Math.cos(angle) * dotDistance;
                    const dotY = y + Math.sin(angle) * dotDistance;
                    
                    // Use different colors for different roles
                    let teamColor = '#2196F3'; // Member
                    if (team.role === 'Owner') teamColor = '#FF5722';
                    else if (team.role === 'Admin') teamColor = '#FF9800';
                    else if (team.role === 'Operator') teamColor = '#9C27B0';
                    
                    ctx.fillStyle = teamColor;
                    ctx.beginPath();
                    ctx.arc(dotX, dotY, dotRadius, 0, 2 * Math.PI);
                    ctx.fill();
                    
                    // Add white border
                    ctx.strokeStyle = 'white';
                    ctx.lineWidth = 1;
                    ctx.stroke();
                });
            }
            
            // Draw name
            ctx.fillStyle = 'white';
            ctx.font = '12px Arial';
            ctx.textAlign = 'center';
            ctx.fillText(node.name, x, y + 4);
        }

        function drawArrow(from, to, color = '#666') {
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
            
            ctx.strokeStyle = color;
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
            
            if (currentTool === 'place') {
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
            } else if (currentTool === 'connect') {
                if (nodeId) {
                    if (!connectStart) {
                        // Start connection
                        connectStart = nodeId;
                    } else if (connectStart !== nodeId) {
                        // Complete connection
                        if (!selectedTeamId) {
                            alert('Please select a team for sync operations first!');
                            return;
                        }
                        ws.send(JSON.stringify({
                            type: 'AddSyncConnection',
                            from: connectStart,
                            to: nodeId,
                            team_id: selectedTeamId
                        }));
                        connectStart = null;
                    } else {
                        // Clicked same node, cancel
                        connectStart = null;
                    }
                } else {
                    // Clicked empty space, cancel
                    connectStart = null;
                }
                draw();
            }
        });

        canvas.addEventListener('mousedown', function(e) {
            const rect = canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            
            if (currentTool === 'select') {
                dragNode = getNodeAt(x, y);
            }
        });

        canvas.addEventListener('mousemove', function(e) {
            const rect = canvas.getBoundingClientRect();
            const x = e.clientX - rect.left;
            const y = e.clientY - rect.top;
            
            mousePos = { x, y };
            
            if (dragNode && currentTool === 'select') {
                ws.send(JSON.stringify({
                    type: 'MoveNode',
                    node_id: dragNode,
                    position: { x: x, y: y }
                }));
            }
            
            // Update canvas cursor
            if (currentTool === 'select') {
                canvas.style.cursor = getNodeAt(x, y) ? 'move' : 'default';
            } else if (currentTool === 'place') {
                canvas.style.cursor = 'crosshair';
            } else if (currentTool === 'connect') {
                canvas.style.cursor = getNodeAt(x, y) ? 'pointer' : 'crosshair';
            }
            
            // Redraw if we're in connect mode to show preview line
            if (currentTool === 'connect' && connectStart) {
                draw();
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

        function removeSyncPeers() {
            if (contextMenuNode) {
                // Remove all connections where this node is involved
                const toRemove = [];
                connections.forEach((conn, key) => {
                    if (conn.from === contextMenuNode || conn.to === contextMenuNode) {
                        toRemove.push({ from: conn.from, to: conn.to, team_id: conn.team_id });
                    }
                });
                
                toRemove.forEach(({ from, to, team_id }) => {
                    ws.send(JSON.stringify({ type: 'RemoveSyncConnection', from, to, team_id }));
                });
                
                document.getElementById('contextMenu').style.display = 'none';
            }
        }

        function viewNodeInfo() {
            if (contextMenuNode) {
                const node = nodes.get(contextMenuNode);
                if (node) {
                    // Count connections
                    let incomingCount = 0;
                    let outgoingCount = 0;
                    connections.forEach(conn => {
                        if (conn.to === contextMenuNode) incomingCount++;
                        if (conn.from === contextMenuNode) outgoingCount++;
                    });
                    
                    alert(`Node Info:\nName: ${node.name}\nStatus: ${JSON.stringify(node.status)}\nDaemon Port: ${node.daemon_port}\nREST Port: ${node.rest_port}\n\nConnections:\nIncoming: ${incomingCount}\nOutgoing: ${outgoingCount}`);
                }
                document.getElementById('contextMenu').style.display = 'none';
            }
        }

        function selectTool(tool) {
            currentTool = tool;
            connectStart = null; // Reset connection state
            
            // Update button states
            document.querySelectorAll('.tool-button').forEach(btn => {
                btn.classList.remove('active');
            });
            document.getElementById(tool + 'Tool').classList.add('active');
            
            // Update instructions
            const header = document.querySelector('.header p');
            if (tool === 'select') {
                header.textContent = 'Click and drag to move nodes. Right-click for options.';
            } else if (tool === 'place') {
                header.textContent = 'Click on empty space to place a new node.';
            } else if (tool === 'connect') {
                header.textContent = 'Click on source node, then click on target node to create sync connection.';
            }
            
            draw();
        }

        function updateTeamsUI() {
            // Update team selector for sync operations
            const selector = document.getElementById('teamSelector');
            const currentValue = selector.value;
            selector.innerHTML = '<option value="">-- Select Team for Sync --</option>';
            
            teams.forEach(team => {
                const option = document.createElement('option');
                option.value = team.id;
                option.textContent = team.name;
                selector.appendChild(option);
            });
            
            // Restore selection if still valid
            if (currentValue && teams.has(currentValue)) {
                selector.value = currentValue;
                selectedTeamId = currentValue;
            }
            
            // Update message team selector
            const messageSelector = document.getElementById('messageTeamSelector');
            const currentMessageValue = messageSelector.value;
            messageSelector.innerHTML = '<option value="">-- Select Team to Send Message --</option>';
            
            teams.forEach(team => {
                const option = document.createElement('option');
                option.value = team.id;
                option.textContent = team.name;
                messageSelector.appendChild(option);
            });
            
            // Restore selection if still valid
            if (currentMessageValue && teams.has(currentMessageValue)) {
                messageSelector.value = currentMessageValue;
            }
            
            // Update message sender selector
            updateMessageSenderSelector();
            
            // Update send button state
            updateSendButtonState();
            
            // Update available teams selector for joining
            const availableTeamsSelector = document.getElementById('availableTeamsSelector');
            const currentAvailableValue = availableTeamsSelector.value;
            availableTeamsSelector.innerHTML = '<option value="">-- Select Team to Join --</option>';
            
            teams.forEach(team => {
                const option = document.createElement('option');
                option.value = team.id;
                option.textContent = `${team.name} (Owner: ${getNodeName(team.owner_node_id)})`;
                availableTeamsSelector.appendChild(option);
            });
            
            // Restore selection if still valid
            if (currentAvailableValue && teams.has(currentAvailableValue)) {
                availableTeamsSelector.value = currentAvailableValue;
            }
            
            // Update node selectors
            updateNodeSelectors();
            
            // Update teams list
            const teamsList = document.getElementById('teamsList');
            teamsList.innerHTML = '';
            
            if (teams.size === 0) {
                teamsList.innerHTML = '<p style="color: #999; font-style: italic;">No teams created yet</p>';
            } else {
                teams.forEach(team => {
                    const teamDiv = document.createElement('div');
                    teamDiv.className = 'team-item';
                    
                    // Find all nodes in this team
                    const teamNodeNames = [];
                    nodes.forEach(node => {
                        if (node.teams && node.teams.some(t => t.id === team.id)) {
                            const role = node.teams.find(t => t.id === team.id)?.role || 'Member';
                            teamNodeNames.push(`${node.name} (${role})`);
                        }
                    });
                    
                    teamDiv.innerHTML = `
                        <strong>${team.name}</strong> (${team.id.substring(0, 8)}...)
                        <div class="team-members">
                            Owner: ${getNodeName(team.owner_node_id)}<br>
                            Members: ${teamNodeNames.length}<br>
                            ${teamNodeNames.length > 0 ? 
                                `<small>${teamNodeNames.join(', ')}</small>` : 
                                '<small>No members yet</small>'
                            }
                        </div>
                    `;
                    teamsList.appendChild(teamDiv);
                });
            }
            
            // Update selected team info
            updateSelectedTeamInfo();
        }

        function updateNodeSelectors() {
            // Update node selector for team creation
            const nodeSelector = document.getElementById('nodeSelector');
            const currentNodeValue = nodeSelector.value;
            nodeSelector.innerHTML = '<option value="">-- Select Node --</option>';
            
            // Update join node selector
            const joinNodeSelector = document.getElementById('joinNodeSelector');
            const currentJoinNodeValue = joinNodeSelector.value;
            joinNodeSelector.innerHTML = '<option value="">-- Select Node --</option>';
            
            nodes.forEach(node => {
                // Add to create team selector (only running nodes)
                if (node.status === 'Running') {
                    const option1 = document.createElement('option');
                    option1.value = node.id;
                    option1.textContent = `${node.name} (${node.status})`;
                    nodeSelector.appendChild(option1);
                    
                    const option2 = document.createElement('option');
                    option2.value = node.id;
                    option2.textContent = `${node.name} (${node.status})`;
                    joinNodeSelector.appendChild(option2);
                }
            });
            
            // Restore selections if still valid
            if (currentNodeValue && nodes.has(currentNodeValue)) {
                nodeSelector.value = currentNodeValue;
            }
            if (currentJoinNodeValue && nodes.has(currentJoinNodeValue)) {
                joinNodeSelector.value = currentJoinNodeValue;
            }
        }

        function selectTeam() {
            const selector = document.getElementById('teamSelector');
            selectedTeamId = selector.value;
            updateSelectedTeamInfo();
        }

        function updateSelectedTeamInfo() {
            const info = document.getElementById('selectedTeamInfo');
            if (selectedTeamId) {
                const team = teams.get(selectedTeamId);
                if (team) {
                    info.textContent = `Selected: ${team.name}`;
                    info.className = '';
                } else {
                    info.textContent = 'Selected team not found';
                    info.className = 'no-team-selected';
                }
            } else {
                info.textContent = 'No team selected for sync operations';
                info.className = 'no-team-selected';
            }
        }

        function getNodeName(nodeId) {
            const node = nodes.get(nodeId);
            return node ? node.name : 'Unknown';
        }

        function createTeamForNode() {
            if (contextMenuNode) {
                const teamName = prompt('Enter team name:');
                if (teamName && teamName.trim()) {
                    ws.send(JSON.stringify({
                        type: 'CreateTeam',
                        node_id: contextMenuNode,
                        team_name: teamName.trim()
                    }));
                }
                document.getElementById('contextMenu').style.display = 'none';
            }
        }

        function joinTeamDialog() {
            if (contextMenuNode) {
                if (teams.size === 0) {
                    alert('No teams available to join. Create a team first.');
                    return;
                }
                
                let teamOptions = '';
                teams.forEach(team => {
                    teamOptions += `${team.name} (${team.id.substring(0, 8)}...)\n`;
                });
                
                const teamId = prompt(`Available teams:\n${teamOptions}\nEnter team ID to join:`);
                if (teamId && teamId.trim()) {
                    const team = teams.get(teamId.trim());
                    if (team) {
                        ws.send(JSON.stringify({
                            type: 'JoinTeam',
                            node_id: contextMenuNode,
                            team_id: teamId.trim(),
                            owner_node_id: team.owner_node_id
                        }));
                    } else {
                        alert('Team not found!');
                    }
                }
                document.getElementById('contextMenu').style.display = 'none';
            }
        }

        function createTeamFromUI() {
            const nodeSelector = document.getElementById('nodeSelector');
            const teamNameInput = document.getElementById('teamNameInput');
            
            const nodeId = nodeSelector.value;
            const teamName = teamNameInput.value.trim();
            
            if (!nodeId) {
                alert('Please select a node to create the team.');
                return;
            }
            
            if (!teamName) {
                alert('Please enter a team name.');
                return;
            }
            
            ws.send(JSON.stringify({
                type: 'CreateTeam',
                node_id: nodeId,
                team_name: teamName
            }));
            
            // Clear the input
            teamNameInput.value = '';
        }

        function joinTeamFromUI() {
            const joinNodeSelector = document.getElementById('joinNodeSelector');
            const availableTeamsSelector = document.getElementById('availableTeamsSelector');
            
            const nodeId = joinNodeSelector.value;
            const teamId = availableTeamsSelector.value;
            
            if (!nodeId) {
                alert('Please select a node to join the team.');
                return;
            }
            
            if (!teamId) {
                alert('Please select a team to join.');
                return;
            }
            
            const team = teams.get(teamId);
            if (!team) {
                alert('Selected team not found.');
                return;
            }
            
            ws.send(JSON.stringify({
                type: 'JoinTeam',
                node_id: nodeId,
                team_id: teamId,
                owner_node_id: team.owner_node_id
            }));
        }

        function addAllNodesToSelectedTeam() {
            if (!selectedTeamId) {
                alert('Please select a team first from the Sync Operations dropdown.');
                return;
            }

            const team = teams.get(selectedTeamId);
            if (!team) {
                alert('Selected team not found.');
                return;
            }

            // Find nodes that are running but not already in this team
            const nodesToAdd = [];
            nodes.forEach(node => {
                if (node.status === 'Running') {
                    // Check if node is already in this team
                    const alreadyInTeam = node.teams && node.teams.some(t => t.id === selectedTeamId);
                    if (!alreadyInTeam) {
                        nodesToAdd.push(node);
                    }
                }
            });

            if (nodesToAdd.length === 0) {
                alert('All running nodes are already in this team.');
                return;
            }

            if (confirm(`Add ${nodesToAdd.length} nodes to team "${team.name}"?`)) {
                nodesToAdd.forEach(node => {
                    ws.send(JSON.stringify({
                        type: 'JoinTeam',
                        node_id: node.id,
                        team_id: selectedTeamId,
                        owner_node_id: team.owner_node_id
                    }));
                });
            }
        }

        function autoSyncTeamMembers() {
            if (!selectedTeamId) {
                alert('Please select a team first from the Sync Operations dropdown.');
                return;
            }

            const team = teams.get(selectedTeamId);
            if (!team) {
                alert('Selected team not found.');
                return;
            }

            // Find all nodes in this team
            const teamNodes = [];
            nodes.forEach(node => {
                if (node.teams && node.teams.some(t => t.id === selectedTeamId)) {
                    teamNodes.push(node);
                }
            });

            if (teamNodes.length < 2) {
                alert('Need at least 2 nodes in the team to create sync connections.');
                return;
            }

            let connectionCount = 0;
            const connectionsToCreate = [];

            // Create bidirectional sync connections between all team members
            for (let i = 0; i < teamNodes.length; i++) {
                for (let j = 0; j < teamNodes.length; j++) {
                    if (i !== j) {
                        const from = teamNodes[i].id;
                        const to = teamNodes[j].id;
                        
                        // Check if connection already exists
                        const connectionKey = `${from}-${to}-${selectedTeamId}`;
                        if (!connections.has(connectionKey)) {
                            connectionsToCreate.push({ from, to });
                            connectionCount++;
                        }
                    }
                }
            }

            if (connectionCount === 0) {
                alert('All team members are already fully synced.');
                return;
            }

            if (confirm(`Create ${connectionCount} sync connections between all team members?`)) {
                connectionsToCreate.forEach(({ from, to }) => {
                    ws.send(JSON.stringify({
                        type: 'AddSyncConnection',
                        from: from,
                        to: to,
                        team_id: selectedTeamId
                    }));
                });
            }
        }

        function quickTeamSetup() {
            const runningNodes = [];
            nodes.forEach(node => {
                if (node.status === 'Running') {
                    runningNodes.push(node);
                }
            });

            if (runningNodes.length === 0) {
                alert('No running nodes available. Place and start some nodes first.');
                return;
            }

            const teamName = prompt('Enter team name for quick setup:');
            if (!teamName || !teamName.trim()) {
                return;
            }

            if (confirm(`Quick setup will:\n1. Create team "${teamName}" with ${runningNodes[0].name} as owner\n2. Add all ${runningNodes.length} nodes to the team\n3. Create full mesh sync connections\n\nProceed?`)) {
                // Step 1: Create team with first node as owner
                const ownerNode = runningNodes[0];
                ws.send(JSON.stringify({
                    type: 'CreateTeam',
                    node_id: ownerNode.id,
                    team_name: teamName.trim()
                }));

                // Store the setup for completion after team is created
                window.pendingQuickSetup = {
                    teamName: teamName.trim(),
                    ownerNodeId: ownerNode.id,
                    allNodes: runningNodes
                };
            }
        }

        // Helper function to complete quick setup after team creation
        function completeQuickSetup(teamId) {
            const setup = window.pendingQuickSetup;
            if (!setup) return;

            // Step 2: Add remaining nodes to team
            const nodesToAdd = setup.allNodes.filter(node => node.id !== setup.ownerNodeId);
            nodesToAdd.forEach(node => {
                setTimeout(() => {
                    ws.send(JSON.stringify({
                        type: 'JoinTeam',
                        node_id: node.id,
                        team_id: teamId,
                        owner_node_id: setup.ownerNodeId
                    }));
                }, 500); // Small delay between requests
            });

            // Step 3: Create sync connections (with delay to let joins complete)
            setTimeout(() => {
                // Set the team as selected
                selectedTeamId = teamId;
                document.getElementById('teamSelector').value = teamId;
                updateSelectedTeamInfo();

                // Auto-sync all members
                setTimeout(() => {
                    autoSyncTeamMembers();
                }, 2000); // Wait for joins to complete
            }, 1000);

            // Clear pending setup
            delete window.pendingQuickSetup;
        }

        // Messaging functions
        function updateSendButtonState() {
            const messageSenderSelector = document.getElementById('messageSenderSelector');
            const messageTeamSelector = document.getElementById('messageTeamSelector');
            const messageInput = document.getElementById('messageInput');
            const sendBtn = document.getElementById('sendMessageBtn');
            
            const hasSender = messageSenderSelector.value !== '';
            const hasTeam = messageTeamSelector.value !== '';
            const hasMessage = messageInput.value.trim() !== '';
            
            sendBtn.disabled = !hasSender || !hasTeam || !hasMessage;
        }

        function updateMessageSenderSelector() {
            const senderSelector = document.getElementById('messageSenderSelector');
            const teamSelector = document.getElementById('messageTeamSelector');
            const currentSenderValue = senderSelector.value;
            
            senderSelector.innerHTML = '<option value="">-- Select Sender Node --</option>';
            
            const selectedTeamId = teamSelector.value;
            if (selectedTeamId) {
                // Show only nodes that are in the selected team and running
                nodes.forEach(node => {
                    if (node.status === 'Running' && 
                        node.teams && 
                        node.teams.some(t => t.id === selectedTeamId)) {
                        const option = document.createElement('option');
                        option.value = node.id;
                        option.textContent = `${node.name} (${node.teams.find(t => t.id === selectedTeamId)?.role || 'Member'})`;
                        senderSelector.appendChild(option);
                    }
                });
            }
            
            // Restore selection if still valid
            if (currentSenderValue && senderSelector.querySelector(`option[value="${currentSenderValue}"]`)) {
                senderSelector.value = currentSenderValue;
            }
            
            updateSendButtonState();
        }

        function sendMessageToNode() {
            if (!contextMenuNode) return;
            
            const node = nodes.get(contextMenuNode);
            if (!node || !node.teams || node.teams.length === 0) {
                alert('This node is not part of any team. Join a team first to send messages.');
                document.getElementById('contextMenu').style.display = 'none';
                return;
            }
            
            // Show available teams for this node
            let teamOptions = 'Available teams for this node:\n';
            node.teams.forEach(team => {
                teamOptions += `- ${team.name} (Role: ${team.role})\n`;
            });
            
            const message = prompt(teamOptions + '\nEnter your message:');
            if (message && message.trim()) {
                // Use the first team for simplicity
                const teamId = node.teams[0].id;
                sendMessageViaNode(contextMenuNode, teamId, message.trim());
            }
            
            document.getElementById('contextMenu').style.display = 'none';
        }

        function sendBroadcastMessage() {
            const messageSenderSelector = document.getElementById('messageSenderSelector');
            const messageTeamSelector = document.getElementById('messageTeamSelector');
            const messageInput = document.getElementById('messageInput');
            
            const senderNodeId = messageSenderSelector.value;
            const teamId = messageTeamSelector.value;
            const message = messageInput.value.trim();
            
            if (!senderNodeId || !teamId || !message) {
                return;
            }
            
            sendMessageViaNode(senderNodeId, teamId, message);
            messageInput.value = '';
            updateSendButtonState();
        }

        function sendMessageViaNode(nodeId, teamId, message) {
            ws.send(JSON.stringify({
                type: 'SendMessage',
                node_id: nodeId,
                team_id: teamId,
                message: message
            }));
        }

        function showMessageBubble(nodeId, message, authorName) {
            const node = nodes.get(nodeId);
            if (!node) return;
            
            // Create bubble element
            const bubble = document.createElement('div');
            bubble.className = 'message-bubble';
            bubble.innerHTML = `<strong>${authorName}:</strong> ${message}`;
            
            // Position bubble above node
            const rect = canvas.getBoundingClientRect();
            bubble.style.left = (rect.left + node.position.x - 100) + 'px';
            bubble.style.top = (rect.top + node.position.y - 60) + 'px';
            
            document.body.appendChild(bubble);
            
            // Animate in
            setTimeout(() => {
                bubble.classList.add('show');
            }, 100);
            
            // Remove after 6 seconds (longer duration for easier visibility)
            setTimeout(() => {
                bubble.classList.remove('show');
                setTimeout(() => {
                    document.body.removeChild(bubble);
                }, 400);
            }, 6000);
        }

        function updateRecentMessages() {
            const container = document.getElementById('recentMessages');
            
            if (recentMessages.length === 0) {
                container.innerHTML = '<p style="color: #999; font-style: italic;">No messages yet</p>';
                return;
            }
            
            let html = '';
            recentMessages.slice(-10).forEach(msg => {
                const time = new Date(msg.timestamp * 1000).toLocaleTimeString();
                html += `
                    <div style="margin-bottom: 8px; padding: 6px; background: #f9f9f9; border-radius: 4px;">
                        <div style="font-weight: bold; color: #333;">${msg.author_name} (${time})</div>
                        <div style="color: #666; margin-top: 2px;">${msg.text}</div>
                    </div>
                `;
            });
            
            container.innerHTML = html;
            container.scrollTop = container.scrollHeight;
        }

        // Message polling functions
        function startMessagePolling() {
            if (messagePollingInterval) {
                clearInterval(messagePollingInterval);
            }
            
            // Poll every 250ms for real-time message display (much faster than 1s daemon sync)
            messagePollingInterval = setInterval(pollForNewMessages, 250);
            
            // Also poll immediately
            setTimeout(pollForNewMessages, 1000);
        }

        function stopMessagePolling() {
            if (messagePollingInterval) {
                clearInterval(messagePollingInterval);
                messagePollingInterval = null;
            }
        }

        function pollForNewMessages() {
            // Get all teams and their member nodes
            const teamNodeMap = new Map();
            let totalPolls = 0;
            
            teams.forEach(team => {
                const teamNodes = [];
                nodes.forEach(node => {
                    if (node.status === 'Running' && 
                        node.teams && 
                        node.teams.some(t => t.id === team.id)) {
                        teamNodes.push(node.id);
                    }
                });
                if (teamNodes.length > 0) {
                    teamNodeMap.set(team.id, teamNodes);
                }
            });

            // Poll messages for each node in each team
            teamNodeMap.forEach((nodeIds, teamId) => {
                nodeIds.forEach(nodeId => {
                    pollMessagesForNode(nodeId, teamId);
                    totalPolls++;
                });
            });
            
            // Debug: Log polling activity periodically
            if (totalPolls > 0 && Math.random() < 0.05) { // Log ~5% of the time to avoid spam
                console.log(`Polling ${totalPolls} node-team combinations for new messages`);
            }
        }

        function pollMessagesForNode(nodeId, teamId) {
            ws.send(JSON.stringify({
                type: 'PollMessages',
                node_id: nodeId,
                team_id: teamId
            }));
        }

        // Event listeners for message panel
        document.getElementById('messageSenderSelector').addEventListener('change', updateSendButtonState);
        document.getElementById('messageTeamSelector').addEventListener('change', function() {
            updateMessageSenderSelector();
            updateSendButtonState();
        });
        document.getElementById('messageInput').addEventListener('input', updateSendButtonState);
        document.getElementById('messageInput').addEventListener('keypress', function(e) {
            if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                sendBroadcastMessage();
            }
        });

        // Initialize
        connectWebSocket();
    </script>
</body>
</html>"#)
}