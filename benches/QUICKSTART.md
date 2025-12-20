# Anode Benchmarks - Quick Start Guide

Get started with benchmarking Anode in 5 minutes.

## Prerequisites

- Rust 1.70+ installed
- Python 3.7+ (for comparison tool)
- ~10GB free disk space (for results)
- Recommended: Dedicated benchmark machine or idle system

## Quick Start

### 1. Run All Benchmarks

```bash
cd benches
make bench
```

This will:
- Run all 5 benchmark suites
- Generate HTML reports
- Save results with timestamp
- Display summary statistics

Expected time: ~15-30 minutes

### 2. View Results

```bash
make view
```

Or open manually:
```
file://../target/criterion/report/index.html
```

### 3. Run Specific Benchmark

```bash
# Run only storage benchmarks
make storage

# Or use cargo directly
cargo bench --bench storage
```

## Common Use Cases

### Development: Quick Iteration

Run fast benchmarks with reduced samples:

```bash
make quick
```

Expected time: ~3-5 minutes

### CI/CD: Regression Detection

Run CI-friendly benchmarks:

```bash
make ci
```

Compare with baseline:

```bash
make compare
```

### Performance Analysis: Deep Dive

Run with profiling:

```bash
make flamegraph-storage
```

This generates a flamegraph showing CPU hotspots.

## Understanding Results

### Key Metrics

1. **Time**: Lower is better
   - Mean: Average execution time
   - Median: Typical execution time
   - StdDev: Variance (lower = more consistent)

2. **Throughput**: Higher is better
   - ops/sec: Operations per second
   - MB/sec: Megabytes per second

3. **Latency Percentiles**:
   - p50 (median): Typical latency
   - p99: 99% of requests complete within this time
   - p999: Worst-case latency (1 in 1000 requests)

### What to Look For

**Good Signs**:
- ✓ Low variance (consistent performance)
- ✓ Flat p999 (no tail latency spikes)
- ✓ Linear scaling with concurrency
- ✓ Stable performance over time

**Warning Signs**:
- ✗ High variance (inconsistent performance)
- ✗ Spiking p999 (tail latency issues)
- ✗ Sublinear scaling (contention)
- ✗ Performance degradation over time

## Benchmark Suites

### 1. Storage (`storage.rs`)

**What it tests**: Core storage engine performance

**Key benchmarks**:
- `metadata_write`: How fast we write object metadata
- `blob_write_1mb`: Throughput for 1MB writes
- `chunk_hash`: Content hashing speed

**When to run**: After storage engine changes

### 2. Throughput (`throughput.rs`)

**What it tests**: System-level throughput

**Key benchmarks**:
- `put_throughput`: Write operations per second
- `get_throughput`: Read operations per second
- `concurrent_clients`: Multi-client performance

**When to run**: After API or concurrency changes

### 3. Latency (`latency.rs`)

**What it tests**: Operation latency distributions

**Key benchmarks**:
- `put_latency`: Write latency distribution
- `get_latency`: Read latency distribution
- `first_byte_latency`: Time to first byte

**When to run**: When optimizing for responsiveness

### 4. Memory (`memory.rs`)

**What it tests**: Memory efficiency

**Key benchmarks**:
- `memory_per_object`: Metadata overhead
- `cache_efficiency`: Cache hit rates
- `allocation_rate`: Allocation pressure

**When to run**: When investigating memory usage

### 5. Parquet (`parquet.rs`)

**What it tests**: Parquet-specific features

**Key benchmarks**:
- `metadata_parse`: Footer parsing speed
- `cache_hit`: Cache performance
- `column_statistics`: Stats extraction

**When to run**: After Parquet cache changes

## Comparison

### Compare with Previous Run

```bash
# Compare with latest
make compare

# Compare specific runs
make compare-runs BASELINE=20240115_143022 CURRENT=20240115_150000
```

### Interpreting Comparison

The comparison tool shows:
- **Improvements**: ✓ Faster than baseline (green)
- **Regressions**: ✗ Slower than baseline (red)
- **Neutral**: ~ Within threshold (gray)

Default threshold: ±5%

## Troubleshooting

### Benchmarks are slow

**Solution**: Run quick benchmarks or specific suite
```bash
make quick
# or
make storage
```

### Results are inconsistent

**Causes**:
- Background processes
- CPU thermal throttling
- Insufficient memory

**Solutions**:
- Close other applications
- Run on dedicated machine
- Check system load with `top` or `htop`

### Out of disk space

**Solution**: Clean old results
```bash
make clean
```

### Comparison fails

**Cause**: No baseline found

**Solution**: Run benchmarks first to create baseline
```bash
make bench
```

## Tips for Best Results

1. **Dedicated Hardware**: Use a quiet system
2. **Consistent Environment**: Same OS, same settings
3. **Multiple Runs**: Run 3x and average
4. **Warm Up**: System needs to warm up (first run is slower)
5. **Monitor Resources**: Watch CPU, memory, disk I/O

## Next Steps

- Read the full [README.md](README.md) for detailed documentation
- Check [bench.toml](bench.toml) for configuration options
- Review individual benchmark files for implementation details

## Getting Help

- Check the [troubleshooting section](#troubleshooting)
- Review Criterion.rs documentation: https://bheisler.github.io/criterion.rs/book/
- Open an issue: https://github.com/your-org/anode/issues

## Quick Reference

```bash
# Common commands
make bench          # Run all benchmarks
make quick          # Fast benchmarks
make storage        # Run storage benchmarks
make compare        # Compare with latest
make view           # View results
make clean          # Clean results
make help           # Show all commands
```
