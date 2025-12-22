//! Example: Custom chaos scenario
//!
//! This example demonstrates how to create and run a custom chaos scenario.

use anode_chaos_tests::scenarios::{ChaosScenario, ScenarioConfig, ScenarioResult};
use anode_chaos_tests::verification::DataVerifier;
use anode_chaos_tests::{ChaosError, Result};
use anode_test_harness::TestCluster;
use async_trait::async_trait;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tracing::{info, warn};

/// Custom scenario: Slow disk simulation
pub struct SlowDiskScenario;

#[async_trait]
impl ChaosScenario for SlowDiskScenario {
    fn name(&self) -> &str {
        "slow_disk"
    }

    fn description(&self) -> &str {
        "Simulates slow disk I/O and verifies system handles degraded performance"
    }

    async fn run(
        &self,
        cluster: &mut TestCluster,
        config: &ScenarioConfig,
    ) -> Result<ScenarioResult> {
        let start_time = Instant::now();
        let mut operations = 0;
        let mut failures = 0;

        info!("Starting slow disk scenario");

        let mut verifier = DataVerifier::new(cluster.get_s3_endpoint());

        // Write baseline data with normal performance
        info!("Writing baseline data...");
        let baseline_start = Instant::now();

        for i in 0..10 {
            verifier
                .write_and_track(
                    "test-bucket",
                    &format!("baseline-{}", i),
                    format!("data-{}", i).into_bytes(),
                )
                .await
                .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;
            operations += 1;
        }

        let baseline_duration = baseline_start.elapsed();
        info!("Baseline writes took {:?}", baseline_duration);

        // Simulate slow disk by adding artificial delays
        // In a real implementation, this would use ionice, cgroups, or similar
        warn!("Simulating slow disk (artificial delays)");

        let slow_start = Instant::now();

        for i in 0..10 {
            // Simulate slow disk with delays
            sleep(Duration::from_millis(100)).await;

            match verifier
                .write_and_track(
                    "test-bucket",
                    &format!("slow-{}", i),
                    format!("data-{}", i).into_bytes(),
                )
                .await
            {
                Ok(_) => operations += 1,
                Err(e) => {
                    failures += 1;
                    warn!("Write failed during slow disk: {}", e);
                },
            }

            sleep(Duration::from_millis(100)).await;
        }

        let slow_duration = slow_start.elapsed();
        info!("Slow disk writes took {:?}", slow_duration);

        // Verify all data
        let integrity_verified = verifier
            .verify_all()
            .await
            .map_err(|e| ChaosError::VerificationFailed(e.to_string()))?;

        // Calculate performance degradation
        let degradation_ratio = slow_duration.as_secs_f64() / baseline_duration.as_secs_f64();
        info!("Performance degradation: {:.2}x slower", degradation_ratio);

        Ok(ScenarioResult {
            name: self.name().to_string(),
            success: failures == 0 && integrity_verified,
            duration: start_time.elapsed(),
            operations,
            failures,
            integrity_verified,
            metrics: serde_json::json!({
                "baseline_duration_ms": baseline_duration.as_millis(),
                "slow_duration_ms": slow_duration.as_millis(),
                "degradation_ratio": degradation_ratio,
            }),
            error: if failures > 0 {
                Some(format!("{} operations failed", failures))
            } else if !integrity_verified {
                Some("Data integrity verification failed".to_string())
            } else {
                None
            },
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    println!("Running custom slow disk scenario...\n");

    // Configuration
    let config = ScenarioConfig {
        num_nodes: 3,
        duration: Duration::from_secs(60),
        verify_integrity: true,
        workload_intensity: 5.0,
        params: serde_json::Value::Null,
    };

    // Create and start cluster
    let mut cluster = TestCluster::new(config.num_nodes)
        .await
        .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

    cluster
        .start()
        .await
        .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

    // Run custom scenario
    let scenario = SlowDiskScenario;
    let result = scenario.run(&mut cluster, &config).await?;

    // Stop cluster
    cluster
        .stop()
        .await
        .map_err(|e| ChaosError::ScenarioFailed(e.to_string()))?;

    // Print results
    println!("\n=================================================");
    println!("Scenario Results");
    println!("=================================================");
    println!("Name: {}", result.name);
    println!("Success: {}", result.success);
    println!("Duration: {:?}", result.duration);
    println!("Operations: {}", result.operations);
    println!("Failures: {}", result.failures);
    println!("Integrity verified: {}", result.integrity_verified);
    println!(
        "Metrics: {}",
        serde_json::to_string_pretty(&result.metrics)?
    );

    if let Some(error) = &result.error {
        println!("Error: {}", error);
    }

    if result.success {
        println!("\n✓ Custom scenario passed!");
        Ok(())
    } else {
        println!("\n✗ Custom scenario failed");
        Err(ChaosError::ScenarioFailed("Test failed".to_string()))
    }
}
