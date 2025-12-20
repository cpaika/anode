//! Anode Test Harness
//!
//! Comprehensive testing framework for distributed systems correctness testing.
//! Provides utilities for:
//! - Multi-node cluster management
//! - Workload simulation
//! - Chaos injection
//! - Correctness assertions
//! - Metrics collection

pub mod cluster;
pub mod simulation;
pub mod assertions;
pub mod chaos;
pub mod metrics;
pub mod workload;
pub mod history;

// Re-export commonly used types
pub use cluster::{TestCluster, TestNode, NodeConfig};
pub use simulation::{Simulation, SimulationResult};
pub use assertions::*;
pub use chaos::{ChaosEvent, ChaosScheduler, ChaosResult};
pub use metrics::TestMetrics;
pub use workload::{Workload, Operation, RandomWorkload, SequentialWorkload, HotspotWorkload, MixedWorkload};
pub use history::{OperationHistory, OperationResult, OperationValue, HistoryEntry};

use thiserror::Error;

/// Common result type for test harness operations
pub type Result<T> = std::result::Result<T, TestHarnessError>;

/// Errors that can occur in the test harness
#[derive(Debug, Error)]
pub enum TestHarnessError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Node failed to start: {0}")]
    NodeStartFailure(String),

    #[error("Node failed to stop: {0}")]
    NodeStopFailure(String),

    #[error("Timeout waiting for operation: {0}")]
    Timeout(String),

    #[error("Cluster operation failed: {0}")]
    ClusterOperationFailed(String),

    #[error("Assertion failed: {0}")]
    AssertionFailed(String),

    #[error("Chaos injection failed: {0}")]
    ChaosInjectionFailed(String),

    #[error("Simulation error: {0}")]
    SimulationError(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Process error: {0}")]
    ProcessError(String),

    #[error("Network error: {0}")]
    NetworkError(String),
}

/// Node identifier
pub type NodeId = u64;

/// Bucket name type
pub type BucketName = String;

/// Object key type
pub type ObjectKey = String;

/// Chunk identifier
pub type ChunkId = String;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_types() {
        let err = TestHarnessError::NodeStartFailure("test".to_string());
        assert!(err.to_string().contains("Node failed to start"));
    }
}
