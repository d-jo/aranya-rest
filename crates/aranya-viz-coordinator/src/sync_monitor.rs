//! Sync event monitoring for animation support
//!
//! This module reads REAL sync events from daemon log files

use std::{collections::HashMap, sync::Arc, time::Duration, path::PathBuf};
use tokio::{sync::broadcast, time::interval, fs::File, io::{AsyncBufReadExt, BufReader, AsyncSeekExt}};
use tracing::{debug, info};
use uuid::Uuid;
use crate::{AppState, WsMessage, daemon_manager::DaemonManager};

/// Monitors sync events from all running daemons
pub struct SyncMonitor {
    state: AppState,
    tx: broadcast::Sender<WsMessage>,
    daemon_manager: Arc<DaemonManager>,
    // Track file positions for each node
    file_positions: Arc<tokio::sync::RwLock<HashMap<Uuid, u64>>>,
}

impl SyncMonitor {
    pub fn new(state: AppState, tx: broadcast::Sender<WsMessage>, daemon_manager: Arc<DaemonManager>) -> Self {
        Self {
            state,
            tx,
            daemon_manager,
            file_positions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Start monitoring sync events
    pub async fn start(self) {
        info!("Starting sync monitor - reading REAL sync events from daemon logs");
        
        // Check every 100ms for new events
        let mut ticker = interval(Duration::from_millis(100));
        
        loop {
            ticker.tick().await;
            
            // Get all nodes and their work directories
            let nodes = {
                let state = self.state.read().await;
                state.nodes.clone()
            };
            
            // Check each node's sync event log
            for (node_id, node) in nodes {
                // Get the work directory from daemon manager's temp dirs
                let sync_event_file = self.get_sync_event_file(node_id).await;
                if let Some(file_path) = sync_event_file {
                    if let Err(e) = self.read_sync_events(node_id, &node.name, &file_path).await {
                        debug!("Failed to read sync events for {}: {}", node.name, e);
                    }
                }
            }
        }
    }
    
    async fn get_sync_event_file(&self, node_id: Uuid) -> Option<PathBuf> {
        // Get the temp dir for this node from daemon manager
        let temp_dirs = self.daemon_manager.temp_dirs.read().await;
        if let Some(temp_dir) = temp_dirs.get(&node_id) {
            let sync_event_file = temp_dir.path().join("run").join("sync_events.log");
            if sync_event_file.exists() {
                return Some(sync_event_file);
            }
        }
        None
    }
    
    async fn read_sync_events(&self, node_id: Uuid, node_name: &str, file_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open(file_path).await?;
        let mut reader = BufReader::new(file);
        
        // Get last read position
        let mut positions = self.file_positions.write().await;
        let last_pos = positions.get(&node_id).copied().unwrap_or(0);
        
        // Seek to last position
        reader.seek(std::io::SeekFrom::Start(last_pos)).await?;
        
        let mut line = String::new();
        let mut new_pos = last_pos;
        
        while reader.read_line(&mut line).await? > 0 {
            new_pos += line.len() as u64;
            
            // Parse the sync event line
            // Format: timestamp,peer_addr,graph_id,commands_count
            if let Some(event) = self.parse_sync_event(&line, node_id, node_name).await {
                // Send animation event
                let _ = self.tx.send(event);
            }
            
            line.clear();
        }
        
        // Update position
        positions.insert(node_id, new_pos);
        
        Ok(())
    }
    
    async fn parse_sync_event(&self, line: &str, from_node_id: Uuid, from_node_name: &str) -> Option<WsMessage> {
        let parts: Vec<&str> = line.trim().split(',').collect();
        if parts.len() != 4 {
            return None;
        }
        
        let _timestamp = parts[0];
        let peer_addr = parts[1];
        let graph_id = parts[2];
        let commands_count: usize = parts[3].parse().ok()?;
        
        info!("REAL SYNC EVENT: {} received {} commands from {} for team {}", 
              from_node_name, commands_count, peer_addr, graph_id);
        
        // Find which node this peer address belongs to
        let state = self.state.read().await;
        
        // The peer_addr is the sync address (daemon_port), not daemon_port+100
        // So we need to find the node with matching daemon_port
        for (to_node_id, to_node) in &state.nodes {
            // Check if peer address matches this node's daemon port
            if peer_addr.contains(&format!(":{}", to_node.daemon_port)) {
                info!("Found peer node {} with daemon port {}", to_node.name, to_node.daemon_port);
                
                // Find the connection between these nodes for this team
                // The sync event logs show the RECEIVER node received commands FROM the peer
                // So from_node_id is the receiver, to_node_id is the sender
                // The connection could be stored as either from->to or to->from
                for ((from, to, team_id), _conn) in &state.connections {
                    // Check if this connection involves both nodes
                    if (*from == from_node_id && *to == *to_node_id) || (*from == *to_node_id && *to == from_node_id) {
                        // Check if the team_id matches the graph_id
                        if team_id == graph_id {
                            info!("Found matching connection: {} -> {} for team {}", 
                                  to_node.name, from_node_name, team_id);
                            // Animation should show the requestor reaching out to pull data from responder
                            // The requestor (from_node_id) reaches out to the responder (to_node_id) to get commands
                            return Some(WsMessage::SyncOperationCompleted {
                                from: from_node_id,   // The requestor (reaches out to pull data)
                                to: *to_node_id,      // The responder (provides the data)
                                team_id: team_id.clone(),
                                effects_count: commands_count,
                            });
                        }
                    }
                }
            }
        }
        
        None
    }
}