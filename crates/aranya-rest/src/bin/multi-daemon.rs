use std::{
    net::{Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context as _, Result};
use clap::Parser;
use tempfile::TempDir;
use tokio::{
    fs,
    process::{Child, Command},
    signal,
    time::sleep,
};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "multi-daemon")]
#[command(about = "Spin up multiple Aranya daemons with REST servers for testing")]
struct Args {
    /// Number of daemon instances to create
    #[arg(short, long, default_value = "2")]
    count: usize,

    /// Base port for REST servers (each daemon gets port + index)
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Path to aranya-daemon binary
    #[arg(long)]
    daemon_path: Option<PathBuf>,

    /// Path to aranya-rest binary
    #[arg(long)]
    rest_path: Option<PathBuf>,

    /// Keep running after setup (default: false, just setup and exit)
    #[arg(short, long)]
    keep_running: bool,
}

#[derive(Debug)]
struct DaemonInstance {
    name: String,
    work_dir: TempDir,
    daemon_proc: Child,
    rest_proc: Child,
    uds_path: PathBuf,
    rest_port: u16,
}

impl DaemonInstance {
    async fn spawn(
        name: String,
        daemon_path: &Path,
        rest_path: &Path,
        rest_port: u16,
    ) -> Result<Self> {
        info!(name, rest_port, "Creating daemon instance");

        // Create temporary directory
        let work_dir = TempDir::with_prefix(&format!("aranya-{}-", name))?;
        let daemon_dir = work_dir.path().join("daemon");
        fs::create_dir_all(&daemon_dir).await?;

        // Create directory structure
        let runtime_dir = daemon_dir.join("run");
        let state_dir = daemon_dir.join("state");
        let cache_dir = daemon_dir.join("cache");
        let logs_dir = daemon_dir.join("logs");
        let config_dir = daemon_dir.join("config");

        for dir in &[&runtime_dir, &state_dir, &cache_dir, &logs_dir, &config_dir] {
            fs::create_dir_all(dir)
                .await
                .with_context(|| format!("unable to create directory: {}", dir.display()))?;
        }

        // Create daemon config
        let cfg_path = daemon_dir.join("config.json");
        let config = format!(
            r#"{{
    "name": "{name}",
    "runtime_dir": "{runtime_dir}",
    "state_dir": "{state_dir}",
    "cache_dir": "{cache_dir}",
    "logs_dir": "{logs_dir}",
    "config_dir": "{config_dir}",
    "sync_addr": "localhost:0"
}}"#,
            name = name,
            runtime_dir = runtime_dir.display(),
            state_dir = state_dir.display(),
            cache_dir = cache_dir.display(),
            logs_dir = logs_dir.display(),
            config_dir = config_dir.display(),
        );

        fs::write(&cfg_path, config).await?;

        // Start daemon
        let mut daemon_cmd = Command::new(daemon_path);
        daemon_cmd
            .kill_on_drop(true)
            .current_dir(&daemon_dir)
            .args(["--config", &cfg_path.to_string_lossy()]);

        info!(name, "Starting daemon");
        let daemon_proc = daemon_cmd
            .spawn()
            .context("unable to spawn daemon")?;

        let uds_path = runtime_dir.join("uds.sock");

        // Wait for daemon to start and create the socket and API key
        let mut retries = 0;
        while !uds_path.exists() && retries < 50 {
            sleep(Duration::from_millis(100)).await;
            retries += 1;
        }

        if !uds_path.exists() {
            return Err(anyhow::anyhow!("Daemon failed to create UDS socket after 5 seconds"));
        }

        // Wait a bit more for API key to be written
        sleep(Duration::from_millis(200)).await;

        // Start REST server
        let mut rest_cmd = Command::new(rest_path);
        rest_cmd
            .kill_on_drop(true)
            .args([
                "--daemon-socket", &uds_path.to_string_lossy(),
                "--bind-addr", &format!("127.0.0.1:{}", rest_port),
            ]);

        info!(name, rest_port, "Starting REST server");
        let rest_proc = rest_cmd
            .spawn()
            .context("unable to spawn REST server")?;

        // Wait a moment for REST server to start
        sleep(Duration::from_millis(500)).await;

        info!(name, rest_port, uds_path = ?uds_path, "Daemon instance ready");

        Ok(DaemonInstance {
            name,
            work_dir,
            daemon_proc,
            rest_proc,
            uds_path,
            rest_port,
        })
    }

    fn daemon_dir(&self) -> PathBuf {
        self.work_dir.path().join("daemon")
    }

    fn api_key_path(&self) -> PathBuf {
        self.daemon_dir().join("run").join("api.pk")
    }
}

async fn find_binaries(args: &Args) -> Result<(PathBuf, PathBuf)> {
    let daemon_path = if let Some(path) = &args.daemon_path {
        path.clone()
    } else {
        // Try to find in target directory
        let candidates = [
            "./target/release/aranya-daemon",
            "./target/debug/aranya-daemon",
            "aranya-daemon", // In PATH
        ];
        
        let mut found = None;
        for candidate in &candidates {
            let path = PathBuf::from(candidate);
            if path.exists() || candidate == &"aranya-daemon" {
                // For the PATH case, we'll let the OS handle it
                found = Some(path);
                break;
            }
        }
        
        found.context("Could not find aranya-daemon binary. Use --daemon-path to specify location.")?
    };

    let rest_path = if let Some(path) = &args.rest_path {
        path.clone()
    } else {
        // Try to find in target directory
        let candidates = [
            "./target/release/aranya-rest",
            "./target/debug/aranya-rest",
            "aranya-rest", // In PATH
        ];
        
        let mut found = None;
        for candidate in &candidates {
            let path = PathBuf::from(candidate);
            if path.exists() || candidate == &"aranya-rest" {
                found = Some(path);
                break;
            }
        }
        
        found.context("Could not find aranya-rest binary. Use --rest-path to specify location.")?
    };

    Ok((daemon_path, rest_path))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "multi_daemon=info,aranya_rest=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    if args.count == 0 {
        return Err(anyhow::anyhow!("Count must be at least 1"));
    }

    info!("Multi-daemon setup starting");
    info!("Creating {} daemon instances starting from port {}", args.count, args.port);

    let (daemon_path, rest_path) = find_binaries(&args).await?;
    info!("Using daemon: {}", daemon_path.display());
    info!("Using REST server: {}", rest_path.display());

    let mut instances = Vec::new();

    // Create all daemon instances
    for i in 0..args.count {
        let name = format!("daemon-{}", i);
        let port = args.port + i as u16;

        match DaemonInstance::spawn(name.clone(), &daemon_path, &rest_path, port).await {
            Ok(instance) => {
                instances.push(instance);
            }
            Err(e) => {
                error!(name, "Failed to create daemon instance: {}", e);
                return Err(e);
            }
        }
    }

    // Print summary
    println!("\n🚀 All daemon instances are running!\n");
    println!("┌─────────────┬──────────────┬─────────────────────────────────────────┐");
    println!("│ Instance    │ REST Port    │ UDS Socket                              │");
    println!("├─────────────┼──────────────┼─────────────────────────────────────────┤");
    
    for instance in &instances {
        println!("│ {:11} │ {:12} │ {:39} │", 
                 instance.name, 
                 instance.rest_port, 
                 instance.uds_path.display());
    }
    
    println!("└─────────────┴──────────────┴─────────────────────────────────────────┘");
    
    println!("\n📋 REST API endpoints:");
    for instance in &instances {
        println!("  • {}: http://127.0.0.1:{}/api/v1/", instance.name, instance.rest_port);
    }

    println!("\n🔧 Example curl commands:");
    if !instances.is_empty() {
        let port = instances[0].rest_port;
        println!("  curl http://127.0.0.1:{}/api/v1/version", port);
        println!("  curl http://127.0.0.1:{}/api/v1/device-id", port);
        println!("  curl -X POST http://127.0.0.1:{}/api/v1/teams -H 'Content-Type: application/json' -d '{{\"config\": {{}}}}'", port);
    }

    if args.keep_running {
        println!("\n⏳ Keeping daemons running... Press Ctrl+C to stop.");
        
        // Wait for interrupt signal
        match signal::ctrl_c().await {
            Ok(()) => {
                println!("\n🛑 Received interrupt signal, shutting down...");
            }
            Err(err) => {
                error!("Failed to listen for interrupt signal: {}", err);
            }
        }
    } else {
        println!("\n✅ Setup complete! Daemons are running in background.");
        println!("   Use --keep-running to prevent this tool from exiting.");
        println!("   Kill daemon processes manually when done testing.");
    }

    // When this function ends, all processes will be killed due to kill_on_drop(true)
    if args.keep_running {
        info!("Cleaning up daemon instances...");
        for mut instance in instances {
            let _ = instance.daemon_proc.kill().await;
            let _ = instance.rest_proc.kill().await;
        }
    }

    Ok(())
}