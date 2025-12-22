# Benchmark Report Generator

This directory contains tools for generating comprehensive benchmark reports from Criterion results.

## Files

- **`generate_report.py`** - Main report generator that creates BENCHMARKS.md
- **`compare.py`** - Comparison tool for analyzing performance changes between runs
- **`run_benchmarks.sh`** - Shell script that runs benchmarks and auto-generates reports

## Usage

### Automatic Generation

The easiest way to generate a report is to use the benchmark runner:

```bash
cd benches
./run_benchmarks.sh
```

This will:
1. Run all benchmarks
2. Collect system information
3. Generate Criterion HTML reports
4. Automatically create `BENCHMARKS.md` in the project root

### Manual Generation

To manually generate a report from existing Criterion results:

```bash
cd benches
python3 generate_report.py --output ../BENCHMARKS.md
```

### Custom Options

```bash
# Specify custom Criterion directory
python3 generate_report.py \
    --criterion-dir /path/to/criterion \
    --output report.md

# Include system info from a results directory
python3 generate_report.py \
    --results-dir results/20240115_143022 \
    --output ../BENCHMARKS.md

# Specify project root
python3 generate_report.py \
    --project-root /path/to/anode \
    --output /path/to/output.md
```

## Report Contents

The generated `BENCHMARKS.md` includes:

### 1. System Information
- Operating system and architecture
- CPU model and core count
- Memory size
- Rust version
- Git commit hash

### 2. Overview Tables
- Summary of all benchmark categories
- Count of benchmarks per category

### 3. Detailed Results by Category
Each benchmark category includes a table with:
- **Benchmark name**: Descriptive name of the test
- **Mean time**: Average execution time (formatted: ns, µs, ms, s)
- **Std dev**: Standard deviation (measure of consistency)
- **Throughput**: Operations per second or MB/s (when applicable)

Categories include:
- Storage engine operations
- System throughput
- Latency distributions
- Memory efficiency
- Parquet performance
- Concurrent operations
- Mixed workloads
- Deduplication
- Raft consensus

### 4. Performance Highlights
- Fastest benchmark in each category
- Key performance metrics

### 5. Performance Targets
- Target metrics for each category
- Performance goals and recommendations

### 6. Usage Instructions
- How to run benchmarks
- How to regenerate the report

## Understanding the Output

### Time Formatting
- **ns** (nanoseconds): < 1 microsecond
- **µs** (microseconds): < 1 millisecond
- **ms** (milliseconds): < 1 second
- **s** (seconds): ≥ 1 second

### Throughput
- **ops/sec**: Operations per second (for element-based benchmarks)
- **B/s, KB/s, MB/s, GB/s**: Bytes per second (for byte-based benchmarks)

### Standard Deviation
Lower standard deviation indicates more consistent performance.
- **Low variance** (< 5% of mean): Highly consistent
- **Medium variance** (5-15% of mean): Normal
- **High variance** (> 15% of mean): May indicate instability

## Data Sources

The report generator parses Criterion benchmark results from:

```
target/criterion/
├── <benchmark_name>/
│   └── <group_name>/
│       └── base/
│           ├── estimates.json    # Statistical estimates (mean, stddev, etc.)
│           └── benchmark.json    # Benchmark configuration (throughput settings)
```

## Customization

To modify the report format or content:

1. Edit `generate_report.py`
2. Look for the `generate_markdown_report()` function
3. Modify the markdown generation as needed
4. Test with `python3 generate_report.py`

### Adding New Sections

To add a new section to the report:

```python
# In generate_markdown_report()
lines.append("## My New Section")
lines.append("")
lines.append("Content here...")
lines.append("")
```

### Changing Table Format

Tables use standard Markdown format:

```markdown
| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |
```

## Troubleshooting

### "Criterion results directory not found"
Run benchmarks first: `cargo bench` or `./run_benchmarks.sh`

### "No benchmark results found"
Criterion directory exists but is empty. Run at least one benchmark.

### "Failed to parse estimates.json"
Criterion JSON format may have changed. Check the parsing logic in `parse_criterion_json()`.

### Report looks wrong
- Verify Criterion results are complete: `ls -R target/criterion/`
- Check for parsing errors in stderr output
- Manually inspect a `estimates.json` file to verify format

## Integration with CI/CD

To integrate report generation into CI/CD:

```yaml
# Example GitHub Actions workflow
- name: Run benchmarks
  run: cd benches && ./run_benchmarks.sh

- name: Upload benchmark report
  uses: actions/upload-artifact@v3
  with:
    name: benchmarks
    path: BENCHMARKS.md

- name: Comment on PR with results
  run: gh pr comment ${{ github.event.pull_request.number }} --body-file BENCHMARKS.md
```

## Performance Regression Detection

While this tool generates reports, use `compare.py` for regression detection:

```bash
# Compare two runs and check for regressions
python3 compare.py \
    --baseline results/before \
    --current results/after \
    --threshold 5.0  # 5% threshold
```

This will exit with an error code if regressions are detected.

## Contributing

To improve the report generator:

1. Test changes with existing benchmark results
2. Ensure backward compatibility with Criterion output format
3. Update this documentation
4. Add examples of new features

## License

Same as the Anode project (Apache 2.0 / MIT dual license).
