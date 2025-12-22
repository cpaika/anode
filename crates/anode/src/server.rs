use anyhow::{Context, Result};
use axum::Router;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::admin::{admin_router, AdminState};
use crate::cluster::{ClusterManager, NodeInfo};
use crate::config::Config;
use crate::health::HealthChecker;
use crate::keystore::KeyStore;
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
    keystore: Option<Arc<KeyStore>>,
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
        let storage_config = anode_storage::StorageConfig {
            data_dir: config.storage.data_dir.join("blobs"),
            metadata_dir: config.storage.data_dir.join("metadata"),
            chunk_size: anode_storage::chunks::CHUNK_SIZE,
            sync_writes: true,
        };
        let storage = Arc::new(
            StorageEngine::open(storage_config)
                .await
                .context("Failed to initialize storage engine")?,
        );

        // Initialize Raft
        info!("Initializing Raft consensus");
        let raft_config = RaftConfig::new(
            config.node.id,
            &config.cluster.name,
            config.storage.data_dir.join("raft"),
        )
        .with_election_timeout(
            config.cluster.election_timeout_ms,
            config.cluster.election_timeout_ms * 2,
        )
        .with_heartbeat_interval(config.cluster.heartbeat_interval_ms);

        let raft = Arc::new(
            anode_raft::RaftNodeBuilder::new(raft_config)
                .storage_engine(storage.clone())
                .build()
                .await
                .context("Failed to initialize Raft")?,
        );

        // Initialize Parquet cache
        info!("Initializing Parquet cache");
        let parquet_cache = Arc::new(ParquetCache::new(config.parquet.cache_size));

        // Initialize cluster manager
        info!("Initializing cluster manager");
        let cluster_manager =
            Arc::new(ClusterManager::new(raft.clone(), config.cluster.placement_groups).await);

        // Initialize health checker
        let health_checker = Arc::new(HealthChecker::new(
            storage.clone(),
            raft.clone(),
            cluster_manager.clone(),
        ));

        // Initialize metrics
        let metrics = Arc::new(Metrics::new());

        // Initialize key store (optional, for access key management)
        info!("Initializing access key store");
        let keystore_path = config.storage.data_dir.join("keys.db");
        let keystore = match KeyStore::open(&keystore_path) {
            Ok(ks) => {
                info!("Access key store initialized");
                Some(Arc::new(ks))
            },
            Err(e) => {
                warn!(
                    "Failed to initialize key store: {}. Access key management will be disabled.",
                    e
                );
                None
            },
        };

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
            keystore,
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
        let addr: SocketAddr = self
            .config
            .node
            .s3_addr
            .parse()
            .context("Invalid S3 bind address")?;

        // Create S3 router with storage backend
        let s3_router = S3Router::new(self.storage.clone(), None);
        let app = s3_router.into_router();

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Check if TLS is configured
        if let Some(ref tls_config) = self.config.s3.tls {
            info!("Starting S3 API server with TLS on {}", addr);

            // Log TLS configuration details
            info!("TLS certificate: {:?}", tls_config.cert_file);
            info!("TLS private key: {:?}", tls_config.key_file);
            if let Some(ref ca_file) = tls_config.ca_file {
                info!("TLS CA file: {:?}", ca_file);
                if tls_config.require_client_cert {
                    info!("Client certificate authentication: required");
                } else {
                    info!("Client certificate authentication: optional");
                }
            }

            // Load TLS configuration
            let tls_config = self
                .load_tls_config(tls_config)
                .context("Failed to load TLS configuration")?;

            let handle = tokio::spawn(async move {
                info!("S3 API server listening on {} (HTTPS)", addr);

                let handle = axum_server::Handle::new();
                let shutdown_handle = handle.clone();

                // Spawn a task to handle graceful shutdown
                tokio::spawn(async move {
                    shutdown_rx.recv().await.ok();
                    info!("S3 API server shutting down");
                    shutdown_handle.shutdown();
                });

                let serve = axum_server::bind_rustls(addr, tls_config)
                    .handle(handle)
                    .serve(app.into_make_service());

                if let Err(e) = serve.await {
                    error!("S3 server error: {}", e);
                }
            });

            Ok(handle)
        } else {
            info!("Starting S3 API server without TLS on {}", addr);

            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .context("Failed to bind S3 server")?;

            let handle = tokio::spawn(async move {
                info!("S3 API server listening on {} (HTTP)", addr);

                let serve = axum::serve(listener, app).with_graceful_shutdown(async move {
                    shutdown_rx.recv().await.ok();
                    info!("S3 API server shutting down");
                });

                if let Err(e) = serve.await {
                    error!("S3 server error: {}", e);
                }
            });

            Ok(handle)
        }
    }

    /// Start the gRPC server for internal cluster communication
    async fn start_grpc_server(&self) -> Result<JoinHandle<()>> {
        let addr: SocketAddr = self
            .config
            .node
            .grpc_addr
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
        let addr: SocketAddr = self
            .config
            .node
            .admin_addr
            .parse()
            .context("Invalid admin bind address")?;

        info!("Starting admin API server on {}", addr);

        let state = AdminState {
            health_checker: self.health_checker.clone(),
            cluster_manager: self.cluster_manager.clone(),
            metrics: self.metrics.clone(),
            raft: self.raft.clone(),
            keystore: self.keystore.clone(),
        };

        let app = admin_router(state);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .context("Failed to bind admin server")?;

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        let handle = tokio::spawn(async move {
            info!("Admin API server listening on {}", addr);

            let serve = axum::serve(listener, app).with_graceful_shutdown(async move {
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

        let mut nodes = BTreeMap::new();
        nodes.insert(
            self.config.node.id,
            openraft::BasicNode {
                addr: self.config.node.grpc_addr.clone(),
            },
        );

        self.raft
            .initialize(nodes)
            .await
            .context("Failed to initialize Raft cluster")?;

        info!(
            "Cluster bootstrapped successfully, node {} is leader",
            self.config.node.id
        );
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

    /// Load TLS configuration from certificate and key files
    fn load_tls_config(
        &self,
        tls_config: &crate::config::TlsConfig,
    ) -> Result<axum_server::tls_rustls::RustlsConfig> {
        use std::fs::File;
        use std::io::BufReader;

        // Install default crypto provider if not already set
        // This is required for rustls 0.23+
        let _ = rustls::crypto::ring::default_provider().install_default();

        info!("Loading TLS certificates");

        // Read certificate file
        let cert_file = File::open(&tls_config.cert_file).with_context(|| {
            format!(
                "Failed to open certificate file: {:?}",
                tls_config.cert_file
            )
        })?;
        let mut cert_reader = BufReader::new(cert_file);

        let certs = rustls_pemfile::certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to parse certificate file")?;

        if certs.is_empty() {
            return Err(anyhow::anyhow!("No certificates found in certificate file"));
        }

        // Read private key file
        let key_file = File::open(&tls_config.key_file).with_context(|| {
            format!("Failed to open private key file: {:?}", tls_config.key_file)
        })?;
        let mut key_reader = BufReader::new(key_file);

        let private_key = rustls_pemfile::private_key(&mut key_reader)
            .context("Failed to parse private key file")?
            .ok_or_else(|| anyhow::anyhow!("No private key found in key file"))?;

        // Build rustls server config
        let mut server_config = if let Some(ref ca_file) = tls_config.ca_file {
            info!("Loading CA certificate for client authentication");

            let ca_file = File::open(ca_file)
                .with_context(|| format!("Failed to open CA file: {:?}", ca_file))?;
            let mut ca_reader = BufReader::new(ca_file);

            let ca_certs = rustls_pemfile::certs(&mut ca_reader)
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to parse CA certificate file")?;

            if ca_certs.is_empty() {
                return Err(anyhow::anyhow!("No CA certificates found in CA file"));
            }

            let mut root_cert_store = rustls::RootCertStore::empty();
            for cert in ca_certs {
                root_cert_store
                    .add(cert)
                    .context("Failed to add CA certificate to root store")?;
            }

            let client_cert_verifier = if tls_config.require_client_cert {
                rustls::server::WebPkiClientVerifier::builder(root_cert_store.into())
                    .build()
                    .context("Failed to build client certificate verifier")?
            } else {
                rustls::server::WebPkiClientVerifier::builder(root_cert_store.into())
                    .allow_unauthenticated()
                    .build()
                    .context("Failed to build client certificate verifier")?
            };

            rustls::ServerConfig::builder()
                .with_client_cert_verifier(client_cert_verifier)
                .with_single_cert(certs, private_key)
                .context("Failed to build TLS server configuration")?
        } else {
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, private_key)
                .context("Failed to build TLS server configuration")?
        };

        // Enable HTTP/2 and HTTP/1.1
        server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

        info!("TLS configuration loaded successfully");

        Ok(axum_server::tls_rustls::RustlsConfig::from_config(
            std::sync::Arc::new(server_config),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_server_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.storage.data_dir = temp_dir.path().to_path_buf();
        let server = Server::new(config).await;
        assert!(server.is_ok());
    }
}
