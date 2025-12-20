use anyhow::{Context, Result};
use axum::Router;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::admin::{admin_router, AdminState};
use crate::cluster::{ClusterManager, NodeInfo};
use crate::config::Config;
use crate::health::HealthChecker;
use crate::metrics::Metrics;

use anode_parquet::ParquetCache;
use anode_raft::{AnodeRaft, RaftConfig};
use anode_s3::S3Router;
use anode_storage::StorageEngine;

/// Main Anode server
pub struct Server {
    config: Config,
    storage: Arc<StorageEngine>,
    raft: Arc<AnodeRaft>,
    parquet_cache: Arc<ParquetCache>,
    cluster_manager: Arc<ClusterManager>,
    health_checker: Arc<HealthChecker>,
    metrics: Arc<Metrics>,
    shutdown_tx: broadcast::Sender<()>,
    tasks: Arc<RwLock<Vec<JoinHandle<()>>>>,
}

impl Server {
    /// Create a new server instance
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing Anode server components");

        // Create data directory if it doesn't exist
        if !config.storage.data_dir.exists() {
            info!("Creating data directory: {:?}", config.storage.data_dir);
            std::fs::create_dir_all(&config.storage.data_dir)
                .context("Failed to create data directory")?;
        }

        // Initialize storage engine
        info!("Initializing storage engine");
        let storage = Arc::new(
            StorageEngine::new(config.storage.data_dir.clone())
                .context("Failed to initialize storage engine")?
        );

        // Initialize Raft
        info!("Initializing Raft consensus");
        let mut raft_config = RaftConfig::default();
        raft_config.election_timeout_min_ms = config.cluster.election_timeout_ms;
        raft_config.election_timeout_max_ms = config.cluster.election_timeout_ms * 2;
        raft_config.heartbeat_interval_ms = config.cluster.heartbeat_interval_ms;
        raft_config.node_id = config.node.id;
        raft_config.data_dir = config.storage.data_dir.join("raft");

        let raft_storage = anode_raft::RaftStorage::new(
            raft_config.data_dir.clone()
        ).await.context("Failed to initialize Raft storage")?;

        let state_machine = anode_raft::StateMachine::new(storage.clone());
        let network = anode_raft::Network::new();

        let raft = Arc::new(
            AnodeRaft::new(
                config.node.id,
                raft_config.to_openraft_config(),
                network.clone(),
                raft_storage,
                state_machine,
            ).await.context("Failed to initialize Raft")?
        );

        // Initialize Parquet cache
        info!("Initializing Parquet cache");
        let parquet_cache = Arc::new(ParquetCache::new(config.parquet.cache_size));

        // Initialize cluster manager
        info!("Initializing cluster manager");
        let cluster_manager = Arc::new(
            ClusterManager::new(
                raft.clone(),
                config.cluster.placement_groups,
            ).await
        );

        // Initialize health checker
        let health_checker = Arc::new(
            HealthChecker::new(
                storage.clone(),
                raft.clone(),
                cluster_manager.clone(),
            )
        );

        // Initialize metrics
        let metrics = Arc::new(Metrics::new());

        // Create shutdown channel
        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config,
            storage,
            raft,
            parquet_cache,
            cluster_manager,
            health_checker,
            metrics,
            shutdown_tx,
            tasks: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Start all server components
    pub async fn start(&self) -> Result<()> {
        info!("Starting server components");

        // Start S3 API server
        let s3_handle = self.start_s3_server().await?;

        // Start gRPC server for internal cluster communication
        let grpc_handle = self.start_grpc_server().await?;

        // Start admin/metrics API server
        let admin_handle = self.start_admin_server().await?;

        // Store task handles
        let mut tasks = self.tasks.write().await;
        tasks.push(s3_handle);
        tasks.push(grpc_handle);
        tasks.push(admin_handle);

        info!("All server components started successfully");
        Ok(())
    }

    /// Start the S3-compatible API server
    async fn start_s3_server(&self) -> Result<JoinHandle<()>> {
        let addr: SocketAddr = self.config.node.s3_addr
            .parse()
            .context("Invalid S3 bind address")?;

        info!("Starting S3 API server on {}", addr);

        // Create S3 router with storage backend
        let s3_router = S3Router::new(self.storage.clone());
        let app = s3_router.build();

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .context("Failed to bind S3 server")?;

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            info!("S3 API server listening on {}", addr);

            let serve = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_rx.recv().await.ok();
                    info!("S3 API server shutting down");
                });

            if let Err(e) = serve.await {
                error!("S3 server error: {}", e);
            }
        });

        Ok(handle)
    }

    /// Start the gRPC server for internal cluster communication
    async fn start_grpc_server(&self) -> Result<JoinHandle<()>> {
        let addr: SocketAddr = self.config.node.grpc_addr
            .parse()
            .context("Invalid gRPC bind address")?;

        info!("Starting gRPC server on {}", addr);

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // TODO: Implement full gRPC service with Raft network
        let handle = tokio::spawn(async move {
            info!("gRPC server listening on {}", addr);

            // Wait for shutdown signal
            shutdown_rx.recv().await.ok();
            info!("gRPC server shutting down");
        });

        Ok(handle)
    }

    /// Start the admin/metrics API server
    async fn start_admin_server(&self) -> Result<JoinHandle<()>> {
        let addr: SocketAddr = self.config.node.admin_addr
            .parse()
            .context("Invalid admin bind address")?;

        info!("Starting admin API server on {}", addr);

        let state = AdminState {
            health_checker: self.health_checker.clone(),
            cluster_manager: self.cluster_manager.clone(),
            metrics: self.metrics.clone(),
            raft: self.raft.clone(),
        };

        let app = admin_router(state);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .context("Failed to bind admin server")?;

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            info!("Admin API server listening on {}", addr);

            let serve = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_rx.recv().await.ok();
                    info!("Admin API server shutting down");
                });

            if let Err(e) = serve.await {
                error!("Admin server error: {}", e);
            }
        });

        Ok(handle)
    }

    /// Bootstrap a new cluster
    pub async fn bootstrap_cluster(&self) -> Result<()> {
        info!("Bootstrapping new Raft cluster");

        let mut nodes = HashMap::new();
        nodes.insert(
            self.config.node.id,
            anode_raft::network::Node {
                addr: self.config.node.grpc_addr.clone(),
                data: Default::default(),
            }
        );

        self.raft
            .initialize(nodes)
            .await
            .context("Failed to initialize Raft cluster")?;

        info!("Cluster bootstrapped successfully, node {} is leader", self.config.node.id);
        Ok(())
    }

    /// Join an existing cluster
    pub async fn join_cluster(&self, addr: &str) -> Result<()> {
        info!("Joining cluster via {}", addr);

        // Add this node to the cluster manager
        let node_info = NodeInfo {
            id: self.config.node.id,
            name: self.config.node.name.clone(),
            grpc_addr: self.config.node.grpc_addr.clone(),
            s3_addr: self.config.node.s3_addr.clone(),
            admin_addr: self.config.node.admin_addr.clone(),
        };

        self.cluster_manager
            .add_node(node_info)
            .await
            .context("Failed to add node to cluster")?;

        info!("Successfully joined cluster");
        Ok(())
    }

    /// Gracefully shutdown the server
    pub async fn shutdown(&self) -> Result<()> {
        info!("Initiating graceful shutdown");

        // Signal all components to shutdown
        if let Err(e) = self.shutdown_tx.send(()) {
            warn!("Failed to send shutdown signal: {}", e);
        }

        // Wait for all tasks to complete
        let mut tasks = self.tasks.write().await;
        for task in tasks.drain(..) {
            if let Err(e) = task.await {
                error!("Task failed during shutdown: {}", e);
            }
        }

        // Shutdown Raft
        info!("Shutting down Raft");
        if let Err(e) = self.raft.shutdown().await {
            error!("Failed to shutdown Raft: {}", e);
        }

        info!("Shutdown complete");
        Ok(())
    }

    /// Wait for the server to shutdown
    pub async fn wait_for_shutdown(&self) {
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        shutdown_rx.recv().await.ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let config = Config::default();
        let server = Server::new(config).await;
        assert!(server.is_ok());
    }
}
