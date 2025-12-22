# Configuration Reference

Complete reference for all Anode configuration options.

## Configuration File Format

Anode uses TOML format for configuration files. The default location is `/etc/anode/config.toml`, but can be overridden with the `--config` flag.

### Configuration Precedence

Configuration values are applied in the following order (highest to lowest priority):

1. **Command-line arguments** - Highest priority
2. **Environment variables** - Only `RUST_LOG` for logging configuration
3. **Configuration file** - TOML file specified via `--config` or default location
4. **Built-in defaults** - Lowest priority

This means command-line arguments will override configuration file values, which in turn override built-in defaults.

### Example Configuration

```toml
[node]
id = 1
name = "anode-1"
s3_addr = "0.0.0.0:9000"
grpc_addr = "0.0.0.0:9001"
admin_addr = "0.0.0.0:9002"

[cluster]
name = "production-cluster"
initial_members = [
    "anode-1:9001",
    "anode-2:9001",
    "anode-3:9001"
]
replication_factor = 3
placement_groups = 128
heartbeat_interval_ms = 1000
election_timeout_ms = 3000

[storage]
data_dir = "/var/lib/anode/data"
chunk_size = "4MB"
sync_writes = false
metadata_cache_size = 104857600  # 100MB
io_threads = 8

[s3]
max_body_size = 5368709120  # 5GB
request_timeout_secs = 300
enable_multipart = true
min_part_size = 5242880  # 5MB
virtual_host_style = false

[raft]
snapshot_interval = 10000
max_log_entries = 100000
compaction_threshold = 50000
enable_compaction = true

[parquet]
enable_cache = true
cache_size = 268435456  # 256MB
row_group_size = 100000
enable_predicate_pushdown = true
enable_column_pruning = true

[metrics]
enabled = true
interval_secs = 15
retention_secs = 0  # infinite

[logging]
level = "info"
json_format = false
file = "/var/log/anode/anode.log"
rotate = true
max_size = 104857600  # 100MB
```

## Configuration Sections

### [node]

Node-specific configuration for this instance.

#### `id` (required)

- **Type**: `integer`
- **Range**: 1 to 2^64-1
- **Description**: Unique identifier for this node in the cluster
- **Example**: `id = 1`

Must be unique across all nodes in the cluster. Once assigned, should not change.

#### `name` (required)

- **Type**: `string`
- **Description**: Human-readable name for this node
- **Example**: `name = "anode-us-east-1a-001"`

Used in logs and monitoring. Can be hostname or any descriptive name.

#### `s3_addr` (required)

- **Type**: `string`
- **Format**: `IP:PORT`
- **Default**: `"0.0.0.0:9000"`
- **Description**: Bind address for S3 HTTP API
- **Example**: `s3_addr = "0.0.0.0:9000"`

Use `0.0.0.0` to listen on all interfaces, or specific IP to bind to one interface.

#### `grpc_addr` (required)

- **Type**: `string`
- **Format**: `IP:PORT`
- **Default**: `"0.0.0.0:9001"`
- **Description**: Bind address for gRPC internal cluster communication
- **Example**: `grpc_addr = "10.0.1.5:9001"`

This port is used for:
- Raft consensus communication
- Inter-node data replication
- Cluster management

Should be accessible from all cluster nodes but not exposed publicly.

#### `admin_addr` (required)

- **Type**: `string`
- **Format**: `IP:PORT`
- **Default**: `"0.0.0.0:9002"`
- **Description**: Bind address for admin and metrics API
- **Example**: `admin_addr = "0.0.0.0:9002"`

Exposes:
- `/health` - Health check endpoint
- `/metrics` - Prometheus metrics
- `/admin/*` - Administrative endpoints

### [cluster]

Cluster-wide configuration. Must be consistent across all nodes.

#### `name` (required)

- **Type**: `string`
- **Description**: Cluster identifier
- **Example**: `name = "prod-us-east-1"`

Used to prevent nodes from different clusters joining each other.

#### `initial_members` (optional)

- **Type**: `array of strings`
- **Format**: `["HOST:PORT", ...]`
- **Default**: `[]`
- **Description**: Initial cluster members for bootstrap
- **Example**:
  ```toml
  initial_members = [
      "anode-1.cluster.local:9090",
      "anode-2.cluster.local:9090",
      "anode-3.cluster.local:9090"
  ]
  ```

Used when bootstrapping a new cluster. Can be empty if using `--bootstrap` or `--join` flags.

#### `replication_factor` (required)

- **Type**: `integer`
- **Range**: 1 to cluster_size
- **Default**: `3`
- **Description**: Number of replicas for each object
- **Example**: `replication_factor = 3`

**Recommendations:**
- Production: 3 (tolerates 1 node failure)
- High availability: 5 (tolerates 2 node failures)
- Testing/development: 1 (no replication)

Must be odd number for Raft quorum. Cannot exceed cluster size.

#### `placement_groups` (required)

- **Type**: `integer`
- **Range**: 1 to 65536
- **Default**: `128`
- **Description**: Number of placement groups for data partitioning
- **Example**: `placement_groups = 256`

More PGs = better distribution and parallelism, but higher overhead.

**Recommendations:**
- Small cluster (3-5 nodes): 64-128
- Medium cluster (6-20 nodes): 128-256
- Large cluster (21+ nodes): 256-512

Cannot be changed after cluster creation without data migration.

#### `heartbeat_interval_ms` (optional)

- **Type**: `integer`
- **Unit**: milliseconds
- **Default**: `1000`
- **Description**: Interval between Raft heartbeats
- **Example**: `heartbeat_interval_ms = 500`

Lower values = faster failure detection, higher network overhead.

**Recommendations:**
- Low latency network (<1ms): 500ms
- Normal network (1-10ms): 1000ms
- High latency network (>10ms): 2000ms

#### `election_timeout_ms` (optional)

- **Type**: `integer`
- **Unit**: milliseconds
- **Default**: `3000`
- **Description**: Timeout before triggering leader election
- **Example**: `election_timeout_ms = 5000`

Must be > `heartbeat_interval_ms`. Longer timeout = more stable but slower failover.

**Recommendations:**
- Low latency: 3000ms
- High latency or loaded: 5000-10000ms

### [storage]

Storage engine configuration.

#### `data_dir` (required)

- **Type**: `path`
- **Default**: `"/var/lib/anode/data"`
- **Description**: Root directory for all data storage
- **Example**: `data_dir = "/mnt/nvme/anode"`

Must have sufficient space for stored objects plus metadata. SSD/NVMe recommended.

Directory structure:
```
data_dir/
├── blobs/        # Object chunks
├── metadata/     # redb database
└── raft/         # Raft logs
```

#### `metadata_dir` (optional)

- **Type**: `path`
- **Default**: `{data_dir}/metadata`
- **Description**: Override metadata storage location
- **Example**: `metadata_dir = "/mnt/ssd/metadata"`

Useful for placing metadata on faster storage (SSD) separate from blob storage (HDD).

#### `chunk_size` (optional)

- **Type**: `string`
- **Format**: `"<number><unit>"` where unit is KB, MB, or GB
- **Default**: `"4MB"`
- **Description**: Size of data chunks for splitting objects
- **Example**: `chunk_size = "8MB"`

Larger chunks = fewer metadata operations, less deduplication opportunity.
Smaller chunks = more metadata overhead, better deduplication.

**Recommendations:**
- Small files (<10MB): 1-2MB chunks
- Large files (>100MB): 4-8MB chunks
- Very large files (>1GB): 8-16MB chunks

Minimum: 1MB. Cannot be changed after cluster creation without data migration.

#### `sync_writes` (optional)

- **Type**: `boolean`
- **Default**: `false`
- **Description**: Whether to fsync after each write
- **Example**: `sync_writes = true`

`true` = higher durability, lower performance (adds ~5-10ms per write)
`false` = higher performance, risk of data loss on power failure

**Recommendations:**
- Production with UPS: `false`
- Production without UPS: `true`
- Development: `false`

#### `metadata_cache_size` (optional)

- **Type**: `integer`
- **Unit**: bytes
- **Default**: `104857600` (100MB)
- **Description**: Size of in-memory metadata cache
- **Example**: `metadata_cache_size = 536870912` (512MB)

Larger cache = better performance for metadata operations.

**Recommendations:**
- Small cluster (<1M objects): 100MB
- Medium cluster (1-10M objects): 500MB
- Large cluster (>10M objects): 1-2GB

#### `io_threads` (optional)

- **Type**: `integer`
- **Default**: Number of CPU cores
- **Description**: Number of threads for I/O operations
- **Example**: `io_threads = 16`

More threads = higher concurrent I/O, more memory usage.

**Recommendations:**
- Match number of CPU cores for balanced workload
- Use 2x CPU cores for I/O-heavy workload
- Use 0.5x CPU cores for CPU-heavy workload

### [s3]

S3 API configuration.

#### `max_body_size` (optional)

- **Type**: `integer`
- **Unit**: bytes
- **Default**: `5368709120` (5GB)
- **Description**: Maximum size for single PUT request
- **Example**: `max_body_size = 10737418240` (10GB)

AWS S3 limit is 5GB for single PUT. Use multipart upload for larger objects.

#### `request_timeout_secs` (optional)

- **Type**: `integer`
- **Unit**: seconds
- **Default**: `300` (5 minutes)
- **Description**: Timeout for S3 requests
- **Example**: `request_timeout_secs = 600`

Increase for large uploads/downloads over slow connections.

#### `enable_multipart` (optional)

- **Type**: `boolean`
- **Default**: `true`
- **Description**: Enable multipart upload support
- **Example**: `enable_multipart = true`

Should always be `true` for production to support large files.

#### `min_part_size` (optional)

- **Type**: `integer`
- **Unit**: bytes
- **Default**: `5242880` (5MB)
- **Description**: Minimum size for multipart upload parts
- **Example**: `min_part_size = 5242880`

AWS S3 minimum is 5MB (except last part). Do not change unless you know what you're doing.

#### `virtual_host_style` (optional)

- **Type**: `boolean`
- **Default**: `false`
- **Description**: Enable virtual-hosted-style requests
- **Example**: `virtual_host_style = true`

`false` = path-style: `http://host/bucket/key`
`true` = virtual-hosted: `http://bucket.host/key`

Requires DNS wildcard or specific bucket DNS entries.

### [raft]

Raft consensus configuration.

#### `snapshot_interval` (optional)

- **Type**: `integer`
- **Unit**: log entries
- **Default**: `10000`
- **Description**: Create snapshot after this many log entries
- **Example**: `snapshot_interval = 50000`

Smaller interval = more snapshots, faster recovery, higher I/O.
Larger interval = fewer snapshots, slower recovery, lower I/O.

#### `max_log_entries` (optional)

- **Type**: `integer`
- **Default**: `100000`
- **Description**: Maximum log entries to keep in memory
- **Example**: `max_log_entries = 200000`

Higher value = more memory usage, faster replay.

#### `compaction_threshold` (optional)

- **Type**: `integer`
- **Default**: `50000`
- **Description**: Trigger log compaction after this many entries
- **Example**: `compaction_threshold = 100000`

Compaction removes old log entries covered by snapshots.

#### `enable_compaction` (optional)

- **Type**: `boolean`
- **Default**: `true`
- **Description**: Enable automatic log compaction
- **Example**: `enable_compaction = true`

Should always be `true` for production to prevent unbounded log growth.

### [parquet]

Parquet-specific optimizations.

#### `enable_cache` (optional)

- **Type**: `boolean`
- **Default**: `true`
- **Description**: Enable Parquet metadata caching
- **Example**: `enable_cache = true`

Caches footer metadata for faster queries.

#### `cache_size` (optional)

- **Type**: `integer`
- **Unit**: bytes
- **Default**: `268435456` (256MB)
- **Description**: Size of Parquet metadata cache
- **Example**: `cache_size = 536870912` (512MB)

Larger cache = more metadata in memory, faster queries.

#### `row_group_size` (optional)

- **Type**: `integer`
- **Default**: `100000`
- **Description**: Default row group size for Parquet files
- **Example**: `row_group_size = 1000000`

This setting is primarily for future use when Anode may generate Parquet files. Currently, Anode reads and stores Parquet files but doesn't generate them, so this setting has minimal impact.

#### `enable_predicate_pushdown` (optional)

- **Type**: `boolean`
- **Default**: `true`
- **Description**: Enable predicate pushdown optimization
- **Example**: `enable_predicate_pushdown = true`

Uses column statistics to skip row groups.

#### `enable_column_pruning` (optional)

- **Type**: `boolean`
- **Default**: `true`
- **Description**: Enable column projection optimization
- **Example**: `enable_column_pruning = true`

Reads only requested columns instead of entire row.

### [metrics]

Metrics and monitoring configuration.

#### `enabled` (optional)

- **Type**: `boolean`
- **Default**: `true`
- **Description**: Enable Prometheus metrics
- **Example**: `enabled = true`

Should be `true` for production monitoring.

#### `interval_secs` (optional)

- **Type**: `integer`
- **Unit**: seconds
- **Default**: `15`
- **Description**: Metrics collection interval
- **Example**: `interval_secs = 30`

Lower interval = more frequent metrics, higher overhead.

#### `retention_secs` (optional)

- **Type**: `integer`
- **Unit**: seconds
- **Default**: `0` (infinite)
- **Description**: How long to retain metrics in memory
- **Example**: `retention_secs = 3600`

`0` = infinite retention (metrics scraped by Prometheus).

### [logging]

Logging configuration.

#### `level` (optional)

- **Type**: `string`
- **Values**: `trace`, `debug`, `info`, `warn`, `error`
- **Default**: `"info"`
- **Description**: Default log level for all modules
- **Example**: `level = "debug"`

**Recommendations:**
- Production: `info` or `warn`
- Troubleshooting: `debug`
- Development: `debug` or `trace`

Can be overridden by the `RUST_LOG` environment variable or `module_levels` setting.

#### `module_levels` (optional)

- **Type**: `string`
- **Default**: `null` (use global `level`)
- **Description**: Per-module log level configuration
- **Format**: `"module1=level1,module2=level2,..."`
- **Example**: `module_levels = "anode_storage=debug,anode_s3=info"`

Allows fine-grained control over logging for specific modules:
```toml
# Enable debug logging for storage, keep others at info
module_levels = "anode_storage=debug"

# Multiple modules with different levels
module_levels = "anode_raft=trace,anode_storage=debug,anode_s3=info"
```

**Priority Order:**
1. `RUST_LOG` environment variable (highest)
2. `module_levels` configuration
3. `level` configuration (lowest)

#### `json_format` (optional)

- **Type**: `boolean`
- **Default**: `false`
- **Description**: Output logs in JSON format
- **Example**: `json_format = true`

`true` = machine-readable JSON logs (for log aggregation)
`false` = human-readable text logs

#### `file` (optional)

- **Type**: `path`
- **Default**: `null` (stdout only)
- **Description**: Log file path
- **Example**: `file = "/var/log/anode/anode.log"`

If not set, logs go to stdout only.

#### `rotate` (optional)

- **Type**: `boolean`
- **Default**: `false`
- **Description**: Enable log rotation
- **Example**: `rotate = true`

Requires `file` to be set.

#### `max_size` (optional)

- **Type**: `integer`
- **Unit**: bytes
- **Default**: `104857600` (100MB)
- **Description**: Maximum log file size before rotation
- **Example**: `max_size = 524288000` (500MB)

Only used if `rotate = true`.

## Environment Variable Overrides

### Logging Configuration

The `RUST_LOG` environment variable can override logging configuration:

```bash
# Set global log level
export RUST_LOG=debug

# Per-module log levels
export RUST_LOG="anode_storage=debug,anode_s3=info"

# More complex filtering
export RUST_LOG="warn,anode_storage=debug,anode_raft=trace"
```

The `RUST_LOG` environment variable takes precedence over both `module_levels` and `level` configuration settings.

**Note**: Currently, Anode does not support general `ANODE_*` environment variables for other configuration options. Use command-line arguments or the configuration file for non-logging settings.

## Command-Line Arguments

Command-line arguments override both config file and environment variables:

```bash
anode \
    --config /etc/anode/config.toml \
    --node-id 1 \
    --s3-addr 0.0.0.0:9000 \
    --grpc-addr 0.0.0.0:9001 \
    --admin-addr 0.0.0.0:9002 \
    --data-dir /var/lib/anode/data \
    --log-level info \
    --bootstrap
```

### Available Flags

- `--config <PATH>` - Configuration file path
- `--node-id <ID>` - Node ID
- `--s3-addr <ADDR>` - S3 API bind address
- `--grpc-addr <ADDR>` - gRPC bind address
- `--admin-addr <ADDR>` - Admin API bind address
- `--data-dir <PATH>` - Data directory
- `--log-level <LEVEL>` - Log level
- `--bootstrap` - Bootstrap new cluster
- `--join <ADDR>` - Join existing cluster at address

## Configuration Validation

Anode validates configuration on startup:

```
✓ Node ID must be > 0
✓ Node name must not be empty
✓ Replication factor must be > 0 and <= cluster size
✓ Placement groups must be > 0
✓ Chunk size must be >= 1MB
✓ Min part size must be >= 5MB
✓ Data directory must be writable
```

If validation fails, Anode will exit with an error message.

## Production Configuration Example

```toml
# Production-ready configuration for 5-node cluster

[node]
id = 1  # Change per node
name = "anode-prod-1"
s3_addr = "0.0.0.0:9000"
grpc_addr = "0.0.0.0:9001"
admin_addr = "0.0.0.0:9002"

[cluster]
name = "production"
initial_members = [
    "anode-1.prod.internal:9001",
    "anode-2.prod.internal:9001",
    "anode-3.prod.internal:9001",
    "anode-4.prod.internal:9001",
    "anode-5.prod.internal:9001"
]
replication_factor = 3
placement_groups = 256
heartbeat_interval_ms = 1000
election_timeout_ms = 3000

[storage]
data_dir = "/mnt/nvme/anode/data"
chunk_size = "8MB"
sync_writes = true  # Durability over performance
metadata_cache_size = 1073741824  # 1GB
io_threads = 16

[s3]
max_body_size = 5368709120  # 5GB
request_timeout_secs = 600
enable_multipart = true
min_part_size = 5242880
virtual_host_style = false

[raft]
snapshot_interval = 50000
max_log_entries = 200000
compaction_threshold = 100000
enable_compaction = true

[parquet]
enable_cache = true
cache_size = 536870912  # 512MB
enable_predicate_pushdown = true
enable_column_pruning = true

[metrics]
enabled = true
interval_secs = 15

[logging]
level = "info"
json_format = true
file = "/var/log/anode/anode.log"
rotate = true
max_size = 524288000  # 500MB
```

## Development Configuration Example

```toml
# Development configuration for local testing

[node]
id = 1
name = "dev-node-1"
s3_addr = "127.0.0.1:9000"
grpc_addr = "127.0.0.1:9001"
admin_addr = "127.0.0.1:9002"

[cluster]
name = "dev-cluster"
replication_factor = 1  # No replication for dev
placement_groups = 16   # Fewer PGs for simplicity

[storage]
data_dir = "/tmp/anode/data"
chunk_size = "1MB"      # Smaller chunks for testing
sync_writes = false     # Performance over durability

[logging]
level = "debug"         # Verbose logging
json_format = false     # Human-readable logs
```

## Configuration Best Practices

1. **Version Control**: Store configuration files in version control
2. **Secrets Management**: Use environment variables for secrets, not config files
3. **Validation**: Always validate configuration before deploying
4. **Documentation**: Comment non-obvious settings
5. **Consistency**: Keep cluster-wide settings consistent across nodes
6. **Monitoring**: Enable metrics and logging in production
7. **Testing**: Test configuration changes in dev/staging first
8. **Backup**: Keep backups of working configurations

## Troubleshooting Configuration Issues

### Configuration not loading

```bash
# Check file exists
ls -la /etc/anode/config.toml

# Check file permissions
chmod 644 /etc/anode/config.toml

# Validate TOML syntax
cat /etc/anode/config.toml | toml-test
```

### Validation errors

```bash
# Run with debug logging to see details
anode --config /etc/anode/config.toml --log-level debug

# Check specific field
grep "validation" /var/log/anode/anode.log
```

### Environment variable not working

```bash
# Check variable is set
env | grep ANODE

# Check precedence (CLI > env > file)
anode --config /etc/anode/config.toml --log-level trace
```
