//! Anode Chaos Testing Framework
//!
//! Comprehensive chaos testing scenarios for distributed fault tolerance testing.
//! Provides utilities for:
//! - Node crash and recovery testing
//! - Network partition simulation
//! - Disk failure scenarios
//! - Network degradation (latency, packet loss, bandwidth limits)
//! - Rolling restarts and cascading failures
//! - Split-brain scenarios

pub mod scenarios;
pub mod network;
pub mod process;
pub mod disk;
pub mod runner;
pub mod verification;

// Re-export commonly used types
pub use runner::{ChaosRunner, ChaosResult, TestReport};
pub use scenarios::{ChaosScenario, ScenarioConfig};
pub use network::{NetworkChaos, NetworkCondition};
pub use process::ProcessChaos;
pub use disk::DiskChaos;
pub use verification::{DataVerifier, VerificationResult};

use anode_test_harness::TestHarnessError;
use thiserror::Error;

/// Result type for chaos testing operations
pub type Result<T> = std::result::Result<T, ChaosError>;

/// Errors that can occur during chaos testing
#[derive(Debug, Error)]
pub enum ChaosError {
    #[error("Test harness error: {0}")]
    Harness(#[from] TestHarnessError),

    #[error("Network chaos failed: {0}")]
    NetworkChaos(String),

    #[error("Process chaos failed: {0}")]
    ProcessChaos(String),

    #[error("Disk chaos failed: {0}")]
    DiskChaos(String),

    #[error("Data verification failed: {0}")]
    VerificationFailed(String),

    #[error("Scenario execution failed: {0}")]
    ScenarioFailed(String),

    #[error("Docker error: {0}")]
    Docker(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Timeout: {0}")]
    Timeout(String),
}

impl From<bollard::errors::Error> for ChaosError {
    fn from(e: bollard::errors::Error) -> Self {
        ChaosError::Docker(e.to_string())
    }
}

/// Common test utilities
pub mod utils {
    use std::time::Duration;
    use tokio::time::sleep;

    /// Retry an async operation with exponential backoff
    pub async fn retry_with_backoff<F, Fut, T, E>(
        mut f: F,
        max_attempts: usize,
        initial_delay: Duration,
    ) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        let mut delay = initial_delay;
        let mut last_error = None;

        for attempt in 0..max_attempts {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_attempts - 1 {
                        sleep(delay).await;
                        delay *= 2;
                    }
                }
            }
        }

        Err(last_error.unwrap())
    }

    /// Wait for a condition to become true
    pub async fn wait_for_condition<F>(
        mut condition: F,
        timeout: Duration,
        check_interval: Duration,
    ) -> bool
    where
        F: FnMut() -> bool,
    {
        let start = std::time::Instant::now();

        while start.elapsed() < timeout {
            if condition() {
                return true;
            }
            sleep(check_interval).await;
        }

        false
    }
}
