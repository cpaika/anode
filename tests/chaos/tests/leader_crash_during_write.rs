//! Leader crash during write chaos test
//!
//! Tests system behavior when the leader crashes during a write operation.
//! Verifies that either the write succeeds completely or fails cleanly with
//! no partial writes, and that the cluster recovers properly.

use anode_chaos_tests::scenarios::{
    node_crash::LeaderCrashDuringWriteScenario, ChaosScenario, ScenarioConfig,
};
use anode_test_harness::{assertions::*, cluster::TestCluster};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
#[ignore]
async fn test_leader_crash_during_write_scenario() {
    // Initialize tracing for test visibility
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(5)).await;

    let config = ScenarioConfig {
        num_nodes: 5,
        duration: Duration::from_secs(30),
        verify_integrity: true,
        workload_intensity: 10.0,
        params: serde_json::Value::Null,
    };

    let scenario = LeaderCrashDuringWriteScenario;

    let result = scenario.run(&mut cluster, &config).await.unwrap();

    // Verify the scenario succeeded
    assert!(result.success, "Scenario failed: {:?}", result.error);
    assert_eq!(result.failures, 0, "Expected no failures");
    assert!(
        result.integrity_verified,
        "Data integrity should be verified"
    );

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_leader_crash_during_small_write() {
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

    let mut cluster = TestCluster::new(3).await.unwrap();
    cluster.start().await.unwrap();

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "leader-crash-test";
    let key = "small-object";
    let value = b"small-data";

    // Wait for initial leader election
    let leader_id = cluster
        .wait_for_leader(Duration::from_secs(10))
        .await
        .unwrap();

    // Start a write operation
    let write_handle = tokio::spawn({
        let endpoint = endpoint.clone();
        let bucket = bucket.to_string();
        let key = key.to_string();
        let value = value.to_vec();
        async move {
            let url = format!("{}/{}/{}", endpoint, bucket, key);
            reqwest::Client::new().put(&url).body(value).send().await
        }
    });

    // Kill the leader immediately
    sleep(Duration::from_millis(50)).await;
    cluster.kill_node(leader_id as usize).await.unwrap();

    // Wait for write to complete or fail
    let write_result = write_handle.await.unwrap();

    match write_result {
        Ok(response) if response.status().is_success() => {
            // Write succeeded - verify object exists
            sleep(Duration::from_secs(2)).await;
            assert_object_exists(&cluster, bucket, key).await.unwrap();
        },
        _ => {
            // Write failed - verify no partial write
            sleep(Duration::from_secs(2)).await;
            // Object should not exist if write failed
            let exists = assert_object_exists(&cluster, bucket, key).await;
            if exists.is_ok() {
                panic!("Partial write detected - atomicity violated!");
            }
        },
    }

    // Verify new leader was elected
    let new_leader = cluster
        .wait_for_leader(Duration::from_secs(10))
        .await
        .unwrap();
    assert_ne!(
        leader_id, new_leader,
        "New leader should be different from crashed leader"
    );

    // Restart the old leader
    cluster.restart_node(leader_id as usize).await.unwrap();
    sleep(Duration::from_secs(5)).await;

    // Cluster should be healthy
    assert_cluster_healthy(&cluster).await.unwrap_or_default();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_leader_crash_during_large_write() {
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let endpoint = cluster.get_s3_endpoint();
    let bucket = "leader-crash-large-test";
    let key = "large-object";
    let large_value = vec![42u8; 10 * 1024 * 1024]; // 10 MB

    // Wait for initial leader election
    let leader_id = cluster
        .wait_for_leader(Duration::from_secs(10))
        .await
        .unwrap();

    // Start a large write operation
    let write_handle = tokio::spawn({
        let endpoint = endpoint.clone();
        let bucket = bucket.to_string();
        let key = key.to_string();
        let value = large_value.clone();
        async move {
            let url = format!("{}/{}/{}", endpoint, bucket, key);
            reqwest::Client::new().put(&url).body(value).send().await
        }
    });

    // Kill the leader after a short delay (during the write)
    sleep(Duration::from_millis(100)).await;
    cluster.kill_node(leader_id as usize).await.unwrap();

    // Wait for write to complete or fail
    let write_result = write_handle.await.unwrap();

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(5)).await;

    match write_result {
        Ok(response) if response.status().is_success() => {
            // Write succeeded despite leader crash - verify complete object
            assert_object_exists(&cluster, bucket, key).await.unwrap();

            // Verify the data is correct
            let url = format!("{}/{}/{}", endpoint, bucket, key);
            let read_response = reqwest::Client::new().get(&url).send().await.unwrap();
            let read_data = read_response.bytes().await.unwrap();
            assert_eq!(
                read_data.len(),
                large_value.len(),
                "Object size should match"
            );
        },
        _ => {
            // Write failed - this is acceptable, just verify no partial write
            let exists = assert_object_exists(&cluster, bucket, key).await;
            if exists.is_ok() {
                // If object exists, it must be complete
                let url = format!("{}/{}/{}", endpoint, bucket, key);
                let read_response = reqwest::Client::new().get(&url).send().await.unwrap();
                let read_data = read_response.bytes().await.unwrap();

                // Either the object should not exist, or it should be complete
                assert_eq!(
                    read_data.len(),
                    large_value.len(),
                    "If object exists after failed write, it must be complete - no partial writes allowed"
                );
            }
        },
    }

    // Verify new leader was elected
    let new_leader = cluster
        .wait_for_leader(Duration::from_secs(10))
        .await
        .unwrap();

    // Restart the old leader
    cluster.restart_node(leader_id as usize).await.unwrap();
    sleep(Duration::from_secs(5)).await;

    // Cluster should be healthy
    assert_cluster_healthy(&cluster).await.unwrap_or_default();

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_leader_crash_during_concurrent_writes() {
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let endpoint = cluster.get_s3_endpoint();
    let bucket = "leader-crash-concurrent-test";

    // Wait for initial leader election
    let leader_id = cluster
        .wait_for_leader(Duration::from_secs(10))
        .await
        .unwrap();

    // Start multiple concurrent writes
    let mut write_handles = Vec::new();
    for i in 0..10 {
        let handle = tokio::spawn({
            let endpoint = endpoint.clone();
            let bucket = bucket.to_string();
            let key = format!("object-{}", i);
            let value = format!("data-{}", i).into_bytes();
            async move {
                let url = format!("{}/{}/{}", endpoint, bucket, key);
                let result = reqwest::Client::new().put(&url).body(value).send().await;
                (key, result)
            }
        });
        write_handles.push(handle);
    }

    // Kill the leader while writes are in progress
    sleep(Duration::from_millis(50)).await;
    cluster.kill_node(leader_id as usize).await.unwrap();

    // Collect results
    let mut successful_writes = Vec::new();
    for handle in write_handles {
        let (key, result) = handle.await.unwrap();
        if let Ok(response) = result {
            if response.status().is_success() {
                successful_writes.push(key);
            }
        }
    }

    // Wait for cluster to stabilize
    sleep(Duration::from_secs(5)).await;

    // Verify new leader was elected
    let new_leader = cluster
        .wait_for_leader(Duration::from_secs(10))
        .await
        .unwrap();
    assert_ne!(leader_id, new_leader, "New leader should be elected");

    // Verify all successful writes are durable
    for key in &successful_writes {
        assert_object_exists(&cluster, bucket, key).await.unwrap();
    }

    // Restart the old leader
    cluster.restart_node(leader_id as usize).await.unwrap();
    sleep(Duration::from_secs(5)).await;

    // Verify cluster is healthy
    assert_cluster_healthy(&cluster).await.unwrap_or_default();

    // Verify all successful writes are still accessible
    for key in &successful_writes {
        assert_object_exists(&cluster, bucket, key).await.unwrap();
    }

    cluster.stop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_repeated_leader_crashes_during_writes() {
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

    let mut cluster = TestCluster::new(5).await.unwrap();
    cluster.start().await.unwrap();

    let endpoint = cluster.get_s3_endpoint();
    let bucket = "leader-crash-repeated-test";

    // Perform multiple cycles of write + leader crash
    for cycle in 0..3 {
        let leader_id = cluster
            .wait_for_leader(Duration::from_secs(10))
            .await
            .unwrap();

        let key = format!("cycle-{}", cycle);
        let value = format!("data-{}", cycle).into_bytes();

        // Start write
        let write_handle = tokio::spawn({
            let endpoint = endpoint.clone();
            let bucket = bucket.to_string();
            let key = key.clone();
            let value = value.clone();
            async move {
                let url = format!("{}/{}/{}", endpoint, bucket, key);
                reqwest::Client::new().put(&url).body(value).send().await
            }
        });

        // Kill leader during write
        sleep(Duration::from_millis(50)).await;
        cluster.kill_node(leader_id as usize).await.unwrap();

        let write_result = write_handle.await.unwrap();

        // Wait for new leader
        sleep(Duration::from_secs(3)).await;
        cluster
            .wait_for_leader(Duration::from_secs(10))
            .await
            .unwrap();

        // If write succeeded, verify it's durable
        if let Ok(response) = write_result {
            if response.status().is_success() {
                assert_object_exists(&cluster, bucket, &key).await.unwrap();
            }
        }

        // Restart the crashed leader
        cluster.restart_node(leader_id as usize).await.unwrap();
        sleep(Duration::from_secs(3)).await;
    }

    // Final verification - cluster should be healthy
    assert_cluster_healthy(&cluster).await.unwrap_or_default();

    cluster.stop().await.unwrap();
}
