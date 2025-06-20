use std::{net::SocketAddr, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use aranya_viz_coordinator::coordinator::Coordinator;

#[derive(Parser)]
#[command(name = "aranya-viz-coordinator")]
#[command(about = "Interactive visualization tool for Aranya daemon networks")]
struct Args {
    /// Address to bind the coordinator server to
    #[arg(long, default_value = "127.0.0.1:3000")]
    bind_addr: SocketAddr,

    /// Directory containing static web assets (optional)
    #[arg(long)]
    static_dir: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    let env_filter = if args.verbose {
        "aranya_viz_coordinator=debug,tower_http=debug,axum=debug"
    } else {
        "aranya_viz_coordinator=info,tower_http=info"
    };

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| env_filter.into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Validate static directory if provided
    if let Some(ref static_dir) = args.static_dir {
        if !static_dir.exists() {
            eprintln!("Static directory does not exist: {}", static_dir.display());
            std::process::exit(1);
        }
        if !static_dir.is_dir() {
            eprintln!("Static path is not a directory: {}", static_dir.display());
            std::process::exit(1);
        }
    }

    // Create and run coordinator
    let coordinator = Coordinator::new(args.bind_addr, args.static_dir);
    coordinator.run().await?;

    Ok(())
}