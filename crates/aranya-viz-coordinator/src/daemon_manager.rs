use std::{
    collections::HashMap,
    net::Ipv4Addr,
    path::PathBuf,
    process::Stdio,
    sync::Arc,
};

use anyhow::{Context, Result};
use aranya_daemon::{config::Config, Daemon, DaemonHandle};
use aranya_util::Addr;
use tokio::{fs, process::Command, sync::RwLock, time::Duration};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::{AppState, Node, NodeStatus};

const SYNC_PEER_ENDPOINT: &str = "/api/v1/sync/peers";

/// Manages the lifecycle of daemon processes and their associated REST servers
pub struct DaemonManager {
    state: AppState,
    daemon_handles: Arc<RwLock<HashMap<Uuid, DaemonHandle>>>,
    rest_handles: Arc<RwLock<HashMap<Uuid, tokio::process::Child>>>,
    temp_dirs: Arc<RwLock<HashMap<Uuid, tempfile::TempDir>>>,
}

impl DaemonManager {
    pub fn new(state: AppState) -> Self {
        Self {
            state,
            daemon_handles: Arc::new(RwLock::new(HashMap::new())),
            rest_handles: Arc::new(RwLock::new(HashMap::new())),
            temp_dirs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[instrument(skip(self))]
    pub async fn start_node(&self, node_id: Uuid) -> Result<()> {
        info!("Starting node {}", node_id);
        
        // Update node status to starting
        {
            let mut state = self.state.write().await;
            if let Some(node) = state.nodes.get_mut(&node_id) {
                node.status = NodeStatus::Starting;
            } else {
                anyhow::bail!("Node {} not found", node_id);
            }
        }

        let node = {
            let state = self.state.read().await;
            state.nodes.get(&node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Node {} not found", node_id))?
        };

        match self.start_daemon_and_rest(&node).await {
            Ok((daemon_handle, rest_child, temp_dir)) => {
                // Store handles
                self.daemon_handles.write().await.insert(node_id, daemon_handle);
                self.rest_handles.write().await.insert(node_id, rest_child);
                self.temp_dirs.write().await.insert(node_id, temp_dir);
                
                // Update status to running
                let mut state = self.state.write().await;
                if let Some(node) = state.nodes.get_mut(&node_id) {
                    node.status = NodeStatus::Running;
                }
                
                info!("Node {} started successfully", node_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to start node {}: {}", node_id, e);
                
                // Update status to error
                let mut state = self.state.write().await;
                if let Some(node) = state.nodes.get_mut(&node_id) {
                    node.status = NodeStatus::Error(e.to_string());
                }
                
                Err(e)
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn stop_node(&self, node_id: Uuid) -> Result<()> {
        info!("Stopping node {}", node_id);

        // Stop REST server
        if let Some(mut rest_child) = self.rest_handles.write().await.remove(&node_id) {
            if let Err(e) = rest_child.kill().await {
                warn!("Failed to kill REST server for node {}: {}", node_id, e);
            }
        }

        // Stop daemon (DaemonHandle should handle cleanup automatically)
        self.daemon_handles.write().await.remove(&node_id);
        
        // Clean up temp directory
        self.temp_dirs.write().await.remove(&node_id);

        // Update node status
        let mut state = self.state.write().await;
        if let Some(node) = state.nodes.get_mut(&node_id) {
            node.status = NodeStatus::Stopped;
        }

        info!("Node {} stopped", node_id);
        Ok(())
    }

    async fn start_daemon_and_rest(&self, node: &Node) -> Result<(DaemonHandle, tokio::process::Child, tempfile::TempDir)> {
        // Create temporary directory for this daemon
        let temp_dir = tempfile::tempdir()
            .context("Failed to create temp directory")?;
        let work_dir = temp_dir.path().to_path_buf();

        // Setup daemon config
        let daemon_addr = Addr::from((Ipv4Addr::LOCALHOST, node.daemon_port));
        let cfg = Config {
            name: node.name.clone(),
            runtime_dir: work_dir.join("run"),
            state_dir: work_dir.join("state"),
            cache_dir: work_dir.join("cache"),
            logs_dir: work_dir.join("log"),
            config_dir: work_dir.join("config"),
            sync_addr: daemon_addr,
            afc: None,
            aqc: None,
        };

        // Create directories
        for dir in [
            &cfg.runtime_dir,
            &cfg.state_dir,
            &cfg.cache_dir,
            &cfg.logs_dir,
            &cfg.config_dir,
        ] {
            fs::create_dir_all(dir).await
                .with_context(|| format!("Unable to create directory: {}", dir.display()))?;
        }

        // Start daemon
        let daemon = Daemon::load(cfg.clone()).await
            .context("Unable to init daemon")?
            .spawn();

        // Give daemon time to setup
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Start REST server
        let rest_child = self.start_rest_server(&cfg, node.rest_port).await
            .context("Failed to start REST server")?;

        Ok((daemon, rest_child, temp_dir))
    }

    async fn start_rest_server(&self, daemon_config: &Config, rest_port: u16) -> Result<tokio::process::Child> {
        let daemon_socket = daemon_config.uds_api_sock();
        let bind_addr = format!("127.0.0.1:{}", rest_port);

        // Find the aranya-rest binary
        let rest_binary = self.find_aranya_rest_binary().await?;

        let mut cmd = Command::new(rest_binary);
        cmd.arg("--daemon-socket")
            .arg(&daemon_socket)
            .arg("--bind-addr")
            .arg(&bind_addr)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd.spawn()
            .context("Failed to spawn REST server process")?;

        // Give REST server time to start
        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(child)
    }

    async fn find_aranya_rest_binary(&self) -> Result<PathBuf> {
        // Try to find the aranya-rest binary in target/debug or target/release
        let current_dir = std::env::current_dir()?;
        
        let debug_path = current_dir.join("target/debug/aranya-rest");
        if debug_path.exists() {
            return Ok(debug_path);
        }
        
        let release_path = current_dir.join("target/release/aranya-rest");
        if release_path.exists() {
            return Ok(release_path);
        }
        
        // Fallback to system PATH
        Ok(PathBuf::from("aranya-rest"))
    }

    #[instrument(skip(self))]
    pub async fn create_team(&self, node_id: Uuid, team_name: &str) -> Result<String> {
        info!("Creating team '{}' for node {}", team_name, node_id);
        
        // Get the node's info
        let node = {
            let state = self.state.read().await;
            state.nodes.get(&node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Node {} not found", node_id))?
        };

        // Make REST API call to create team
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}/api/v1/teams", node.rest_port);
        
        let create_request = serde_json::json!({
            "config": {}
        });

        let response = client
            .post(&url)
            .json(&create_request)
            .send()
            .await
            .context("Failed to send create team request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to create team: {}", error_text);
        }

        let response_json: serde_json::Value = response.json().await
            .context("Failed to parse create team response")?;
        
        let team_id = response_json["team_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No team_id in response"))?
            .to_string();

        info!("Successfully created team '{}' with ID {} for node {}", team_name, team_id, node_id);
        Ok(team_id)
    }

    #[instrument(skip(self))]
    pub async fn join_team(&self, node_id: Uuid, team_id: &str, owner_node_id: Uuid) -> Result<()> {
        info!("Node {} joining team {}", node_id, team_id);
        
        // Get both nodes' info
        let (node, owner_node) = {
            let state = self.state.read().await;
            let node = state.nodes.get(&node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Node {} not found", node_id))?;
            let owner = state.nodes.get(&owner_node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Owner node {} not found", owner_node_id))?;
            (node, owner)
        };

        // First, get the key bundle from the joining node
        let client = reqwest::Client::new();
        let key_bundle_url = format!("http://127.0.0.1:{}/api/v1/key-bundle", node.rest_port);
        
        let key_response = client
            .get(&key_bundle_url)
            .send()
            .await
            .context("Failed to get key bundle from joining node")?;

        if !key_response.status().is_success() {
            anyhow::bail!("Failed to get key bundle from joining node");
        }

        let key_bundle: serde_json::Value = key_response.json().await
            .context("Failed to parse key bundle response")?;

        // Add the device to the team via the owner node
        let add_device_url = format!("http://127.0.0.1:{}/api/v1/teams/{}/devices", owner_node.rest_port, team_id);
        
        let add_request = serde_json::json!({
            "keys": key_bundle
        });

        let response = client
            .post(&add_device_url)
            .json(&add_request)
            .send()
            .await
            .context("Failed to send join team request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to join team: {}", error_text);
        }

        info!("Successfully added node {} to team {}", node_id, team_id);
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn configure_sync_peer(&self, from_node_id: Uuid, to_node_id: Uuid, team_id: &str, interval_secs: u32, sync_now: bool) -> Result<()> {
        info!("Configuring sync peer from {} to {} with interval {}s", from_node_id, to_node_id, interval_secs);
        
        // Get the nodes' info
        let (from_node, to_node) = {
            let state = self.state.read().await;
            let from = state.nodes.get(&from_node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Source node {} not found", from_node_id))?;
            let to = state.nodes.get(&to_node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Target node {} not found", to_node_id))?;
            (from, to)
        };

        // Make REST API call to add sync peer
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}{}", from_node.rest_port, SYNC_PEER_ENDPOINT);
        
        // Create sync peer request using the actual team ID and provided config
        let sync_peer_request = serde_json::json!({
            "addr": format!("127.0.0.1:{}", to_node.daemon_port),
            "team_id": team_id,
            "config": {
                "interval_secs": interval_secs,
                "sync_now": sync_now
            }
        });

        let response = client
            .post(&url)
            .json(&sync_peer_request)
            .send()
            .await
            .context("Failed to send sync peer request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to add sync peer: {}", error_text);
        }

        info!("Successfully configured sync peer from {} to {}", from_node_id, to_node_id);
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn remove_sync_peer(&self, from_node_id: Uuid, to_node_id: Uuid, team_id: &str) -> Result<()> {
        info!("Removing sync peer from {} to {}", from_node_id, to_node_id);
        
        // Get the nodes' info
        let (from_node, to_node) = {
            let state = self.state.read().await;
            let from = state.nodes.get(&from_node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Source node {} not found", from_node_id))?;
            let to = state.nodes.get(&to_node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Target node {} not found", to_node_id))?;
            (from, to)
        };

        // Make REST API call to remove sync peer
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}{}", from_node.rest_port, SYNC_PEER_ENDPOINT);
        
        // Use the actual team ID
        let remove_request = serde_json::json!({
            "addr": format!("127.0.0.1:{}", to_node.daemon_port),
            "team_id": team_id
        });

        let response = client
            .delete(&url)
            .json(&remove_request)
            .send()
            .await
            .context("Failed to send remove sync peer request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to remove sync peer: {}", error_text);
        }

        info!("Successfully removed sync peer from {} to {}", from_node_id, to_node_id);
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn send_message(&self, node_id: Uuid, team_id: &str, message: &str) -> Result<String> {
        info!("Sending message from node {} to team {}", node_id, team_id);
        
        let node = {
            let state = self.state.read().await;
            state.nodes.get(&node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Node {} not found", node_id))?
        };

        // Make REST API call to send message
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}/api/v1/teams/{}/messages", node.rest_port, team_id);
        
        let send_request = serde_json::json!({
            "text": message
        });

        let response = client
            .post(&url)
            .json(&send_request)
            .send()
            .await
            .context("Failed to send message request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to send message: {}", error_text);
        }

        let response_json: serde_json::Value = response.json().await
            .context("Failed to parse send message response")?;
        
        let message_id = response_json["message_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No message_id in response"))?
            .to_string();

        info!("Successfully sent message from node {} with ID {}", node_id, message_id);
        Ok(message_id)
    }

    #[instrument(skip(self))]
    pub async fn query_messages(&self, node_id: Uuid, team_id: &str) -> Result<Vec<MessageInfo>> {
        let node = {
            let state = self.state.read().await;
            state.nodes.get(&node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Node {} not found", node_id))?
        };

        // Make REST API call to query messages
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}/api/v1/teams/{}/messages", node.rest_port, team_id);

        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to query messages")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to query messages: {}", error_text);
        }

        let messages: Vec<serde_json::Value> = response.json().await
            .context("Failed to parse query messages response")?;

        let mut result = Vec::new();
        for msg in messages {
            let id = msg["id"].as_str().unwrap_or("").to_string();
            let author_id = msg["author_id"].as_str().unwrap_or("").to_string();
            let text = msg["text"].as_str().unwrap_or("").to_string();
            let timestamp = msg["timestamp"].as_u64().unwrap_or(0);
            
            result.push(MessageInfo {
                id,
                author_id,
                text,
                timestamp,
            });
        }

        Ok(result)
    }

    #[instrument(skip(self))]
    pub async fn assign_role(&self, acting_node_id: Uuid, team_id: &str, target_node_id: Uuid, role: &str) -> Result<()> {
        info!("Assigning role '{}' to node {} in team {} via acting node {}", role, target_node_id, team_id, acting_node_id);
        
        // Get the acting node's info
        let acting_node = {
            let state = self.state.read().await;
            state.nodes.get(&acting_node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Acting node {} not found", acting_node_id))?
        };

        // Get the target node's device ID from its REST API
        let target_device_id = self.get_device_id(target_node_id).await?;

        // Make REST API call to assign role via the acting node
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}/api/v1/teams/{}/roles/assign", acting_node.rest_port, team_id);
        
        let assign_request = serde_json::json!({
            "device_id": target_device_id,
            "role": role
        });

        let response = client
            .post(&url)
            .json(&assign_request)
            .send()
            .await
            .context("Failed to send assign role request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to assign role: {}", error_text);
        }

        info!("Successfully assigned role '{}' to node {} in team {}", role, target_node_id, team_id);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_device_id(&self, node_id: Uuid) -> Result<String> {
        let node = {
            let state = self.state.read().await;
            state.nodes.get(&node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Node {} not found", node_id))?
        };

        // Make REST API call to get device ID
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}/api/v1/device-id", node.rest_port);

        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to get device ID")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to get device ID: {}", error_text);
        }

        let response_json: serde_json::Value = response.json().await
            .context("Failed to parse device ID response")?;
        
        let device_id = response_json["device_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No device_id in response"))?
            .to_string();

        Ok(device_id)
    }

    #[instrument(skip(self))]
    pub async fn remove_device_from_team(&self, node_id: Uuid, team_id: &str) -> Result<()> {
        info!("Removing device {} from team {}", node_id, team_id);
        
        // Get the device ID from the node
        let device_id = self.get_device_id(node_id).await?;

        // Find a team owner or admin node to perform the removal
        let owner_node = {
            let state = self.state.read().await;
            state.teams.get(team_id)
                .map(|team| team.owner_node_id)
                .ok_or_else(|| anyhow::anyhow!("Team {} not found", team_id))?
        };

        let owner_node_info = {
            let state = self.state.read().await;
            state.nodes.get(&owner_node)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Owner node {} not found", owner_node))?
        };

        // Make REST API call to remove device from team via the owner node
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}/api/v1/teams/{}/devices/{}", owner_node_info.rest_port, team_id, device_id);

        let response = client
            .delete(&url)
            .send()
            .await
            .context("Failed to send remove device request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to remove device from team: {}", error_text);
        }

        info!("Successfully removed device {} from team {}", node_id, team_id);
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn revoke_role(&self, acting_node_id: Uuid, team_id: &str, target_node_id: Uuid, role: &str) -> Result<()> {
        info!("Revoking role '{}' from node {} in team {} via acting node {}", role, target_node_id, team_id, acting_node_id);
        
        // Get the acting node's info
        let acting_node = {
            let state = self.state.read().await;
            state.nodes.get(&acting_node_id).cloned()
                .ok_or_else(|| anyhow::anyhow!("Acting node {} not found", acting_node_id))?
        };

        // Get the target node's device ID from its REST API
        let target_device_id = self.get_device_id(target_node_id).await?;

        // Make REST API call to revoke role via the acting node
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{}/api/v1/teams/{}/roles/revoke", acting_node.rest_port, team_id);
        
        let revoke_request = serde_json::json!({
            "device_id": target_device_id,
            "role": role
        });

        let response = client
            .post(&url)
            .json(&revoke_request)
            .send()
            .await
            .context("Failed to send revoke role request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to revoke role: {}", error_text);
        }

        info!("Successfully revoked role '{}' from node {} in team {} (demoted to Member)", role, target_node_id, team_id);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MessageInfo {
    pub id: String,
    pub author_id: String,
    pub text: String,
    pub timestamp: u64,
}