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
                    .filter(|(from, to)| *from == node_id || *to == node_id)
                    .cloned()
                    .collect();
                
                for (from, to) in connections_to_remove {
                    state_guard.connections.remove(&(from, to));
                    let _ = tx.send(WsMessage::SyncConnectionRemoved { from, to });
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

        WsMessage::AddSyncConnection { from, to } => {
            let connection = SyncConnection {
                from,
                to,
                status: crate::ConnectionStatus::Pending,
            };

            {
                let mut state_guard = state.write().await;
                state_guard.connections.insert((from, to), connection.clone());
            }

            let _ = tx.send(WsMessage::SyncConnectionAdded { connection });

            // TODO: Actually configure the sync connection between daemons
            // This would involve making REST API calls to the daemon's sync endpoints
        }

        WsMessage::RemoveSyncConnection { from, to } => {
            {
                let mut state_guard = state.write().await;
                state_guard.connections.remove(&(from, to));
            }

            let _ = tx.send(WsMessage::SyncConnectionRemoved { from, to });

            // TODO: Remove the sync connection from the daemons
        }

        WsMessage::GetState => {
            let (nodes, connections) = {
                let state_guard = state.read().await;
                (
                    state_guard.nodes.values().cloned().collect(),
                    state_guard.connections.values().cloned().collect(),
                )
            };

            let _ = tx.send(WsMessage::State { nodes, connections });
        }

        _ => {
            // Handle other message types or ignore
        }
    }

    Ok(())
}