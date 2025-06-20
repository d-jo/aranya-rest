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
}