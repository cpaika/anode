use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Invalid configuration: {0}")]
    ValidationError(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub node: NodeConfig,
    pub cluster: ClusterConfig,
    pub storage: StorageConfig,
    pub s3: S3Config,
    pub raft: RaftConfig,
    pub parquet: ParquetConfig,
    pub metrics: MetricsConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NodeConfig {
    /// Unique node identifier
    pub id: u64,

    /// Human-readable node name
    pub name: String,

    /// S3 API bind address (e.g., "0.0.0.0:9000")
    pub s3_addr: String,

    /// gRPC API bind address for internal cluster communication (e.g., "0.0.0.0:9001")
    pub grpc_addr: String,

    /// Admin/metrics API bind address (e.g., "0.0.0.0:9002")
    pub admin_addr: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClusterConfig {
    /// Cluster name
    pub name: String,

    /// Initial cluster members for bootstrap (gRPC addresses)
    pub initial_members: Vec<String>,

    /// Number of replicas for each object (default: 3)
    pub replication_factor: u32,

    /// Number of placement groups for distributing data (default: 128)
    pub placement_groups: u32,

    /// Heartbeat interval in milliseconds
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_ms: u64,

    /// Election timeout in milliseconds
    #[serde(default = "default_election_timeout")]
    pub election_timeout_ms: u64,

    /// TLS configuration for internal inter-node communication (optional)
    #[serde(default)]
    pub internal_tls: Option<InternalTlsConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    /// Root directory for data storage
    pub data_dir: PathBuf,

    /// Directory for metadata storage (defaults to data_dir/metadata)
    #[serde(default)]
    pub metadata_dir: Option<PathBuf>,

    /// Chunk size for object splitting (e.g., "4MB", "8MB")
    #[serde(default = "default_chunk_size")]
    pub chunk_size: String,

    /// Whether to sync writes to disk (fsync)
    #[serde(default = "default_sync_writes")]
    pub sync_writes: bool,

    /// Maximum cache size in bytes (for metadata)
    #[serde(default = "default_metadata_cache_size")]
    pub metadata_cache_size: usize,

    /// Number of worker threads for I/O operations
    #[serde(default = "default_io_threads")]
    pub io_threads: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct S3Config {
    /// Maximum request body size in bytes
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,

    /// Request timeout in seconds
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,

    /// Enable multipart upload support
    #[serde(default = "default_true")]
    pub enable_multipart: bool,

    /// Minimum part size for multipart uploads (5MB minimum per S3 spec)
    #[serde(default = "default_min_part_size")]
    pub min_part_size: usize,

    /// Enable virtual-hosted-style requests (bucket.domain.com)
    #[serde(default)]
    pub virtual_host_style: bool,

    /// TLS configuration (optional)
    #[serde(default)]
    pub tls: Option<TlsConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TlsConfig {
    /// Path to TLS certificate file (PEM format)
    pub cert_file: PathBuf,

    /// Path to TLS private key file (PEM format)
    pub key_file: PathBuf,

    /// Path to CA certificate file for client certificate authentication (optional)
    #[serde(default)]
    pub ca_file: Option<PathBuf>,

    /// Require client certificate authentication
    #[serde(default)]
    pub require_client_cert: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InternalTlsConfig {
    /// Enable TLS for internal inter-node communication (gRPC)
    #[serde(default)]
    pub enabled: bool,

    /// Path to server TLS certificate file (PEM format)
    /// This is the certificate presented by the server during TLS handshake
    pub cert_file: PathBuf,

    /// Path to server TLS private key file (PEM format)
    pub key_file: PathBuf,

    /// Path to CA certificate file for validating peer certificates
    /// When set, enables mTLS - peers must present valid certificates signed by this CA
    #[serde(default)]
    pub ca_file: Option<PathBuf>,

    /// Require client certificate authentication (mTLS)
    /// When true, all connecting peers must present valid client certificates
    /// Only effective if ca_file is set
    #[serde(default)]
    pub require_client_cert: bool,

    /// List of allowed DNS names in peer certificates (optional)
    /// If set, peer certificates must contain one of these DNS names in their SAN
    /// Provides additional security beyond CA validation
    #[serde(default)]
    pub allowed_dns_names: Vec<String>,

    /// Enable TLS certificate verification
    /// If false, accepts any certificate (useful for testing, NOT recommended for production)
    #[serde(default = "default_true")]
    pub verify_certificates: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RaftConfig {
    /// Snapshot interval in log entries
    #[serde(default = "default_snapshot_interval")]
    pub snapshot_interval: u64,

    /// Maximum number of log entries to keep in memory
    #[serde(default = "default_max_log_entries")]
    pub max_log_entries: u64,

    /// Compaction threshold (trigger compaction after this many log entries)
    #[serde(default = "default_compaction_threshold")]
    pub compaction_threshold: u64,

    /// Enable log compaction
    #[serde(default = "default_true")]
    pub enable_compaction: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ParquetConfig {
    /// Enable Parquet metadata caching
    #[serde(default = "default_true")]
    pub enable_cache: bool,

    /// Parquet cache size in bytes
    #[serde(default = "default_parquet_cache_size")]
    pub cache_size: usize,

    /// Number of rows per row group
    #[serde(default = "default_row_group_size")]
    pub row_group_size: usize,

    /// Enable predicate pushdown optimization
    #[serde(default = "default_true")]
    pub enable_predicate_pushdown: bool,

    /// Enable column pruning optimization
    #[serde(default = "default_true")]
    pub enable_column_pruning: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetricsConfig {
    /// Enable Prometheus metrics
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Metrics collection interval in seconds
    #[serde(default = "default_metrics_interval")]
    pub interval_secs: u64,

    /// Retention period for metrics in seconds (0 = infinite)
    #[serde(default)]
    pub retention_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Per-module log levels (e.g., "anode_storage=debug,anode_s3=info")
    /// If set, this takes precedence over the global level setting
    /// Format: "module1=level1,module2=level2,..."
    #[serde(default)]
    pub module_levels: Option<String>,

    /// Enable JSON formatted logs
    #[serde(default)]
    pub json_format: bool,

    /// Log file path (None = stdout only)
    #[serde(default)]
    pub file: Option<PathBuf>,

    /// Enable log rotation
    #[serde(default)]
    pub rotate: bool,

    /// Maximum log file size before rotation (in bytes)
    #[serde(default = "default_max_log_size")]
    pub max_size: usize,
}

// Default value functions
fn default_heartbeat_interval() -> u64 {
    1000
}
fn default_election_timeout() -> u64 {
    3000
}
fn default_chunk_size() -> String {
    "4MB".to_string()
}
fn default_sync_writes() -> bool {
    false
}
fn default_metadata_cache_size() -> usize {
    100 * 1024 * 1024
} // 100MB
fn default_io_threads() -> usize {
    num_cpus::get()
}
fn default_max_body_size() -> usize {
    5 * 1024 * 1024 * 1024
} // 5GB
fn default_request_timeout() -> u64 {
    300
}
fn default_true() -> bool {
    true
}
fn default_min_part_size() -> usize {
    5 * 1024 * 1024
} // 5MB
fn default_snapshot_interval() -> u64 {
    10000
}
fn default_max_log_entries() -> u64 {
    100000
}
fn default_compaction_threshold() -> u64 {
    50000
}
fn default_parquet_cache_size() -> usize {
    256 * 1024 * 1024
} // 256MB
fn default_row_group_size() -> usize {
    100000
}
fn default_metrics_interval() -> u64 {
    15
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_max_log_size() -> usize {
    100 * 1024 * 1024
} // 100MB

impl Config {
    /// Load configuration from a TOML file
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate node config
        if self.node.id == 0 {
            return Err(ConfigError::ValidationError(
                "Node ID must be greater than 0".to_string(),
            ));
        }

        if self.node.name.is_empty() {
            return Err(ConfigError::ValidationError(
                "Node name cannot be empty".to_string(),
            ));
        }

        // Validate cluster config
        if self.cluster.replication_factor == 0 {
            return Err(ConfigError::ValidationError(
                "Replication factor must be greater than 0".to_string(),
            ));
        }

        if self.cluster.placement_groups == 0 {
            return Err(ConfigError::ValidationError(
                "Placement groups must be greater than 0".to_string(),
            ));
        }

        // Validate storage config
        let chunk_size = parse_size(&self.storage.chunk_size).ok_or_else(|| {
            ConfigError::ValidationError(format!("Invalid chunk size: {}", self.storage.chunk_size))
        })?;

        if chunk_size < 1024 * 1024 {
            // Minimum 1MB
            return Err(ConfigError::ValidationError(
                "Chunk size must be at least 1MB".to_string(),
            ));
        }

        // Validate S3 config
        if self.s3.min_part_size < 5 * 1024 * 1024 {
            return Err(ConfigError::ValidationError(
                "Minimum part size must be at least 5MB (S3 requirement)".to_string(),
            ));
        }

        // Validate S3 TLS config if present
        if let Some(ref tls) = self.s3.tls {
            if !tls.cert_file.exists() {
                return Err(ConfigError::ValidationError(format!(
                    "TLS certificate file not found: {:?}",
                    tls.cert_file
                )));
            }
            if !tls.key_file.exists() {
                return Err(ConfigError::ValidationError(format!(
                    "TLS private key file not found: {:?}",
                    tls.key_file
                )));
            }
            if let Some(ref ca_file) = tls.ca_file {
                if !ca_file.exists() {
                    return Err(ConfigError::ValidationError(format!(
                        "TLS CA file not found: {:?}",
                        ca_file
                    )));
                }
            }
            if tls.require_client_cert && tls.ca_file.is_none() {
                return Err(ConfigError::ValidationError(
                    "Client certificate authentication requires ca_file to be set".to_string(),
                ));
            }
        }

        // Validate internal TLS config if present
        if let Some(ref tls) = self.cluster.internal_tls {
            if tls.enabled {
                if !tls.cert_file.exists() {
                    return Err(ConfigError::ValidationError(format!(
                        "Internal TLS certificate file not found: {:?}",
                        tls.cert_file
                    )));
                }
                if !tls.key_file.exists() {
                    return Err(ConfigError::ValidationError(format!(
                        "Internal TLS private key file not found: {:?}",
                        tls.key_file
                    )));
                }
                if let Some(ref ca_file) = tls.ca_file {
                    if !ca_file.exists() {
                        return Err(ConfigError::ValidationError(format!(
                            "Internal TLS CA file not found: {:?}",
                            ca_file
                        )));
                    }
                }
                if tls.require_client_cert && tls.ca_file.is_none() {
                    return Err(ConfigError::ValidationError(
                        "Internal TLS client certificate authentication requires ca_file to be set"
                            .to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    /// Save configuration to a TOML file
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let contents = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::ValidationError(e.to_string()))?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node: NodeConfig {
                id: 1,
                name: "anode-1".to_string(),
                s3_addr: "0.0.0.0:9000".to_string(),
                grpc_addr: "0.0.0.0:9001".to_string(),
                admin_addr: "0.0.0.0:9002".to_string(),
            },
            cluster: ClusterConfig {
                name: "anode-cluster".to_string(),
                initial_members: vec![],
                replication_factor: 3,
                placement_groups: 128,
                heartbeat_interval_ms: default_heartbeat_interval(),
                election_timeout_ms: default_election_timeout(),
                internal_tls: None,
            },
            storage: StorageConfig {
                data_dir: PathBuf::from("/var/lib/anode/data"),
                metadata_dir: None,
                chunk_size: default_chunk_size(),
                sync_writes: default_sync_writes(),
                metadata_cache_size: default_metadata_cache_size(),
                io_threads: default_io_threads(),
            },
            s3: S3Config {
                max_body_size: default_max_body_size(),
                request_timeout_secs: default_request_timeout(),
                enable_multipart: default_true(),
                min_part_size: default_min_part_size(),
                virtual_host_style: false,
                tls: None,
            },
            raft: RaftConfig {
                snapshot_interval: default_snapshot_interval(),
                max_log_entries: default_max_log_entries(),
                compaction_threshold: default_compaction_threshold(),
                enable_compaction: default_true(),
            },
            parquet: ParquetConfig {
                enable_cache: default_true(),
                cache_size: default_parquet_cache_size(),
                row_group_size: default_row_group_size(),
                enable_predicate_pushdown: default_true(),
                enable_column_pruning: default_true(),
            },
            metrics: MetricsConfig {
                enabled: default_true(),
                interval_secs: default_metrics_interval(),
                retention_secs: 0,
            },
            logging: LoggingConfig {
                level: default_log_level(),
                module_levels: None,
                json_format: false,
                file: None,
                rotate: false,
                max_size: default_max_log_size(),
            },
        }
    }
}

/// Parse human-readable size strings (e.g., "4MB", "1GB")
fn parse_size(s: &str) -> Option<usize> {
    let s = s.trim().to_uppercase();
    let (num_str, multiplier) = if s.ends_with("KB") {
        (&s[..s.len() - 2], 1024)
    } else if s.ends_with("MB") {
        (&s[..s.len() - 2], 1024 * 1024)
    } else if s.ends_with("GB") {
        (&s[..s.len() - 2], 1024 * 1024 * 1024)
    } else if s.ends_with('K') {
        (&s[..s.len() - 1], 1024)
    } else if s.ends_with('M') {
        (&s[..s.len() - 1], 1024 * 1024)
    } else if s.ends_with('G') {
        (&s[..s.len() - 1], 1024 * 1024 * 1024)
    } else {
        return s.parse().ok();
    };

    num_str.trim().parse::<usize>().ok().map(|n| n * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1024"), Some(1024));
        assert_eq!(parse_size("4KB"), Some(4 * 1024));
        assert_eq!(parse_size("4MB"), Some(4 * 1024 * 1024));
        assert_eq!(parse_size("1GB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_size("4M"), Some(4 * 1024 * 1024));
        assert_eq!(parse_size("invalid"), None);
    }

    #[test]
    fn test_default_config_validates() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }
}
