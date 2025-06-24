use std::sync::Arc;

use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::{
    daemon_manager::DaemonManager,
    AppState, Node, NodeStatus, SyncConnection, WsMessage,
};

/// WebSocket upgrade handler
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State((state, daemon_manager, tx)): State<(AppState, Arc<DaemonManager>, broadcast::Sender<WsMessage>)>,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state, daemon_manager, tx))
}

#[instrument(skip_all)]
async fn handle_socket(
    socket: WebSocket,
    state: AppState,
    daemon_manager: Arc<DaemonManager>,
    tx: broadcast::Sender<WsMessage>,
) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = tx.subscribe();

    info!("WebSocket connection established");

    // Spawn task to handle outgoing messages
    let sender_task = tokio::spawn(async move {
        while let Ok(message) = rx.recv().await {
            let message_json = match serde_json::to_string(&message) {
                Ok(json) => json,
                Err(e) => {
                    error!("Failed to serialize message: {}", e);
                    continue;
                }
            };

            if let Err(e) = sender.send(axum::extract::ws::Message::Text(message_json.into())).await {
                warn!("Failed to send WebSocket message: {}", e);
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(message) = receiver.next().await {
        let message = match message {
            Ok(axum::extract::ws::Message::Text(text)) => {
                match serde_json::from_str::<WsMessage>(&text) {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!("Failed to parse WebSocket message: {}", e);
                        continue;
                    }
                }
            }
            Ok(axum::extract::ws::Message::Close(_)) => {
                info!("WebSocket connection closed");
                break;
            }
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
            _ => continue,
        };

        if let Err(e) = handle_message(message, &state, &daemon_manager, &tx).await {
            error!("Error handling WebSocket message: {}", e);
        }
    }

    sender_task.abort();
    info!("WebSocket connection closed");
}

#[instrument(skip_all)]
async fn handle_message(
    message: WsMessage,
    state: &AppState,
    daemon_manager: &Arc<DaemonManager>,
    tx: &broadcast::Sender<WsMessage>,
) -> anyhow::Result<()> {
    match message {
        WsMessage::CreateNode { name, position } => {
            let node_id = Uuid::new_v4();
            
            let node = {
                let mut state_guard = state.write().await;
                let (daemon_port, rest_port) = state_guard.port_allocator.allocate_ports()?;
                
                let node = Node {
                    id: node_id,
                    name: name.clone(),
                    position,
                    daemon_port,
                    rest_port,
                    status: NodeStatus::Stopped,
                    teams: Vec::new(),
                };
                
                state_guard.nodes.insert(node_id, node.clone());
                node
            };

            // Broadcast node creation
            let _ = tx.send(WsMessage::NodeCreated { node: node.clone() });

            // Start the node
            tokio::spawn({
                let daemon_manager = daemon_manager.clone();
                let tx = tx.clone();
                async move {
                    if let Err(e) = daemon_manager.start_node(node_id).await {
                        error!("Failed to start node {}: {}", node_id, e);
                        let _ = tx.send(WsMessage::NodeStatusChanged {
                            node_id,
                            status: NodeStatus::Error(e.to_string()),
                        });
                    } else {
                        let _ = tx.send(WsMessage::NodeStatusChanged {
                            node_id,
                            status: NodeStatus::Running,
                        });
                    }
                }
            });
        }

        WsMessage::DeleteNode { node_id } => {
            // Stop the node first
            if let Err(e) = daemon_manager.stop_node(node_id).await {
                warn!("Failed to stop node {}: {}", node_id, e);
            }

            // Remove from state
            {
                let mut state_guard = state.write().await;
                state_guard.nodes.remove(&node_id);
                
                // Remove all connections involving this node
                let connections_to_remove: Vec<_> = state_guard.connections.keys()
                    .filter(|(from, to, _)| *from == node_id || *to == node_id)
                    .cloned()
                    .collect();
                
                for (from, to, team_id) in connections_to_remove {
                    state_guard.connections.remove(&(from, to, team_id.clone()));
                    let _ = tx.send(WsMessage::SyncConnectionRemoved { from, to, team_id });
                }
            }

            let _ = tx.send(WsMessage::NodeDeleted { node_id });
        }

        WsMessage::MoveNode { node_id, position } => {
            {
                let mut state_guard = state.write().await;
                if let Some(node) = state_guard.nodes.get_mut(&node_id) {
                    node.position = position.clone();
                }
            }

            let _ = tx.send(WsMessage::NodeMoved { node_id, position });
        }

        WsMessage::AddSyncConnection { from, to, team_id, interval_secs, sync_now } => {
            let connection = SyncConnection {
                from,
                to,
                team_id: team_id.clone(),
                status: crate::ConnectionStatus::Pending,
            };

            {
                let mut state_guard = state.write().await;
                state_guard.connections.insert((from, to, team_id.clone()), connection.clone());
            }

            let _ = tx.send(WsMessage::SyncConnectionAdded { connection: connection.clone() });

            // Configure the actual sync connection
            tokio::spawn({
                let daemon_manager = daemon_manager.clone();
                let state = state.clone();
                let tx = tx.clone();
                async move {
                    match daemon_manager.configure_sync_peer(from, to, &team_id, interval_secs, sync_now).await {
                        Ok(_) => {
                            // Update connection status
                            {
                                let mut state_guard = state.write().await;
                                if let Some(conn) = state_guard.connections.get_mut(&(from, to, team_id.clone())) {
                                    conn.status = crate::ConnectionStatus::Connected;
                                }
                            }
                            let _ = tx.send(WsMessage::SyncConnectionStatusChanged {
                                from,
                                to,
                                team_id: team_id.clone(),
                                status: crate::ConnectionStatus::Connected,
                            });
                        }
                        Err(e) => {
                            error!("Failed to configure sync connection from {} to {}: {}", from, to, e);
                            // Update connection status
                            {
                                let mut state_guard = state.write().await;
                                if let Some(conn) = state_guard.connections.get_mut(&(from, to, team_id.clone())) {
                                    conn.status = crate::ConnectionStatus::Failed(e.to_string());
                                }
                            }
                            let _ = tx.send(WsMessage::SyncConnectionStatusChanged {
                                from,
                                to,
                                team_id: team_id.clone(),
                                status: crate::ConnectionStatus::Failed(e.to_string()),
                            });
                        }
                    }
                }
            });
        }

        WsMessage::RemoveSyncConnection { from, to, team_id } => {
            {
                let mut state_guard = state.write().await;
                state_guard.connections.remove(&(from, to, team_id.clone()));
            }

            let _ = tx.send(WsMessage::SyncConnectionRemoved { from, to, team_id: team_id.clone() });

            // Remove the sync connection from the daemon
            tokio::spawn({
                let daemon_manager = daemon_manager.clone();
                async move {
                    if let Err(e) = daemon_manager.remove_sync_peer(from, to, &team_id).await {
                        warn!("Failed to remove sync connection from {} to {}: {}", from, to, e);
                    }
                }
            });
        }

        WsMessage::CreateTeam { node_id, team_name } => {
            match daemon_manager.create_team(node_id, &team_name).await {
                Ok(team_id) => {
                    let team = crate::TeamInfo {
                        id: team_id.clone(),
                        name: team_name.clone(),
                        role: Some("Owner".to_string()),
                    };
                    
                    // Update node's teams
                    {
                        let mut state_guard = state.write().await;
                        if let Some(node) = state_guard.nodes.get_mut(&node_id) {
                            node.teams.push(team.clone());
                        }
                        
                        // Add to global teams list
                        state_guard.teams.insert(team_id.clone(), crate::Team {
                            id: team_id.clone(),
                            name: team_name.clone(),
                            owner_node_id: node_id,
                            members: vec![crate::TeamMember {
                                node_id,
                                role: "Owner".to_string(),
                            }],
                        });
                    }

                    let _ = tx.send(WsMessage::TeamCreated { node_id, team });
                }
                Err(e) => {
                    error!("Failed to create team for node {}: {}", node_id, e);
                    let _ = tx.send(WsMessage::Error { 
                        message: format!("Failed to create team: {}", e) 
                    });
                }
            }
        }

        WsMessage::JoinTeam { node_id, team_id, owner_node_id } => {
            match daemon_manager.join_team(node_id, &team_id, owner_node_id).await {
                Ok(_) => {
                    let team_name = {
                        let state_guard = state.read().await;
                        state_guard.teams.get(&team_id).map(|t| t.name.clone())
                            .unwrap_or_else(|| "Unknown Team".to_string())
                    };
                    
                    let team = crate::TeamInfo {
                        id: team_id.clone(),
                        name: team_name,
                        role: Some("Member".to_string()),
                    };
                    
                    // Update node's teams
                    {
                        let mut state_guard = state.write().await;
                        if let Some(node) = state_guard.nodes.get_mut(&node_id) {
                            node.teams.push(team.clone());
                        }
                        
                        // Add to team members
                        if let Some(team_data) = state_guard.teams.get_mut(&team_id) {
                            team_data.members.push(crate::TeamMember {
                                node_id,
                                role: "Member".to_string(),
                            });
                        }
                    }

                    let _ = tx.send(WsMessage::TeamJoined { node_id, team });
                }
                Err(e) => {
                    error!("Failed to join team for node {}: {}", node_id, e);
                    let _ = tx.send(WsMessage::Error { 
                        message: format!("Failed to join team: {}", e) 
                    });
                }
            }
        }

        WsMessage::SendMessage { node_id, team_id, message } => {
            match daemon_manager.send_message(node_id, &team_id, &message).await {
                Ok(message_id) => {
                    // Broadcast that a message was sent
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    
                    let _ = tx.send(WsMessage::MessageSent {
                        node_id,
                        message_id: message_id.clone(),
                        author_id: node_id,
                        team_id: team_id.clone(),
                        text: message,
                        timestamp: now,
                    });
                    
                    // Start polling for message reception on other nodes
                    tokio::spawn({
                        let daemon_manager = daemon_manager.clone();
                        let state = state.clone();
                        let tx = tx.clone();
                        let team_id = team_id.clone();
                        let message_id = message_id.clone();
                        async move {
                            // Wait a bit for message to propagate
                            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                            
                            // Find all nodes in the team
                            let team_nodes = {
                                let state_guard = state.read().await;
                                state_guard.nodes.values()
                                    .filter(|node| node.teams.iter().any(|t| t.id == team_id))
                                    .filter(|node| node.status == crate::NodeStatus::Running)
                                    .filter(|node| node.id != node_id) // Exclude sender
                                    .map(|node| node.id)
                                    .collect::<Vec<_>>()
                            };
                            
                            // Check each node for the message
                            for target_node_id in team_nodes {
                                match daemon_manager.query_messages(target_node_id, &team_id).await {
                                    Ok(messages) => {
                                        // Look for the message we just sent
                                        for msg in messages {
                                            if msg.id == message_id {
                                                let _ = tx.send(WsMessage::MessageReceived {
                                                    node_id: target_node_id,
                                                    message_id: msg.id,
                                                    author_id: node_id,
                                                    team_id: team_id.clone(),
                                                    text: msg.text,
                                                    timestamp: msg.timestamp,
                                                });
                                                break;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to query messages for node {}: {}", target_node_id, e);
                                    }
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to send message from node {}: {}", node_id, e);
                    let _ = tx.send(WsMessage::Error {
                        message: format!("Failed to send message: {}", e)
                    });
                }
            }
        }

        WsMessage::PollMessages { node_id, team_id } => {
            // Query messages for the specific node and team
            match daemon_manager.query_messages(node_id, &team_id).await {
                Ok(messages) => {
                    // Send each message as a MessageReceived event
                    for msg in messages {
                        // For now, just use the polling node_id as author
                        // In a real implementation, we'd need to map device IDs to node IDs
                        let author_node_id = node_id;
                        
                        let _ = tx.send(WsMessage::MessageReceived {
                            node_id,
                            message_id: msg.id,
                            author_id: author_node_id,
                            team_id: team_id.clone(),
                            text: msg.text,
                            timestamp: msg.timestamp,
                        });
                    }
                }
                Err(e) => {
                    // Log but don't send error to avoid spam - polling failures are expected
                    tracing::debug!("Failed to poll messages for node {}: {}", node_id, e);
                }
            }
        }

        WsMessage::RemoveDeviceFromTeam { node_id, team_id } => {
            match daemon_manager.remove_device_from_team(node_id, &team_id).await {
                Ok(_) => {
                    // Update the node's teams in state
                    {
                        let mut state_guard = state.write().await;
                        if let Some(target_node) = state_guard.nodes.get_mut(&node_id) {
                            // Remove the team from the node's teams array
                            target_node.teams.retain(|t| t.id != team_id);
                        }
                        
                        // Remove from global teams structure
                        if let Some(team) = state_guard.teams.get_mut(&team_id) {
                            team.members.retain(|m| m.node_id != node_id);
                        }
                    }
                    
                    let _ = tx.send(WsMessage::DeviceRemovedFromTeam { 
                        node_id, 
                        team_id: team_id.clone() 
                    });
                }
                Err(e) => {
                    error!("Failed to remove device from team for node {}: {}", node_id, e);
                    let _ = tx.send(WsMessage::Error { 
                        message: format!("Failed to remove device from team: {}", e) 
                    });
                }
            }
        }

        WsMessage::RevokeRole { node_id, team_id, target_node_id, role } => {
            match daemon_manager.revoke_role(node_id, &team_id, target_node_id, &role).await {
                Ok(_) => {
                    // Update the target node's role in state (demote to Member)
                    {
                        let mut state_guard = state.write().await;
                        if let Some(target_node) = state_guard.nodes.get_mut(&target_node_id) {
                            // Update the role in the node's teams array - revocation always demotes to Member
                            if let Some(team_info) = target_node.teams.iter_mut().find(|t| t.id == team_id) {
                                team_info.role = Some("Member".to_string());
                            }
                        }
                        
                        // Update in global teams structure
                        if let Some(team) = state_guard.teams.get_mut(&team_id) {
                            if let Some(member) = team.members.iter_mut().find(|m| m.node_id == target_node_id) {
                                member.role = "Member".to_string(); // Revocation always demotes to Member
                            }
                        }
                    }
                    
                    let _ = tx.send(WsMessage::RoleRevoked { 
                        node_id, 
                        team_id: team_id.clone(), 
                        target_node_id,
                        role: "Member".to_string() // Always demotes to Member according to policy
                    });
                }
                Err(e) => {
                    error!("Failed to revoke role for node {}: {}", target_node_id, e);
                    let _ = tx.send(WsMessage::Error { 
                        message: format!("Failed to revoke role: {}", e) 
                    });
                }
            }
        }

        WsMessage::AssignRole { node_id, team_id, target_node_id, role } => {
            match daemon_manager.assign_role(node_id, &team_id, target_node_id, &role).await {
                Ok(_) => {
                    // Update the target node's role in state
                    {
                        let mut state_guard = state.write().await;
                        if let Some(target_node) = state_guard.nodes.get_mut(&target_node_id) {
                            // Update the role in the node's teams array
                            if let Some(team_info) = target_node.teams.iter_mut().find(|t| t.id == team_id) {
                                team_info.role = Some(role.clone());
                            }
                        }
                        
                        // Update in global teams structure
                        if let Some(team) = state_guard.teams.get_mut(&team_id) {
                            if let Some(member) = team.members.iter_mut().find(|m| m.node_id == target_node_id) {
                                member.role = role.clone();
                            }
                        }
                    }
                    
                    let _ = tx.send(WsMessage::RoleAssigned { 
                        node_id, 
                        team_id: team_id.clone(), 
                        target_node_id,
                        role: role.clone() 
                    });
                }
                Err(e) => {
                    error!("Failed to assign role for node {}: {}", target_node_id, e);
                    let _ = tx.send(WsMessage::Error { 
                        message: format!("Failed to assign role: {}", e) 
                    });
                }
            }
        }

        WsMessage::GetState => {
            let (nodes, connections, teams) = {
                let state_guard = state.read().await;
                (
                    state_guard.nodes.values().cloned().collect(),
                    state_guard.connections.values().cloned().collect(),
                    state_guard.teams.values().cloned().collect(),
                )
            };

            let _ = tx.send(WsMessage::State { nodes, connections, teams });
        }

        _ => {
            // Handle other message types or ignore
        }
    }

    Ok(())
}