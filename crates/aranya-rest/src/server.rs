use std::{net::SocketAddr, path::PathBuf};

use aranya_daemon_api::{crypto::PublicApiKey, CS};
use axum::{
    routing::{delete, get, post},
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
            // Version and device info
            .route("/api/v1/version", get(handlers::get_version))
            .route("/api/v1/local-addr", get(handlers::get_local_addr))
            .route("/api/v1/key-bundle", get(handlers::get_key_bundle))
            .route("/api/v1/device-id", get(handlers::get_device_id))
            // Sync peer management
            .route("/api/v1/sync/peers", post(handlers::add_sync_peer))
            .route("/api/v1/sync/now", post(handlers::sync_now))
            .route("/api/v1/sync/peers", delete(handlers::remove_sync_peer))
            // Team management
            .route("/api/v1/teams", post(handlers::create_team))
            .route("/api/v1/teams/{team_id}", delete(handlers::close_team))
            .route(
                "/api/v1/teams/{team_id}/devices",
                get(handlers::query_devices_on_team),
            )
            // Device management within teams
            .route(
                "/api/v1/teams/{team_id}/devices",
                post(handlers::add_device_to_team),
            )
            .route(
                "/api/v1/teams/{team_id}/devices/{device_id}",
                delete(handlers::remove_device_from_team),
            )
            .route(
                "/api/v1/teams/{team_id}/devices/{device_id}/role",
                get(handlers::query_device_role),
            )
            .route(
                "/api/v1/teams/{team_id}/devices/{device_id}/keybundle",
                get(handlers::query_device_keybundle),
            )
            // Role management
            .route(
                "/api/v1/teams/{team_id}/roles/assign",
                post(handlers::assign_role),
            )
            .route(
                "/api/v1/teams/{team_id}/roles/revoke",
                post(handlers::revoke_role),
            )
            // Network identifier management
            .route(
                "/api/v1/teams/{team_id}/net-identifiers/assign",
                post(handlers::assign_net_identifier),
            )
            .route(
                "/api/v1/teams/{team_id}/net-identifiers/remove",
                post(handlers::remove_net_identifier),
            )
            // Label management
            .route("/api/v1/teams/{team_id}/labels", get(handlers::query_labels))
            .route(
                "/api/v1/teams/{team_id}/labels",
                post(handlers::create_label),
            )
            .route(
                "/api/v1/teams/{team_id}/labels/{label_id}",
                delete(handlers::delete_label),
            )
            .route(
                "/api/v1/teams/{team_id}/labels/assign",
                post(handlers::assign_label),
            )
            .route(
                "/api/v1/teams/{team_id}/labels/revoke",
                post(handlers::revoke_label),
            )
            // Message management
            .route(
                "/api/v1/teams/{team_id}/messages",
                post(handlers::send_message),
            )
            .route(
                "/api/v1/teams/{team_id}/messages",
                get(handlers::query_messages),
            )
            .layer(ServiceBuilder::new().layer(CorsLayer::permissive()))
            .with_state(self.daemon_client.clone())
    }

    pub async fn serve(self) -> Result<(), RestError> {
        let router = self.router();

        info!("Starting REST server on {}", self.bind_addr);

        let listener = tokio::net::TcpListener::bind(self.bind_addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }
}
