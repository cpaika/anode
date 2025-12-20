# Benchmark Infrastructure Architecture

This document describes the architecture and design of the Anode benchmark infrastructure.

## Overview

The benchmark infrastructure is designed to provide comprehensive, reliable, and reproducible performance measurements for the Anode object storage system.

## Design Principles

1. **Comprehensive Coverage**: Measure all critical paths (storage, network, memory, I/O)
2. **Realistic Workloads**: Use representative data sizes and access patterns
3. **Statistical Rigor**: Use Criterion.rs for proper statistical analysis
4. **Reproducibility**: Ensure benchmarks can be reproduced across environments
5. **Automation**: Support CI/CD integration for regression detection
6. **Observability**: Provide detailed reports and comparisons

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    Benchmark Infrastructure                  │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   Storage    │  │  Throughput  │  │   Latency    │      │
│  │  Benchmarks  │  │  Benchmarks  │  │  Benchmarks  │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                  │                  │               │
│  ┌──────┴───────┐  ┌──────┴───────┐  ┌──────┴───────┐      │
│  │    Memory    │  │   Parquet    │  │              │      │
│  │  Benchmarks  │  │  Benchmarks  │  │              │      │
│  └──────┬───────┘  └──────┬───────┘  └──────────────┘      │
│         │                  │                                 │
│         └──────────┬───────┘                                 │
│                    │                                         │
│         ┌──────────▼──────────┐                              │
│         │   Shared Utilities  │                              │
│         │  (src/lib.rs)       │                              │
│         └──────────┬──────────┘                              │
│                    │                                         │
│         ┌──────────▼──────────┐                              │
│         │   Criterion.rs      │                              │
│         │   (Statistical      │                              │
│         │    Analysis)        │                              │
│         └──────────┬──────────┘                              │
│                    │                                         │
│         ┌──────────▼──────────┐                              │
│         │   Results &         │                              │
│         │   Reports           │                              │
│         │  - HTML             │                              │
│         │  - JSON             │                              │
│         │  - Comparisons      │                              │
│         └─────────────────────┘                              │
│                                                               │
└─────────────────────────────────────────────────────────────┘
```

## Components

### 1. Benchmark Suites

#### Storage Benchmarks (`storage.rs`)
- **Purpose**: Microbenchmarks for storage engine primitives
- **Metrics**: Time per operation, throughput
- **Scope**: Metadata ops, blob I/O, hashing, listing

**Key Functions**:
- `bench_metadata_write()`: Measure metadata write performance
- `bench_blob_write_*()`: Measure blob write at various sizes
- `bench_chunk_hash()`: Measure hashing performance

#### Throughput Benchmarks (`throughput.rs`)
- **Purpose**: System-level throughput measurements
- **Metrics**: Operations/sec, MB/sec
- **Scope**: PUT, GET, mixed workloads, concurrency

**Key Functions**:
- `bench_put_throughput()`: Measure write throughput
- `bench_concurrent_clients()`: Test scaling with clients
- `bench_mixed_workload()`: Realistic read/write ratios

#### Latency Benchmarks (`latency.rs`)
- **Purpose**: Operation latency distributions
- **Metrics**: p50, p99, p999 latencies
- **Scope**: All operations, cold/warm cache

**Key Functions**:
- `bench_*_latency()`: Measure latency for each operation
- Uses `iter_custom()` for precise latency measurement
- Calculates percentiles from raw latency samples

#### Memory Benchmarks (`memory.rs`)
- **Purpose**: Memory efficiency and allocation patterns
- **Metrics**: Memory per object, cache efficiency, allocation rate
- **Scope**: Object metadata, connections, caches, fragmentation

**Key Functions**:
- `bench_memory_per_object()`: Measure metadata overhead
- `bench_cache_efficiency()`: LRU cache performance
- `bench_allocation_rate()`: Track allocation pressure

#### Parquet Benchmarks (`parquet.rs`)
- **Purpose**: Parquet-specific feature performance
- **Metrics**: Parse time, cache hit rate, extraction speed
- **Scope**: Metadata parsing, caching, query optimization

**Key Functions**:
- `bench_parquet_metadata_parse()`: Footer parsing
- `bench_parquet_cache_*()`: Cache hit/miss performance
- `bench_column_statistics()`: Stats extraction

### 2. Shared Utilities (`src/lib.rs`)

Provides common functionality for all benchmarks:

**Data Generation**:
- `generate_random_data()`: Create test data
- `generate_parquet_file()`: Create mock Parquet files
- `generate_workload()`: Create operation sequences

**Measurement**:
- `measure_cpu_memory()`: Capture CPU and memory stats
- `calculate_percentiles()`: Compute latency percentiles
- `OpTimer`: Precise timing for operations

**Structures**:
- `BenchmarkResult`: Comprehensive result container
- `CpuMemoryStats`: CPU and memory measurements
- `WorkloadConfig`: Workload generation configuration

### 3. Tooling

#### Benchmark Runner (`run_benchmarks.sh`)

Orchestrates benchmark execution:

```
┌─────────────────┐
│ Collect System  │
│ Information     │
└────────┬────────┘
         │
┌────────▼────────┐
│ Run Benchmarks  │
│ (Sequential)    │
└────────┬────────┘
         │
┌────────▼────────┐
│ Generate        │
│ Summary         │
└────────┬────────┘
         │
┌────────▼────────┐
│ Compare with    │
│ Baseline (opt)  │
└────────┬────────┘
         │
┌────────▼────────┐
│ Save Results    │
│ with Timestamp  │
└─────────────────┘
```

**Features**:
- System information collection
- Sequential benchmark execution
- Result aggregation
- Baseline comparison
- Timestamped result storage

#### Comparison Tool (`compare.py`)

Analyzes performance changes:

```
┌─────────────────┐
│ Parse Baseline  │
│ Results         │
└────────┬────────┘
         │
┌────────▼────────┐
│ Parse Current   │
│ Results         │
└────────┬────────┘
         │
┌────────▼────────┐
│ Calculate       │
│ Differences     │
└────────┬────────┘
         │
┌────────▼────────┐
│ Classify        │
│ Changes         │
│ (↑ ↓ →)        │
└────────┬────────┘
         │
┌────────▼────────┐
│ Generate        │
│ Reports         │
│ (HTML + Text)   │
└─────────────────┘
```

**Features**:
- JSON result parsing
- Statistical comparison
- Regression/improvement detection
- HTML and text report generation
- Configurable thresholds

#### Makefile

Provides convenient targets:

```makefile
make bench          # Run all benchmarks
make quick          # Fast benchmarks (dev)
make ci             # CI-friendly benchmarks
make compare        # Compare with latest
make flamegraph-*   # Generate flamegraph
make clean          # Clean results
```

### 4. Configuration (`bench.toml`)

Centralized configuration for all benchmarks:

```toml
[general]
sample_size = 100
measurement_time = 10
regression_threshold = 5.0

[storage]
object_sizes = [4096, 65536, 1048576]

[latency]
percentiles = [50, 90, 95, 99, 99.9]
```

**Benefits**:
- Single source of truth
- Easy to adjust for different environments
- Supports CI vs. local configurations

## Data Flow

### 1. Benchmark Execution

```
Input Data → Benchmark Function → Criterion → Statistical Analysis → Results
     ↓              ↓                  ↓              ↓                 ↓
Random Data    Black Box         Iterations    Mean/StdDev        JSON/HTML
Parquet Files  Real Ops          Samples       Percentiles        Reports
Workloads      Timers            Warmup        Regressions        Artifacts
```

### 2. Result Storage

```
Target/Criterion/
├── benchmark_name/
│   ├── group_name/
│   │   ├── report/
│   │   │   ├── index.html      # Interactive report
│   │   │   ├── violin.svg      # Latency distribution
│   │   │   └── ...
│   │   └── estimates.json      # Raw statistics
│   └── ...
└── report/
    └── index.html              # Aggregate report

Benches/Results/
├── YYYYMMDD_HHMMSS/
│   ├── system_info.txt         # System configuration
│   ├── summary.txt             # Run summary
│   ├── *.log                   # Benchmark logs
│   ├── comparison.html         # Comparison report
│   └── comparison.txt          # Text summary
└── latest → YYYYMMDD_HHMMSS    # Symlink to latest
```

## Integration Points

### 1. CI/CD Integration

```yaml
# .github/workflows/bench.yml
- Run benchmarks
- Compare with main branch baseline
- Upload artifacts
- Comment on PR
- Fail on regression (configurable)
```

### 2. Local Development

```bash
# Quick iteration during development
make quick

# Full benchmark suite
make bench

# Compare changes
make compare
```

### 3. Production Monitoring

Benchmarks can be run on production-like hardware to:
- Establish performance baselines
- Track performance over time
- Validate hardware upgrades
- Capacity planning

## Metrics and KPIs

### Storage Engine
- **Metadata write latency**: < 100µs (p99)
- **Metadata read latency**: < 50µs (p99)
- **Blob write throughput**: > 500 MB/sec
- **Blob read throughput**: > 800 MB/sec
- **Chunk hash rate**: > 1 GB/sec

### System Throughput
- **Small objects (4KB)**: > 10,000 ops/sec
- **Large objects (1MB)**: > 500 ops/sec
- **Mixed workload**: > 5,000 ops/sec
- **Concurrent (16 clients)**: > 20,000 ops/sec

### Latency
- **PUT (1MB) p99**: < 10ms
- **GET (1MB) p99**: < 5ms
- **HEAD p99**: < 1ms
- **LIST p99**: < 10ms

### Memory
- **Per-object overhead**: < 1KB
- **Per-connection overhead**: < 10KB
- **Cache hit rate**: > 80%

### Parquet
- **Metadata parse**: < 1ms
- **Cache hit latency**: < 10µs
- **Cache miss latency**: < 2ms

## Best Practices

### Writing Benchmarks

1. **Use realistic data**: Match production workloads
2. **Warm up caches**: Run iterations before measuring
3. **Prevent optimizations**: Use `black_box()`
4. **Annotate throughput**: Help Criterion calculate MB/sec
5. **Group related tests**: Use benchmark groups

Example:
```rust
fn bench_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("operation");
    group.throughput(Throughput::Bytes(size as u64));

    group.bench_function("name", |b| {
        b.iter(|| {
            black_box(perform_operation())
        });
    });

    group.finish();
}
```

### Running Benchmarks

1. **Quiet system**: Close other applications
2. **Consistent environment**: Same OS, same settings
3. **Multiple runs**: Average multiple runs
4. **Baseline tracking**: Compare against stable baselines
5. **Document changes**: Note configuration differences

### Interpreting Results

1. **Look at variance**: High variance indicates inconsistency
2. **Check percentiles**: p99/p999 show tail latency
3. **Compare trends**: Track over time, not just point-in-time
4. **Understand context**: Hardware, workload, configuration
5. **Investigate regressions**: Don't ignore small slowdowns

## Future Enhancements

1. **Distributed benchmarks**: Multi-node cluster testing
2. **Longevity tests**: Extended duration stress tests
3. **Real-world workloads**: Replay production traffic
4. **Automated baseline updates**: Track performance over time
5. **Performance budgets**: Enforce performance SLOs in CI
6. **Flamegraph integration**: Automatic profiling on regressions
7. **Historical tracking**: Database of benchmark results
8. **Comparative analysis**: Compare with other systems (MinIO, S3)

## References

- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Flamegraph Guide](https://www.brendangregg.com/flamegraphs.html)
- [Benchmarking Best Practices](https://github.com/bheisler/criterion.rs/blob/master/book/src/user_guide/benchmarking_tips.md)

## Maintenance

The benchmark infrastructure should be updated when:

1. **New features are added**: Add corresponding benchmarks
2. **API changes**: Update benchmark code
3. **Performance targets change**: Update thresholds
4. **Hardware changes**: Re-establish baselines
5. **Dependencies update**: Verify benchmarks still work

## Support

For questions or issues:
1. Check the [README.md](README.md) and [QUICKSTART.md](QUICKSTART.md)
2. Review Criterion.rs documentation
3. Open an issue with benchmark results and system info
