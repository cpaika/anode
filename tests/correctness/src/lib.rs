//! Correctness tests for Anode distributed object storage
//!
//! This crate contains integration tests that verify correctness properties
//! of the distributed system, including:
//! - Linearizability
//! - Durability
//! - Consistency
//! - Replication
//! - Recovery

pub mod consistency;
pub mod durability;
pub mod linearizability;
pub mod recovery;
pub mod replication;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize tracing for tests
pub fn init_tracing() {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}

/// Common test utilities
pub mod common {
    use std::time::Duration;

    /// Default timeout for tests
    pub const TEST_TIMEOUT: Duration = Duration::from_secs(300);

    /// Default cluster size
    pub const DEFAULT_CLUSTER_SIZE: usize = 3;

    /// Replication factor
    pub const REPLICATION_FACTOR: usize = 3;

    /// Default test data size
    pub const DEFAULT_DATA_SIZE: usize = 4096;

    /// Create a test bucket name
    pub fn test_bucket(test_name: &str) -> String {
        format!("test-{}", test_name)
    }

    /// Create a test object key
    pub fn test_key(index: usize) -> String {
        format!("key-{:08}", index)
    }

    /// Generate random test data
    pub fn random_data(size: usize) -> Vec<u8> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..size).map(|_| rng.gen()).collect()
    }

    /// Generate deterministic test data
    pub fn deterministic_data(seed: u64, size: usize) -> Vec<u8> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        (0..size).map(|_| rng.gen()).collect()
    }

    /// Create a test value with specific content
    pub fn test_value(content: &str) -> Vec<u8> {
        content.as_bytes().to_vec()
    }
}
