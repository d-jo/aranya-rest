//! REST API server for Aranya daemon v4.0.0
//!
//! This module provides the HTTP server setup and routing for the
//! Aranya REST API, supporting custom RBAC roles and AFC channels.

use std::{net::SocketAddr, path::PathBuf};

use aranya_daemon_api::{crypto::PublicApiKey, CS};
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::{client::DaemonClient, handlers, RestError};

pub struct RestServer {
    daemon_client: DaemonClient,
    bind_addr: SocketAddr,
}

impl RestServer {
    pub async fn new(
        daemon_uds_path: PathBuf,
        daemon_api_key: PublicApiKey<CS>,
        bind_addr: SocketAddr,
    ) -> Result<Self, RestError> {
        let daemon_client = DaemonClient::new(daemon_uds_path, daemon_api_key).await?;

        Ok(Self {
            daemon_client,
            bind_addr,
        })
    }

    pub fn router(&self) -> Router {
        Router::new()
            // ================================================================
            // Device & System Info (v4.0.0)
            // ================================================================
            .route("/api/v1/version", get(handlers::get_version))
            .route("/api/v1/local-addr", get(handlers::get_local_addr))
            .route("/api/v1/key-bundle", get(handlers::get_key_bundle))
            .route("/api/v1/device-id", get(handlers::get_device_id))
            // ================================================================
            // Sync Peer Management (v4.0.0)
            // ================================================================
            .route("/api/v1/sync/peers", post(handlers::add_sync_peer))
            .route("/api/v1/sync/now", post(handlers::sync_now))
            .route("/api/v1/sync/peers", delete(handlers::remove_sync_peer))
            .route(
                "/api/v1/teams/{team_id}/sync/peers",
                get(handlers::query_sync_peers),
            )
            .route(
                "/api/v1/teams/{team_id}/sync/peers/{addr}/config",
                get(handlers::query_sync_peer_config),
            )
            .route(
                "/api/v1/sync/peers/config",
                put(handlers::update_sync_peer_config),
            )
            // PSK encryption for sync (v4.0.0)
            .route(
                "/api/v1/teams/{team_id}/sync/encrypt-psk-seed",
                post(handlers::encrypt_psk_seed_for_peer),
            )
            // ================================================================
            // Team Management (v4.0.0)
            // ================================================================
            .route("/api/v1/teams", post(handlers::create_team))
            .route("/api/v1/teams/{team_id}", delete(handlers::close_team))
            .route("/api/v1/teams/{team_id}/add", post(handlers::add_team))
            .route("/api/v1/teams/{team_id}/remove", delete(handlers::remove_team))
            .route(
                "/api/v1/teams/{team_id}/devices",
                get(handlers::query_devices_on_team),
            )
            // ================================================================
            // Device Management (v4.0.0)
            // ================================================================
            .route(
                "/api/v1/teams/{team_id}/devices",
                post(handlers::add_device_to_team),
            )
            .route(
                "/api/v1/teams/{team_id}/devices/{device_id}",
                delete(handlers::remove_device_from_team),
            )
            .route(
                "/api/v1/teams/{team_id}/devices/{device_id}/keybundle",
                get(handlers::query_device_keybundle),
            )
            .route(
                "/api/v1/teams/{team_id}/devices/{device_id}/role",
                get(handlers::query_device_role),
            )
            .route(
                "/api/v1/teams/{team_id}/devices/{device_id}/labels",
                get(handlers::query_device_labels),
            )
            // ================================================================
            // Role Management - v4.0.0 Custom RBAC Roles
            // ================================================================
            // Setup default roles for a team
            .route(
                "/api/v1/teams/{team_id}/roles/setup-defaults",
                post(handlers::setup_default_roles),
            )
            // Query all roles on a team
            .route(
                "/api/v1/teams/{team_id}/roles",
                get(handlers::query_team_roles),
            )
            // Query roles that own a specific role
            .route(
                "/api/v1/teams/{team_id}/roles/{role_id}/owners",
                get(handlers::query_role_owners),
            )
            // Query devices by role
            .route(
                "/api/v1/teams/{team_id}/roles/{role_id}/devices",
                get(handlers::query_devices_by_role),
            )
            // Query all device roles (aggregated view)
            .route(
                "/api/v1/teams/{team_id}/device-roles",
                get(handlers::query_all_device_roles),
            )
            // Assign role to device
            .route(
                "/api/v1/teams/{team_id}/roles/assign",
                post(handlers::assign_role),
            )
            // Revoke role from device
            .route(
                "/api/v1/teams/{team_id}/roles/revoke",
                post(handlers::revoke_role),
            )
            // Change device role
            .route(
                "/api/v1/teams/{team_id}/roles/change",
                post(handlers::change_role),
            )
            // Bulk assign roles
            .route(
                "/api/v1/teams/{team_id}/roles/bulk-assign",
                post(handlers::bulk_assign_role),
            )
            // ================================================================
            // Label Management - v4.0.0 (requires managing role)
            // ================================================================
            // Query all labels on a team
            .route(
                "/api/v1/teams/{team_id}/labels",
                get(handlers::query_labels),
            )
            // Create a label (requires managing_role_id)
            .route(
                "/api/v1/teams/{team_id}/labels",
                post(handlers::create_label),
            )
            // Query a specific label
            .route(
                "/api/v1/teams/{team_id}/labels/{label_id}",
                get(handlers::query_label),
            )
            // Delete a label
            .route(
                "/api/v1/teams/{team_id}/labels/{label_id}",
                delete(handlers::delete_label),
            )
            // Add managing role to a label
            .route(
                "/api/v1/teams/{team_id}/labels/managing-role",
                post(handlers::add_label_managing_role),
            )
            // Assign label to device
            .route(
                "/api/v1/teams/{team_id}/labels/assign",
                post(handlers::assign_label),
            )
            // Revoke label from device
            .route(
                "/api/v1/teams/{team_id}/labels/revoke",
                post(handlers::revoke_label),
            )
            // ================================================================
            // AFC (Aranya Fast Channels) - v4.0.0 (replaces AQC)
            // ================================================================
            // Create a send channel
            .route(
                "/api/v1/teams/{team_id}/afc/channels",
                post(handlers::create_afc_channel),
            )
            // Accept a receive channel
            .route(
                "/api/v1/teams/{team_id}/afc/channels/accept",
                post(handlers::accept_afc_channel),
            )
            // Delete a channel
            .route(
                "/api/v1/afc/channels",
                delete(handlers::delete_afc_channel),
            )
            // ================================================================
            // Middleware
            // ================================================================
            .layer(ServiceBuilder::new().layer(CorsLayer::permissive()))
            .with_state(self.daemon_client.clone())
    }

    pub async fn serve(self) -> Result<(), RestError> {
        let router = self.router();

        info!("Starting Aranya REST server v4.0.0 on {}", self.bind_addr);

        let listener = tokio::net::TcpListener::bind(self.bind_addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }
}
