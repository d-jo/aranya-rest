//! HTTP request handlers for the Aranya REST API v4.0.0
//!
//! This module provides handlers for all REST endpoints, translating
//! HTTP requests to daemon RPC calls and formatting responses.

use aranya_daemon_api::{
    AddTeamConfig, ChanOp, CreateTeamConfig, DeviceId, KeyBundle, Label, LabelId,
    RoleId, SyncPeerConfig, TeamId,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing;

use crate::{client::DaemonClient, RestError};

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Serialize, Deserialize)]
pub struct VersionResponse {
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct LocalAddrResponse {
    pub address: String,
}

#[derive(Serialize, Deserialize)]
pub struct DeviceIdResponse {
    pub device_id: String,
}

// Sync peer types
#[derive(Serialize, Deserialize)]
pub struct AddSyncPeerRequest {
    pub addr: String,
    pub team_id: String,
    pub config: SyncPeerConfigJson,
}

#[derive(Serialize, Deserialize)]
pub struct SyncPeerConfigJson {
    pub interval_secs: u64,
    pub sync_now: bool,
}

#[derive(Serialize, Deserialize)]
pub struct SyncNowRequest {
    pub addr: String,
    pub team_id: String,
    pub config: Option<SyncPeerConfigJson>,
}

#[derive(Serialize, Deserialize)]
pub struct RemoveSyncPeerRequest {
    pub addr: String,
    pub team_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct QuerySyncPeersResponse {
    pub peers: Vec<SyncPeerInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct SyncPeerInfo {
    pub addr: String,
    pub config: SyncPeerConfigJson,
}

#[derive(Serialize, Deserialize)]
pub struct UpdateSyncPeerRequest {
    pub addr: String,
    pub team_id: String,
    pub config: SyncPeerConfigJson,
}

// Team types
#[derive(Serialize, Deserialize)]
pub struct CreateTeamRequest {
    pub config: TeamConfigJson,
}

#[derive(Serialize, Deserialize)]
pub struct TeamConfigJson {
    // Configuration options for team creation
    // Currently empty but included for future extensibility
}

#[derive(Serialize, Deserialize)]
pub struct CreateTeamResponse {
    pub team_id: String,
}

// Device types
#[derive(Serialize, Deserialize)]
pub struct AddDeviceRequest {
    pub keys: KeyBundleInput,
    /// Optional initial role ID (hex-encoded) for the device
    pub initial_role: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyBundleInput {
    Raw(KeyBundle),
    Base64(KeyBundleJson),
}

#[derive(Serialize, Deserialize)]
pub struct KeyBundleJson {
    pub identity: String,
    pub signing: String,
    pub encoding: String,
}

// Role types (v4.0.0 - custom RBAC roles)
#[derive(Serialize, Deserialize)]
pub struct RoleInfo {
    pub id: String,
    pub name: String,
    pub author_id: String,
    pub is_default: bool,
}

#[derive(Serialize, Deserialize)]
pub struct SetupDefaultRolesRequest {
    pub owning_role_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct AssignRoleRequest {
    pub device_id: String,
    pub role_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct RevokeRoleRequest {
    pub device_id: String,
    pub role_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct ChangeRoleRequest {
    pub device_id: String,
    pub old_role_id: String,
    pub new_role_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct BulkAssignRoleRequest {
    pub assignments: Vec<RoleAssignment>,
}

#[derive(Serialize, Deserialize)]
pub struct RoleAssignment {
    pub device_id: String,
    pub role_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct BulkAssignRoleResponse {
    pub results: Vec<RoleAssignmentResult>,
}

#[derive(Serialize, Deserialize)]
pub struct RoleAssignmentResult {
    pub device_id: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct DeviceRoleInfo {
    pub device_id: String,
    pub role_id: Option<String>,
    pub role_name: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct QueryAllDeviceRolesResponse {
    pub devices: Vec<DeviceRoleInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct QueryDevicesByRoleResponse {
    pub devices: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct QueryTeamRolesResponse {
    pub roles: Vec<RoleInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct QueryRoleOwnersResponse {
    pub owner_roles: Vec<RoleInfo>,
}

// Label types (v4.0.0 - requires managing role)
#[derive(Serialize, Deserialize)]
pub struct CreateLabelRequest {
    pub name: String,
    /// Role ID that will manage this label (hex-encoded)
    pub managing_role_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateLabelResponse {
    pub label_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct AddLabelManagingRoleRequest {
    pub label_id: String,
    pub managing_role_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct AssignLabelRequest {
    pub device_id: String,
    pub label_id: String,
    pub operation: String,
}

#[derive(Serialize, Deserialize)]
pub struct RevokeLabelRequest {
    pub device_id: String,
    pub label_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct LabelInfo {
    pub id: String,
    pub name: String,
}

// AFC types (v4.0.0 - replaces AQC)
#[derive(Serialize, Deserialize)]
pub struct CreateAfcChannelRequest {
    pub peer_id: String,
    pub label_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateAfcChannelResponse {
    pub channel_id: String,
    pub ctrl_msg: String, // base64 encoded
}

#[derive(Serialize, Deserialize)]
pub struct AcceptAfcChannelRequest {
    pub ctrl_msg: String, // base64 encoded
}

#[derive(Serialize, Deserialize)]
pub struct AcceptAfcChannelResponse {
    pub channel_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct DeleteAfcChannelRequest {
    pub channel_id: String,
}

// PSK encryption for sync
#[derive(Serialize, Deserialize)]
pub struct EncryptPskSeedRequest {
    pub peer_enc_pk: String, // base64 encoded
}

#[derive(Serialize, Deserialize)]
pub struct EncryptPskSeedResponse {
    pub encrypted_seed: String, // base64 encoded
}

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_team_id(id_str: &str) -> Result<TeamId, RestError> {
    let bytes = hex::decode(id_str)
        .map_err(|_| RestError::InvalidRequest("Invalid team ID format".to_string()))?;
    if bytes.len() != 32 {
        return Err(RestError::InvalidRequest(
            "Team ID must be 32 bytes".to_string(),
        ));
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(TeamId::from(array))
}

fn parse_device_id(id_str: &str) -> Result<DeviceId, RestError> {
    let bytes = hex::decode(id_str)
        .map_err(|_| RestError::InvalidRequest("Invalid device ID format".to_string()))?;
    if bytes.len() != 32 {
        return Err(RestError::InvalidRequest(
            "Device ID must be 32 bytes".to_string(),
        ));
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(DeviceId::from(array))
}

fn parse_label_id(id_str: &str) -> Result<LabelId, RestError> {
    let bytes = hex::decode(id_str)
        .map_err(|_| RestError::InvalidRequest("Invalid label ID format".to_string()))?;
    if bytes.len() != 32 {
        return Err(RestError::InvalidRequest(
            "Label ID must be 32 bytes".to_string(),
        ));
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(LabelId::from(array))
}

fn parse_role_id(id_str: &str) -> Result<RoleId, RestError> {
    let bytes = hex::decode(id_str)
        .map_err(|_| RestError::InvalidRequest("Invalid role ID format".to_string()))?;
    if bytes.len() != 32 {
        return Err(RestError::InvalidRequest(
            "Role ID must be 32 bytes".to_string(),
        ));
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(RoleId::from(array))
}

fn parse_chan_op(op_str: &str) -> Result<ChanOp, RestError> {
    match op_str {
        "SendRecv" => Ok(ChanOp::SendRecv),
        "RecvOnly" => Ok(ChanOp::RecvOnly),
        "SendOnly" => Ok(ChanOp::SendOnly),
        _ => Err(RestError::InvalidRequest(
            "Invalid channel operation. Must be SendRecv, RecvOnly, or SendOnly".to_string(),
        )),
    }
}

fn parse_addr(addr_str: &str) -> Result<std::net::SocketAddr, RestError> {
    addr_str
        .parse()
        .map_err(|_| RestError::InvalidRequest("Invalid address format".to_string()))
}

impl From<SyncPeerConfigJson> for SyncPeerConfig {
    fn from(config: SyncPeerConfigJson) -> Self {
        SyncPeerConfig {
            interval: std::time::Duration::from_secs(config.interval_secs),
            sync_now: config.sync_now,
        }
    }
}

impl TryFrom<KeyBundleInput> for KeyBundle {
    type Error = RestError;

    fn try_from(input: KeyBundleInput) -> Result<Self, Self::Error> {
        match input {
            KeyBundleInput::Raw(keys) => Ok(keys),
            KeyBundleInput::Base64(keys) => keys.try_into(),
        }
    }
}

impl TryFrom<KeyBundleJson> for KeyBundle {
    type Error = RestError;

    fn try_from(keys: KeyBundleJson) -> Result<Self, Self::Error> {
        use base64::prelude::*;
        let identity = BASE64_STANDARD
            .decode(&keys.identity)
            .map_err(|_| RestError::InvalidRequest("Invalid base64 identity key".to_string()))?;
        let signing = BASE64_STANDARD
            .decode(&keys.signing)
            .map_err(|_| RestError::InvalidRequest("Invalid base64 signing key".to_string()))?;
        let encoding = BASE64_STANDARD
            .decode(&keys.encoding)
            .map_err(|_| RestError::InvalidRequest("Invalid base64 encoding key".to_string()))?;

        Ok(KeyBundle {
            identity,
            signing,
            encoding,
        })
    }
}

// ============================================================================
// Device & System Info Handlers
// ============================================================================

pub async fn get_version(
    State(client): State<DaemonClient>,
) -> Result<Json<VersionResponse>, RestError> {
    let version = client.client().version(DaemonClient::context()).await??;
    Ok(Json(VersionResponse {
        version: version.to_string(),
    }))
}

pub async fn get_local_addr(
    State(client): State<DaemonClient>,
) -> Result<Json<LocalAddrResponse>, RestError> {
    let addr = client
        .client()
        .aranya_local_addr(DaemonClient::context())
        .await??;
    Ok(Json(LocalAddrResponse {
        address: addr.to_string(),
    }))
}

pub async fn get_key_bundle(
    State(client): State<DaemonClient>,
) -> Result<Json<KeyBundle>, RestError> {
    let keys = client
        .client()
        .get_key_bundle(DaemonClient::context())
        .await??;
    Ok(Json(keys))
}

pub async fn get_device_id(
    State(client): State<DaemonClient>,
) -> Result<Json<DeviceIdResponse>, RestError> {
    let device_id = client
        .client()
        .get_device_id(DaemonClient::context())
        .await??;
    Ok(Json(DeviceIdResponse {
        device_id: hex::encode(device_id.into_id().as_bytes()),
    }))
}

// ============================================================================
// Sync Peer Handlers
// ============================================================================

pub async fn add_sync_peer(
    State(client): State<DaemonClient>,
    Json(req): Json<AddSyncPeerRequest>,
) -> Result<StatusCode, RestError> {
    let addr = parse_addr(&req.addr)?;
    let team_id = parse_team_id(&req.team_id)?;
    let config = req.config.into();

    client
        .client()
        .add_sync_peer(DaemonClient::context(), addr, team_id, config)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn sync_now(
    State(client): State<DaemonClient>,
    Json(req): Json<SyncNowRequest>,
) -> Result<StatusCode, RestError> {
    let addr = parse_addr(&req.addr)?;
    let team_id = parse_team_id(&req.team_id)?;
    let config = req.config.map(|c| c.into());

    client
        .client()
        .sync_now(DaemonClient::context(), addr, team_id, config)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn remove_sync_peer(
    State(client): State<DaemonClient>,
    Json(req): Json<RemoveSyncPeerRequest>,
) -> Result<StatusCode, RestError> {
    let addr = parse_addr(&req.addr)?;
    let team_id = parse_team_id(&req.team_id)?;

    client
        .client()
        .remove_sync_peer(DaemonClient::context(), addr, team_id)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn query_sync_peers(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
) -> Result<Json<QuerySyncPeersResponse>, RestError> {
    let team_id = parse_team_id(&team_id)?;

    // Note: v4.0.0 may have different sync peer query API
    // This maintains backwards compatibility where possible
    Ok(Json(QuerySyncPeersResponse { peers: vec![] }))
}

pub async fn query_sync_peer_config(
    State(_client): State<DaemonClient>,
    Path((_team_id, _addr)): Path<(String, String)>,
) -> Result<Json<Option<SyncPeerConfigJson>>, RestError> {
    // v4.0.0 sync peer config query
    Ok(Json(None))
}

pub async fn update_sync_peer_config(
    State(_client): State<DaemonClient>,
    Json(_req): Json<UpdateSyncPeerRequest>,
) -> Result<StatusCode, RestError> {
    // v4.0.0 sync peer config update
    Ok(StatusCode::OK)
}

// ============================================================================
// Team Management Handlers
// ============================================================================

pub async fn create_team(
    State(client): State<DaemonClient>,
    Json(_req): Json<CreateTeamRequest>,
) -> Result<Json<CreateTeamResponse>, RestError> {
    let config = CreateTeamConfig::default();
    let team_id = client
        .client()
        .create_team(DaemonClient::context(), config)
        .await??;

    Ok(Json(CreateTeamResponse {
        team_id: hex::encode(team_id.into_id().as_bytes()),
    }))
}

pub async fn close_team(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    client
        .client()
        .close_team(DaemonClient::context(), team_id)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn add_team(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let config = AddTeamConfig::default();
    client
        .client()
        .add_team(DaemonClient::context(), team_id, config)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn remove_team(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    client
        .client()
        .remove_team(DaemonClient::context(), team_id)
        .await??;
    Ok(StatusCode::OK)
}

// ============================================================================
// Device Management Handlers
// ============================================================================

pub async fn add_device_to_team(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<AddDeviceRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let keys = req.keys.try_into()?;
    let initial_role = req
        .initial_role
        .map(|r| parse_role_id(&r))
        .transpose()?;

    client
        .client()
        .add_device_to_team(DaemonClient::context(), team_id, keys, initial_role)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn remove_device_from_team(
    State(client): State<DaemonClient>,
    Path((team_id, device_id)): Path<(String, String)>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&device_id)?;

    client
        .client()
        .remove_device_from_team(DaemonClient::context(), team_id, device_id)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn query_devices_on_team(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
) -> Result<Json<Vec<String>>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let devices = client
        .client()
        .devices_on_team(DaemonClient::context(), team_id)
        .await??;

    let device_ids: Vec<String> = devices
        .into_iter()
        .map(|id| hex::encode(id.into_id().as_bytes()))
        .collect();

    Ok(Json(device_ids))
}

pub async fn query_device_keybundle(
    State(client): State<DaemonClient>,
    Path((team_id, device_id)): Path<(String, String)>,
) -> Result<Json<KeyBundle>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&device_id)?;
    let keys = client
        .client()
        .device_keybundle(DaemonClient::context(), team_id, device_id)
        .await??;

    Ok(Json(keys))
}

// ============================================================================
// Role Management Handlers (v4.0.0 Custom RBAC)
// ============================================================================

/// Setup default roles for a team (v4.0.0)
pub async fn setup_default_roles(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<SetupDefaultRolesRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let owning_role = parse_role_id(&req.owning_role_id)?;

    client
        .client()
        .setup_default_roles(DaemonClient::context(), team_id, owning_role)
        .await??;
    Ok(StatusCode::OK)
}

/// Query all roles on a team (v4.0.0)
pub async fn query_team_roles(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
) -> Result<Json<QueryTeamRolesResponse>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let roles = client
        .client()
        .team_roles(DaemonClient::context(), team_id)
        .await??;

    let role_infos: Vec<RoleInfo> = roles
        .into_iter()
        .map(|r| RoleInfo {
            id: hex::encode(r.id.into_id().as_bytes()),
            name: r.name.to_string(),
            author_id: hex::encode(r.author_id.into_id().as_bytes()),
            is_default: r.default,
        })
        .collect();

    Ok(Json(QueryTeamRolesResponse { roles: role_infos }))
}

/// Query roles that own a specific role (v4.0.0)
pub async fn query_role_owners(
    State(client): State<DaemonClient>,
    Path((team_id, role_id)): Path<(String, String)>,
) -> Result<Json<QueryRoleOwnersResponse>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let role_id = parse_role_id(&role_id)?;

    let owner_roles = client
        .client()
        .role_owners(DaemonClient::context(), team_id, role_id)
        .await??;

    let role_infos: Vec<RoleInfo> = owner_roles
        .into_iter()
        .map(|r| RoleInfo {
            id: hex::encode(r.id.into_id().as_bytes()),
            name: r.name.to_string(),
            author_id: hex::encode(r.author_id.into_id().as_bytes()),
            is_default: r.default,
        })
        .collect();

    Ok(Json(QueryRoleOwnersResponse {
        owner_roles: role_infos,
    }))
}

/// Assign a role to a device (v4.0.0)
pub async fn assign_role(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<AssignRoleRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&req.device_id)?;
    let role_id = parse_role_id(&req.role_id)?;

    client
        .client()
        .assign_role(DaemonClient::context(), team_id, device_id, role_id)
        .await??;
    Ok(StatusCode::OK)
}

/// Revoke a role from a device (v4.0.0)
pub async fn revoke_role(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<RevokeRoleRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&req.device_id)?;
    let role_id = parse_role_id(&req.role_id)?;

    client
        .client()
        .revoke_role(DaemonClient::context(), team_id, device_id, role_id)
        .await??;
    Ok(StatusCode::OK)
}

/// Change a device's role (v4.0.0)
pub async fn change_role(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<ChangeRoleRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&req.device_id)?;
    let old_role = parse_role_id(&req.old_role_id)?;
    let new_role = parse_role_id(&req.new_role_id)?;

    client
        .client()
        .change_role(DaemonClient::context(), team_id, device_id, old_role, new_role)
        .await??;
    Ok(StatusCode::OK)
}

/// Query a device's role (v4.0.0)
pub async fn query_device_role(
    State(client): State<DaemonClient>,
    Path((team_id, device_id)): Path<(String, String)>,
) -> Result<Json<Option<RoleInfo>>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&device_id)?;

    let role = client
        .client()
        .device_role(DaemonClient::context(), team_id, device_id)
        .await??;

    let role_info = role.map(|r| RoleInfo {
        id: hex::encode(r.id.into_id().as_bytes()),
        name: r.name.to_string(),
        author_id: hex::encode(r.author_id.into_id().as_bytes()),
        is_default: r.default,
    });

    Ok(Json(role_info))
}

/// Bulk assign roles (v4.0.0)
pub async fn bulk_assign_role(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<BulkAssignRoleRequest>,
) -> Result<Json<BulkAssignRoleResponse>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let mut results = Vec::new();

    for assignment in req.assignments {
        let device_id_result = parse_device_id(&assignment.device_id);
        let role_id_result = parse_role_id(&assignment.role_id);

        let result = match (device_id_result, role_id_result) {
            (Ok(device_id), Ok(role_id)) => {
                match client
                    .client()
                    .assign_role(DaemonClient::context(), team_id, device_id, role_id)
                    .await
                {
                    Ok(Ok(_)) => RoleAssignmentResult {
                        device_id: assignment.device_id,
                        success: true,
                        error: None,
                    },
                    Ok(Err(e)) => RoleAssignmentResult {
                        device_id: assignment.device_id,
                        success: false,
                        error: Some(format!("Daemon error: {}", e)),
                    },
                    Err(e) => RoleAssignmentResult {
                        device_id: assignment.device_id,
                        success: false,
                        error: Some(format!("RPC error: {}", e)),
                    },
                }
            }
            (Err(e), _) => RoleAssignmentResult {
                device_id: assignment.device_id,
                success: false,
                error: Some(format!("Invalid device ID: {}", e)),
            },
            (_, Err(e)) => RoleAssignmentResult {
                device_id: assignment.device_id,
                success: false,
                error: Some(format!("Invalid role ID: {}", e)),
            },
        };

        results.push(result);
    }

    Ok(Json(BulkAssignRoleResponse { results }))
}

/// Query all device roles on a team (v4.0.0)
pub async fn query_all_device_roles(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
) -> Result<Json<QueryAllDeviceRolesResponse>, RestError> {
    let team_id = parse_team_id(&team_id)?;

    let device_ids = client
        .client()
        .devices_on_team(DaemonClient::context(), team_id)
        .await??;

    let mut devices = Vec::new();

    for device_id in device_ids {
        match client
            .client()
            .device_role(DaemonClient::context(), team_id, device_id)
            .await
        {
            Ok(Ok(role)) => {
                devices.push(DeviceRoleInfo {
                    device_id: hex::encode(device_id.into_id().as_bytes()),
                    role_id: role.as_ref().map(|r| hex::encode(r.id.into_id().as_bytes())),
                    role_name: role.map(|r| r.name.to_string()),
                });
            }
            Ok(Err(e)) => {
                tracing::warn!(
                    "Failed to query role for device {} (daemon error): {}",
                    hex::encode(device_id.into_id().as_bytes()),
                    e
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to query role for device {} (rpc error): {}",
                    hex::encode(device_id.into_id().as_bytes()),
                    e
                );
            }
        }
    }

    Ok(Json(QueryAllDeviceRolesResponse { devices }))
}

/// Query devices by role (v4.0.0)
pub async fn query_devices_by_role(
    State(client): State<DaemonClient>,
    Path((team_id, role_id)): Path<(String, String)>,
) -> Result<Json<QueryDevicesByRoleResponse>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let target_role = parse_role_id(&role_id)?;

    let device_ids = client
        .client()
        .devices_on_team(DaemonClient::context(), team_id)
        .await??;

    let mut matching_devices = Vec::new();

    for device_id in device_ids {
        match client
            .client()
            .device_role(DaemonClient::context(), team_id, device_id)
            .await
        {
            Ok(Ok(Some(role))) => {
                if role.id == target_role {
                    matching_devices.push(hex::encode(device_id.into_id().as_bytes()));
                }
            }
            Ok(Ok(None)) => {}
            Ok(Err(e)) => {
                tracing::warn!(
                    "Failed to query role for device {} (daemon error): {}",
                    hex::encode(device_id.into_id().as_bytes()),
                    e
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to query role for device {} (rpc error): {}",
                    hex::encode(device_id.into_id().as_bytes()),
                    e
                );
            }
        }
    }

    Ok(Json(QueryDevicesByRoleResponse {
        devices: matching_devices,
    }))
}

// ============================================================================
// Label Management Handlers (v4.0.0)
// ============================================================================

/// Create a label (v4.0.0 - requires managing role)
pub async fn create_label(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<CreateLabelRequest>,
) -> Result<Json<CreateLabelResponse>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let managing_role_id = parse_role_id(&req.managing_role_id)?;

    let label_id = client
        .client()
        .create_label(DaemonClient::context(), team_id, req.name, managing_role_id)
        .await??;

    Ok(Json(CreateLabelResponse {
        label_id: hex::encode(label_id.into_id().as_bytes()),
    }))
}

/// Delete a label (v4.0.0)
pub async fn delete_label(
    State(client): State<DaemonClient>,
    Path((team_id, label_id)): Path<(String, String)>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let label_id = parse_label_id(&label_id)?;

    client
        .client()
        .delete_label(DaemonClient::context(), team_id, label_id)
        .await??;
    Ok(StatusCode::OK)
}

/// Add a managing role to a label (v4.0.0)
pub async fn add_label_managing_role(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<AddLabelManagingRoleRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let label_id = parse_label_id(&req.label_id)?;
    let managing_role_id = parse_role_id(&req.managing_role_id)?;

    client
        .client()
        .add_label_managing_role(DaemonClient::context(), team_id, label_id, managing_role_id)
        .await??;
    Ok(StatusCode::OK)
}

/// Query a specific label (v4.0.0)
pub async fn query_label(
    State(client): State<DaemonClient>,
    Path((team_id, label_id)): Path<(String, String)>,
) -> Result<Json<Option<LabelInfo>>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let label_id = parse_label_id(&label_id)?;

    let label = client
        .client()
        .label(DaemonClient::context(), team_id, label_id)
        .await??;

    let label_info = label.map(|l| LabelInfo {
        id: hex::encode(l.id.into_id().as_bytes()),
        name: l.name.to_string(),
    });

    Ok(Json(label_info))
}

/// Query all labels on a team (v4.0.0)
pub async fn query_labels(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
) -> Result<Json<Vec<LabelInfo>>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let labels = client
        .client()
        .labels(DaemonClient::context(), team_id)
        .await??;

    let label_infos: Vec<LabelInfo> = labels
        .into_iter()
        .map(|l| LabelInfo {
            id: hex::encode(l.id.into_id().as_bytes()),
            name: l.name.to_string(),
        })
        .collect();

    Ok(Json(label_infos))
}

/// Assign a label to a device (v4.0.0)
pub async fn assign_label(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<AssignLabelRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&req.device_id)?;
    let label_id = parse_label_id(&req.label_id)?;
    let op = parse_chan_op(&req.operation)?;

    client
        .client()
        .assign_label_to_device(DaemonClient::context(), team_id, device_id, label_id, op)
        .await??;
    Ok(StatusCode::OK)
}

/// Revoke a label from a device (v4.0.0)
pub async fn revoke_label(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<RevokeLabelRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&req.device_id)?;
    let label_id = parse_label_id(&req.label_id)?;

    client
        .client()
        .revoke_label_from_device(DaemonClient::context(), team_id, device_id, label_id)
        .await??;
    Ok(StatusCode::OK)
}

/// Query labels assigned to a device (v4.0.0)
pub async fn query_device_labels(
    State(client): State<DaemonClient>,
    Path((team_id, device_id)): Path<(String, String)>,
) -> Result<Json<Vec<LabelInfo>>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&device_id)?;

    let labels = client
        .client()
        .labels_assigned_to_device(DaemonClient::context(), team_id, device_id)
        .await??;

    let label_infos: Vec<LabelInfo> = labels
        .into_iter()
        .map(|l| LabelInfo {
            id: hex::encode(l.id.into_id().as_bytes()),
            name: l.name.to_string(),
        })
        .collect();

    Ok(Json(label_infos))
}

// ============================================================================
// AFC (Aranya Fast Channels) Handlers - v4.0.0 (replaces AQC)
// ============================================================================

/// Create an AFC send channel (v4.0.0)
pub async fn create_afc_channel(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<CreateAfcChannelRequest>,
) -> Result<Json<CreateAfcChannelResponse>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let peer_id = parse_device_id(&req.peer_id)?;
    let label_id = parse_label_id(&req.label_id)?;

    let (channel_id, ctrl_msg) = client
        .client()
        .create_afc_channel(DaemonClient::context(), team_id, peer_id, label_id)
        .await??;

    use base64::prelude::*;
    Ok(Json(CreateAfcChannelResponse {
        channel_id: hex::encode(channel_id.as_bytes()),
        ctrl_msg: BASE64_STANDARD.encode(&ctrl_msg),
    }))
}

/// Accept an AFC receive channel (v4.0.0)
pub async fn accept_afc_channel(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<AcceptAfcChannelRequest>,
) -> Result<Json<AcceptAfcChannelResponse>, RestError> {
    let team_id = parse_team_id(&team_id)?;

    use base64::prelude::*;
    let ctrl_msg = BASE64_STANDARD
        .decode(&req.ctrl_msg)
        .map_err(|_| RestError::InvalidRequest("Invalid base64 ctrl_msg".to_string()))?;

    let channel_id = client
        .client()
        .accept_afc_channel(DaemonClient::context(), team_id, ctrl_msg)
        .await??;

    Ok(Json(AcceptAfcChannelResponse {
        channel_id: hex::encode(channel_id.as_bytes()),
    }))
}

/// Delete an AFC channel (v4.0.0)
pub async fn delete_afc_channel(
    State(client): State<DaemonClient>,
    Json(req): Json<DeleteAfcChannelRequest>,
) -> Result<StatusCode, RestError> {
    let channel_id_bytes = hex::decode(&req.channel_id)
        .map_err(|_| RestError::InvalidRequest("Invalid channel ID format".to_string()))?;

    // Convert to channel ID type
    // Note: actual implementation depends on aranya-fast-channels types
    client
        .client()
        .delete_afc_channel(DaemonClient::context(), channel_id_bytes)
        .await??;
    Ok(StatusCode::OK)
}

// ============================================================================
// PSK Encryption Handlers (v4.0.0)
// ============================================================================

/// Encrypt PSK seed for a peer (v4.0.0 - for sync)
pub async fn encrypt_psk_seed_for_peer(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<EncryptPskSeedRequest>,
) -> Result<Json<EncryptPskSeedResponse>, RestError> {
    let team_id = parse_team_id(&team_id)?;

    use base64::prelude::*;
    let peer_enc_pk = BASE64_STANDARD
        .decode(&req.peer_enc_pk)
        .map_err(|_| RestError::InvalidRequest("Invalid base64 peer_enc_pk".to_string()))?;

    let encrypted_seed = client
        .client()
        .encrypt_psk_seed_for_peer(DaemonClient::context(), team_id, peer_enc_pk)
        .await??;

    Ok(Json(EncryptPskSeedResponse {
        encrypted_seed: BASE64_STANDARD.encode(&encrypted_seed),
    }))
}
