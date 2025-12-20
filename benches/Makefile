# Makefile for Anode Benchmarks
#
# Provides convenient targets for running benchmarks

.PHONY: all bench clean help storage throughput latency memory parquet compare profile

# Default target
all: bench

# Run all benchmarks
bench:
	@echo "Running all benchmarks..."
	@./run_benchmarks.sh

# Run specific benchmarks
storage:
	@echo "Running storage benchmarks..."
	@cargo bench --bench storage

throughput:
	@echo "Running throughput benchmarks..."
	@cargo bench --bench throughput

latency:
	@echo "Running latency benchmarks..."
	@cargo bench --bench latency

memory:
	@echo "Running memory benchmarks..."
	@cargo bench --bench memory

parquet:
	@echo "Running parquet benchmarks..."
	@cargo bench --bench parquet

# Run benchmarks and compare with latest
compare:
	@echo "Running benchmarks with comparison..."
	@./run_benchmarks.sh --compare-with latest

# Run benchmarks with profiling
profile:
	@echo "Running benchmarks with profiling..."
	@./run_benchmarks.sh --profile

# Clean benchmark results and build artifacts
clean:
	@echo "Cleaning benchmark results..."
	@rm -rf results/*
	@rm -rf ../target/criterion
	@echo "Clean complete"

# Clean everything including caches
clean-all: clean
	@echo "Cleaning all build artifacts..."
	@cargo clean
	@echo "Clean complete"

# View latest benchmark results
view:
	@echo "Opening latest benchmark results..."
	@if [ -d "../target/criterion/report" ]; then \
		open ../target/criterion/report/index.html || xdg-open ../target/criterion/report/index.html || echo "Please open ../target/criterion/report/index.html in your browser"; \
	else \
		echo "No benchmark results found. Run 'make bench' first."; \
	fi

# List available benchmark results
list:
	@echo "Available benchmark results:"
	@if [ -d "results" ]; then \
		ls -lt results/ | grep "^d" | awk '{print $$9}'; \
	else \
		echo "No results directory found"; \
	fi

# Compare two specific benchmark runs
compare-runs:
	@echo "Usage: make compare-runs BASELINE=<timestamp> CURRENT=<timestamp>"
	@if [ -z "$(BASELINE)" ] || [ -z "$(CURRENT)" ]; then \
		echo "Error: BASELINE and CURRENT must be specified"; \
		exit 1; \
	fi
	@python3 compare.py --baseline results/$(BASELINE) --current results/$(CURRENT)

# Quick benchmarks (reduced sample size for faster iteration)
quick:
	@echo "Running quick benchmarks..."
	@cargo bench --bench storage -- --sample-size 10
	@cargo bench --bench throughput -- --sample-size 10
	@cargo bench --bench latency -- --sample-size 10

# Install required tools
install-tools:
	@echo "Installing benchmark tools..."
	@cargo install cargo-criterion || echo "cargo-criterion already installed"
	@cargo install flamegraph || echo "flamegraph already installed"
	@pip3 install --upgrade matplotlib pandas || echo "Python packages installation failed"
	@echo "Tools installation complete"

# Generate flamegraph for a specific benchmark
flamegraph-%:
	@echo "Generating flamegraph for $*..."
	@cargo flamegraph --bench $* -- --bench
	@echo "Flamegraph saved to flamegraph.svg"

# Run benchmarks suitable for CI
ci:
	@echo "Running CI benchmarks (reduced sample size)..."
	@cargo bench --bench storage -- --sample-size 20
	@cargo bench --bench throughput -- --sample-size 20
	@cargo bench --bench latency -- --sample-size 20
	@cargo bench --bench memory -- --sample-size 20
	@cargo bench --bench parquet -- --sample-size 20

# Check benchmark code for issues
check:
	@echo "Checking benchmark code..."
	@cargo check --benches
	@cargo clippy --benches -- -D warnings

# Format benchmark code
fmt:
	@echo "Formatting benchmark code..."
	@cargo fmt --all

# Generate documentation for benchmarks
docs:
	@echo "Generating benchmark documentation..."
	@cargo doc --no-deps --document-private-items --open

# Show help
help:
	@echo "Anode Benchmark Makefile"
	@echo ""
	@echo "Available targets:"
	@echo "  all              - Run all benchmarks (default)"
	@echo "  bench            - Run all benchmarks"
	@echo "  storage          - Run storage benchmarks"
	@echo "  throughput       - Run throughput benchmarks"
	@echo "  latency          - Run latency benchmarks"
	@echo "  memory           - Run memory benchmarks"
	@echo "  parquet          - Run parquet benchmarks"
	@echo "  compare          - Run benchmarks and compare with latest"
	@echo "  profile          - Run benchmarks with profiling enabled"
	@echo "  quick            - Run quick benchmarks (reduced samples)"
	@echo "  ci               - Run CI-friendly benchmarks"
	@echo "  clean            - Clean benchmark results"
	@echo "  clean-all        - Clean all build artifacts"
	@echo "  view             - View latest benchmark results in browser"
	@echo "  list             - List available benchmark results"
	@echo "  compare-runs     - Compare two specific runs"
	@echo "  install-tools    - Install required tools"
	@echo "  flamegraph-NAME  - Generate flamegraph for specific benchmark"
	@echo "  check            - Check benchmark code for issues"
	@echo "  fmt              - Format benchmark code"
	@echo "  docs             - Generate and open documentation"
	@echo "  help             - Show this help message"
	@echo ""
	@echo "Examples:"
	@echo "  make bench                                     # Run all benchmarks"
	@echo "  make storage                                   # Run only storage benchmarks"
	@echo "  make compare                                   # Compare with latest results"
	@echo "  make compare-runs BASELINE=20240115_143022 CURRENT=20240115_150000"
	@echo "  make flamegraph-storage                        # Generate flamegraph for storage"
	@echo "  make quick                                     # Quick benchmarks for development"
