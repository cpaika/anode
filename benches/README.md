# Anode Benchmark Suite

Comprehensive benchmark infrastructure for the Anode object storage system.

## Overview

The benchmark suite measures:

- **Storage Engine Performance**: Metadata operations, blob I/O, chunk hashing
- **System Throughput**: Requests/sec, MB/sec, concurrent client handling
- **Latency Distribution**: p50, p99, p999 latencies for all operations
- **Memory Efficiency**: Allocations, cache hit rates, memory per object
- **Parquet Performance**: Metadata parsing, cache efficiency, query optimization
- **Concurrent Operations**: Multi-client PUT/GET performance and scalability
- **Mixed Workloads**: Realistic read/write patterns and ratios
- **Deduplication**: Content-addressable storage and space efficiency
- **Raft Consensus**: Log operations, replication, and cluster performance

## Benchmark Categories

### 1. Storage Benchmarks (`storage.rs`)

Microbenchmarks for the core storage engine:

- **Metadata operations**: Write/read performance for object metadata (redb-backed)
- **Blob operations**: Write/read performance at various sizes (4KB, 1MB, 100MB)
- **Chunk hashing**: SHA-256 hashing for content-addressable storage
- **List operations**: Prefix-based object listing performance

### 2. Throughput Benchmarks (`throughput.rs`)

System-level throughput measurements:

- **PUT throughput**: Objects per second for various sizes
- **GET throughput**: Read operations per second
- **Mixed workload**: Realistic read/write ratios (70/30, etc.)
- **Concurrent clients**: Performance under load (1, 4, 16, 64 clients)
- **Parallel uploads**: Multipart upload throughput
- **Sustained throughput**: Long-running performance stability

### 3. Latency Benchmarks (`latency.rs`)

Operation latency distributions:

- **PUT latency**: Write latency across object sizes
- **GET latency**: Read latency distributions
- **HEAD latency**: Metadata-only operation latency
- **LIST latency**: Listing operation latency
- **Time-to-first-byte**: Streaming read performance
- **Cold vs. warm reads**: Cache impact on latency

### 4. Memory Benchmarks (`memory.rs`)

Memory efficiency and allocation patterns:

- **Memory per object**: Overhead for storing object metadata
- **Memory per connection**: Connection pool overhead
- **Cache efficiency**: LRU cache hit rates and memory usage
- **Allocation rate**: Allocation pressure under different workloads
- **Memory fragmentation**: Impact of allocation patterns
- **Chunk metadata overhead**: Content-addressable storage overhead

### 5. Parquet Benchmarks (`parquet.rs`)

Parquet-specific performance:

- **Metadata parsing**: Parquet footer parsing performance
- **Cache hit/miss**: Cache performance for metadata
- **Footer reading**: Optimized footer-only reads
- **Row group extraction**: Row group metadata extraction
- **Column statistics**: Statistics extraction for query optimization
- **Schema parsing**: Schema parsing and caching
- **Cache eviction**: LRU eviction performance

### 6. Concurrent PUT Benchmarks (`concurrent_put.rs`)

Concurrent write operation performance:

- **Single client baseline**: Sequential PUT operations
- **Concurrent PUTs**: Multiple simultaneous uploads (2, 4, 8, 16, 32 clients)
- **Scalability**: Performance scaling with client count
- **Small/large objects**: Concurrency impact across object sizes

### 7. Concurrent GET Benchmarks (`concurrent_get.rs`)

Concurrent read operation performance:

- **Single client baseline**: Sequential GET operations
- **Concurrent GETs**: Multiple simultaneous downloads (2, 4, 8, 16, 32 clients)
- **Read scalability**: Performance scaling with concurrent readers
- **Cache effects**: Impact of concurrent reads on cache efficiency

### 8. Mixed Workload Benchmarks (`mixed_workload.rs`)

Realistic mixed read/write scenarios:

- **Read-heavy**: 90% reads, 10% writes
- **Balanced**: 50% reads, 50% writes
- **Write-heavy**: 10% reads, 90% writes
- **Realistic patterns**: Simulated production workloads

### 9. Deduplication Benchmarks (`deduplication.rs`)

Content-addressable storage efficiency:

- **Hash calculation**: SHA-256 hashing performance
- **Duplicate detection**: Identifying duplicate content
- **Space savings**: Deduplication ratio and storage efficiency
- **Chunk-level deduplication**: Content-defined chunking performance

### 10. Raft Log Benchmarks (`raft_log.rs`)

Raft consensus log operations:

- **Log append**: Write performance for consensus log
- **Log read**: Sequential and random log reads
- **Log compaction**: Snapshot and compaction performance
- **Persistence overhead**: Disk sync and durability costs

### 11. Raft Replication Benchmarks (`raft_replication.rs`)

Raft cluster replication:

- **Leader election**: Election time and convergence
- **Log replication**: Follower synchronization performance
- **Commit latency**: Time to commit across replicas
- **Network overhead**: Replication bandwidth and latency

## Running Benchmarks

### Quick Start

Run all benchmarks:

```bash
cd benches
./run_benchmarks.sh
```

### Run Specific Benchmarks

```bash
# Run only storage benchmarks
./run_benchmarks.sh --benchmark storage

# Run multiple specific benchmarks
./run_benchmarks.sh --benchmark storage --benchmark latency
```

### Using Cargo Directly

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench storage

# Run specific test within a benchmark
cargo bench --bench storage -- metadata_write
```

## Comparing Results

### Compare with Previous Run

```bash
# Compare with latest run
./run_benchmarks.sh --compare-with latest

# Compare with specific run
./run_benchmarks.sh --compare-with 20240115_143022
```

### Manual Comparison

```bash
python3 compare.py \
    --baseline results/20240115_143022 \
    --current results/20240115_150000 \
    --output comparison.html
```

## Generating Reports

### Benchmark Report (BENCHMARKS.md)

Generate a comprehensive markdown report with all benchmark results:

```bash
# Generate report from latest benchmark run
python3 generate_report.py --output ../BENCHMARKS.md

# Specify custom paths
python3 generate_report.py \
    --criterion-dir ../target/criterion \
    --results-dir results/latest \
    --output ../BENCHMARKS.md
```

The report includes:
- **Overview tables**: Summary of all benchmark categories
- **Detailed results**: Per-category tables with mean times, standard deviations, and throughput
- **Performance highlights**: Fastest benchmarks in each category
- **Target metrics**: Performance goals and recommendations

The report is automatically generated by `run_benchmarks.sh` after running benchmarks.

## Understanding Results

### Criterion Output

Benchmarks use [Criterion.rs](https://github.com/bheisler/criterion.rs) which provides:

- **HTML reports**: View at `../target/criterion/report/index.html`
- **Statistical analysis**: Mean, median, standard deviation
- **Regression detection**: Automatic detection of performance changes
- **Plots**: Time series and violin plots

### Result Files

After running benchmarks, results are saved in `benches/results/<timestamp>/`:

```
results/
└── 20240115_143022/
    ├── system_info.txt      # System configuration
    ├── summary.txt          # Run summary
    ├── storage.log          # Storage benchmark output
    ├── throughput.log       # Throughput benchmark output
    ├── latency.log          # Latency benchmark output
    ├── memory.log           # Memory benchmark output
    ├── parquet.log          # Parquet benchmark output
    ├── comparison.txt       # Text comparison (if --compare-with used)
    └── comparison.html      # HTML comparison (if --compare-with used)
```

### Interpreting Metrics

#### Throughput
- **Higher is better**
- Measured in operations/sec or MB/sec
- Look for sustained performance under load

#### Latency
- **Lower is better**
- Key percentiles:
  - **p50**: Median latency (typical case)
  - **p99**: 99th percentile (tail latency)
  - **p999**: 99.9th percentile (worst case)
- Watch for tail latency spikes

#### Memory
- **Lower is better** for memory usage
- **Higher is better** for cache hit rates
- Monitor allocation rates and fragmentation

## Performance Targets

### Storage Engine
- Metadata write: < 100µs per operation
- Metadata read: < 50µs per operation
- Blob write (1MB): > 500 MB/sec
- Blob read (1MB): > 800 MB/sec
- Chunk hashing: > 1 GB/sec

### System Throughput
- Small objects (4KB): > 10,000 ops/sec
- Large objects (1MB): > 500 ops/sec
- Mixed workload: > 5,000 ops/sec
- Concurrent clients (16): > 20,000 ops/sec

### Latency
- PUT (1MB): p99 < 10ms
- GET (1MB): p99 < 5ms
- HEAD: p99 < 1ms
- LIST (100 objects): p99 < 10ms

### Memory
- Memory per object: < 1KB
- Memory per connection: < 10KB
- Cache hit rate: > 80% (warm cache)

### Parquet
- Metadata parse: < 1ms per file
- Cache hit: < 10µs
- Cache miss: < 2ms
- Footer read: < 500µs

## Continuous Integration

Benchmarks are run automatically on:

- **Pull requests**: Compare against main branch baseline
- **Nightly builds**: Track performance over time
- **Release candidates**: Validate performance before release

See `.github/workflows/bench.yml` for CI configuration.

## Profiling

### CPU Profiling

Use `cargo flamegraph` for CPU profiling:

```bash
# Profile storage benchmarks
cargo flamegraph --bench storage
```

### Memory Profiling

Use `heaptrack` or `valgrind` for memory profiling:

```bash
# Linux
heaptrack cargo bench --bench memory

# macOS
instruments -t "Allocations" cargo bench --bench memory
```

## Adding New Benchmarks

1. Create a new benchmark file in `benches/`:

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn my_benchmark(c: &mut Criterion) {
    c.bench_function("my_operation", |b| {
        b.iter(|| {
            // Code to benchmark
        });
    });
}

criterion_group!(benches, my_benchmark);
criterion_main!(benches);
```

2. Add it to `Cargo.toml`:

```toml
[[bench]]
name = "my_benchmark"
harness = false
```

3. Update `run_benchmarks.sh` to include it.

## Best Practices

### Writing Benchmarks

1. **Use realistic data**: Generate data similar to production workloads
2. **Warm up caches**: Run a few iterations before measuring
3. **Avoid optimization**: Use `criterion::black_box()` to prevent compiler optimizations
4. **Measure what matters**: Focus on user-facing metrics
5. **Include context**: Add throughput annotations for better reporting

### Interpreting Results

1. **Run multiple times**: Ensure results are stable
2. **Check system load**: Avoid benchmarking under load
3. **Compare apples to apples**: Same hardware, same configuration
4. **Look for trends**: Track performance over time
5. **Investigate regressions**: Don't ignore small slowdowns

### CI/CD Integration

1. **Set thresholds**: Fail builds on >5% regression
2. **Track baselines**: Keep historical performance data
3. **Automate comparison**: Use `compare.py` in CI
4. **Report results**: Post summaries to PRs

## Troubleshooting

### Benchmarks are slow

- Reduce sample size in benchmark code
- Run specific benchmarks instead of all
- Use faster hardware or cloud instances

### Results are inconsistent

- Check for background processes
- Disable CPU frequency scaling
- Pin processes to specific CPU cores
- Use a dedicated benchmark machine

### Out of memory

- Reduce benchmark iteration counts
- Run benchmarks sequentially, not in parallel
- Increase system memory or swap

## Resources

- [Criterion.rs User Guide](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Flamegraph Guide](https://www.brendangregg.com/flamegraphs.html)

## License

Same as the Anode project (Apache 2.0 / MIT dual license).
