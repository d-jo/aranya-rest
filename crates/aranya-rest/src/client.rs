use std::path::PathBuf;

use anyhow::Context;
use aranya_daemon_api::{
    crypto::{
        txp::{self, LengthDelimitedCodec},
        PublicApiKey,
    },
    DaemonApiClient, CS,
};
use tarpc::{client, context};
use tokio::net::UnixStream;

use crate::RestError;

/// Client wrapper for communicating with the Aranya daemon via tarpc RPC.
///
/// This client handles the encrypted transport layer (TXP) and provides
/// access to the daemon's API for the REST server handlers.
#[derive(Clone)]
pub struct DaemonClient {
    client: DaemonApiClient,
}

impl DaemonClient {
    /// Create a new daemon client connected via Unix domain socket.
    ///
    /// # Arguments
    /// * `uds_path` - Path to the daemon's Unix domain socket
    /// * `api_key` - Public API key for encrypted transport
    pub async fn new(uds_path: PathBuf, api_key: PublicApiKey<CS>) -> Result<Self, RestError> {
        let stream = UnixStream::connect(&uds_path)
            .await
            .context("Failed to connect to daemon UDS")?;

        let info = uds_path.as_os_str().as_encoded_bytes();
        let codec = LengthDelimitedCodec::builder()
            .max_frame_length(usize::MAX)
            .new_codec();

        let transport = txp::client(stream, codec, aranya_crypto::Rng, api_key, info);
        let client = DaemonApiClient::new(client::Config::default(), transport).spawn();

        Ok(Self { client })
    }

    /// Get a reference to the underlying daemon API client.
    pub fn client(&self) -> &DaemonApiClient {
        &self.client
    }

    /// Get the current tarpc context for RPC calls.
    pub fn context() -> context::Context {
        context::current()
    }
}
