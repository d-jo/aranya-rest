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

#[derive(Clone)]
pub struct DaemonClient {
    client: DaemonApiClient,
}

impl DaemonClient {
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

    pub fn client(&self) -> &DaemonApiClient {
        &self.client
    }

    pub fn context() -> context::Context {
        context::current()
    }
}
