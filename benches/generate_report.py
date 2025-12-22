#!/usr/bin/env python3
"""
Benchmark report generator for Anode

This script parses Criterion benchmark results and generates a comprehensive
BENCHMARKS.md report with tables, charts, and performance metrics.
"""

import json
import argparse
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass
from datetime import datetime
from collections import defaultdict


@dataclass
class BenchmarkMetric:
    """Represents a single benchmark metric"""
    name: str
    group: str
    mean_ns: float
    stddev_ns: float
    throughput_bytes: Optional[int] = None
    throughput_elements: Optional[int] = None


def parse_criterion_json(criterion_dir: Path) -> Dict[str, List[BenchmarkMetric]]:
    """
    Parse Criterion benchmark results from JSON files

    Returns a dictionary mapping benchmark categories to their metrics
    """
    benchmarks = defaultdict(list)

    if not criterion_dir.exists():
        print(f"Warning: Criterion directory not found: {criterion_dir}", file=sys.stderr)
        return benchmarks

    # Criterion saves results in target/criterion/<benchmark_name>/<group>/base/estimates.json
    for bench_dir in criterion_dir.iterdir():
        if not bench_dir.is_dir():
            continue

        benchmark_category = bench_dir.name

        # Look for estimates.json files
        for estimates_file in bench_dir.rglob("estimates.json"):
            try:
                with open(estimates_file, 'r') as f:
                    data = json.load(f)

                # Extract benchmark name from path
                # Path structure: <category>/<group>/base/estimates.json
                relative_path = estimates_file.relative_to(bench_dir)
                group_name = str(relative_path.parent.parent)

                # Extract mean time and standard deviation
                mean_ns = data.get("mean", {}).get("point_estimate", 0)
                stddev_ns = data.get("std_dev", {}).get("point_estimate", 0)

                # Try to find throughput info from benchmark.json
                benchmark_json = estimates_file.parent / "benchmark.json"
                throughput_bytes = None
                throughput_elements = None

                if benchmark_json.exists():
                    with open(benchmark_json, 'r') as bf:
                        bench_data = json.load(bf)
                        throughput = bench_data.get("throughput")
                        if throughput:
                            if "Bytes" in throughput:
                                throughput_bytes = throughput["Bytes"]
                            elif "Elements" in throughput:
                                throughput_elements = throughput["Elements"]

                metric = BenchmarkMetric(
                    name=f"{benchmark_category}/{group_name}",
                    group=benchmark_category,
                    mean_ns=mean_ns,
                    stddev_ns=stddev_ns,
                    throughput_bytes=throughput_bytes,
                    throughput_elements=throughput_elements
                )

                benchmarks[benchmark_category].append(metric)

            except Exception as e:
                print(f"Warning: Failed to parse {estimates_file}: {e}", file=sys.stderr)

    return benchmarks


def format_time(ns: float) -> str:
    """Format nanoseconds into human-readable time"""
    if ns < 1_000:
        return f"{ns:.2f} ns"
    elif ns < 1_000_000:
        return f"{ns / 1_000:.2f} µs"
    elif ns < 1_000_000_000:
        return f"{ns / 1_000_000:.2f} ms"
    else:
        return f"{ns / 1_000_000_000:.2f} s"


def format_throughput(mean_ns: float, throughput_bytes: Optional[int], throughput_elements: Optional[int]) -> str:
    """Calculate and format throughput from mean time and throughput configuration"""
    if mean_ns == 0:
        return "N/A"

    ops_per_sec = 1_000_000_000 / mean_ns  # Convert ns to ops/sec

    if throughput_bytes is not None:
        bytes_per_sec = ops_per_sec * throughput_bytes
        if bytes_per_sec >= 1_000_000_000:
            return f"{bytes_per_sec / 1_000_000_000:.2f} GB/s"
        elif bytes_per_sec >= 1_000_000:
            return f"{bytes_per_sec / 1_000_000:.2f} MB/s"
        elif bytes_per_sec >= 1_000:
            return f"{bytes_per_sec / 1_000:.2f} KB/s"
        else:
            return f"{bytes_per_sec:.2f} B/s"
    elif throughput_elements is not None:
        return f"{ops_per_sec:.0f} ops/sec"
    else:
        return f"{ops_per_sec:.0f} ops/sec"


def generate_markdown_report(
    benchmarks: Dict[str, List[BenchmarkMetric]],
    output_file: Path,
    system_info: Optional[Dict] = None
):
    """Generate a comprehensive BENCHMARKS.md report"""

    lines = []

    # Header
    lines.append("# Anode Benchmark Results")
    lines.append("")
    lines.append(f"Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    lines.append("")

    # System information
    if system_info:
        lines.append("## System Information")
        lines.append("")
        lines.append(f"- **OS**: {system_info.get('os', 'Unknown')}")
        lines.append(f"- **Architecture**: {system_info.get('arch', 'Unknown')}")
        lines.append(f"- **CPU**: {system_info.get('cpu', 'Unknown')}")
        lines.append(f"- **Memory**: {system_info.get('memory', 'Unknown')}")
        lines.append(f"- **Rust Version**: {system_info.get('rust_version', 'Unknown')}")
        lines.append(f"- **Commit**: {system_info.get('commit', 'Unknown')}")
        lines.append("")

    # Table of Contents
    lines.append("## Table of Contents")
    lines.append("")
    for category in sorted(benchmarks.keys()):
        category_title = category.replace('_', ' ').title()
        lines.append(f"- [{category_title}](#{category.lower().replace('_', '-')})")
    lines.append("")

    # Overview
    lines.append("## Overview")
    lines.append("")
    total_benchmarks = sum(len(metrics) for metrics in benchmarks.values())
    lines.append(f"Total benchmarks: **{total_benchmarks}**")
    lines.append("")
    lines.append("| Category | Benchmarks |")
    lines.append("|----------|------------|")
    for category, metrics in sorted(benchmarks.items()):
        category_title = category.replace('_', ' ').title()
        lines.append(f"| {category_title} | {len(metrics)} |")
    lines.append("")

    # Detailed results per category
    for category, metrics in sorted(benchmarks.items()):
        if not metrics:
            continue

        category_title = category.replace('_', ' ').title()
        lines.append(f"## {category_title}")
        lines.append("")

        # Add category description
        descriptions = {
            "storage": "Core storage engine performance including metadata operations, blob I/O, and chunk hashing.",
            "throughput": "System-level throughput measurements for various object sizes and workload patterns.",
            "latency": "Operation latency distributions showing p50, p99, and p999 percentiles.",
            "memory": "Memory efficiency metrics including allocations, cache performance, and memory overhead.",
            "parquet": "Parquet-specific performance for metadata parsing, caching, and query optimization.",
            "concurrent_put": "Concurrent PUT operation performance with multiple simultaneous uploads.",
            "concurrent_get": "Concurrent GET operation performance with multiple simultaneous downloads.",
            "mixed_workload": "Mixed read/write workload performance simulating realistic usage patterns.",
            "deduplication": "Content-addressable storage deduplication performance and efficiency.",
            "raft_log": "Raft consensus log performance for append, read, and compaction operations.",
            "raft_replication": "Raft replication performance for leader election and log replication.",
        }

        if category in descriptions:
            lines.append(descriptions[category])
            lines.append("")

        # Create table
        lines.append("| Benchmark | Mean Time | Std Dev | Throughput |")
        lines.append("|-----------|-----------|---------|------------|")

        for metric in sorted(metrics, key=lambda x: x.name):
            # Clean up benchmark name for display
            display_name = metric.name.replace(f"{category}/", "").replace('_', ' ')
            mean_time = format_time(metric.mean_ns)
            stddev_time = format_time(metric.stddev_ns)
            throughput = format_throughput(metric.mean_ns, metric.throughput_bytes, metric.throughput_elements)

            lines.append(f"| {display_name} | {mean_time} | ±{stddev_time} | {throughput} |")

        lines.append("")

    # Performance highlights
    lines.append("## Performance Highlights")
    lines.append("")

    # Find fastest/slowest in each category
    highlights = []

    for category, metrics in benchmarks.items():
        if not metrics:
            continue

        category_title = category.replace('_', ' ').title()

        # Find fastest benchmark in this category
        fastest = min(metrics, key=lambda x: x.mean_ns)
        fastest_name = fastest.name.replace(f"{category}/", "").replace('_', ' ')
        fastest_time = format_time(fastest.mean_ns)
        fastest_throughput = format_throughput(fastest.mean_ns, fastest.throughput_bytes, fastest.throughput_elements)

        highlights.append(f"- **{category_title}** - Fastest: *{fastest_name}* ({fastest_time}, {fastest_throughput})")

    for highlight in highlights:
        lines.append(highlight)

    lines.append("")

    # Recommendations
    lines.append("## Performance Targets")
    lines.append("")
    lines.append("The following are target performance metrics for Anode:")
    lines.append("")
    lines.append("### Storage Engine")
    lines.append("- Metadata write: < 100µs per operation")
    lines.append("- Metadata read: < 50µs per operation")
    lines.append("- Blob write (1MB): > 500 MB/s")
    lines.append("- Blob read (1MB): > 800 MB/s")
    lines.append("- Chunk hashing: > 1 GB/s")
    lines.append("")
    lines.append("### System Throughput")
    lines.append("- Small objects (4KB): > 10,000 ops/sec")
    lines.append("- Large objects (1MB): > 500 ops/sec")
    lines.append("- Mixed workload: > 5,000 ops/sec")
    lines.append("- Concurrent clients (16): > 20,000 ops/sec")
    lines.append("")
    lines.append("### Latency")
    lines.append("- PUT (1MB): p99 < 10ms")
    lines.append("- GET (1MB): p99 < 5ms")
    lines.append("- HEAD: p99 < 1ms")
    lines.append("- LIST (100 objects): p99 < 10ms")
    lines.append("")
    lines.append("### Memory")
    lines.append("- Memory per object: < 1KB")
    lines.append("- Memory per connection: < 10KB")
    lines.append("- Cache hit rate: > 80% (warm cache)")
    lines.append("")
    lines.append("### Raft Consensus")
    lines.append("- Log append: < 1ms")
    lines.append("- Log replication: < 10ms")
    lines.append("- Leader election: < 500ms")
    lines.append("")

    # How to run
    lines.append("## Running Benchmarks")
    lines.append("")
    lines.append("```bash")
    lines.append("# Run all benchmarks")
    lines.append("cd benches")
    lines.append("./run_benchmarks.sh")
    lines.append("")
    lines.append("# Run specific benchmark")
    lines.append("cargo bench --bench storage")
    lines.append("")
    lines.append("# Generate report")
    lines.append("./generate_report.py --output BENCHMARKS.md")
    lines.append("```")
    lines.append("")

    # Footer
    lines.append("---")
    lines.append("")
    lines.append("*Generated by Anode benchmark report generator*")
    lines.append("")
    lines.append("For more details on benchmarking methodology, see [benches/README.md](benches/README.md).")
    lines.append("")

    # Write to file
    with open(output_file, 'w') as f:
        f.write('\n'.join(lines))


def get_system_info(results_dir: Optional[Path] = None) -> Dict[str, str]:
    """Extract system information from results directory or current system"""
    info = {}

    if results_dir and results_dir.exists():
        system_info_file = results_dir / "system_info.txt"
        if system_info_file.exists():
            with open(system_info_file, 'r') as f:
                content = f.read()
                # Parse basic info (simple extraction)
                for line in content.split('\n'):
                    if 'OS:' in line:
                        info['os'] = line.split('OS:')[1].strip()
                    elif 'Architecture:' in line:
                        info['arch'] = line.split('Architecture:')[1].strip()
                    elif 'Commit:' in line:
                        info['commit'] = line.split('Commit:')[1].strip()

    return info


def main():
    parser = argparse.ArgumentParser(
        description="Generate BENCHMARKS.md report from Criterion results"
    )
    parser.add_argument(
        "--criterion-dir",
        type=Path,
        help="Path to Criterion results directory (default: ../target/criterion)"
    )
    parser.add_argument(
        "--results-dir",
        type=Path,
        help="Path to benchmark results directory for system info"
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("BENCHMARKS.md"),
        help="Output file path (default: BENCHMARKS.md)"
    )
    parser.add_argument(
        "--project-root",
        type=Path,
        help="Project root directory (auto-detected if not specified)"
    )

    args = parser.parse_args()

    # Determine project root
    if args.project_root:
        project_root = args.project_root
    else:
        # Assume script is in benches/ directory
        script_dir = Path(__file__).parent
        project_root = script_dir.parent

    # Determine Criterion directory
    if args.criterion_dir:
        criterion_dir = args.criterion_dir
    else:
        criterion_dir = project_root / "target" / "criterion"

    if not criterion_dir.exists():
        print(f"Error: Criterion results directory not found: {criterion_dir}", file=sys.stderr)
        print("Run benchmarks first with: cargo bench", file=sys.stderr)
        sys.exit(1)

    # Parse benchmark results
    print(f"Parsing Criterion results from {criterion_dir}...")
    benchmarks = parse_criterion_json(criterion_dir)

    if not benchmarks:
        print("Warning: No benchmark results found", file=sys.stderr)
        sys.exit(1)

    total_benchmarks = sum(len(metrics) for metrics in benchmarks.values())
    print(f"Found {total_benchmarks} benchmarks across {len(benchmarks)} categories")

    # Get system information
    system_info = get_system_info(args.results_dir)

    # Generate report
    print(f"Generating report to {args.output}...")
    generate_markdown_report(benchmarks, args.output, system_info)

    print(f"✓ Report generated successfully: {args.output}")
    print(f"  View at: file://{args.output.absolute()}")

    sys.exit(0)


if __name__ == "__main__":
    main()
