//! Example: Run all chaos scenarios
//!
//! This example demonstrates how to run all chaos scenarios sequentially
//! and generate a comprehensive test report.

use anode_chaos_tests::runner::ChaosRunner;
use anode_chaos_tests::scenarios::cascade_failure::*;
use anode_chaos_tests::scenarios::network_partition::*;
use anode_chaos_tests::scenarios::node_crash::*;
use anode_chaos_tests::scenarios::rolling_restart::*;
use anode_chaos_tests::scenarios::slow_network::*;
use anode_chaos_tests::scenarios::split_brain::*;
use anode_chaos_tests::scenarios::ScenarioConfig;
use anode_test_harness::TestCluster;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    println!("=================================================");
    println!("Anode Chaos Testing Suite");
    println!("=================================================\n");

    // Configuration for all scenarios
    let config = ScenarioConfig {
        num_nodes: 5,
        duration: Duration::from_secs(60),
        verify_integrity: true,
        workload_intensity: 10.0,
        params: serde_json::Value::Null,
    };

    // Create test cluster
    println!("Creating {} node cluster...", config.num_nodes);
    let mut cluster = TestCluster::new(config.num_nodes).await?;
    cluster.start().await?;
    println!("Cluster started successfully\n");

    // Create chaos runner
    let mut runner = ChaosRunner::new(config.clone());

    // Add all scenarios
    println!("Registering chaos scenarios...");

    // Node crash scenarios
    runner = runner.add_scenario(Box::new(RandomNodeCrashScenario));
    runner = runner.add_scenario(Box::new(LeaderCrashDuringWriteScenario));
    runner = runner.add_scenario(Box::new(MultipleNodeCrashScenario));
    runner = runner.add_scenario(Box::new(FullClusterCrashScenario));

    // Network partition scenarios
    runner = runner.add_scenario(Box::new(MinorityPartitionScenario));
    runner = runner.add_scenario(Box::new(LeaderPartitionScenario));
    runner = runner.add_scenario(Box::new(SymmetricPartitionScenario));

    // Network degradation scenarios
    runner = runner.add_scenario(Box::new(HighLatencyScenario));
    runner = runner.add_scenario(Box::new(PacketLossScenario));
    runner = runner.add_scenario(Box::new(NetworkJitterScenario));

    // Rolling restart scenarios
    runner = runner.add_scenario(Box::new(RollingRestartScenario));
    runner = runner.add_scenario(Box::new(ContinuousWorkloadRollingRestartScenario));

    // Split-brain scenarios
    runner = runner.add_scenario(Box::new(SplitBrainScenario));
    runner = runner.add_scenario(Box::new(FlappingPartitionScenario));

    // Cascade failure scenarios
    runner = runner.add_scenario(Box::new(CascadeFailureScenario));
    runner = runner.add_scenario(Box::new(RapidCascadeScenario));
    runner = runner.add_scenario(Box::new(RecoveryUnderLoadScenario));

    println!("Registered {} scenarios\n", runner.scenario_count());

    // Run all scenarios
    println!("Starting chaos testing...\n");
    let report = runner.run_all(&mut cluster).await?;

    // Stop cluster
    println!("\nStopping cluster...");
    cluster.stop().await?;

    // Print final report
    println!("\n=================================================");
    println!("Final Report");
    println!("=================================================");
    report.print_summary();

    // Save report to JSON
    let report_json = serde_json::to_string_pretty(&report)?;
    std::fs::write("chaos_test_report.json", report_json)?;
    println!("\nDetailed report saved to: chaos_test_report.json");

    // Exit with appropriate code
    if report.all_passed() {
        println!("\n✓ All chaos tests passed!");
        Ok(())
    } else {
        println!("\n✗ Some chaos tests failed");
        std::process::exit(1);
    }
}
