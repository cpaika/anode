# Cluster Management Module

This module provides comprehensive cluster bootstrap, discovery, membership, and rebalancing functionality for the Anode distributed storage system.

## Overview

The cluster module is organized into four main submodules:

1. **bootstrap** - Cluster initialization and joining
2. **discovery** - Node discovery mechanisms
3. **membership** - Membership management and health checking
4. **rebalance** - Data rebalancing across cluster nodes

## Modules

### bootstrap.rs

Handles cluster bootstrapping and node joining:

```rust
use anode::cluster::{ClusterBootstrap, BootstrapConfig};

let config = BootstrapConfig {
    node_id: 1,
    advertise_addr: "127.0.0.1:6000".to_string(),
    initial_members: vec![],
    join_addr: None,
    timeout: Duration::from_secs(30),
};

let bootstrap = ClusterBootstrap::new(config, raft, storage);

// Bootstrap a new cluster
bootstrap.bootstrap_new_cluster().await?;

// Or join an existing cluster
bootstrap.join_cluster("127.0.0.1:6001").await?;

// Or auto-detect
bootstrap.auto_bootstrap().await?;
```

**Key Features:**
- Bootstrap new single or multi-node clusters
- Join existing clusters
- Auto-detection of bootstrap vs join mode
- Timeout-based readiness checks
- Leader election waiting

### discovery.rs

Provides multiple node discovery mechanisms:

```rust
use anode::cluster::{Discovery, StaticDiscovery, DnsDiscovery, FileDiscovery, KubernetesDiscovery};

let mut discovery = Discovery::new();

// Static discovery
let static_nodes = vec![
    NodeInfo::new(1, "node1".to_string(), "127.0.0.1:6001".to_string()),
    NodeInfo::new(2, "node2".to_string(), "127.0.0.1:6002".to_string()),
];
discovery.add_method(Arc::new(StaticDiscovery::new(static_nodes)));

// DNS discovery (SRV records)
discovery.add_method(Arc::new(DnsDiscovery::new("anode.service.consul".to_string())));

// File-based discovery
discovery.add_method(Arc::new(FileDiscovery::new("/etc/anode/nodes.json".into())));

// Kubernetes discovery
discovery.add_method(Arc::new(KubernetesDiscovery::new(
    "default".to_string(),
    "anode".to_string(),
)));

// Discover nodes from all sources
let nodes = discovery.discover_all().await?;
```

**Supported Discovery Methods:**
- `StaticDiscovery` - Static list of nodes
- `DnsDiscovery` - DNS SRV record lookups
- `FileDiscovery` - JSON file with node list (watched for changes)
- `KubernetesDiscovery` - Kubernetes service endpoints

**Features:**
- Multiple discovery methods can be used simultaneously
- Automatic deduplication of discovered nodes
- Graceful handling of individual method failures

### membership.rs

Manages cluster membership and node health:

```rust
use anode::cluster::{MembershipManager, NodeInfo};

let manager = MembershipManager::new(raft, discovery);

// Add a node
let node = NodeInfo::new(2, "node2".to_string(), "127.0.0.1:6002".to_string());
manager.add_node(node).await?;

// Remove a node
manager.remove_node(2).await?;

// Get cluster members
let members = manager.get_members().await;

// Get leader
let leader = manager.get_leader().await;

// Check if we're the leader
if manager.is_leader().await {
    println!("I am the leader!");
}

// Health check all nodes
let health = manager.health_check().await;
for node_health in health {
    println!("Node {}: {:?}", node_health.node_id, node_health.status);
}

// Handle node failure
manager.handle_node_failure(2).await?;
```

**Key Features:**
- Add/remove nodes from cluster
- Automatic Raft membership updates
- Health monitoring with status tracking
- Leader detection and tracking
- Node failure handling
- Learner-to-voter promotion

**Health Status Types:**
- `Healthy` - Node is responding normally
- `Degraded` - Node is responding but with issues
- `Unhealthy` - Node is not responding
- `Unknown` - Health status not yet determined

### rebalance.rs

Handles data rebalancing when cluster topology changes:

```rust
use anode::cluster::{Rebalancer, RebalanceConfig};

let config = RebalanceConfig {
    max_concurrent_transfers: 4,
    rate_limit_bytes: 100 * 1024 * 1024, // 100 MB/s
    auto_rebalance: false,
    balance_threshold: 0.1, // 10%
};

let rebalancer = Rebalancer::new(membership, storage, config);

// Calculate rebalance plan
let plan = rebalancer.plan().await?;
println!("Plan: {} transfers, {} bytes, estimated {:?}",
    plan.transfers.len(),
    plan.estimated_bytes,
    plan.estimated_duration
);

// Execute the plan
let result = rebalancer.execute(&plan).await?;
println!("Transferred {} chunks ({} bytes) in {:?}",
    result.chunks_transferred,
    result.bytes_transferred,
    result.duration
);

// Check status
let status = rebalancer.status().await;
match status {
    RebalanceStatus::Idle => println!("No rebalance in progress"),
    RebalanceStatus::Planning => println!("Planning rebalance"),
    RebalanceStatus::InProgress { completed_transfers, total_transfers, .. } => {
        println!("Progress: {}/{}", completed_transfers, total_transfers);
    },
    RebalanceStatus::Completed { result } => println!("Completed: {:?}", result),
    RebalanceStatus::Failed { error } => println!("Failed: {}", error),
}

// Check if rebalance is needed
if rebalancer.is_rebalance_needed().await? {
    println!("Cluster is imbalanced, rebalance recommended");
}
```

**Key Features:**
- Automatic rebalance planning based on cluster state
- Concurrent chunk transfers with configurable limits
- Rate limiting to prevent network saturation
- Progress tracking and status reporting
- Balance score calculation
- Configurable balance thresholds
- Estimated duration calculation

**Rebalance Plan:**
- List of chunk transfers (source → destination)
- Estimated total bytes to transfer
- Estimated duration based on rate limits
- Considers placement group constraints

## Error Handling

Each module defines its own error types:

- `BootstrapError` - Bootstrap and joining errors
- `DiscoveryError` - Node discovery failures
- `MembershipError` - Membership management errors
- `RebalanceError` - Rebalancing failures

All errors implement `std::error::Error` and can be converted to/from related errors.

## Integration Example

Here's a complete example of setting up a cluster node:

```rust
use anode::cluster::*;
use std::sync::Arc;
use std::time::Duration;

async fn setup_cluster_node() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize storage and raft
    let storage = Arc::new(StorageEngine::open(storage_config).await?);
    let raft = Arc::new(AnodeRaft::new(/* ... */).await?);

    // Set up discovery
    let mut discovery = Discovery::new();
    discovery.add_method(Arc::new(StaticDiscovery::from_addresses(vec![
        "127.0.0.1:6001".to_string(),
        "127.0.0.1:6002".to_string(),
        "127.0.0.1:6003".to_string(),
    ])));
    let discovery = Arc::new(discovery);

    // Bootstrap the cluster
    let bootstrap_config = BootstrapConfig {
        node_id: 1,
        advertise_addr: "127.0.0.1:6001".to_string(),
        initial_members: vec![],
        join_addr: Some("127.0.0.1:6002".to_string()),
        timeout: Duration::from_secs(30),
    };

    let bootstrap = ClusterBootstrap::new(
        bootstrap_config,
        raft.clone(),
        storage.clone(),
    );

    let status = bootstrap.auto_bootstrap().await?;
    println!("Cluster status: {} nodes", status.total_nodes);

    // Set up membership management
    let membership = Arc::new(MembershipManager::new(raft.clone(), discovery));

    // Periodic health checks
    tokio::spawn({
        let membership = membership.clone();
        async move {
            loop {
                let health = membership.health_check().await;
                for node_health in health {
                    if node_health.status != HealthStatus::Healthy {
                        membership.handle_node_failure(node_health.node_id).await.ok();
                    }
                }
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
    });

    // Set up rebalancing
    let rebalance_config = RebalanceConfig::default();
    let rebalancer = Arc::new(Rebalancer::new(
        membership.clone(),
        storage.clone(),
        rebalance_config,
    ));

    // Periodic rebalancing check
    tokio::spawn({
        let rebalancer = rebalancer.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_secs(300)).await;

                if rebalancer.is_rebalance_needed().await.unwrap_or(false) {
                    if let Ok(plan) = rebalancer.plan().await {
                        rebalancer.execute(&plan).await.ok();
                    }
                }
            }
        }
    });

    Ok(())
}
```

## Testing

Each module includes comprehensive unit tests. Run tests with:

```bash
cargo test --lib cluster
```

Run tests for a specific module:

```bash
cargo test --lib cluster::bootstrap
cargo test --lib cluster::discovery
cargo test --lib cluster::membership
cargo test --lib cluster::rebalance
```

## Thread Safety

All types in this module are designed to be thread-safe:

- `ClusterBootstrap` - Send + Sync
- `Discovery` - Send + Sync via Arc<dyn DiscoveryMethod>
- `MembershipManager` - Send + Sync with internal RwLock
- `Rebalancer` - Send + Sync with internal RwLock

## Performance Considerations

### Discovery
- Discovery methods are called in parallel
- Results are cached to avoid repeated lookups
- Failed discovery methods don't block successful ones

### Membership
- Membership cache uses RwLock for concurrent reads
- Health checks can run concurrently
- Raft operations are serialized by the Raft layer

### Rebalancing
- Chunk transfers are concurrent up to `max_concurrent_transfers`
- Rate limiting prevents network saturation
- Progress tracking has minimal overhead

## Future Enhancements

Potential improvements for future versions:

1. **Bootstrap**
   - Automatic cluster healing
   - Multi-region bootstrapping
   - Witness nodes for quorum

2. **Discovery**
   - etcd/Consul integration
   - mDNS for local development
   - AWS/GCP cloud provider discovery

3. **Membership**
   - Automated failure recovery
   - Graceful node shutdown
   - Node weight/capacity tracking

4. **Rebalancing**
   - Smart chunk placement based on access patterns
   - Cost-aware rebalancing (network zones)
   - Incremental rebalancing
   - Automatic rebalancing on membership changes
