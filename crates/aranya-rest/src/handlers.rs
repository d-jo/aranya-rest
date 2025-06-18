use aranya_daemon_api::{
    ChanOp, DeviceId, KeyBundle, LabelId, NetIdentifier, Role, SyncPeerConfig, TeamConfig, TeamId,
};
use aranya_util::Addr;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{client::DaemonClient, RestError};

// Request/Response types for JSON serialization
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
pub struct CreateTeamRequest {
    pub config: TeamConfigJson,
}

#[derive(Serialize, Deserialize)]
pub struct TeamConfigJson {
    // Currently empty but included for future extensibility
}

#[derive(Serialize, Deserialize)]
pub struct CreateTeamResponse {
    pub team_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct AddDeviceRequest {
    pub keys: KeyBundleJson,
}

#[derive(Serialize, Deserialize)]
pub struct KeyBundleJson {
    pub identity: String, // base64 encoded
    pub signing: String,  // base64 encoded
    pub encoding: String, // base64 encoded
}

#[derive(Serialize, Deserialize)]
pub struct AssignRoleRequest {
    pub device_id: String,
    pub role: String, // "Owner", "Admin", "Operator", "Member"
}

#[derive(Serialize, Deserialize)]
pub struct AssignNetIdentifierRequest {
    pub device_id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateLabelRequest {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateLabelResponse {
    pub label_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct AssignLabelRequest {
    pub device_id: String,
    pub label_id: String,
    pub operation: String, // "SendRecv", "RecvOnly", "SendOnly"
}

#[derive(Serialize, Deserialize)]
pub struct CreateChannelRequest {
    pub peer: String,
    pub label_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct CreateChannelResponse {
    pub ctrl: Vec<String>, // base64 encoded ctrl messages
    pub psks: Value,       // PSK data as JSON
}

#[derive(Serialize, Deserialize)]
pub struct ReceiveCtrlRequest {
    pub ctrl: Vec<String>, // base64 encoded ctrl messages
}

#[derive(Serialize, Deserialize)]
pub struct ReceiveCtrlResponse {
    pub label_id: String,
    pub psks: Value,
}

// Helper functions for type conversions
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

fn parse_role(role_str: &str) -> Result<Role, RestError> {
    match role_str {
        "Owner" => Ok(Role::Owner),
        "Admin" => Ok(Role::Admin),
        "Operator" => Ok(Role::Operator),
        "Member" => Ok(Role::Member),
        _ => Err(RestError::InvalidRequest("Invalid role".to_string())),
    }
}

fn parse_chan_op(op_str: &str) -> Result<ChanOp, RestError> {
    match op_str {
        "SendRecv" => Ok(ChanOp::SendRecv),
        "RecvOnly" => Ok(ChanOp::RecvOnly),
        "SendOnly" => Ok(ChanOp::SendOnly),
        _ => Err(RestError::InvalidRequest(
            "Invalid channel operation".to_string(),
        )),
    }
}

fn parse_addr(addr_str: &str) -> Result<Addr, RestError> {
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

impl From<TeamConfigJson> for TeamConfig {
    fn from(_config: TeamConfigJson) -> Self {
        TeamConfig {}
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

// Handler functions

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

pub async fn create_team(
    State(client): State<DaemonClient>,
    Json(req): Json<CreateTeamRequest>,
) -> Result<Json<CreateTeamResponse>, RestError> {
    let config = req.config.into();
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

pub async fn add_device_to_team(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<AddDeviceRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let keys = req.keys.try_into()?;

    client
        .client()
        .add_device_to_team(DaemonClient::context(), team_id, keys)
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

pub async fn assign_role(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<AssignRoleRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&req.device_id)?;
    let role = parse_role(&req.role)?;

    client
        .client()
        .assign_role(DaemonClient::context(), team_id, device_id, role)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn revoke_role(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<AssignRoleRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&req.device_id)?;
    let role = parse_role(&req.role)?;

    client
        .client()
        .revoke_role(DaemonClient::context(), team_id, device_id, role)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn assign_net_identifier(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<AssignNetIdentifierRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&req.device_id)?;
    let name = NetIdentifier(req.name);

    client
        .client()
        .assign_aqc_net_identifier(DaemonClient::context(), team_id, device_id, name)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn remove_net_identifier(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<AssignNetIdentifierRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&req.device_id)?;
    let name = NetIdentifier(req.name);

    client
        .client()
        .remove_aqc_net_identifier(DaemonClient::context(), team_id, device_id, name)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn create_label(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<CreateLabelRequest>,
) -> Result<Json<CreateLabelResponse>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let label_id = client
        .client()
        .create_label(DaemonClient::context(), team_id, req.name)
        .await??;

    Ok(Json(CreateLabelResponse {
        label_id: hex::encode(label_id.into_id().as_bytes()),
    }))
}

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
        .assign_label(DaemonClient::context(), team_id, device_id, label_id, op)
        .await??;
    Ok(StatusCode::OK)
}

pub async fn revoke_label(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
    Json(req): Json<AssignLabelRequest>,
) -> Result<StatusCode, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&req.device_id)?;
    let label_id = parse_label_id(&req.label_id)?;

    client
        .client()
        .revoke_label(DaemonClient::context(), team_id, device_id, label_id)
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
        .query_devices_on_team(DaemonClient::context(), team_id)
        .await??;

    let device_ids: Vec<String> = devices
        .into_iter()
        .map(|id| hex::encode(id.into_id().as_bytes()))
        .collect();

    Ok(Json(device_ids))
}

pub async fn query_device_role(
    State(client): State<DaemonClient>,
    Path((team_id, device_id)): Path<(String, String)>,
) -> Result<Json<String>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&device_id)?;
    let role = client
        .client()
        .query_device_role(DaemonClient::context(), team_id, device_id)
        .await??;

    let role_str = match role {
        Role::Owner => "Owner",
        Role::Admin => "Admin",
        Role::Operator => "Operator",
        Role::Member => "Member",
    };

    Ok(Json(role_str.to_string()))
}

pub async fn query_device_keybundle(
    State(client): State<DaemonClient>,
    Path((team_id, device_id)): Path<(String, String)>,
) -> Result<Json<KeyBundle>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let device_id = parse_device_id(&device_id)?;
    let keys = client
        .client()
        .query_device_keybundle(DaemonClient::context(), team_id, device_id)
        .await??;

    Ok(Json(keys))
}

pub async fn query_labels(
    State(client): State<DaemonClient>,
    Path(team_id): Path<String>,
) -> Result<Json<Vec<aranya_daemon_api::Label>>, RestError> {
    let team_id = parse_team_id(&team_id)?;
    let labels = client
        .client()
        .query_labels(DaemonClient::context(), team_id)
        .await??;

    Ok(Json(labels))
}
