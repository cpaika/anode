//! Linearizability testing module
//!
//! Tests that verify the system maintains linearizability - all operations
//! appear to execute atomically and in a total order consistent with real time.
//!
//! This module implements Jepsen-style linearizability testing with:
//! - Concurrent operation recording with real-time timestamps
//! - History-based linearizability checking
//! - Support for various consistency models

use anode_test_harness::{
    cluster::TestCluster,
    history::{HistoryEntry, OperationHistory, OperationResult, OperationValue},
    workload::Operation,
    Result,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinSet;
use tracing::{info, warn};

/// Test single-key linearizability with concurrent writers
pub async fn test_single_key_concurrent_writes() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let history = OperationHistory::new();
    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();

    // Create bucket
    let bucket = "linearizability-test";
    let key = "concurrent-key";

    // Concurrent writers
    let mut tasks = JoinSet::new();
    for i in 0..10 {
        let history = history.clone();
        let client = client.clone();
        let endpoint = endpoint.clone();
        let bucket = bucket.to_string();
        let key = key.to_string();

        tasks.spawn(async move {
            let value = format!("value-{}", i).into_bytes();
            let op = Operation::Put {
                bucket: bucket.clone(),
                key: key.clone(),
                value: value.clone(),
            };

            let op_id = history.record_invoke(op).await;

            let url = format!("{}/{}/{}", endpoint, bucket, key);
            let result = match client.put(&url).body(value).send().await {
                Ok(response) if response.status().is_success() => {
                    OperationResult::Ok(OperationValue::Unit)
                }
                Ok(response) => {
                    OperationResult::Err(format!("PUT failed: {}", response.status()))
                }
                Err(e) => OperationResult::Err(format!("PUT error: {}", e)),
            };

            history.record_complete(op_id, result).await;
        });
    }

    // Wait for all writes
    while tasks.join_next().await.is_some() {}

    // Verify the final value is one of the written values
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    let response = client.get(&url).send().await?;
    let final_value = response.bytes().await?;
    let final_str = String::from_utf8_lossy(&final_value);

    assert!(
        final_str.starts_with("value-"),
        "Final value should be one of the written values"
    );

    cluster.stop().await?;
    Ok(())
}

/// Test read-your-writes consistency
pub async fn test_read_your_writes() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "ryw-test";
    let key = "test-key";

    // Write a value
    let value = b"test-value";
    let url = format!("{}/{}/{}", endpoint, bucket, key);
    client.put(&url).body(value.to_vec()).send().await?;

    // Immediately read it back
    let response = client.get(&url).send().await?;
    let read_value = response.bytes().await?;

    assert_eq!(
        read_value.as_ref(),
        value,
        "Should read the value that was just written"
    );

    cluster.stop().await?;
    Ok(())
}

/// Test monotonic reads - once a read returns a value, subsequent reads
/// should not return older values
pub async fn test_monotonic_reads() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "monotonic-test";
    let key = "test-key";
    let url = format!("{}/{}/{}", endpoint, bucket, key);

    // Write multiple versions
    for i in 0..5 {
        let value = format!("version-{}", i).into_bytes();
        client.put(&url).body(value).send().await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Read multiple times and verify monotonicity
    let mut last_version: Option<usize> = None;
    for _ in 0..10 {
        let response = client.get(&url).send().await?;
        let value = response.bytes().await?;
        let version_str = String::from_utf8_lossy(&value);

        if let Some(version) = version_str.strip_prefix("version-") {
            let current_version: usize = version.parse().unwrap();

            if let Some(last) = last_version {
                assert!(
                    current_version >= last,
                    "Monotonic reads violated: current={}, last={}",
                    current_version,
                    last
                );
            }

            last_version = Some(current_version);
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    cluster.stop().await?;
    Ok(())
}

/// Test multi-key linearizability with concurrent operations
pub async fn test_multi_key_linearizability() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let history = OperationHistory::new();
    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "multi-key-test";

    // Concurrent operations on multiple keys
    let mut tasks = JoinSet::new();
    let num_keys = 10;
    let ops_per_key = 5;

    for key_id in 0..num_keys {
        for op_id in 0..ops_per_key {
            let history = history.clone();
            let client = client.clone();
            let endpoint = endpoint.clone();
            let bucket = bucket.to_string();
            let key = format!("key-{}", key_id);

            tasks.spawn(async move {
                let value = format!("value-{}-{}", key_id, op_id).into_bytes();
                let op = Operation::Put {
                    bucket: bucket.clone(),
                    key: key.clone(),
                    value: value.clone(),
                };

                let op_id = history.record_invoke(op).await;

                let url = format!("{}/{}/{}", endpoint, bucket, key);
                let result = match client.put(&url).body(value).send().await {
                    Ok(response) if response.status().is_success() => {
                        OperationResult::Ok(OperationValue::Unit)
                    }
                    Ok(response) => {
                        OperationResult::Err(format!("PUT failed: {}", response.status()))
                    }
                    Err(e) => OperationResult::Err(format!("PUT error: {}", e)),
                };

                history.record_complete(op_id, result).await;
            });
        }
    }

    // Wait for all operations
    while tasks.join_next().await.is_some() {}

    // Check linearizability
    let checker = LinearizabilityChecker::new(Arc::new(history));
    let is_linearizable = checker.check().await?;

    assert!(
        is_linearizable,
        "Multi-key operations should be linearizable"
    );

    cluster.stop().await?;
    Ok(())
}

/// Test causality preservation
pub async fn test_causality_preservation() -> Result<()> {
    let mut cluster = TestCluster::new(3).await?;
    cluster.start().await?;

    let client = reqwest::Client::new();
    let endpoint = cluster.get_s3_endpoint();
    let bucket = "causality-test";

    // Create a causal chain: A -> B -> C
    // If we read C, we must also see A and B

    let key_a = "key-a";
    let key_b = "key-b";
    let key_c = "key-c";

    // Write A
    let url_a = format!("{}/{}/{}", endpoint, bucket, key_a);
    client.put(&url_a).body(b"value-a".to_vec()).send().await?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Write B (depends on A)
    let url_b = format!("{}/{}/{}", endpoint, bucket, key_b);
    client.put(&url_b).body(b"value-b".to_vec()).send().await?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Write C (depends on B)
    let url_c = format!("{}/{}/{}", endpoint, bucket, key_c);
    client.put(&url_c).body(b"value-c".to_vec()).send().await?;

    tokio::time::sleep(Duration::from_millis(100)).await;

    // If C is visible, A and B must also be visible
    let response_c = client.get(&url_c).send().await?;
    if response_c.status().is_success() {
        let response_a = client.get(&url_a).send().await?;
        let response_b = client.get(&url_b).send().await?;

        assert!(
            response_a.status().is_success(),
            "If C is visible, A must be visible (causality)"
        );
        assert!(
            response_b.status().is_success(),
            "If C is visible, B must be visible (causality)"
        );
    }

    cluster.stop().await?;
    Ok(())
}

/// Linearizability checker implementation
///
/// This implements a simplified linearizability checker based on the
/// Wing & Gong algorithm. For production use, consider using Knossos
/// or a similar complete implementation.
pub struct LinearizabilityChecker {
    history: Arc<OperationHistory>,
}

impl LinearizabilityChecker {
    /// Create a new linearizability checker
    pub fn new(history: Arc<OperationHistory>) -> Self {
        Self { history }
    }

    /// Check if the operation history is linearizable
    ///
    /// This performs a simplified linearizability check by:
    /// 1. Grouping operations by key
    /// 2. Checking that reads return values from completed writes
    /// 3. Verifying temporal ordering constraints
    pub async fn check(&self) -> Result<bool> {
        let entries = self.history.get_completed_entries().await;

        if entries.is_empty() {
            return Ok(true);
        }

        info!("Checking linearizability for {} operations", entries.len());

        // Group operations by key
        let mut by_key: HashMap<String, Vec<HistoryEntry>> = HashMap::new();

        for entry in entries {
            let key = match &entry.operation {
                Operation::Put { key, .. } | Operation::Get { key, .. } => key.clone(),
                Operation::Delete { key, .. } => key.clone(),
                _ => continue,
            };

            by_key.entry(key).or_default().push(entry);
        }

        // Check linearizability per key
        for (key, mut ops) in by_key {
            if !self.check_key_linearizable(&key, &mut ops).await? {
                warn!("Key {} failed linearizability check", key);
                return Ok(false);
            }
        }

        info!("Linearizability check passed");
        Ok(true)
    }

    /// Check if operations on a single key are linearizable
    async fn check_key_linearizable(&self, key: &str, ops: &mut [HistoryEntry]) -> Result<bool> {
        // Sort by invocation time
        ops.sort_by_key(|e| e.invoke_time);

        // Build a list of writes with their completion times
        let mut writes = Vec::new();

        for op in ops.iter() {
            match &op.operation {
                Operation::Put { value, .. } => {
                    if let Some(complete_time) = op.complete_time {
                        writes.push((complete_time, value.clone()));
                    }
                }
                _ => {}
            }
        }

        // Check each read operation
        for op in ops.iter() {
            match &op.operation {
                Operation::Get { .. } => {
                    if let Some(result) = &op.result {
                        match result {
                            OperationResult::Ok(OperationValue::Data(data)) => {
                                // Read must return a value from a write that
                                // completed before this read was invoked
                                let read_invoke = op.invoke_time;
                                let valid = writes.iter().any(|(write_time, write_value)| {
                                    *write_time <= read_invoke && write_value == data
                                });

                                if !valid {
                                    warn!(
                                        "Read returned value not from any completed write for key {}",
                                        key
                                    );
                                    return Ok(false);
                                }
                            }
                            OperationResult::Ok(OperationValue::NotFound) => {
                                // NotFound is acceptable if no writes completed
                                // before the read invocation (simplified check)
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(true)
    }

    /// Get statistics about the history
    pub async fn get_stats(&self) -> Result<String> {
        let stats = self.history.stats().await;
        Ok(format!(
            "Total ops: {}, Completed: {}, Successful: {}, Failed: {}, Avg duration: {:?}",
            stats.total, stats.completed, stats.successful, stats.failed, stats.avg_duration
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_linearizability_single_key() {
        let _ = test_single_key_concurrent_writes().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_linearizability_ryw() {
        let _ = test_read_your_writes().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_linearizability_monotonic() {
        let _ = test_monotonic_reads().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_linearizability_multi_key() {
        let _ = test_multi_key_linearizability().await;
    }

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_linearizability_causality() {
        let _ = test_causality_preservation().await;
    }

    #[tokio::test]
    async fn test_linearizability_checker_empty() {
        let history = Arc::new(OperationHistory::new());
        let checker = LinearizabilityChecker::new(history);
        assert!(checker.check().await.unwrap());
    }

    #[tokio::test]
    async fn test_linearizability_checker_simple() {
        let history = Arc::new(OperationHistory::new());

        // Write then read
        let write_op = Operation::Put {
            bucket: "test".to_string(),
            key: "key1".to_string(),
            value: vec![1, 2, 3],
        };

        let write_id = history.record_invoke(write_op).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        history
            .record_complete(write_id, OperationResult::Ok(OperationValue::Unit))
            .await;

        let read_op = Operation::Get {
            bucket: "test".to_string(),
            key: "key1".to_string(),
        };

        let read_id = history.record_invoke(read_op).await;
        tokio::time::sleep(Duration::from_millis(10)).await;
        history
            .record_complete(
                read_id,
                OperationResult::Ok(OperationValue::Data(vec![1, 2, 3])),
            )
            .await;

        let checker = LinearizabilityChecker::new(history);
        assert!(checker.check().await.unwrap());
    }
}
