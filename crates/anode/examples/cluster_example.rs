/// Example of setting up an Anode cluster node with bootstrap, discovery, membership, and rebalancing
///
/// Run with: cargo run --example cluster_example
use anode::cluster::*;
use anode_raft::create_raft_node;
use anode_storage::{StorageConfig, StorageEngine};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("=== Anode Cluster Example ===\n");

    // Setup storage
    let temp_dir = TempDir::new()?;
    let storage_config = StorageConfig {
        data_dir: temp_dir.path().join("blobs"),
        metadata_dir: temp_dir.path().join("metadata"),
        chunk_size: anode_storage::chunks::CHUNK_SIZE,
        sync_writes: false,
    };
    let storage = Arc::new(StorageEngine::open(storage_config).await?);
    println!("✓ Storage initialized");

    // Setup Raft
    let raft = Arc::new(
        create_raft_node(1, "example-cluster", temp_dir.path().join("raft"), storage.clone())
            .await?,
    );
    println!("✓ Raft initialized");

    // Setup discovery with static nodes
    let mut discovery = Discovery::new();
    let static_nodes = vec![
        NodeInfo::new(1, "node1".to_string(), "127.0.0.1:6001".to_string()),
        NodeInfo::new(2, "node2".to_string(), "127.0.0.1:6002".to_string()),
        NodeInfo::new(3, "node3".to_string(), "127.0.0.1:6003".to_string()),
    ];
    discovery.add_method(Arc::new(StaticDiscovery::new(static_nodes)));
    let discovery = Arc::new(discovery);
    println!("✓ Discovery configured with 3 static nodes");

    // Bootstrap the cluster
    println!("\n--- Bootstrapping Cluster ---");
    let bootstrap_config = BootstrapConfig {
        node_id: 1,
        advertise_addr: "127.0.0.1:6001".to_string(),
        initial_members: vec![],
        join_addr: None,
        timeout: Duration::from_secs(10),
    };

    let bootstrap = ClusterBootstrap::new(bootstrap_config, raft.clone(), storage.clone());

    match bootstrap.bootstrap_new_cluster().await {
        Ok(()) => println!("✓ Cluster bootstrapped successfully"),
        Err(e) => println!("✗ Bootstrap failed: {}", e),
    }

    // Wait for cluster to be ready
    match bootstrap.wait_ready(Duration::from_secs(5)).await {
        Ok(()) => println!("✓ Cluster is ready"),
        Err(e) => println!("✗ Cluster not ready: {}", e),
    }

    // Setup membership management
    println!("\n--- Membership Management ---");
    let membership = Arc::new(MembershipManager::new(raft.clone(), discovery.clone()));

    let members = membership.get_members().await;
    println!("Current members: {}", members.len());

    if let Some(leader_id) = membership.get_leader().await {
        println!("Current leader: node {}", leader_id);
    }

    if membership.is_leader().await {
        println!("✓ This node is the leader");
    }

    // Simulate adding a node (in a real scenario, this would be a new node joining)
    println!("\nAttempting to add a new node...");
    let new_node = NodeInfo::new(2, "node2".to_string(), "127.0.0.1:6002".to_string());

    // Note: This will fail in single-node test environment but shows the API
    match membership.add_node(new_node).await {
        Ok(()) => println!("✓ Node added successfully"),
        Err(e) => println!(
            "Note: Add node returned: {} (expected in test environment)",
            e
        ),
    }

    // Health check
    println!("\n--- Health Check ---");
    let health_results = membership.health_check().await;
    for health in health_results {
        println!(
            "Node {}: {:?} (last seen: {:?})",
            health.node_id,
            health.status,
            health.last_seen.elapsed().unwrap_or(Duration::from_secs(0))
        );
    }

    // Setup rebalancing
    println!("\n--- Rebalancing ---");
    let rebalance_config = RebalanceConfig {
        max_concurrent_transfers: 4,
        rate_limit_bytes: 100 * 1024 * 1024, // 100 MB/s
        auto_rebalance: false,
        balance_threshold: 0.1,
    };

    let rebalancer = Rebalancer::new(membership.clone(), storage.clone(), rebalance_config);

    // Check if rebalance is needed
    match rebalancer.is_rebalance_needed().await {
        Ok(true) => println!("⚠ Rebalance is needed"),
        Ok(false) => println!("✓ Cluster is balanced"),
        Err(e) => println!("Note: Balance check returned: {}", e),
    }

    // Try to create a rebalance plan
    match rebalancer.plan().await {
        Ok(plan) => {
            println!("\nRebalance Plan:");
            println!("  Transfers: {}", plan.transfers.len());
            println!("  Estimated bytes: {}", plan.estimated_bytes);
            println!("  Estimated duration: {:?}", plan.estimated_duration);

            // Execute the plan
            match rebalancer.execute(&plan).await {
                Ok(result) => {
                    println!("\nRebalance Result:");
                    println!("  Chunks transferred: {}", result.chunks_transferred);
                    println!("  Bytes transferred: {}", result.bytes_transferred);
                    println!("  Duration: {:?}", result.duration);
                    if !result.errors.is_empty() {
                        println!("  Errors: {:?}", result.errors);
                    }
                },
                Err(e) => println!("Rebalance execution failed: {}", e),
            }
        },
        Err(e) => println!(
            "Note: Rebalance planning: {} (expected with limited nodes)",
            e
        ),
    }

    // Show final status
    let status = rebalancer.status().await;
    println!("\nFinal rebalance status: {:?}", status);

    println!("\n=== Cluster Example Complete ===");
    println!("\nThis example demonstrated:");
    println!("  ✓ Cluster bootstrapping");
    println!("  ✓ Node discovery");
    println!("  ✓ Membership management");
    println!("  ✓ Health checking");
    println!("  ✓ Rebalancing (planning and execution)");

    Ok(())
}
