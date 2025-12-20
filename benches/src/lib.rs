// Shared benchmark utilities

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::time::{Duration, Instant};
use sysinfo::{ProcessExt, System, SystemExt};

/// Comprehensive benchmark result including performance metrics
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    /// Operations per second
    pub ops_per_second: f64,
    /// Bytes per second throughput
    pub bytes_per_second: f64,
    /// 50th percentile latency in microseconds
    pub p50_latency_us: u64,
    /// 99th percentile latency in microseconds
    pub p99_latency_us: u64,
    /// 99.9th percentile latency in microseconds
    pub p999_latency_us: u64,
    /// CPU time in user space (milliseconds)
    pub cpu_time_user_ms: u64,
    /// CPU time in system space (milliseconds)
    pub cpu_time_system_ms: u64,
    /// Peak resident set size in bytes
    pub peak_rss_bytes: u64,
    /// Allocations per operation
    pub allocations_per_op: u64,
}

impl BenchmarkResult {
    pub fn new() -> Self {
        Self {
            ops_per_second: 0.0,
            bytes_per_second: 0.0,
            p50_latency_us: 0,
            p99_latency_us: 0,
            p999_latency_us: 0,
            cpu_time_user_ms: 0,
            cpu_time_system_ms: 0,
            peak_rss_bytes: 0,
            allocations_per_op: 0,
        }
    }
}

impl Default for BenchmarkResult {
    fn default() -> Self {
        Self::new()
    }
}

/// CPU and memory statistics
#[derive(Debug, Clone)]
pub struct CpuMemoryStats {
    /// CPU time in user space
    pub cpu_user: Duration,
    /// CPU time in system space
    pub cpu_system: Duration,
    /// Peak memory usage in bytes
    pub peak_memory: u64,
    /// Memory used at start in bytes
    pub start_memory: u64,
    /// Memory used at end in bytes
    pub end_memory: u64,
}

/// Measure CPU and memory usage while running a function
///
/// This function captures CPU and memory metrics before and after
/// running the provided closure, allowing for precise measurement
/// of resource consumption.
pub fn measure_cpu_memory<F, R>(f: F) -> (R, CpuMemoryStats)
where
    F: FnOnce() -> R,
{
    let mut system = System::new_all();
    system.refresh_all();

    let pid = sysinfo::get_current_pid().expect("Failed to get current PID");
    let process = system.process(pid).expect("Failed to get process info");

    let start_cpu_user = process.cpu_usage() as u64;
    let start_memory = process.memory();

    // Run the benchmark
    let result = f();

    // Measure again
    system.refresh_all();
    let process = system.process(pid).expect("Failed to get process info");

    let end_cpu_user = process.cpu_usage() as u64;
    let end_memory = process.memory();
    let peak_memory = end_memory.max(start_memory);

    let stats = CpuMemoryStats {
        cpu_user: Duration::from_millis(end_cpu_user - start_cpu_user),
        cpu_system: Duration::from_millis(0), // sysinfo doesn't provide system time separately
        peak_memory,
        start_memory,
        end_memory,
    };

    (result, stats)
}

/// Generate random data of specified size
///
/// Uses a seeded RNG for reproducibility in benchmarks.
pub fn generate_random_data(size: usize) -> Vec<u8> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut data = vec![0u8; size];
    rng.fill(&mut data[..]);
    data
}

/// Generate a mock Parquet file with specified number of rows
///
/// This creates a simple Parquet-like structure for testing.
/// In a real implementation, this would use the parquet crate.
pub fn generate_parquet_file(rows: usize) -> Vec<u8> {
    // Mock implementation - in reality would use parquet crate
    // For now, just generate structured data that looks like Parquet
    let mut data = Vec::new();

    // Magic number "PAR1"
    data.extend_from_slice(b"PAR1");

    // Generate some mock row data
    let row_data = generate_random_data(rows * 100); // ~100 bytes per row
    data.extend_from_slice(&row_data);

    // Footer magic
    data.extend_from_slice(b"PAR1");

    data
}

/// Create a latency distribution from a set of measurements
pub fn calculate_percentiles(mut latencies: Vec<Duration>) -> (u64, u64, u64) {
    latencies.sort();
    let len = latencies.len();

    if len == 0 {
        return (0, 0, 0);
    }

    let p50_idx = (len as f64 * 0.50) as usize;
    let p99_idx = (len as f64 * 0.99) as usize;
    let p999_idx = (len as f64 * 0.999) as usize;

    let p50 = latencies.get(p50_idx.min(len - 1)).unwrap_or(&Duration::ZERO);
    let p99 = latencies.get(p99_idx.min(len - 1)).unwrap_or(&Duration::ZERO);
    let p999 = latencies.get(p999_idx.min(len - 1)).unwrap_or(&Duration::ZERO);

    (
        p50.as_micros() as u64,
        p99.as_micros() as u64,
        p999.as_micros() as u64,
    )
}

/// Timer for measuring individual operations
pub struct OpTimer {
    start: Instant,
}

impl OpTimer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

impl Default for OpTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for workload generation
#[derive(Debug, Clone)]
pub struct WorkloadConfig {
    /// Number of operations to perform
    pub operations: usize,
    /// Size of objects in bytes
    pub object_size: usize,
    /// Number of concurrent clients
    pub concurrency: usize,
    /// Read/write ratio (0.0 = all writes, 1.0 = all reads)
    pub read_ratio: f64,
}

impl Default for WorkloadConfig {
    fn default() -> Self {
        Self {
            operations: 1000,
            object_size: 1024 * 1024, // 1MB
            concurrency: 1,
            read_ratio: 0.5,
        }
    }
}

/// Generate a workload pattern (sequence of operations)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Read,
    Write,
    Delete,
    List,
}

pub fn generate_workload(config: &WorkloadConfig) -> Vec<Operation> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut ops = Vec::with_capacity(config.operations);

    for _ in 0..config.operations {
        let r: f64 = rng.gen();
        if r < config.read_ratio {
            ops.push(Operation::Read);
        } else {
            ops.push(Operation::Write);
        }
    }

    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_data() {
        let data = generate_random_data(1024);
        assert_eq!(data.len(), 1024);

        // Check that it's not all zeros
        let sum: u64 = data.iter().map(|&x| x as u64).sum();
        assert!(sum > 0);
    }

    #[test]
    fn test_generate_parquet_file() {
        let data = generate_parquet_file(100);
        assert!(data.len() > 0);

        // Check magic numbers
        assert_eq!(&data[0..4], b"PAR1");
        assert_eq!(&data[data.len()-4..], b"PAR1");
    }

    #[test]
    fn test_calculate_percentiles() {
        let latencies = vec![
            Duration::from_micros(10),
            Duration::from_micros(20),
            Duration::from_micros(30),
            Duration::from_micros(40),
            Duration::from_micros(50),
        ];

        let (p50, p99, p999) = calculate_percentiles(latencies);
        assert!(p50 > 0);
        assert!(p99 > 0);
        assert!(p999 > 0);
    }

    #[test]
    fn test_workload_generation() {
        let config = WorkloadConfig {
            operations: 100,
            object_size: 1024,
            concurrency: 1,
            read_ratio: 0.7,
        };

        let workload = generate_workload(&config);
        assert_eq!(workload.len(), 100);

        let read_count = workload.iter().filter(|&&op| op == Operation::Read).count();
        // Should be approximately 70% reads
        assert!(read_count > 50 && read_count < 90);
    }
}
