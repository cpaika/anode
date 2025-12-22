//! Chaos test runner
//!
//! Orchestrates the execution of chaos scenarios and produces test reports.

use crate::scenarios::{ChaosScenario, ScenarioConfig, ScenarioResult};
use crate::{ChaosError, Result};
use anode_test_harness::TestCluster;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Result of applying a chaos event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosResult {
    /// Name of the scenario
    pub scenario_name: String,
    /// Whether the scenario passed
    pub passed: bool,
    /// Duration the scenario took
    pub duration: Duration,
    /// Number of operations performed
    pub operations: usize,
    /// Number of failures encountered
    pub failures: usize,
    /// Data integrity verified
    pub integrity_verified: bool,
    /// Error message if failed
    pub error: Option<String>,
}

impl From<ScenarioResult> for ChaosResult {
    fn from(result: ScenarioResult) -> Self {
        Self {
            scenario_name: result.name,
            passed: result.success,
            duration: result.duration,
            operations: result.operations,
            failures: result.failures,
            integrity_verified: result.integrity_verified,
            error: result.error,
        }
    }
}

/// Complete test report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    /// Number of scenarios that passed
    pub passed: usize,
    /// Number of scenarios that failed
    pub failed: usize,
    /// Total duration of all scenarios
    pub total_duration: Duration,
    /// Individual scenario results
    pub scenarios: Vec<ChaosResult>,
}

impl TestReport {
    /// Create a new empty test report
    pub fn new() -> Self {
        Self {
            passed: 0,
            failed: 0,
            total_duration: Duration::from_secs(0),
            scenarios: Vec::new(),
        }
    }

    /// Add a scenario result to the report
    pub fn add_result(&mut self, result: ChaosResult) {
        if result.passed {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        self.total_duration += result.duration;
        self.scenarios.push(result);
    }

    /// Print a summary of the report
    pub fn print_summary(&self) {
        info!("==========================================");
        info!("Chaos Testing Report");
        info!("==========================================");
        info!("Total scenarios: {}", self.scenarios.len());
        info!("Passed: {}", self.passed);
        info!("Failed: {}", self.failed);
        info!("Total duration: {:?}", self.total_duration);
        info!("==========================================");

        for scenario in &self.scenarios {
            let status = if scenario.passed { "PASS" } else { "FAIL" };
            info!(
                "[{}] {} - {:?} ({} ops, {} failures)",
                status,
                scenario.scenario_name,
                scenario.duration,
                scenario.operations,
                scenario.failures
            );
            if let Some(error) = &scenario.error {
                info!("      Error: {}", error);
            }
        }
    }

    /// Check if all scenarios passed
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }
}

impl Default for TestReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Chaos test runner
pub struct ChaosRunner {
    scenarios: Vec<Box<dyn ChaosScenario>>,
    config: ScenarioConfig,
}

impl ChaosRunner {
    /// Create a new chaos runner
    pub fn new(config: ScenarioConfig) -> Self {
        Self {
            scenarios: Vec::new(),
            config,
        }
    }

    /// Add a scenario to run
    pub fn add_scenario(mut self, scenario: Box<dyn ChaosScenario>) -> Self {
        self.scenarios.push(scenario);
        self
    }

    /// Add multiple scenarios
    pub fn add_scenarios(mut self, scenarios: Vec<Box<dyn ChaosScenario>>) -> Self {
        self.scenarios.extend(scenarios);
        self
    }

    /// Run all scenarios
    pub async fn run_all(&mut self, cluster: &mut TestCluster) -> Result<TestReport> {
        info!("Running {} chaos scenarios", self.scenarios.len());

        let start_time = Instant::now();
        let mut report = TestReport::new();

        for scenario in &self.scenarios {
            info!("========================================");
            info!("Running scenario: {}", scenario.name());
            info!("Description: {}", scenario.description());
            info!("========================================");

            let result = match scenario.run(cluster, &self.config).await {
                Ok(result) => result,
                Err(e) => {
                    warn!("Scenario {} failed with error: {}", scenario.name(), e);
                    ScenarioResult {
                        name: scenario.name().to_string(),
                        success: false,
                        duration: Duration::from_secs(0),
                        operations: 0,
                        failures: 1,
                        integrity_verified: false,
                        metrics: serde_json::json!({}),
                        error: Some(e.to_string()),
                    }
                },
            };

            let chaos_result = ChaosResult::from(result);
            report.add_result(chaos_result);

            // Give cluster time to stabilize between scenarios
            info!("Waiting for cluster to stabilize...");
            tokio::time::sleep(Duration::from_secs(5)).await;
        }

        info!("All scenarios completed in {:?}", start_time.elapsed());
        report.print_summary();

        Ok(report)
    }

    /// Run a specific scenario by name
    pub async fn run_scenario(
        &mut self,
        cluster: &mut TestCluster,
        name: &str,
    ) -> Result<ChaosResult> {
        let scenario = self
            .scenarios
            .iter()
            .find(|s| s.name() == name)
            .ok_or_else(|| ChaosError::ScenarioFailed(format!("Scenario '{}' not found", name)))?;

        info!("Running scenario: {}", scenario.name());
        info!("Description: {}", scenario.description());

        let result = scenario.run(cluster, &self.config).await?;

        Ok(ChaosResult::from(result))
    }

    /// Get the list of scenario names
    pub fn scenario_names(&self) -> Vec<String> {
        self.scenarios
            .iter()
            .map(|s| s.name().to_string())
            .collect()
    }

    /// Get the number of scenarios
    pub fn scenario_count(&self) -> usize {
        self.scenarios.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_report() {
        let mut report = TestReport::new();
        assert_eq!(report.passed, 0);
        assert_eq!(report.failed, 0);

        let result = ChaosResult {
            scenario_name: "test".to_string(),
            passed: true,
            duration: Duration::from_secs(10),
            operations: 100,
            failures: 0,
            integrity_verified: true,
            error: None,
        };

        report.add_result(result);
        assert_eq!(report.passed, 1);
        assert_eq!(report.failed, 0);
        assert!(report.all_passed());
    }

    #[test]
    fn test_chaos_runner_creation() {
        let config = ScenarioConfig::default();
        let runner = ChaosRunner::new(config);
        assert_eq!(runner.scenario_count(), 0);
    }
}
