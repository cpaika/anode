use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod server;
mod cluster;
mod health;
mod metrics;
mod admin;

use config::Config;
use server::Server;

#[derive(Parser, Debug)]
#[command(name = "anode")]
#[command(about = "Distributed object storage for small clusters", version)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "/etc/anode/config.toml")]
    config: PathBuf,

    /// Node ID (overrides config)
    #[arg(long)]
    node_id: Option<u64>,

    /// Bind address for S3 API
    #[arg(long)]
    s3_addr: Option<String>,

    /// Bind address for gRPC (internal cluster communication)
    #[arg(long)]
    grpc_addr: Option<String>,

    /// Bind address for admin/metrics API
    #[arg(long)]
    admin_addr: Option<String>,

    /// Bootstrap a new cluster
    #[arg(long)]
    bootstrap: bool,

    /// Join existing cluster at this address
    #[arg(long)]
    join: Option<String>,

    /// Data directory
    #[arg(long)]
    data_dir: Option<PathBuf>,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize tracing/logging
    init_logging(&cli.log_level)?;

    info!("Starting Anode server");
    info!("Config file: {:?}", cli.config);

    // Load configuration from file
    let mut config = if cli.config.exists() {
        info!("Loading configuration from {:?}", cli.config);
        Config::load(&cli.config).map_err(|e| {
            error!("Failed to load config: {}", e);
            anyhow::anyhow!("Configuration error: {}", e)
        })?
    } else {
        warn!("Config file not found, using defaults");
        Config::default()
    };

    // Apply CLI overrides
    if let Some(node_id) = cli.node_id {
        config.node.id = node_id;
    }
    if let Some(s3_addr) = cli.s3_addr {
        config.node.s3_addr = s3_addr;
    }
    if let Some(grpc_addr) = cli.grpc_addr {
        config.node.grpc_addr = grpc_addr;
    }
    if let Some(admin_addr) = cli.admin_addr {
        config.node.admin_addr = admin_addr;
    }
    if let Some(data_dir) = cli.data_dir {
        config.storage.data_dir = data_dir;
    }

    // Validate configuration
    config.validate().map_err(|e| {
        error!("Configuration validation failed: {}", e);
        anyhow::anyhow!("Invalid configuration: {}", e)
    })?;

    info!("Node ID: {}", config.node.id);
    info!("Node name: {}", config.node.name);
    info!("S3 API: {}", config.node.s3_addr);
    info!("gRPC API: {}", config.node.grpc_addr);
    info!("Admin API: {}", config.node.admin_addr);
    info!("Data directory: {:?}", config.storage.data_dir);

    // Create server instance
    let server = Server::new(config).await.map_err(|e| {
        error!("Failed to create server: {}", e);
        e
    })?;

    // Handle bootstrap or join cluster
    if cli.bootstrap {
        info!("Bootstrapping new cluster");
        server.bootstrap_cluster().await?;
    } else if let Some(join_addr) = cli.join {
        info!("Joining existing cluster at {}", join_addr);
        server.join_cluster(&join_addr).await?;
    }

    // Start the server
    info!("Starting server components");
    server.start().await?;

    // Wait for shutdown signal
    match wait_for_shutdown_signal().await {
        Ok(signal_name) => {
            info!("Received {} signal, starting graceful shutdown", signal_name);
        }
        Err(e) => {
            error!("Error waiting for shutdown signal: {}", e);
        }
    }

    // Graceful shutdown with timeout
    info!("Shutting down server");
    let shutdown_timeout = tokio::time::Duration::from_secs(30);

    match tokio::time::timeout(shutdown_timeout, server.shutdown()).await {
        Ok(Ok(())) => {
            info!("Server shut down gracefully");
        }
        Ok(Err(e)) => {
            error!("Error during shutdown: {}", e);
            return Err(e);
        }
        Err(_) => {
            error!("Shutdown timeout exceeded, forcing exit");
            return Err(anyhow::anyhow!("Shutdown timeout"));
        }
    }

    info!("Anode server stopped");
    Ok(())
}

/// Initialize logging with the specified log level
fn init_logging(log_level: &str) -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level))
        .map_err(|e| anyhow::anyhow!("Invalid log level: {}", e))?;

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_line_number(true)
        )
        .init();

    Ok(())
}

/// Wait for shutdown signal (SIGTERM or SIGINT)
async fn wait_for_shutdown_signal() -> Result<&'static str> {
    #[cfg(unix)]
    {
        use signal::unix::{signal, SignalKind};

        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        tokio::select! {
            _ = sigterm.recv() => Ok("SIGTERM"),
            _ = sigint.recv() => Ok("SIGINT"),
        }
    }

    #[cfg(not(unix))]
    {
        signal::ctrl_c().await?;
        Ok("CTRL+C")
    }
}
