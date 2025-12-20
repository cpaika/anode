# Benchmark Infrastructure Index

Quick reference to all benchmark files and their purposes.

## Core Benchmark Files

### Benchmark Suites (Rust)

| File | Purpose | Benchmarks | Metrics |
|------|---------|------------|---------|
| `storage.rs` | Storage engine microbenchmarks | Metadata ops, blob I/O, hashing, listing | Time, throughput |
| `throughput.rs` | System throughput | PUT/GET/mixed workloads, concurrency | Ops/sec, MB/sec |
| `latency.rs` | Latency distributions | All operations, cache impact | p50, p99, p999 |
| `memory.rs` | Memory efficiency | Object overhead, cache efficiency, allocations | Bytes, hit rate |
| `parquet.rs` | Parquet features | Metadata parsing, cache, statistics | Parse time, hit rate |

### Shared Code

| File | Purpose |
|------|---------|
| `src/lib.rs` | Shared utilities (data generation, measurement, helpers) |

## Configuration

| File | Purpose |
|------|---------|
| `Cargo.toml` | Rust package configuration, benchmark definitions |
| `bench.toml` | Benchmark configuration (sample sizes, thresholds, etc.) |

## Tooling

| File | Purpose | Language |
|------|---------|----------|
| `run_benchmarks.sh` | Main benchmark runner script | Bash |
| `compare.py` | Result comparison tool | Python |
| `Makefile` | Convenient make targets | Make |

## Documentation

| File | Purpose |
|------|---------|
| `README.md` | Comprehensive documentation |
| `QUICKSTART.md` | 5-minute quick start guide |
| `ARCHITECTURE.md` | Architecture and design documentation |
| `INDEX.md` | This file - quick reference |

## Other Files

| File | Purpose |
|------|---------|
| `.gitignore` | Ignore benchmark results and artifacts |

## Directory Structure

```
benches/
├── Cargo.toml              # Benchmark package config
├── bench.toml              # Benchmark settings
├── Makefile                # Convenient commands
├── .gitignore              # Ignore patterns
│
├── src/
│   └── lib.rs              # Shared utilities
│
├── storage.rs              # Storage benchmarks
├── throughput.rs           # Throughput benchmarks
├── latency.rs              # Latency benchmarks
├── memory.rs               # Memory benchmarks
├── parquet.rs              # Parquet benchmarks
│
├── run_benchmarks.sh       # Runner script
├── compare.py              # Comparison tool
│
├── README.md               # Main documentation
├── QUICKSTART.md           # Quick start guide
├── ARCHITECTURE.md         # Architecture docs
└── INDEX.md                # This file
```

## Quick Commands

```bash
# Run all benchmarks
make bench

# Run specific benchmark
make storage

# Quick benchmarks (dev)
make quick

# Compare with latest
make compare

# View results
make view

# Clean results
make clean

# Show help
make help
```

## Benchmark Counts

- **Total benchmark suites**: 5
- **Storage benchmarks**: 9
- **Throughput benchmarks**: 6
- **Latency benchmarks**: 8
- **Memory benchmarks**: 6
- **Parquet benchmarks**: 8
- **Total individual benchmarks**: ~37

## Key Metrics Tracked

1. **Performance**
   - Time per operation
   - Throughput (ops/sec, MB/sec)
   - Latency percentiles (p50, p99, p999)

2. **Efficiency**
   - Memory per object
   - Cache hit rate
   - Allocation rate

3. **Scalability**
   - Concurrent client performance
   - Object count scaling
   - Cache size scaling

## File Sizes (Approximate)

- Benchmark code: ~2,000 lines
- Documentation: ~1,500 lines
- Tooling: ~1,000 lines
- Total: ~4,500 lines

## Dependencies

### Rust
- `criterion` - Statistical benchmarking
- `tokio` - Async runtime
- `rand` - Random data generation
- `sysinfo` - System information
- `tempfile` - Temporary files

### Python
- Standard library only (no external deps)

### System
- Bash shell
- Make
- Python 3.7+

## Integration Points

- **CI/CD**: `.github/workflows/bench.yml`
- **Crates**: `anode-storage`, `anode-protocol`, `anode-client`
- **Results**: `target/criterion/` and `results/`

## Next Steps

1. Read [QUICKSTART.md](QUICKSTART.md) to run your first benchmark
2. Review [README.md](README.md) for comprehensive documentation
3. Study [ARCHITECTURE.md](ARCHITECTURE.md) for design details
4. Run `make help` to see all available commands
