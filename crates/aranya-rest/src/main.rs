use std::{net::SocketAddr, path::PathBuf};

use anyhow::Context;
use aranya_daemon_api::{crypto::PublicApiKey, CS};
use aranya_rest::RestServer;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "aranya-rest")]
#[command(about = "A REST API server for Aranya daemon")]
struct Args {
    /// Path to the daemon's Unix domain socket
    #[arg(long, default_value = "/tmp/aranya-daemon.sock")]
    daemon_socket: PathBuf,

    /// API key for daemon authentication (hex encoded)
    #[arg(long)]
    api_key: String,

    /// Address to bind the REST server to
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind_addr: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aranya_rest=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    // Parse API key from hex string
    let api_key_bytes = hex::decode(&args.api_key).context("Failed to decode API key as hex")?;
    let api_key: PublicApiKey<CS> =
        PublicApiKey::decode(&api_key_bytes).context("Invalid API key format")?;

    let server = RestServer::new(args.daemon_socket, api_key, args.bind_addr).await?;

    server.serve().await?;

    Ok(())
}
