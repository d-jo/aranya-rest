use std::{
    collections::HashMap,
    sync::Arc,
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

pub mod coordinator;
pub mod daemon_manager;
pub mod websocket;

/// Represents a node in the visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub name: String,
    pub position: Position,
    pub daemon_port: u16,
    pub rest_port: u16,
    pub status: NodeStatus,
    pub teams: Vec<TeamInfo>, // Teams this node belongs to
}

/// Team information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamInfo {
    pub id: String,
    pub name: String,
    pub role: Option<String>, // Owner, Admin, Operator, Member
}

/// Position on the canvas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

/// Node status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

/// Sync connection between two nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConnection {
    pub from: Uuid, // The node that requested the sync
    pub to: Uuid,   // The node that is being synced to
    pub team_id: String, // The team this sync is for
    pub status: ConnectionStatus,
}

/// Connection status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Pending,
    Connected,
    Failed(String),
}

/// WebSocket message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WsMessage {
    // Client -> Server
    CreateNode { name: String, position: Position },
    DeleteNode { node_id: Uuid },
    MoveNode { node_id: Uuid, position: Position },
    AddSyncConnection { from: Uuid, to: Uuid, team_id: String },
    RemoveSyncConnection { from: Uuid, to: Uuid, team_id: String },
    
    // Team operations
    CreateTeam { node_id: Uuid, team_name: String },
    JoinTeam { node_id: Uuid, team_id: String, owner_node_id: Uuid },
    LeaveTeam { node_id: Uuid, team_id: String },
    AssignRole { node_id: Uuid, team_id: String, target_node_id: Uuid, role: String },
    
    // Message operations
    SendMessage { node_id: Uuid, team_id: String, message: String },
    PollMessages { node_id: Uuid, team_id: String },
    
    // Server -> Client
    NodeCreated { node: Node },
    NodeDeleted { node_id: Uuid },
    NodeMoved { node_id: Uuid, position: Position },
    NodeStatusChanged { node_id: Uuid, status: NodeStatus },
    NodeTeamsChanged { node_id: Uuid, teams: Vec<TeamInfo> },
    SyncConnectionAdded { connection: SyncConnection },
    SyncConnectionRemoved { from: Uuid, to: Uuid, team_id: String },
    SyncConnectionStatusChanged { from: Uuid, to: Uuid, team_id: String, status: ConnectionStatus },
    
    // Team events
    TeamCreated { node_id: Uuid, team: TeamInfo },
    TeamJoined { node_id: Uuid, team: TeamInfo },
    TeamLeft { node_id: Uuid, team_id: String },
    RoleAssigned { node_id: Uuid, team_id: String, role: String },
    
    // Message events
    MessageSent { node_id: Uuid, message_id: String, author_id: Uuid, team_id: String, text: String, timestamp: u64 },
    MessageReceived { node_id: Uuid, message_id: String, author_id: Uuid, team_id: String, text: String, timestamp: u64 },
    
    // Bidirectional
    GetState,
    State { nodes: Vec<Node>, connections: Vec<SyncConnection>, teams: Vec<Team> },
    Error { message: String },
}

/// Team representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub owner_node_id: Uuid,
    pub members: Vec<TeamMember>,
}

/// Team member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub node_id: Uuid,
    pub role: String,
}

/// Shared application state
pub type AppState = Arc<RwLock<AppStateInner>>;

#[derive(Debug)]
pub struct AppStateInner {
    pub nodes: HashMap<Uuid, Node>,
    pub connections: HashMap<(Uuid, Uuid, String), SyncConnection>, // (from, to, team_id)
    pub teams: HashMap<String, Team>,
    pub port_allocator: PortAllocator,
}

impl AppStateInner {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            connections: HashMap::new(),
            teams: HashMap::new(),
            port_allocator: PortAllocator::new(8000, 9000), // Use ports 8000-8999 for daemons, 9000-9999 for REST
        }
    }
}

/// Simple port allocator
#[derive(Debug)]
pub struct PortAllocator {
    daemon_next: u16,
    rest_next: u16,
    daemon_max: u16,
    rest_max: u16,
}

impl PortAllocator {
    pub fn new(daemon_start: u16, rest_start: u16) -> Self {
        Self {
            daemon_next: daemon_start,
            rest_next: rest_start,
            daemon_max: daemon_start + 999,
            rest_max: rest_start + 999,
        }
    }
    
    pub fn allocate_ports(&mut self) -> Result<(u16, u16)> {
        if self.daemon_next >= self.daemon_max || self.rest_next >= self.rest_max {
            anyhow::bail!("Port range exhausted");
        }
        
        let daemon_port = self.daemon_next;
        let rest_port = self.rest_next;
        
        self.daemon_next += 1;
        self.rest_next += 1;
        
        Ok((daemon_port, rest_port))
    }
}