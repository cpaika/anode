#!/bin/bash
#
# Benchmark runner script for Anode
#
# This script runs all benchmarks and generates comprehensive reports including:
# - HTML reports via Criterion
# - JSON output for comparison
# - CPU profiling data
# - Memory profiling data

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
BENCHMARK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$BENCHMARK_DIR")"
RESULTS_DIR="${BENCHMARK_DIR}/results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_SUBDIR="${RESULTS_DIR}/${TIMESTAMP}"

# Create results directory
mkdir -p "${RESULTS_SUBDIR}"

echo -e "${GREEN}=== Anode Benchmark Suite ===${NC}"
echo "Results will be saved to: ${RESULTS_SUBDIR}"
echo ""

# Function to run a benchmark
run_benchmark() {
    local bench_name=$1
    local description=$2

    echo -e "${YELLOW}Running ${bench_name} benchmark...${NC}"
    echo "Description: ${description}"

    local start_time=$(date +%s)

    # Run the benchmark
    if cargo bench --bench "${bench_name}" -- --save-baseline "${TIMESTAMP}" 2>&1 | tee "${RESULTS_SUBDIR}/${bench_name}.log"; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        echo -e "${GREEN}✓ ${bench_name} completed in ${duration}s${NC}"
        echo ""
    else
        echo -e "${RED}✗ ${bench_name} failed${NC}"
        echo ""
        return 1
    fi
}

# Function to check system info
collect_system_info() {
    echo -e "${YELLOW}Collecting system information...${NC}"

    cat > "${RESULTS_SUBDIR}/system_info.txt" <<EOF
Benchmark Run: ${TIMESTAMP}
Date: $(date)

System Information:
-------------------
Hostname: $(hostname)
OS: $(uname -s)
Kernel: $(uname -r)
Architecture: $(uname -m)

CPU Information:
----------------
EOF

    if command -v lscpu &> /dev/null; then
        lscpu >> "${RESULTS_SUBDIR}/system_info.txt"
    elif command -v sysctl &> /dev/null; then
        sysctl -a | grep -E "machdep.cpu|hw\.(cpu|physmem)" >> "${RESULTS_SUBDIR}/system_info.txt"
    fi

    cat >> "${RESULTS_SUBDIR}/system_info.txt" <<EOF

Memory Information:
-------------------
EOF

    if command -v free &> /dev/null; then
        free -h >> "${RESULTS_SUBDIR}/system_info.txt"
    elif command -v vm_stat &> /dev/null; then
        vm_stat >> "${RESULTS_SUBDIR}/system_info.txt"
    fi

    cat >> "${RESULTS_SUBDIR}/system_info.txt" <<EOF

Rust Version:
-------------
$(rustc --version)
$(cargo --version)

Git Information:
----------------
Commit: $(git rev-parse HEAD 2>/dev/null || echo "Not in git repo")
Branch: $(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "Not in git repo")
Dirty: $(git diff --quiet 2>/dev/null && echo "No" || echo "Yes")
EOF

    echo -e "${GREEN}✓ System information collected${NC}"
    echo ""
}

# Parse command line arguments
BENCHMARKS=()
RUN_ALL=true
COMPARE_WITH=""
PROFILE=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --benchmark)
            BENCHMARKS+=("$2")
            RUN_ALL=false
            shift 2
            ;;
        --compare-with)
            COMPARE_WITH="$2"
            shift 2
            ;;
        --profile)
            PROFILE=true
            shift
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --benchmark <name>   Run specific benchmark (can be specified multiple times)"
            echo "  --compare-with <id>  Compare with previous run (use timestamp or 'latest')"
            echo "  --profile           Enable CPU/memory profiling"
            echo "  --help              Show this help message"
            echo ""
            echo "Available benchmarks:"
            echo "  storage     - Storage engine microbenchmarks"
            echo "  throughput  - System throughput benchmarks"
            echo "  latency     - Latency distribution benchmarks"
            echo "  memory      - Memory efficiency benchmarks"
            echo "  parquet     - Parquet-specific benchmarks"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Default benchmarks if none specified
if [ ${#BENCHMARKS[@]} -eq 0 ]; then
    BENCHMARKS=("storage" "throughput" "latency" "memory" "parquet")
fi

# Collect system information
collect_system_info

# Run benchmarks
TOTAL_START=$(date +%s)
FAILED_BENCHMARKS=()

for bench in "${BENCHMARKS[@]}"; do
    case $bench in
        storage)
            run_benchmark "storage" "Storage engine microbenchmarks (metadata, blobs, chunks)" || FAILED_BENCHMARKS+=("storage")
            ;;
        throughput)
            run_benchmark "throughput" "System throughput benchmarks (ops/sec, MB/sec)" || FAILED_BENCHMARKS+=("throughput")
            ;;
        latency)
            run_benchmark "latency" "Latency distribution benchmarks (p50, p99, p999)" || FAILED_BENCHMARKS+=("latency")
            ;;
        memory)
            run_benchmark "memory" "Memory efficiency benchmarks (allocations, cache)" || FAILED_BENCHMARKS+=("memory")
            ;;
        parquet)
            run_benchmark "parquet" "Parquet-specific benchmarks (metadata, cache)" || FAILED_BENCHMARKS+=("parquet")
            ;;
        *)
            echo -e "${RED}Unknown benchmark: ${bench}${NC}"
            FAILED_BENCHMARKS+=("${bench}")
            ;;
    esac
done

TOTAL_END=$(date +%s)
TOTAL_DURATION=$((TOTAL_END - TOTAL_START))

# Summary
echo ""
echo -e "${GREEN}=== Benchmark Summary ===${NC}"
echo "Total time: ${TOTAL_DURATION}s"
echo "Results saved to: ${RESULTS_SUBDIR}"

if [ ${#FAILED_BENCHMARKS[@]} -eq 0 ]; then
    echo -e "${GREEN}All benchmarks completed successfully!${NC}"
else
    echo -e "${RED}Failed benchmarks: ${FAILED_BENCHMARKS[*]}${NC}"
fi

# Create a "latest" symlink
ln -sfn "${TIMESTAMP}" "${RESULTS_DIR}/latest"

# Generate summary report
cat > "${RESULTS_SUBDIR}/summary.txt" <<EOF
Anode Benchmark Summary
=======================

Run ID: ${TIMESTAMP}
Date: $(date)
Total Duration: ${TOTAL_DURATION}s

Benchmarks Run:
EOF

for bench in "${BENCHMARKS[@]}"; do
    if [[ ! " ${FAILED_BENCHMARKS[@]} " =~ " ${bench} " ]]; then
        echo "  ✓ ${bench}" >> "${RESULTS_SUBDIR}/summary.txt"
    else
        echo "  ✗ ${bench} (FAILED)" >> "${RESULTS_SUBDIR}/summary.txt"
    fi
done

echo "" >> "${RESULTS_SUBDIR}/summary.txt"
echo "View HTML reports:" >> "${RESULTS_SUBDIR}/summary.txt"
echo "  file://${PROJECT_ROOT}/target/criterion/report/index.html" >> "${RESULTS_SUBDIR}/summary.txt"

# Run comparison if requested
if [ -n "${COMPARE_WITH}" ]; then
    echo ""
    echo -e "${YELLOW}Running comparison with baseline ${COMPARE_WITH}...${NC}"

    if [ "${COMPARE_WITH}" = "latest" ]; then
        COMPARE_WITH=$(ls -t "${RESULTS_DIR}" | grep -E '^[0-9]{8}_[0-9]{6}$' | sed -n 2p)
    fi

    if [ -n "${COMPARE_WITH}" ]; then
        if [ -f "${BENCHMARK_DIR}/compare.py" ]; then
            python3 "${BENCHMARK_DIR}/compare.py" \
                --baseline "${RESULTS_DIR}/${COMPARE_WITH}" \
                --current "${RESULTS_SUBDIR}" \
                --output "${RESULTS_SUBDIR}/comparison.html"
        else
            echo -e "${YELLOW}Warning: compare.py not found, skipping comparison${NC}"
        fi
    else
        echo -e "${YELLOW}Warning: Could not find baseline to compare with${NC}"
    fi
fi

# Generate benchmark report
echo ""
echo -e "${YELLOW}Generating benchmark report...${NC}"
if [ -f "${BENCHMARK_DIR}/generate_report.py" ]; then
    if python3 "${BENCHMARK_DIR}/generate_report.py" \
        --results-dir "${RESULTS_SUBDIR}" \
        --output "${PROJECT_ROOT}/BENCHMARKS.md"; then
        echo -e "${GREEN}✓ Benchmark report generated${NC}"
        echo "  - Report: ${PROJECT_ROOT}/BENCHMARKS.md"
    else
        echo -e "${YELLOW}Warning: Failed to generate benchmark report${NC}"
    fi
else
    echo -e "${YELLOW}Warning: generate_report.py not found, skipping report generation${NC}"
fi

echo ""
echo -e "${GREEN}Done!${NC}"
echo "View results:"
echo "  - System info: ${RESULTS_SUBDIR}/system_info.txt"
echo "  - Summary: ${RESULTS_SUBDIR}/summary.txt"
echo "  - HTML report: file://${PROJECT_ROOT}/target/criterion/report/index.html"
echo "  - Markdown report: ${PROJECT_ROOT}/BENCHMARKS.md"

if [ ${#FAILED_BENCHMARKS[@]} -ne 0 ]; then
    exit 1
fi
