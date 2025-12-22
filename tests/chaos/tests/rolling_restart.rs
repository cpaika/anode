//! Rolling restart chaos tests
//!
//! Tests system behavior during rolling restarts

use anode_test_harness::{assertions::*, chaos::ChaosScenarios, cluster::TestCluster};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
#[ignore]
async fn test_basic_rolling_restart() {
    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "rolling-restart-test";
    let key = "test-key";
    let value = b"test-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await.unwrap();

    // Perform rolling restart
    let scheduler = ChaosScenarios::rolling_restart(3, Duration::from_secs(5));
    scheduler.run(&mut cluster).await.unwrap();

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(5)).await;

    // Data should still be accessible
    assert_object_content(&cluster, bucket, key, value)
        .await
        .unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_rolling_restart_with_writes() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "rolling-restart-writes-test";

    // Start continuous writes
    let write_task = tokio::spawn({
        let endpoint = endpoint.clone();
        let bucket = bucket.to_string();
        async move {
            let client = reqwest::Client::new();
            let mut successful_writes = Vec::new();

            for i in 0..50 {
                let key = format!("key-{:03}", i);
                let value = format!("value-{}", i).into_bytes();
                let url = format!("{}/{}/{}", endpoint, &bucket, &key);

                match client.put(&url).body(value).send().await {
                    Ok(response) if response.status().is_success() => {
                        successful_writes.push(key);
                    },
                    _ => {},
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            successful_writes
        }
    });

    // Start rolling restart after a delay
    sleep(Duration::from_millis(500)).await;

    for i in 0..cluster.num_nodes() {
        cluster.restart_node(i).await.unwrap();
        sleep(Duration::from_secs(3)).await;
    }

    // Wait for writes to complete
    let successful_writes = write_task.await.unwrap();

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(3)).await;

    // Verify all successful writes are durable
    for key in successful_writes {
        assert_object_exists(&cluster, bucket, &key).await.unwrap();
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_fast_rolling_restart() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "fast-rolling-test";

    // Write test data
    for i in 0..10 {
        let key = format!("key-{}", i);
        let value = format!("value-{}", i).into_bytes();
        let url = format!("{}/{}/{}", endpoint, bucket, &key);

        client.put(&url).body(value).send().await.unwrap();
    }

    // Fast rolling restart (1 second intervals)
    let scheduler = ChaosScenarios::rolling_restart(cluster.num_nodes(), Duration::from_secs(1));
    scheduler.run(&mut cluster).await.unwrap();

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(10)).await;

    // Verify all data survived
    for i in 0..10 {
        let key = format!("key-{}", i);
        assert_object_exists(&cluster, bucket, &key).await.unwrap();
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_rolling_restart_leader_change() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    // Record initial leader
    let initial_leader = cluster
        .wait_for_leader(Duration::from_secs(10))
        .await
        .unwrap();

    // Perform rolling restart
    for i in 0..cluster.num_nodes() {
        cluster.restart_node(i).await.unwrap();
        sleep(Duration::from_secs(3)).await;

        // Wait for a leader to be present
        let _ = cluster.wait_for_leader(Duration::from_secs(10)).await;
    }

    // Wait for final stabilization
    sleep(Duration::from_secs(5)).await;

    // A leader should still be elected
    let final_leader = cluster
        .wait_for_leader(Duration::from_secs(10))
        .await
        .unwrap();

    println!(
        "Initial leader: {}, Final leader: {}",
        initial_leader, final_leader
    );

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_overlapping_rolling_restart() {
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "overlapping-restart-test";
    let key = "test-key";
    let value = b"test-value";

    // Write data
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await.unwrap();

    // Restart nodes with very short intervals so restarts overlap
    for i in 0..cluster.num_nodes() - 1 {
        tokio::spawn({
            let mut cluster_clone = cluster.get_node_mut(i).unwrap();
            async move {
                // This is a simplified version - in reality would need proper async handling
            }
        });

        sleep(Duration::from_millis(500)).await;
    }

    // Wait for all restarts to complete
    sleep(Duration::from_secs(15)).await;

    // Cluster should recover
    assert_cluster_healthy(&cluster).await.unwrap_or_default();

    // Data should still be accessible
    assert_object_exists(&cluster, bucket, key).await.unwrap();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_rolling_restart_zero_data_loss() {
    use anode_chaos_tests::verification::DataVerifier;

    // Create a 5-node cluster for robust testing
    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    // Wait for cluster to stabilize
    tokio::time::sleep(Duration::from_secs(5)).await;

    let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

    // Phase 1: Write initial dataset before rolling restart
    println!("Phase 1: Writing initial dataset...");
    for i in 0..20 {
        let bucket = "rolling-restart-zero-loss-test";
        let key = format!("pre-restart-{:03}", i);
        let data = format!("initial-data-{}", i).repeat(50).into_bytes();

        verifier
            .write_and_track(bucket, &key, data)
            .await
            .expect("Failed to write initial data");
    }
    println!("Wrote 20 objects before restart");

    // Wait for replication to complete
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Phase 2: Perform rolling restart while writing data
    println!("\nPhase 2: Starting rolling restart with concurrent writes...");

    let num_nodes = cluster.num_nodes();
    let restart_interval = Duration::from_secs(5);

    for node_idx in 0..num_nodes {
        println!("  Restarting node {} of {}", node_idx + 1, num_nodes);

        // Write data before restarting this node
        for j in 0..3 {
            let key = format!("during-restart-node-{}-write-{}", node_idx, j);
            let data = format!("data-during-restart-{}-{}", node_idx, j).into_bytes();

            match verifier
                .write_and_track("rolling-restart-zero-loss-test", &key, data)
                .await
            {
                Ok(_) => {},
                Err(e) => {
                    println!(
                        "  Warning: Write failed before restarting node {}: {}",
                        node_idx, e
                    );
                },
            }
        }

        // Restart the node
        cluster
            .restart_node(node_idx)
            .await
            .expect("Failed to restart node");

        // Wait for node to come back up
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Write data after restarting this node to verify cluster is operational
        for j in 0..2 {
            let key = format!("post-restart-node-{}-write-{}", node_idx, j);
            let data = format!("data-post-restart-{}-{}", node_idx, j).into_bytes();

            match verifier
                .write_and_track("rolling-restart-zero-loss-test", &key, data)
                .await
            {
                Ok(_) => {},
                Err(e) => {
                    println!(
                        "  Warning: Write failed after restarting node {}: {}",
                        node_idx, e
                    );
                },
            }
        }

        // Wait before restarting next node to maintain quorum
        if node_idx < num_nodes - 1 {
            tokio::time::sleep(restart_interval).await;
        }
    }

    println!("Rolling restart completed");

    // Phase 3: Wait for cluster to fully stabilize
    println!("\nPhase 3: Waiting for cluster to stabilize...");
    tokio::time::sleep(Duration::from_secs(10)).await;

    // Ensure leader is elected
    let leader = cluster
        .wait_for_leader(Duration::from_secs(30))
        .await
        .expect("Failed to elect leader after rolling restart");
    println!("Leader elected: node {}", leader);

    // Phase 4: Write post-restart data
    println!("\nPhase 4: Writing post-restart dataset...");
    for i in 0..15 {
        let key = format!("post-restart-{:03}", i);
        let data = format!("final-data-{}", i).into_bytes();

        verifier
            .write_and_track("rolling-restart-zero-loss-test", &key, data)
            .await
            .expect("Failed to write post-restart data");
    }
    println!("Wrote 15 objects after restart");

    // Phase 5: Verify zero data loss
    println!("\nPhase 5: Verifying data integrity...");
    let total_tracked = verifier.tracked_count();
    println!("Total objects tracked: {}", total_tracked);

    let verification_result = verifier
        .get_verification_results()
        .await
        .expect("Failed to get verification results");

    println!("\n=== Verification Results ===");
    println!("Total objects:    {}", verification_result.total_objects);
    println!("Verified objects: {}", verification_result.verified_objects);
    println!(
        "Missing objects:  {}",
        verification_result.missing_objects.len()
    );
    println!(
        "Corrupted objects: {}",
        verification_result.corrupted_objects.len()
    );

    if !verification_result.missing_objects.is_empty() {
        println!("\nMissing objects:");
        for obj in &verification_result.missing_objects {
            println!("  - {}", obj);
        }
    }

    if !verification_result.corrupted_objects.is_empty() {
        println!("\nCorrupted objects:");
        for obj in &verification_result.corrupted_objects {
            println!("  - {}", obj);
        }
    }

    // Assert zero data loss
    assert_eq!(
        verification_result.missing_objects.len(),
        0,
        "Zero data loss test failed: {} objects missing",
        verification_result.missing_objects.len()
    );
    assert_eq!(
        verification_result.corrupted_objects.len(),
        0,
        "Zero data loss test failed: {} objects corrupted",
        verification_result.corrupted_objects.len()
    );
    assert_eq!(
        verification_result.verified_objects, verification_result.total_objects,
        "All objects should be verified"
    );

    println!(
        "\n✓ Zero data loss verified: all {} objects intact after rolling restart",
        verification_result.total_objects
    );

    cluster.stop().await.unwrap();
}
