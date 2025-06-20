use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};

use anyhow::{Context, Result};
use aranya_client::{client::Client, SyncPeerConfig, TeamConfig};
use aranya_daemon::{config::Config, Daemon, DaemonHandle};
use aranya_daemon_api::{DeviceId, KeyBundle, Role, TeamId};
use aranya_rest::RestServer;
use aranya_util::Addr;
use backon::{ExponentialBuilder, Retryable as _};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;
use tokio::{fs, time};
use tracing::{info, instrument, trace};

const SYNC_INTERVAL: Duration = Duration::from_millis(100);
const SLEEP_INTERVAL: Duration = Duration::from_millis(500);
const LONG_SLEEP_INTERVAL: Duration = Duration::from_millis(1000);

#[derive(Serialize, Deserialize)]
struct VersionResponse {
    version: String,
}

#[derive(Serialize, Deserialize)]
struct CreateTeamResponse {
    team_id: String,
}

#[derive(Serialize, Deserialize)]
struct CreateTeamRequest {
    config: TeamConfigJson,
}

#[derive(Serialize, Deserialize)]
struct TeamConfigJson {
    // Currently empty but included for future extensibility
}

#[derive(Serialize, Deserialize)]
struct AddDeviceRequest {
    keys: KeyBundle,
}

#[derive(Serialize, Deserialize)]
struct AssignRoleRequest {
    device_id: String,
    role: String,
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

    // REST API methods for team operations
    async fn rest_create_team(&self) -> Result<TeamId> {
        let url = format!("http://{}/api/v1/teams", self.rest_server_addr);
        let request = CreateTeamRequest {
            config: TeamConfigJson {},
        };
        
        let response = reqwest::Client::new()
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to create team: {}", response.status());
        }

        let team_response: CreateTeamResponse = response.json().await?;
        let team_id_bytes = hex::decode(&team_response.team_id)?;
        let mut array = [0u8; 32];
        if team_id_bytes.len() != 32 {
            anyhow::bail!("Invalid team ID length: expected 32 bytes, got {}", team_id_bytes.len());
        }
        array.copy_from_slice(&team_id_bytes);
        Ok(TeamId::from(array))
    }

    async fn rest_add_device_to_team(&self, team_id: TeamId, device_keys: &KeyBundle) -> Result<()> {
        let url = format!("http://{}/api/v1/teams/{}/devices", self.rest_server_addr, hex::encode(team_id.into_id().as_bytes()));
        let request = AddDeviceRequest {
            keys: device_keys.clone(),
        };
        
        let response = reqwest::Client::new()
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to add device to team: {}", response.status());
        }

        Ok(())
    }

    async fn rest_assign_role(&self, team_id: TeamId, device_id: DeviceId, role: Role) -> Result<()> {
        let url = format!("http://{}/api/v1/teams/{}/roles/assign", self.rest_server_addr, hex::encode(team_id.into_id().as_bytes()));
        let role_str = match role {
            Role::Owner => "Owner",
            Role::Admin => "Admin", 
            Role::Operator => "Operator",
            Role::Member => "Member",
        };
        let request = AssignRoleRequest {
            device_id: hex::encode(device_id.into_id().as_bytes()),
            role: role_str.to_string(),
        };
        
        let response = reqwest::Client::new()
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to assign role: {}", response.status());
        }

        Ok(())
    }

    async fn aranya_local_addr(&self) -> Result<SocketAddr> {
        Ok(self.client.local_addr().await?)
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

// REST-enabled Team Context - mirrors the TeamCtx pattern from aranya-client tests
pub struct RestTeamCtx {
    pub owner: RestDeviceCtx,
    pub admin: RestDeviceCtx,
    pub operator: RestDeviceCtx,
    pub membera: RestDeviceCtx,
    pub memberb: RestDeviceCtx,
}

impl RestTeamCtx {
    pub async fn new(name: &str) -> Result<Self> {
        info!("Creating RestTeamCtx for '{}'", name);
        
        let owner = RestDeviceCtx::new(&format!("{}-owner", name)).await?;
        let admin = RestDeviceCtx::new(&format!("{}-admin", name)).await?;
        let operator = RestDeviceCtx::new(&format!("{}-operator", name)).await?;
        let membera = RestDeviceCtx::new(&format!("{}-membera", name)).await?;
        let memberb = RestDeviceCtx::new(&format!("{}-memberb", name)).await?;

        Ok(Self {
            owner,
            admin,
            operator,
            membera,
            memberb,
        })
    }

    fn devices(&mut self) -> [&mut RestDeviceCtx; 5] {
        [
            &mut self.owner,
            &mut self.admin,
            &mut self.operator,
            &mut self.membera,
            &mut self.memberb,
        ]
    }

    pub async fn add_all_sync_peers(&mut self, team_id: TeamId) -> Result<()> {
        let config = SyncPeerConfig::builder().interval(SYNC_INTERVAL).build()?;
        let mut devices = self.devices();
        for i in 0..devices.len() {
            let (device, peers) = devices[i..].split_first_mut().expect("expected device");
            for peer in peers {
                device
                    .client
                    .team(team_id)
                    .add_sync_peer(peer.aranya_local_addr().await?.into(), config.clone())
                    .await?;
                peer.client
                    .team(team_id)
                    .add_sync_peer(device.aranya_local_addr().await?.into(), config.clone())
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn add_all_device_roles_via_client(&mut self, team_id: TeamId) -> Result<()> {
        // Use the client library approach (same as original TeamCtx)
        let mut owner_team = self.owner.client.team(team_id);
        let mut admin_team = self.admin.client.team(team_id);
        let mut operator_team = self.operator.client.team(team_id);

        // Add the admin as a new device, and assign its role.
        info!("adding admin to team via client");
        owner_team.add_device_to_team(self.admin.pk.clone()).await?;
        owner_team.assign_role(self.admin.id, Role::Admin).await?;

        sleep(SLEEP_INTERVAL).await;

        // Add the operator as a new device.
        info!("adding operator to team via client");
        owner_team
            .add_device_to_team(self.operator.pk.clone())
            .await?;

        sleep(SLEEP_INTERVAL).await;

        // Assign the operator its role.
        admin_team
            .assign_role(self.operator.id, Role::Operator)
            .await?;

        sleep(SLEEP_INTERVAL).await;

        // Add members as new devices.
        info!("adding members to team via client");
        operator_team
            .add_device_to_team(self.membera.pk.clone())
            .await?;

        operator_team
            .add_device_to_team(self.memberb.pk.clone())
            .await?;

        sleep(SLEEP_INTERVAL).await;

        Ok(())
    }

    pub async fn add_all_device_roles_via_rest(&mut self, team_id: TeamId) -> Result<()> {
        // Use the REST API approach
        info!("adding admin to team via REST");
        self.owner.rest_add_device_to_team(team_id, &self.admin.pk).await?;
        self.owner.rest_assign_role(team_id, self.admin.id, Role::Admin).await?;

        sleep(SLEEP_INTERVAL).await;

        info!("adding operator to team via REST");
        self.owner.rest_add_device_to_team(team_id, &self.operator.pk).await?;

        sleep(SLEEP_INTERVAL).await;

        self.admin.rest_assign_role(team_id, self.operator.id, Role::Operator).await?;

        sleep(SLEEP_INTERVAL).await;

        info!("adding members to team via REST");
        self.operator.rest_add_device_to_team(team_id, &self.membera.pk).await?;
        self.operator.rest_add_device_to_team(team_id, &self.memberb.pk).await?;

        sleep(SLEEP_INTERVAL).await;

        Ok(())
    }
}

#[tokio::test]
#[test_log::test]
async fn test_rest_team_workflow_comprehensive() -> Result<()> {
    info!("Starting comprehensive REST team workflow test");

    // Create a team context with 5 devices, each with its own daemon and REST server
    let mut team = RestTeamCtx::new("comprehensive-test").await?;

    // Step 1: Create team via client library (owner)
    info!("Creating team via client library");
    let cfg = TeamConfig::builder().build()?;
    let team_id = team
        .owner
        .client
        .create_team(cfg)
        .await
        .expect("expected to create team");
    info!("Created team with ID: {:?}", team_id);

    // Step 2: Create team via REST API (owner) - verify consistency
    info!("Creating team via REST API to verify consistency");
    let rest_team_id = team.owner.rest_create_team().await?;
    info!("Created team via REST with ID: {:?}", rest_team_id);

    // Step 3: Set up sync peers between all devices
    info!("Setting up sync peers");
    team.add_all_sync_peers(team_id).await?;

    // Step 4: Add devices and assign roles via client library
    info!("Adding devices and assigning roles via client library");
    team.add_all_device_roles_via_client(team_id).await?;

    // Step 5: Create another team and use REST API for device management  
    info!("Creating second team and using REST API for device management");
    let rest_team_id2 = team.owner.rest_create_team().await?;
    
    // Set up sync peers for the second team as well
    info!("Setting up sync peers for second team");
    team.add_all_sync_peers(rest_team_id2).await?;
    
    // Allow time for all devices to learn about the new team
    sleep(SLEEP_INTERVAL).await;
    
    team.add_all_device_roles_via_rest(rest_team_id2).await?;

    // Step 6: Verify all devices can query the team via client library
    info!("Verifying all devices can query team information");
    for device in [&mut team.owner, &mut team.admin, &mut team.operator, &mut team.membera, &mut team.memberb] {
        let mut queries = device.client.queries(team_id);
        let devices_count = queries.devices_on_team().await?.iter().count();
        info!("Device sees {} devices in team", devices_count);
        assert_eq!(devices_count, 5, "All devices should see 5 devices in the team");
    }

    // Step 7: Verify REST endpoints work for all devices
    info!("Verifying REST endpoints work for all devices");
    for device in [&team.owner, &team.admin, &team.operator, &team.membera, &team.memberb] {
        let version = device.get_version().await?;
        assert!(!version.is_empty(), "Version should not be empty");
        info!("Device REST server returned version: {}", version);
    }

    info!("Comprehensive REST team workflow test completed successfully!");
    Ok(())
}

// ====== Device Info Endpoint Tests ======

#[derive(Serialize, Deserialize)]
struct LocalAddrResponse {
    address: String,
}

#[derive(Serialize, Deserialize)]
struct DeviceIdResponse {
    device_id: String,
}

impl RestDeviceCtx {
    async fn get_local_addr(&self) -> Result<String> {
        let url = format!("http://{}/api/v1/local-addr", self.rest_server_addr);
        let response = reqwest::get(&url).await?;
        
        if !response.status().is_success() {
            anyhow::bail!("HTTP request failed with status: {}", response.status());
        }

        let addr_response: LocalAddrResponse = response.json().await?;
        Ok(addr_response.address)
    }

    async fn get_key_bundle_rest(&self) -> Result<KeyBundle> {
        let url = format!("http://{}/api/v1/key-bundle", self.rest_server_addr);
        let response = reqwest::get(&url).await?;
        
        if !response.status().is_success() {
            anyhow::bail!("HTTP request failed with status: {}", response.status());
        }

        let key_bundle: KeyBundle = response.json().await?;
        Ok(key_bundle)
    }

    async fn get_device_id_rest(&self) -> Result<DeviceId> {
        let url = format!("http://{}/api/v1/device-id", self.rest_server_addr);
        let response = reqwest::get(&url).await?;
        
        if !response.status().is_success() {
            anyhow::bail!("HTTP request failed with status: {}", response.status());
        }

        let device_response: DeviceIdResponse = response.json().await?;
        let device_id_bytes = hex::decode(&device_response.device_id)?;
        let mut array = [0u8; 32];
        if device_id_bytes.len() != 32 {
            anyhow::bail!("Invalid device ID length: expected 32 bytes, got {}", device_id_bytes.len());
        }
        array.copy_from_slice(&device_id_bytes);
        Ok(DeviceId::from(array))
    }
}

#[tokio::test]
#[test_log::test]
async fn test_device_info_endpoints() -> Result<()> {
    info!("Starting device info endpoints test");
    
    let device = RestDeviceCtx::new("device-info-test").await?;
    
    // Test version endpoint (already tested, but verify again)
    let version = device.get_version().await?;
    assert!(!version.is_empty(), "Version should not be empty");
    
    // Test local-addr endpoint
    let local_addr = device.get_local_addr().await?;
    assert!(!local_addr.is_empty(), "Local address should not be empty");
    info!("Local address: {}", local_addr);
    
    // Test key-bundle endpoint
    let key_bundle_rest = device.get_key_bundle_rest().await?;
    let key_bundle_client = device.pk.clone();
    // Key bundles should match between REST API and client
    assert_eq!(key_bundle_rest.clone(), key_bundle_client, "Key bundles should match");
    
    // Test device-id endpoint
    let device_id_rest = device.get_device_id_rest().await?;
    let device_id_client = device.id;
    // Device IDs should match between REST API and client
    assert_eq!(device_id_rest, device_id_client, "Device IDs should match");
    
    info!("All device info endpoints working correctly");
    Ok(())
}

#[tokio::test]
#[test_log::test]
async fn test_device_info_multi_daemon() -> Result<()> {
    info!("Starting multi-daemon device info test");
    
    let device1 = RestDeviceCtx::new("device-1").await?;
    let device2 = RestDeviceCtx::new("device-2").await?;
    
    // Test that each device has unique identifiers
    let id1 = device1.get_device_id_rest().await?;
    let id2 = device2.get_device_id_rest().await?;
    assert_ne!(id1, id2, "Device IDs should be unique");
    
    let kb1 = device1.get_key_bundle_rest().await?;
    let kb2 = device2.get_key_bundle_rest().await?;
    assert_ne!(kb1, kb2, "Key bundles should be unique");
    
    let addr1 = device1.get_local_addr().await?;
    let addr2 = device2.get_local_addr().await?;
    assert_ne!(addr1, addr2, "Local addresses should be unique");
    
    info!("Multi-daemon device info test completed successfully");
    Ok(())
}

// ====== Sync Peer Management Tests ======

#[derive(Serialize, Deserialize)]
struct AddSyncPeerRequest {
    addr: String,
    team_id: String,
    config: SyncPeerConfigJson,
}

#[derive(Serialize, Deserialize)]
struct SyncPeerConfigJson {
    interval_secs: u64,
    sync_now: bool,
}

#[derive(Serialize, Deserialize)]
struct SyncNowRequest {
    addr: String,
    team_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<SyncPeerConfigJson>,
}

#[derive(Serialize, Deserialize)]
struct RemoveSyncPeerRequest {
    addr: String,
    team_id: String,
}

impl RestDeviceCtx {
    async fn rest_add_sync_peer(&self, team_id: TeamId, peer_addr: &str, interval_secs: Option<u64>) -> Result<()> {
        let url = format!("http://{}/api/v1/sync/peers", self.rest_server_addr);
        let request = AddSyncPeerRequest {
            addr: peer_addr.to_string(),
            team_id: hex::encode(team_id.into_id().as_bytes()),
            config: SyncPeerConfigJson {
                interval_secs: interval_secs.unwrap_or(1),
                sync_now: false,
            },
        };
        
        let response = reqwest::Client::new()
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to add sync peer: {}", response.status());
        }

        Ok(())
    }

    async fn rest_sync_now(&self, team_id: TeamId, peer_addr: &str, config: Option<SyncPeerConfigJson>) -> Result<()> {
        let url = format!("http://{}/api/v1/sync/now", self.rest_server_addr);
        let request = SyncNowRequest {
            addr: peer_addr.to_string(),
            team_id: hex::encode(team_id.into_id().as_bytes()),
            config,
        };
        
        let response = reqwest::Client::new()
            .post(&url)  
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to sync now: {}", response.status());
        }

        Ok(())
    }

    async fn rest_remove_sync_peer(&self, team_id: TeamId, peer_addr: &str) -> Result<()> {
        let url = format!("http://{}/api/v1/sync/peers", self.rest_server_addr);
        let request = RemoveSyncPeerRequest {
            addr: peer_addr.to_string(),
            team_id: hex::encode(team_id.into_id().as_bytes()),
        };
        
        let response = reqwest::Client::new()
            .delete(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to remove sync peer: {}", response.status());
        }

        Ok(())
    }
}

#[tokio::test]
#[test_log::test]
async fn test_sync_peer_management() -> Result<()> {
    info!("Starting sync peer management test");
    
    // Create two devices 
    let mut device1 = RestDeviceCtx::new("sync-device-1").await?;
    let mut device2 = RestDeviceCtx::new("sync-device-2").await?;
    
    // Create a team on device1
    let cfg = aranya_client::TeamConfig::builder().build()?;
    let team_id = device1.client.create_team(cfg).await?;
    
    // Get device addresses  
    let device1_addr = device1.get_local_addr().await?;
    let device2_addr = device2.get_local_addr().await?;
    
    // Add device1 as sync peer on device2 via REST
    info!("Adding sync peer via REST API");
    device2.rest_add_sync_peer(team_id, &device1_addr, Some(1)).await?;
    
    // Add device2 as sync peer on device1 via REST  
    device1.rest_add_sync_peer(team_id, &device2_addr, Some(1)).await?;
    
    // Test sync now functionality
    info!("Testing sync now via REST API");
    let sync_config = SyncPeerConfigJson {
        interval_secs: 1,
        sync_now: true,
    };
    device1.rest_sync_now(team_id, &device2_addr, Some(sync_config)).await?;
    
    // Test removing sync peers
    info!("Removing sync peers via REST API");
    device1.rest_remove_sync_peer(team_id, &device2_addr).await?;
    device2.rest_remove_sync_peer(team_id, &device1_addr).await?;
    
    info!("Sync peer management test completed successfully");
    Ok(())
}

// ====== Team Management Tests ======

#[derive(Serialize, Deserialize)]
struct QueryDevicesResponse {
    devices: Vec<QueryDeviceInfo>,
}

#[derive(Serialize, Deserialize)]
struct QueryDeviceInfo {
    device_id: String,
    keybundle: KeyBundle,
}

impl RestDeviceCtx {
    async fn rest_query_devices_on_team(&self, team_id: TeamId) -> Result<Vec<String>> {
        let url = format!("http://{}/api/v1/teams/{}/devices", self.rest_server_addr, hex::encode(team_id.into_id().as_bytes()));
        let response = reqwest::get(&url).await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to query devices on team: {}", response.status());
        }

        let device_ids: Vec<String> = response.json().await?;
        Ok(device_ids)
    }

    async fn rest_query_devices_on_team_with_retry(&self, team_id: TeamId) -> Result<Vec<String>> {
        let mut attempts = 0;
        let max_retries = 5;
        
        loop {
            match self.rest_query_devices_on_team(team_id).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    let err_str = err.to_string();
                    if (err_str.contains("storage error") || err_str.contains("no such storage")) && attempts < max_retries {
                        info!("Storage error on attempt {}/{}, retrying after {:?}: {}", attempts + 1, max_retries + 1, SLEEP_INTERVAL, err_str);
                        attempts += 1;
                        sleep(SLEEP_INTERVAL).await;
                    } else {
                        return Err(err);
                    }
                }
            }
        }
    }

    async fn rest_close_team(&self, team_id: TeamId) -> Result<()> {
        let url = format!("http://{}/api/v1/teams/{}", self.rest_server_addr, hex::encode(team_id.into_id().as_bytes()));
        let response = reqwest::Client::new()
            .delete(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to close team: {}", response.status());
        }

        Ok(())
    }
}

#[tokio::test]
#[test_log::test]
async fn test_team_management_endpoints() -> Result<()> {
    info!("Starting team management endpoints test");
    
    let device = RestDeviceCtx::new("team-mgmt-test").await?;
    
    // Test create team via REST (already tested, but verify again)
    let team_id = device.rest_create_team().await?;
    info!("Created team via REST: {:?}", team_id);
    
    // Test query devices on team (initially should just have the owner)
    let device_ids = device.rest_query_devices_on_team(team_id).await?;
    assert_eq!(device_ids.len(), 1, "Team should initially have 1 device (owner)");
    info!("Found {} devices on team", device_ids.len());
    
    // Verify the device info matches
    let device_id_str = &device_ids[0];
    let device_id_bytes = hex::decode(device_id_str)?;
    let mut array = [0u8; 32];
    array.copy_from_slice(&device_id_bytes);
    let returned_device_id = DeviceId::from(array);
    assert_eq!(returned_device_id, device.id, "Device ID should match");
    
    // TODO: Test close team when daemon API implements it
    // info!("Closing team via REST");
    // device.rest_close_team(team_id).await?;
    
    info!("Team management endpoints test completed successfully");
    Ok(())
}

#[tokio::test]
#[test_log::test]
async fn test_team_lifecycle_multi_daemon() -> Result<()> {
    info!("Starting multi-daemon team lifecycle test");
    
    let device1 = RestDeviceCtx::new("team-lifecycle-1").await?;
    let device2 = RestDeviceCtx::new("team-lifecycle-2").await?;
    
    // Create team on device1
    let team_id = device1.rest_create_team().await?;
    
    // Add device2 to team
    device1.rest_add_device_to_team(team_id, &device2.pk).await?;
    
    // Set up sync peers so devices can exchange information
    let device1_addr = device1.get_local_addr().await?;
    let device2_addr = device2.get_local_addr().await?;
    
    info!("Setting up sync peers for team information exchange");
    device1.rest_add_sync_peer(team_id, &device2_addr, Some(1)).await?;
    device2.rest_add_sync_peer(team_id, &device1_addr, Some(1)).await?;
    
    // Wait longer for sync to propagate and storage to be established
    sleep(LONG_SLEEP_INTERVAL).await;
    
    // Query devices from both devices with retry for storage errors
    let device_ids1 = device1.rest_query_devices_on_team_with_retry(team_id).await?;
    let device_ids2 = device2.rest_query_devices_on_team_with_retry(team_id).await?;
    
    // Both should see 2 devices
    assert_eq!(device_ids1.len(), 2, "Device1 should see 2 devices");
    assert_eq!(device_ids2.len(), 2, "Device2 should see 2 devices");
    
    // Both lists should contain the same device IDs (order might differ)
    let ids1: std::collections::HashSet<String> = device_ids1.into_iter().collect();
    let ids2: std::collections::HashSet<String> = device_ids2.into_iter().collect();
    assert_eq!(ids1, ids2, "Both devices should see the same device IDs");
    
    info!("Multi-daemon team lifecycle test completed successfully");
    Ok(())
}

// ====== Device Management Within Teams Tests ======

#[derive(Serialize, Deserialize)]
struct RemoveDeviceRequest {
    // Empty body for device removal - device_id is in the URL path
}


impl RestDeviceCtx {
    async fn rest_remove_device_from_team(&self, team_id: TeamId, device_id: DeviceId) -> Result<()> {
        let url = format!(
            "http://{}/api/v1/teams/{}/devices/{}", 
            self.rest_server_addr, 
            hex::encode(team_id.into_id().as_bytes()),
            hex::encode(device_id.into_id().as_bytes())
        );
        
        let response = reqwest::Client::new()
            .delete(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to remove device from team: {}", response.status());
        }

        Ok(())
    }

    async fn rest_query_device_role(&self, team_id: TeamId, device_id: DeviceId) -> Result<Role> {
        let url = format!(
            "http://{}/api/v1/teams/{}/devices/{}/role", 
            self.rest_server_addr, 
            hex::encode(team_id.into_id().as_bytes()),
            hex::encode(device_id.into_id().as_bytes())
        );
        
        let response = reqwest::get(&url).await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to query device role: {}", response.status());
        }

        let role_str: String = response.json().await?;
        let role = match role_str.as_str() {
            "Owner" => Role::Owner,
            "Admin" => Role::Admin,
            "Operator" => Role::Operator, 
            "Member" => Role::Member,
            _ => anyhow::bail!("Unknown role: {}", role_str),
        };
        Ok(role)
    }

    async fn rest_query_device_role_with_retry(&self, team_id: TeamId, device_id: DeviceId) -> Result<Role> {
        let mut attempts = 0;
        let max_retries = 5;
        
        loop {
            match self.rest_query_device_role(team_id, device_id).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    let err_str = err.to_string();
                    if (err_str.contains("storage error") || err_str.contains("no such storage")) && attempts < max_retries {
                        info!("Storage error on attempt {}/{}, retrying after {:?}: {}", attempts + 1, max_retries + 1, SLEEP_INTERVAL, err_str);
                        attempts += 1;
                        sleep(SLEEP_INTERVAL).await;
                    } else {
                        return Err(err);
                    }
                }
            }
        }
    }

    async fn rest_query_device_keybundle(&self, team_id: TeamId, device_id: DeviceId) -> Result<KeyBundle> {
        let url = format!(
            "http://{}/api/v1/teams/{}/devices/{}/keybundle", 
            self.rest_server_addr, 
            hex::encode(team_id.into_id().as_bytes()),
            hex::encode(device_id.into_id().as_bytes())
        );
        
        let response = reqwest::get(&url).await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to query device keybundle: {}", response.status());
        }

        let keybundle: KeyBundle = response.json().await?;
        Ok(keybundle)
    }
}

#[tokio::test]
#[test_log::test]
async fn test_device_management_within_teams() -> Result<()> {
    info!("Starting device management within teams test");
    
    let device1 = RestDeviceCtx::new("device-mgmt-1").await?;
    let device2 = RestDeviceCtx::new("device-mgmt-2").await?;
    let device3 = RestDeviceCtx::new("device-mgmt-3").await?;
    
    // Create team on device1
    let team_id = device1.rest_create_team().await?;
    
    // Add device2 and device3 to team
    device1.rest_add_device_to_team(team_id, &device2.pk).await?;
    device1.rest_add_device_to_team(team_id, &device3.pk).await?;
    
    // Set up sync peers for all devices
    let device1_addr = device1.get_local_addr().await?;
    let device2_addr = device2.get_local_addr().await?;
    let device3_addr = device3.get_local_addr().await?;
    
    info!("Setting up sync peers between all devices");
    // Device1 <-> Device2  
    device1.rest_add_sync_peer(team_id, &device2_addr, Some(1)).await?;
    device2.rest_add_sync_peer(team_id, &device1_addr, Some(1)).await?;
    
    // Device1 <-> Device3
    device1.rest_add_sync_peer(team_id, &device3_addr, Some(1)).await?;
    device3.rest_add_sync_peer(team_id, &device1_addr, Some(1)).await?;
    
    // Device2 <-> Device3
    device2.rest_add_sync_peer(team_id, &device3_addr, Some(1)).await?;
    device3.rest_add_sync_peer(team_id, &device2_addr, Some(1)).await?;
    
    // Wait longer for sync to complete
    sleep(LONG_SLEEP_INTERVAL).await;
    
    // Verify all devices see 3 devices (with retry for storage errors)
    let device_ids1 = device1.rest_query_devices_on_team_with_retry(team_id).await?;
    let device_ids2 = device2.rest_query_devices_on_team_with_retry(team_id).await?;
    let device_ids3 = device3.rest_query_devices_on_team_with_retry(team_id).await?;
    
    assert_eq!(device_ids1.len(), 3, "Device1 should see 3 devices");
    assert_eq!(device_ids2.len(), 3, "Device2 should see 3 devices");
    assert_eq!(device_ids3.len(), 3, "Device3 should see 3 devices");
    
    // Test querying specific device roles
    let device1_role = device1.rest_query_device_role_with_retry(team_id, device1.id).await?;
    assert_eq!(device1_role, Role::Owner, "Device1 should be Owner");
    
    // Test querying specific device keybundles
    let device2_keybundle = device1.rest_query_device_keybundle(team_id, device2.id).await?;
    assert_eq!(device2_keybundle, device2.pk, "Device2 keybundle should match");
    
    // Test removing a device from team
    info!("Removing device3 from team");
    device1.rest_remove_device_from_team(team_id, device3.id).await?;
    
    // Wait for sync
    sleep(LONG_SLEEP_INTERVAL).await;
    
    // Verify device count decreased
    let device_ids_after_removal = device1.rest_query_devices_on_team_with_retry(team_id).await?;
    assert_eq!(device_ids_after_removal.len(), 2, "Should have 2 devices after removal");
    
    info!("Device management within teams test completed successfully");
    Ok(())
}

// ====== Role Management Tests ======

#[derive(Serialize, Deserialize)]
struct RevokeRoleRequest {
    device_id: String,
    role: String,
}

impl RestDeviceCtx {
    async fn rest_revoke_role(&self, team_id: TeamId, device_id: DeviceId, role: Role) -> Result<()> {
        let url = format!("http://{}/api/v1/teams/{}/roles/revoke", self.rest_server_addr, hex::encode(team_id.into_id().as_bytes()));
        let role_str = match role {
            Role::Owner => "Owner",
            Role::Admin => "Admin", 
            Role::Operator => "Operator",
            Role::Member => "Member",
        };
        let request = RevokeRoleRequest {
            device_id: hex::encode(device_id.into_id().as_bytes()),
            role: role_str.to_string(),
        };
        
        let response = reqwest::Client::new()
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to revoke role: {}", response.status());
        }

        Ok(())
    }
}

#[tokio::test]
#[test_log::test]
async fn test_role_management_comprehensive() -> Result<()> {
    info!("Starting comprehensive role management test");
    
    // Create 4 devices to test the full hierarchy: Owner -> Admin -> Operator -> Member
    let owner = RestDeviceCtx::new("role-owner").await?;
    let admin = RestDeviceCtx::new("role-admin").await?;
    let operator = RestDeviceCtx::new("role-operator").await?;
    let member = RestDeviceCtx::new("role-member").await?;
    
    // Create team on owner
    let team_id = owner.rest_create_team().await?;
    
    // Add all devices to team
    owner.rest_add_device_to_team(team_id, &admin.pk).await?;
    owner.rest_add_device_to_team(team_id, &operator.pk).await?;
    owner.rest_add_device_to_team(team_id, &member.pk).await?;
    
    // Set up sync peers between all devices for proper role propagation
    let owner_addr = owner.get_local_addr().await?;
    let admin_addr = admin.get_local_addr().await?;
    let operator_addr = operator.get_local_addr().await?;
    let member_addr = member.get_local_addr().await?;
    
    info!("Setting up sync peers for role management");
    // Owner with all others
    owner.rest_add_sync_peer(team_id, &admin_addr, Some(1)).await?;
    owner.rest_add_sync_peer(team_id, &operator_addr, Some(1)).await?;
    owner.rest_add_sync_peer(team_id, &member_addr, Some(1)).await?;
    
    // Admin with all others
    admin.rest_add_sync_peer(team_id, &owner_addr, Some(1)).await?;
    admin.rest_add_sync_peer(team_id, &operator_addr, Some(1)).await?;
    admin.rest_add_sync_peer(team_id, &member_addr, Some(1)).await?;
    
    // Operator with all others
    operator.rest_add_sync_peer(team_id, &owner_addr, Some(1)).await?;
    operator.rest_add_sync_peer(team_id, &admin_addr, Some(1)).await?;
    operator.rest_add_sync_peer(team_id, &member_addr, Some(1)).await?;
    
    // Member with all others
    member.rest_add_sync_peer(team_id, &owner_addr, Some(1)).await?;
    member.rest_add_sync_peer(team_id, &admin_addr, Some(1)).await?;
    member.rest_add_sync_peer(team_id, &operator_addr, Some(1)).await?;
    
    // Test role assignments following hierarchy
    info!("Assigning Admin role");
    owner.rest_assign_role(team_id, admin.id, Role::Admin).await?;
    sleep(LONG_SLEEP_INTERVAL).await;
    
    info!("Assigning Operator role (by Admin)");
    admin.rest_assign_role(team_id, operator.id, Role::Operator).await?;
    sleep(LONG_SLEEP_INTERVAL).await;
    
    // Note: Operators can only add devices to teams, not assign roles
    // Members added by operators get default Member role automatically
    info!("Member gets default Member role (operators cannot assign roles)");
    sleep(LONG_SLEEP_INTERVAL).await;
    
    // Verify roles from different devices (with retry for storage errors)
    let owner_role = owner.rest_query_device_role_with_retry(team_id, owner.id).await?;
    let admin_role = admin.rest_query_device_role_with_retry(team_id, admin.id).await?;
    let operator_role = operator.rest_query_device_role_with_retry(team_id, operator.id).await?;
    let member_role = member.rest_query_device_role_with_retry(team_id, member.id).await?;
    
    assert_eq!(owner_role, Role::Owner, "Owner should have Owner role");
    assert_eq!(admin_role, Role::Admin, "Admin should have Admin role");
    assert_eq!(operator_role, Role::Operator, "Operator should have Operator role");
    assert_eq!(member_role, Role::Member, "Member should have default Member role");
    
    // Test role queries from different devices to ensure sync worked
    let admin_role_from_member = member.rest_query_device_role_with_retry(team_id, admin.id).await?;
    assert_eq!(admin_role_from_member, Role::Admin, "Member should see Admin's role correctly");
    
    // Test role revocation
    info!("Testing role revocation");
    admin.rest_revoke_role(team_id, operator.id, Role::Operator).await?;
    sleep(LONG_SLEEP_INTERVAL).await;
    
    // Verify role was revoked (should be Member now)
    let operator_role_after_revoke = owner.rest_query_device_role_with_retry(team_id, operator.id).await?;
    assert_eq!(operator_role_after_revoke, Role::Member, "Operator should be Member after role revocation");
    
    info!("Comprehensive role management test completed successfully");
    Ok(())
}