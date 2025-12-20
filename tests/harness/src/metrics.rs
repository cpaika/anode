//! Test metrics collection and reporting

use hdrhistogram::Histogram;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Collects metrics during test execution
#[derive(Clone)]
pub struct TestMetrics {
    inner: Arc<Mutex<TestMetricsInner>>,
}

struct TestMetricsInner {
    /// Total number of operations executed
    operations_total: u64,
    /// Number of successful operations
    operations_success: u64,
    /// Number of failed operations
    operations_failed: u64,
    /// Latency histogram in microseconds
    latency_histogram: Histogram<u64>,
    /// Throughput samples (ops/sec)
    throughput_samples: Vec<f64>,
    /// Start time
    start_time: std::time::Instant,
}

impl TestMetrics {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TestMetricsInner {
                operations_total: 0,
                operations_success: 0,
                operations_failed: 0,
                latency_histogram: Histogram::new(3).expect("Failed to create histogram"),
                throughput_samples: Vec::new(),
                start_time: std::time::Instant::now(),
            })),
        }
    }

    /// Record a successful operation
    pub async fn record_success(&self, latency: Duration) {
        let mut inner = self.inner.lock().await;
        inner.operations_total += 1;
        inner.operations_success += 1;

        // Record latency in microseconds
        let latency_us = latency.as_micros() as u64;
        let _ = inner.latency_histogram.record(latency_us);
    }

    /// Record a failed operation
    pub async fn record_failure(&self, latency: Duration) {
        let mut inner = self.inner.lock().await;
        inner.operations_total += 1;
        inner.operations_failed += 1;

        // Record latency in microseconds even for failures
        let latency_us = latency.as_micros() as u64;
        let _ = inner.latency_histogram.record(latency_us);
    }

    /// Record a throughput sample
    pub async fn record_throughput(&self, ops_per_sec: f64) {
        let mut inner = self.inner.lock().await;
        inner.throughput_samples.push(ops_per_sec);
    }

    /// Get a snapshot of the current metrics
    pub async fn snapshot(&self) -> MetricsSnapshot {
        let inner = self.inner.lock().await;

        let elapsed = inner.start_time.elapsed();
        let throughput = if elapsed.as_secs_f64() > 0.0 {
            inner.operations_total as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        let success_rate = if inner.operations_total > 0 {
            inner.operations_success as f64 / inner.operations_total as f64
        } else {
            0.0
        };

        MetricsSnapshot {
            operations_total: inner.operations_total,
            operations_success: inner.operations_success,
            operations_failed: inner.operations_failed,
            success_rate,
            throughput,
            latency_min: Duration::from_micros(inner.latency_histogram.min()),
            latency_max: Duration::from_micros(inner.latency_histogram.max()),
            latency_mean: Duration::from_micros(inner.latency_histogram.mean() as u64),
            latency_p50: Duration::from_micros(inner.latency_histogram.value_at_quantile(0.50)),
            latency_p95: Duration::from_micros(inner.latency_histogram.value_at_quantile(0.95)),
            latency_p99: Duration::from_micros(inner.latency_histogram.value_at_quantile(0.99)),
            latency_p999: Duration::from_micros(inner.latency_histogram.value_at_quantile(0.999)),
            elapsed,
        }
    }

    /// Reset all metrics
    pub async fn reset(&self) {
        let mut inner = self.inner.lock().await;
        inner.operations_total = 0;
        inner.operations_success = 0;
        inner.operations_failed = 0;
        inner.latency_histogram.clear();
        inner.throughput_samples.clear();
        inner.start_time = std::time::Instant::now();
    }

    /// Export metrics as JSON
    pub async fn export_json(&self) -> serde_json::Result<String> {
        let snapshot = self.snapshot().await;
        serde_json::to_string_pretty(&snapshot)
    }

    /// Print metrics to stdout
    pub async fn print(&self) {
        let snapshot = self.snapshot().await;
        println!("{}", snapshot);
    }
}

impl Default for TestMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of metrics at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub operations_total: u64,
    pub operations_success: u64,
    pub operations_failed: u64,
    pub success_rate: f64,
    pub throughput: f64,
    pub latency_min: Duration,
    pub latency_max: Duration,
    pub latency_mean: Duration,
    pub latency_p50: Duration,
    pub latency_p95: Duration,
    pub latency_p99: Duration,
    pub latency_p999: Duration,
    pub elapsed: Duration,
}

impl std::fmt::Display for MetricsSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Test Metrics")?;
        writeln!(f, "============")?;
        writeln!(f, "Operations:")?;
        writeln!(f, "  Total:       {}", self.operations_total)?;
        writeln!(f, "  Success:     {}", self.operations_success)?;
        writeln!(f, "  Failed:      {}", self.operations_failed)?;
        writeln!(f, "  Success Rate: {:.2}%", self.success_rate * 100.0)?;
        writeln!(f)?;
        writeln!(f, "Performance:")?;
        writeln!(f, "  Throughput:  {:.2} ops/sec", self.throughput)?;
        writeln!(f, "  Duration:    {:?}", self.elapsed)?;
        writeln!(f)?;
        writeln!(f, "Latency:")?;
        writeln!(f, "  Min:         {:?}", self.latency_min)?;
        writeln!(f, "  Mean:        {:?}", self.latency_mean)?;
        writeln!(f, "  P50:         {:?}", self.latency_p50)?;
        writeln!(f, "  P95:         {:?}", self.latency_p95)?;
        writeln!(f, "  P99:         {:?}", self.latency_p99)?;
        writeln!(f, "  P99.9:       {:?}", self.latency_p999)?;
        writeln!(f, "  Max:         {:?}", self.latency_max)?;
        Ok(())
    }
}

/// Builder for creating metric reports
pub struct MetricsReporter {
    snapshots: Vec<MetricsSnapshot>,
    name: String,
}

impl MetricsReporter {
    /// Create a new metrics reporter
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            snapshots: Vec::new(),
            name: name.into(),
        }
    }

    /// Add a snapshot to the report
    pub fn add_snapshot(&mut self, snapshot: MetricsSnapshot) {
        self.snapshots.push(snapshot);
    }

    /// Generate a markdown report
    pub fn generate_markdown(&self) -> String {
        let mut report = String::new();

        report.push_str(&format!("# Test Report: {}\n\n", self.name));

        for (i, snapshot) in self.snapshots.iter().enumerate() {
            report.push_str(&format!("## Snapshot {}\n\n", i + 1));
            report.push_str("| Metric | Value |\n");
            report.push_str("|--------|-------|\n");
            report.push_str(&format!(
                "| Total Operations | {} |\n",
                snapshot.operations_total
            ));
            report.push_str(&format!(
                "| Successful Operations | {} |\n",
                snapshot.operations_success
            ));
            report.push_str(&format!(
                "| Failed Operations | {} |\n",
                snapshot.operations_failed
            ));
            report.push_str(&format!(
                "| Success Rate | {:.2}% |\n",
                snapshot.success_rate * 100.0
            ));
            report.push_str(&format!("| Throughput | {:.2} ops/sec |\n", snapshot.throughput));
            report.push_str(&format!("| Latency (min) | {:?} |\n", snapshot.latency_min));
            report.push_str(&format!("| Latency (mean) | {:?} |\n", snapshot.latency_mean));
            report.push_str(&format!("| Latency (p50) | {:?} |\n", snapshot.latency_p50));
            report.push_str(&format!("| Latency (p95) | {:?} |\n", snapshot.latency_p95));
            report.push_str(&format!("| Latency (p99) | {:?} |\n", snapshot.latency_p99));
            report.push_str(&format!("| Latency (max) | {:?} |\n", snapshot.latency_max));
            report.push('\n');
        }

        report
    }

    /// Generate a CSV report
    pub fn generate_csv(&self) -> String {
        let mut csv = String::new();

        // Header
        csv.push_str("snapshot,total_ops,success_ops,failed_ops,success_rate,throughput,");
        csv.push_str("latency_min_us,latency_mean_us,latency_p50_us,latency_p95_us,");
        csv.push_str("latency_p99_us,latency_max_us\n");

        // Data rows
        for (i, snapshot) in self.snapshots.iter().enumerate() {
            csv.push_str(&format!(
                "{},{},{},{},{:.4},{:.2},{},{},{},{},{},{}\n",
                i + 1,
                snapshot.operations_total,
                snapshot.operations_success,
                snapshot.operations_failed,
                snapshot.success_rate,
                snapshot.throughput,
                snapshot.latency_min.as_micros(),
                snapshot.latency_mean.as_micros(),
                snapshot.latency_p50.as_micros(),
                snapshot.latency_p95.as_micros(),
                snapshot.latency_p99.as_micros(),
                snapshot.latency_max.as_micros(),
            ));
        }

        csv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_recording() {
        let metrics = TestMetrics::new();

        metrics.record_success(Duration::from_millis(10)).await;
        metrics.record_success(Duration::from_millis(20)).await;
        metrics.record_failure(Duration::from_millis(15)).await;

        let snapshot = metrics.snapshot().await;

        assert_eq!(snapshot.operations_total, 3);
        assert_eq!(snapshot.operations_success, 2);
        assert_eq!(snapshot.operations_failed, 1);
        assert!((snapshot.success_rate - 0.6667).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_metrics_reset() {
        let metrics = TestMetrics::new();

        metrics.record_success(Duration::from_millis(10)).await;
        metrics.reset().await;

        let snapshot = metrics.snapshot().await;
        assert_eq!(snapshot.operations_total, 0);
    }

    #[tokio::test]
    async fn test_metrics_snapshot() {
        let metrics = TestMetrics::new();

        for _ in 0..100 {
            metrics.record_success(Duration::from_millis(10)).await;
        }

        let snapshot = metrics.snapshot().await;
        assert_eq!(snapshot.operations_total, 100);
        assert!(snapshot.latency_p50 >= Duration::from_micros(0));
    }

    #[test]
    fn test_metrics_reporter() {
        let mut reporter = MetricsReporter::new("Test Report");

        let snapshot = MetricsSnapshot {
            operations_total: 100,
            operations_success: 95,
            operations_failed: 5,
            success_rate: 0.95,
            throughput: 100.0,
            latency_min: Duration::from_millis(1),
            latency_max: Duration::from_millis(100),
            latency_mean: Duration::from_millis(10),
            latency_p50: Duration::from_millis(8),
            latency_p95: Duration::from_millis(50),
            latency_p99: Duration::from_millis(80),
            latency_p999: Duration::from_millis(95),
            elapsed: Duration::from_secs(1),
        };

        reporter.add_snapshot(snapshot);

        let markdown = reporter.generate_markdown();
        assert!(markdown.contains("Test Report"));

        let csv = reporter.generate_csv();
        assert!(csv.contains("total_ops"));
    }
}
