#!/usr/bin/env python3
"""
Benchmark comparison tool for Anode

This script compares benchmark results between two runs and generates
a comprehensive comparison report highlighting regressions and improvements.
"""

import json
import argparse
import sys
import os
from pathlib import Path
from typing import Dict, List, Tuple, Optional
from dataclasses import dataclass
from enum import Enum


class ChangeType(Enum):
    IMPROVEMENT = "improvement"
    REGRESSION = "regression"
    NEUTRAL = "neutral"


@dataclass
class BenchmarkMetric:
    """Represents a single benchmark metric"""
    name: str
    value: float
    unit: str


@dataclass
class BenchmarkComparison:
    """Represents a comparison between baseline and current metrics"""
    name: str
    baseline: float
    current: float
    change_pct: float
    change_type: ChangeType
    unit: str


def parse_criterion_json(criterion_dir: Path) -> Dict[str, Dict[str, float]]:
    """
    Parse Criterion benchmark results from JSON files

    Returns a dictionary mapping benchmark names to their metrics
    """
    results = {}

    # Criterion saves results in target/criterion/<benchmark_name>/<group>/base/estimates.json
    if not criterion_dir.exists():
        return results

    for bench_dir in criterion_dir.iterdir():
        if not bench_dir.is_dir():
            continue

        # Look for estimates.json files
        for estimates_file in bench_dir.rglob("estimates.json"):
            try:
                with open(estimates_file, 'r') as f:
                    data = json.load(f)

                    # Extract benchmark name from path
                    relative_path = estimates_file.relative_to(criterion_dir)
                    bench_name = str(relative_path.parent.parent)

                    # Extract mean time
                    if "mean" in data:
                        mean_time = data["mean"]["point_estimate"]
                        results[bench_name] = {
                            "mean_ns": mean_time,
                            "stddev_ns": data.get("std_dev", {}).get("point_estimate", 0)
                        }
            except Exception as e:
                print(f"Warning: Failed to parse {estimates_file}: {e}", file=sys.stderr)

    return results


def calculate_change(baseline: float, current: float, threshold: float = 5.0) -> ChangeType:
    """
    Determine if a change is an improvement, regression, or neutral

    Args:
        baseline: Baseline value
        current: Current value
        threshold: Percentage threshold for significance (default 5%)
    """
    if baseline == 0:
        return ChangeType.NEUTRAL

    change_pct = ((current - baseline) / baseline) * 100

    if abs(change_pct) < threshold:
        return ChangeType.NEUTRAL
    elif change_pct < 0:
        # For time-based metrics, lower is better
        return ChangeType.IMPROVEMENT
    else:
        return ChangeType.REGRESSION


def compare_benchmarks(
    baseline: Dict[str, Dict[str, float]],
    current: Dict[str, Dict[str, float]],
    threshold: float = 5.0
) -> List[BenchmarkComparison]:
    """
    Compare baseline and current benchmark results

    Returns a list of comparisons
    """
    comparisons = []

    # Find common benchmarks
    common_benchmarks = set(baseline.keys()) & set(current.keys())

    for bench_name in sorted(common_benchmarks):
        baseline_mean = baseline[bench_name]["mean_ns"]
        current_mean = current[bench_name]["mean_ns"]

        if baseline_mean == 0:
            continue

        change_pct = ((current_mean - baseline_mean) / baseline_mean) * 100
        change_type = calculate_change(baseline_mean, current_mean, threshold)

        comparisons.append(BenchmarkComparison(
            name=bench_name,
            baseline=baseline_mean,
            current=current_mean,
            change_pct=change_pct,
            change_type=change_type,
            unit="ns"
        ))

    return comparisons


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


def generate_html_report(
    comparisons: List[BenchmarkComparison],
    baseline_info: Dict,
    current_info: Dict,
    output_file: Path
):
    """Generate an HTML comparison report"""

    # Sort by absolute change percentage
    comparisons.sort(key=lambda x: abs(x.change_pct), reverse=True)

    # Count changes
    improvements = [c for c in comparisons if c.change_type == ChangeType.IMPROVEMENT]
    regressions = [c for c in comparisons if c.change_type == ChangeType.REGRESSION]
    neutral = [c for c in comparisons if c.change_type == ChangeType.NEUTRAL]

    html = f"""
<!DOCTYPE html>
<html>
<head>
    <title>Anode Benchmark Comparison</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
            background-color: #f5f5f5;
        }}
        h1 {{
            color: #333;
            border-bottom: 3px solid #4CAF50;
            padding-bottom: 10px;
        }}
        .summary {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            margin: 20px 0;
        }}
        .summary-card {{
            background: white;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }}
        .summary-card h3 {{
            margin: 0 0 10px 0;
            color: #666;
            font-size: 14px;
            text-transform: uppercase;
        }}
        .summary-card .value {{
            font-size: 32px;
            font-weight: bold;
            margin: 0;
        }}
        .improvement {{ color: #4CAF50; }}
        .regression {{ color: #f44336; }}
        .neutral {{ color: #999; }}

        table {{
            width: 100%;
            background: white;
            border-radius: 8px;
            overflow: hidden;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            border-collapse: collapse;
        }}
        th {{
            background: #4CAF50;
            color: white;
            padding: 15px;
            text-align: left;
            font-weight: 600;
        }}
        td {{
            padding: 12px 15px;
            border-bottom: 1px solid #eee;
        }}
        tr:hover {{
            background: #f9f9f9;
        }}
        .change-badge {{
            padding: 4px 8px;
            border-radius: 4px;
            font-weight: 600;
            font-size: 12px;
        }}
        .change-badge.improvement {{
            background: #e8f5e9;
            color: #2e7d32;
        }}
        .change-badge.regression {{
            background: #ffebee;
            color: #c62828;
        }}
        .change-badge.neutral {{
            background: #f5f5f5;
            color: #666;
        }}
        .meta-info {{
            background: white;
            padding: 15px;
            border-radius: 8px;
            margin: 20px 0;
            font-size: 14px;
            color: #666;
        }}
    </style>
</head>
<body>
    <h1>Anode Benchmark Comparison</h1>

    <div class="meta-info">
        <strong>Baseline:</strong> {baseline_info.get('timestamp', 'Unknown')}<br>
        <strong>Current:</strong> {current_info.get('timestamp', 'Unknown')}
    </div>

    <div class="summary">
        <div class="summary-card">
            <h3>Total Benchmarks</h3>
            <p class="value">{len(comparisons)}</p>
        </div>
        <div class="summary-card">
            <h3>Improvements</h3>
            <p class="value improvement">{len(improvements)}</p>
        </div>
        <div class="summary-card">
            <h3>Regressions</h3>
            <p class="value regression">{len(regressions)}</p>
        </div>
        <div class="summary-card">
            <h3>Neutral</h3>
            <p class="value neutral">{len(neutral)}</p>
        </div>
    </div>

    <h2>Detailed Results</h2>
    <table>
        <thead>
            <tr>
                <th>Benchmark</th>
                <th>Baseline</th>
                <th>Current</th>
                <th>Change</th>
                <th>Status</th>
            </tr>
        </thead>
        <tbody>
"""

    for comp in comparisons:
        status_class = comp.change_type.value
        status_text = comp.change_type.value.upper()
        change_sign = "+" if comp.change_pct > 0 else ""

        html += f"""
            <tr>
                <td>{comp.name}</td>
                <td>{format_time(comp.baseline)}</td>
                <td>{format_time(comp.current)}</td>
                <td class="{status_class}">{change_sign}{comp.change_pct:.2f}%</td>
                <td><span class="change-badge {status_class}">{status_text}</span></td>
            </tr>
"""

    html += """
        </tbody>
    </table>

    <div class="meta-info" style="margin-top: 40px;">
        Generated by Anode benchmark comparison tool
    </div>
</body>
</html>
"""

    with open(output_file, 'w') as f:
        f.write(html)


def generate_text_report(
    comparisons: List[BenchmarkComparison],
    baseline_info: Dict,
    current_info: Dict
) -> str:
    """Generate a text-based comparison report"""

    # Sort by absolute change percentage
    comparisons.sort(key=lambda x: abs(x.change_pct), reverse=True)

    # Count changes
    improvements = [c for c in comparisons if c.change_type == ChangeType.IMPROVEMENT]
    regressions = [c for c in comparisons if c.change_type == ChangeType.REGRESSION]
    neutral = [c for c in comparisons if c.change_type == ChangeType.NEUTRAL]

    report = []
    report.append("=" * 80)
    report.append("ANODE BENCHMARK COMPARISON")
    report.append("=" * 80)
    report.append("")
    report.append(f"Baseline: {baseline_info.get('timestamp', 'Unknown')}")
    report.append(f"Current:  {current_info.get('timestamp', 'Unknown')}")
    report.append("")
    report.append("SUMMARY")
    report.append("-" * 80)
    report.append(f"Total benchmarks:  {len(comparisons)}")
    report.append(f"Improvements:      {len(improvements)} ✓")
    report.append(f"Regressions:       {len(regressions)} ✗")
    report.append(f"Neutral:           {len(neutral)}")
    report.append("")

    if regressions:
        report.append("REGRESSIONS (most significant first)")
        report.append("-" * 80)
        for comp in regressions[:10]:  # Top 10 regressions
            report.append(f"  {comp.name}")
            report.append(f"    Baseline: {format_time(comp.baseline)}")
            report.append(f"    Current:  {format_time(comp.current)}")
            report.append(f"    Change:   +{comp.change_pct:.2f}% (SLOWER)")
            report.append("")

    if improvements:
        report.append("IMPROVEMENTS (most significant first)")
        report.append("-" * 80)
        for comp in improvements[:10]:  # Top 10 improvements
            report.append(f"  {comp.name}")
            report.append(f"    Baseline: {format_time(comp.baseline)}")
            report.append(f"    Current:  {format_time(comp.current)}")
            report.append(f"    Change:   {comp.change_pct:.2f}% (FASTER)")
            report.append("")

    report.append("=" * 80)

    return "\n".join(report)


def main():
    parser = argparse.ArgumentParser(
        description="Compare Anode benchmark results between two runs"
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        required=True,
        help="Path to baseline results directory"
    )
    parser.add_argument(
        "--current",
        type=Path,
        required=True,
        help="Path to current results directory"
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Output HTML report file"
    )
    parser.add_argument(
        "--threshold",
        type=float,
        default=5.0,
        help="Percentage threshold for significance (default: 5.0)"
    )
    parser.add_argument(
        "--format",
        choices=["html", "text", "both"],
        default="both",
        help="Output format (default: both)"
    )

    args = parser.parse_args()

    # Find Criterion results in target/criterion
    # Assume baseline and current point to timestamp directories in benches/results
    project_root = args.baseline.parent.parent
    criterion_dir = project_root / "target" / "criterion"

    if not criterion_dir.exists():
        print(f"Error: Criterion results directory not found: {criterion_dir}", file=sys.stderr)
        sys.exit(1)

    # Parse results
    print("Parsing baseline results...")
    baseline_results = parse_criterion_json(criterion_dir)

    print("Parsing current results...")
    current_results = parse_criterion_json(criterion_dir)

    if not baseline_results or not current_results:
        print("Warning: No benchmark results found to compare", file=sys.stderr)
        sys.exit(1)

    # Compare
    print(f"Comparing {len(baseline_results)} benchmarks...")
    comparisons = compare_benchmarks(baseline_results, current_results, args.threshold)

    if not comparisons:
        print("No common benchmarks found to compare", file=sys.stderr)
        sys.exit(1)

    # Prepare metadata
    baseline_info = {"timestamp": args.baseline.name}
    current_info = {"timestamp": args.current.name}

    # Generate reports
    if args.format in ["text", "both"]:
        text_report = generate_text_report(comparisons, baseline_info, current_info)
        print("\n" + text_report)

        # Save to file
        text_output = args.current / "comparison.txt"
        with open(text_output, 'w') as f:
            f.write(text_report)
        print(f"\nText report saved to: {text_output}")

    if args.format in ["html", "both"]:
        html_output = args.output or (args.current / "comparison.html")
        generate_html_report(comparisons, baseline_info, current_info, html_output)
        print(f"HTML report saved to: {html_output}")
        print(f"View at: file://{html_output.absolute()}")

    # Exit with error if there are regressions
    regressions = [c for c in comparisons if c.change_type == ChangeType.REGRESSION]
    if regressions:
        print(f"\nWarning: {len(regressions)} performance regressions detected!")
        sys.exit(1)

    sys.exit(0)


if __name__ == "__main__":
    main()
