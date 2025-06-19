use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use anyhow::{Context, Result};
use aranya_client::Client;
use aranya_daemon::{config::Config, Daemon, DaemonHandle};
use aranya_daemon_api::{DeviceId, KeyBundle};
use aranya_rest::RestServer;
use aranya_util::Addr;
use backon::{ExponentialBuilder, Retryable as _};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use tokio::{fs, time};
use tracing::{info, instrument, trace};

const SLEEP_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Serialize, Deserialize)]
struct VersionResponse {
    version: String,
}

#[instrument(skip_all)]
async fn sleep(duration: Duration) {
    trace!(?duration, "sleeping");
    time::sleep(duration).await;
}

struct RestDeviceCtx {
    pub client: Client,
    pub pk: KeyBundle,
    pub id: DeviceId,
    pub rest_server_addr: SocketAddr,
    #[allow(unused)]
    pub daemon: DaemonHandle,
    #[allow(unused)]
    pub temp_dir: TempDir,
    #[allow(unused)]
    pub rest_handle: tokio::task::JoinHandle<Result<(), std::io::Error>>,
}

impl RestDeviceCtx {
    async fn new(name: &str) -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let work_dir = temp_dir.path().to_path_buf();
        
        let addr_any = Addr::from((Ipv4Addr::LOCALHOST, 0));
        let rest_bind_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));

        // Setup daemon config
        let cfg = Config {
            name: name.into(),
            runtime_dir: work_dir.join("run"),
            state_dir: work_dir.join("state"),
            cache_dir: work_dir.join("cache"),
            logs_dir: work_dir.join("log"),
            config_dir: work_dir.join("config"),
            sync_addr: addr_any,
            afc: None,
            aqc: None,
        };

        for dir in [
            &cfg.runtime_dir,
            &cfg.state_dir,
            &cfg.cache_dir,
            &cfg.logs_dir,
            &cfg.config_dir,
        ] {
            fs::create_dir_all(dir)
                .await
                .with_context(|| format!("unable to create directory: {}", dir.display()))?;
        }
        
        let uds_path = cfg.uds_api_sock();

        // Load and start daemon from config
        let daemon = Daemon::load(cfg.clone())
            .await
            .context("unable to init daemon")?
            .spawn();

        // Give daemon time to setup UDS API and write the public key
        sleep(SLEEP_INTERVAL).await;

        // Initialize the client
        let mut client = (|| {
            Client::builder()
                .with_daemon_uds_path(&uds_path)
                .with_daemon_aqc_addr(&addr_any)
                .connect()
        })
        .retry(ExponentialBuilder::default())
        .await
        .context("unable to init client")?;

        // Get device id and key bundle
        let pk = client.get_key_bundle().await.expect("expected key bundle");
        let id = client.get_device_id().await.expect("expected device id");

        // Read the daemon's API key for REST server authentication
        let daemon_api_key_path = cfg.api_pk_path();
        let daemon_api_key_bytes = fs::read(&daemon_api_key_path)
            .await
            .with_context(|| format!("unable to read API key from {}", daemon_api_key_path.display()))?;
        let daemon_api_key = aranya_daemon_api::crypto::PublicApiKey::decode(&daemon_api_key_bytes)
            .context("unable to decode API key")?;

        // Create and start the REST server with a proper listener to get the bound address
        let listener = tokio::net::TcpListener::bind(rest_bind_addr).await?;
        let rest_server_addr = listener.local_addr()?;
        
        let rest_server = RestServer::new(uds_path, daemon_api_key, rest_bind_addr).await?;
        let router = rest_server.router();
        
        // Spawn the REST server in a background task
        let rest_handle = tokio::spawn(async move {
            axum::serve(listener, router).await
        });

        // Give REST server time to start
        sleep(SLEEP_INTERVAL).await;

        Ok(Self {
            client,
            pk,
            id,
            rest_server_addr,
            daemon,
            temp_dir,
            rest_handle,
        })
    }

    async fn get_version(&self) -> Result<String> {
        let url = format!("http://{}/api/v1/version", self.rest_server_addr);
        let response = reqwest::get(&url).await?;
        
        if !response.status().is_success() {
            anyhow::bail!("HTTP request failed with status: {}", response.status());
        }

        let version_response: VersionResponse = response.json().await?;
        Ok(version_response.version)
    }
}

#[tokio::test]
#[test_log::test]
async fn test_rest_server_version_single_daemon() -> Result<()> {
    info!("Starting test_rest_server_version_single_daemon");
    
    let device = RestDeviceCtx::new("test-device").await?;
    
    // Test the version endpoint
    let version = device.get_version().await?;
    
    // Verify we got a version string (should be semver format)
    assert!(!version.is_empty(), "Version should not be empty");
    assert!(version.contains('.'), "Version should contain dots (semver format)");
    
    info!("Successfully retrieved version: {}", version);
    
    Ok(())
}

#[tokio::test]
#[test_log::test]
async fn test_rest_server_version_multi_daemon() -> Result<()> {
    info!("Starting test_rest_server_version_multi_daemon");
    
    // Create multiple devices with REST servers
    let device1 = RestDeviceCtx::new("device-1").await?;
    let device2 = RestDeviceCtx::new("device-2").await?;
    
    // Test version endpoint on both devices
    let version1 = device1.get_version().await?;
    let version2 = device2.get_version().await?;
    
    // Both should return valid version strings
    assert!(!version1.is_empty(), "Device 1 version should not be empty");
    assert!(!version2.is_empty(), "Device 2 version should not be empty");
    assert!(version1.contains('.'), "Device 1 version should contain dots");
    assert!(version2.contains('.'), "Device 2 version should contain dots");
    
    // Both should return the same version (same daemon binary)
    assert_eq!(version1, version2, "Both devices should return the same version");
    
    info!("Successfully retrieved versions from both devices: {} and {}", version1, version2);
    
    Ok(())
}