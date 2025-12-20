//! Common chaos test scenarios

use anode_test_harness::{
    chaos::{ChaosEvent, ChaosScheduler, ChaosScenarios},
    cluster::TestCluster,
    Result,
};
use std::time::Duration;
use tokio::time::sleep;

/// Run a kill and restart scenario
pub async fn run_kill_restart_scenario(cluster: &mut TestCluster) -> Result<()> {
    let scheduler = ChaosScenarios::kill_and_restart(0, Duration::from_secs(10));
    scheduler.run(cluster).await?;
    Ok(())
}

/// Run a network partition scenario
pub async fn run_partition_scenario(cluster: &mut TestCluster) -> Result<()> {
    let scheduler = ChaosScenarios::partition_and_heal(vec![0, 1], Duration::from_secs(15));
    scheduler.run(cluster).await?;
    Ok(())
}

/// Run a rolling restart scenario
pub async fn run_rolling_restart_scenario(cluster: &mut TestCluster) -> Result<()> {
    let scheduler = ChaosScenarios::rolling_restart(
        cluster.num_nodes(),
        Duration::from_secs(5),
    );
    scheduler.run(cluster).await?;
    Ok(())
}

/// Run a split brain scenario
pub async fn run_split_brain_scenario(cluster: &mut TestCluster) -> Result<()> {
    let scheduler = ChaosScenarios::split_brain(cluster.num_nodes());
    scheduler.run(cluster).await?;

    // Wait a bit for the cluster to detect the partition
    sleep(Duration::from_secs(10)).await;

    // Heal the partition
    for i in 0..cluster.num_nodes() {
        cluster.heal_partition(i).await?;
    }

    Ok(())
}

/// Run cascading failures scenario
pub async fn run_cascading_failures_scenario(cluster: &mut TestCluster) -> Result<()> {
    let scheduler = ChaosScenarios::cascading_failures(
        cluster.num_nodes() - 1, // Leave one node alive
        Duration::from_secs(2),
    );
    scheduler.run(cluster).await?;
    Ok(())
}

/// Run random chaos scenario
pub async fn run_random_chaos_scenario(
    cluster: &mut TestCluster,
    duration: Duration,
    intensity: f64,
) -> Result<()> {
    let scheduler = ChaosScheduler::random(duration, intensity);
    scheduler.run(cluster).await?;
    Ok(())
}

/// Run multiple concurrent failures
pub async fn run_concurrent_failures_scenario(cluster: &mut TestCluster) -> Result<()> {
    let mut scheduler = ChaosScheduler::new();

    // Kill multiple nodes at the same time
    for i in 0..2 {
        scheduler = scheduler.add_event(Duration::from_secs(0), ChaosEvent::KillNode(i));
    }

    // Restart them after a delay
    for i in 0..2 {
        scheduler = scheduler.add_event(Duration::from_secs(10), ChaosEvent::RestartNode(i));
    }

    scheduler.run(cluster).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running cluster
    async fn test_kill_restart_scenario() {
        let mut cluster = TestCluster::new(3).await.unwrap();
        cluster.start().await.unwrap();

        let result = run_kill_restart_scenario(&mut cluster).await;
        assert!(result.is_ok());

        cluster.stop().await.unwrap();
    }
}
