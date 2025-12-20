# GitHub Actions Workflows

This directory contains comprehensive CI/CD workflows for the Anode project.

## Workflows Overview

### Core CI/CD

#### `ci.yml` - Main CI Pipeline
**Triggers:** Push to main, Pull requests

Runs comprehensive checks on all code changes:
- **check**: Code formatting (rustfmt) and linting (clippy)
- **test**: Unit and integration tests on Ubuntu and macOS
- **build**: Release binary compilation
- **doc**: Documentation generation
- **coverage**: Code coverage with llvm-cov (uploads to Codecov)
- **security**: Security audits with cargo-audit and cargo-deny

Features:
- Parallel job execution for speed
- Cargo registry and target directory caching
- Cross-platform testing (Linux, macOS)
- Fail-fast disabled for test matrix

#### `release.yml` - Release Automation
**Triggers:** Version tags (v*)

Automates the entire release process:
- Builds multi-platform binaries (linux-amd64, linux-arm64, darwin-amd64, darwin-arm64)
- Creates and pushes multi-arch Docker images
- Packages and publishes Helm charts
- Generates changelog from git commits
- Creates GitHub releases with all artifacts

Features:
- Cross-compilation support
- Binary stripping for size optimization
- SHA256 checksums for all artifacts
- GitHub Container Registry integration
- Semantic versioning support

### Testing

#### `chaos.yml` - Chaos Engineering
**Triggers:** Nightly schedule (2 AM UTC), Manual dispatch

Runs chaos testing scenarios:
- Docker Compose-based cluster setup
- Network partition simulation (toxiproxy)
- Node failure scenarios
- Disk failure simulation
- Memory pressure testing
- Automated test result collection
- Configurable duration and scenarios

Features:
- 60-minute timeout
- Log aggregation from all containers
- Test result artifacts
- Automated cleanup
- PR commenting with results

#### `bench.yml` - Performance Benchmarks
**Triggers:** Push to main, Pull requests, Manual

Comprehensive benchmark suite:
- Criterion-based microbenchmarks
- System-level benchmarks
- Performance regression detection
- Baseline comparison (vs main branch)
- PR performance comments
- Result visualization

Features:
- Benchmark result storage
- Historical comparison
- Alert on >150% performance degradation
- Artifact upload for analysis

#### `k3d.yml` - Kubernetes Integration Tests
**Triggers:** Push to main, Pull requests, Manual

Full Kubernetes integration testing:
- K3d cluster provisioning
- Helm chart deployment (3-node cluster)
- Health check verification
- Integration test suite execution
- Scaling tests (3→5→3 replicas)
- Rolling update tests
- Pod failure resilience tests
- Chaos experiments (optional)

Features:
- Local registry for fast image loading
- Comprehensive log collection
- Helm test execution
- Automatic cleanup

### Code Quality

#### `quality.yml` - Code Quality Checks
**Triggers:** Push to main, Pull requests, Manual

Enforces code quality standards:
- Format checking (rustfmt)
- Comprehensive linting (clippy with pedantic rules)
- Unused dependency detection (cargo-udeps)
- MSRV (Minimum Supported Rust Version) verification
- Code complexity analysis (cargo-geiger)
- Binary size analysis (cargo-bloat)

Features:
- Strict clippy rules
- Unsafe code detection
- Size regression tracking

#### `security.yml` - Security Auditing
**Triggers:** Daily schedule (3 AM UTC), Dependency changes, Manual

Automated security scanning:
- Cargo advisory database checks (cargo-audit)
- License and source verification (cargo-deny)
- Dependency review for PRs
- CodeQL static analysis
- Docker image vulnerability scanning (Trivy)
- SARIF report upload to GitHub Security

Features:
- Multiple security tools
- GitHub Security integration
- Automatic issue creation for vulnerabilities
- License compliance checking

#### `docs.yml` - Documentation
**Triggers:** Changes to Rust files or Markdown, Manual

Documentation building and deployment:
- Rustdoc generation with all features
- GitHub Pages deployment
- Markdown linting
- Link checking
- Spell checking

Features:
- Automatic docs.rs configuration
- Private item documentation
- Automated deployment on main
- Link validation

### Automation

#### `stale.yml` - Stale Issue Management
**Triggers:** Daily schedule

Automatically manages inactive issues and PRs:
- Marks issues stale after 60 days
- Marks PRs stale after 30 days
- Auto-closes stale items after grace period
- Exempts pinned/critical items

#### `labeler.yml` - Automatic Labeling
**Triggers:** Pull request events

Automatically labels PRs based on:
- Changed file paths (area labels)
- PR size (xs/s/m/l/xl)
- Component affected

Features:
- Sync labels on updates
- Size warnings for large PRs
- Comprehensive area coverage

## Workflow Dependencies

### `dependabot.yml` - Dependency Management

Automated dependency updates for:
- Cargo dependencies (weekly, Mondays 9 AM)
- GitHub Actions (weekly)
- Docker base images (weekly)

Features:
- Grouped updates for related crates
- Auto-review assignment
- Proper commit message prefixes
- Major version update controls

## Configuration Files

### `.github/labeler.yml`
Defines automatic labeling rules based on file paths.

### `.github/markdown-link-check.json`
Configuration for link checking in documentation.

### `.github/cspell.json`
Spell checker configuration with project-specific dictionary.

## Usage

### Running Workflows Manually

Most workflows support manual triggering via `workflow_dispatch`:

```bash
gh workflow run chaos.yml -f duration=60 -f scenarios="network-partition,pod-kill"
gh workflow run bench.yml
gh workflow run k3d.yml
```

### Workflow Secrets Required

- `CODECOV_TOKEN` - For code coverage uploads
- `GITHUB_TOKEN` - Automatically provided by GitHub Actions

### Caching Strategy

Workflows use GitHub Actions cache for:
- Cargo registry index
- Cargo git dependencies
- Target directories (per workflow type)
- Docker buildx layers

Cache keys include `Cargo.lock` hash for proper invalidation.

## Best Practices

1. **CI Workflow** runs on every PR - must pass before merge
2. **Security** runs daily - review alerts promptly
3. **Chaos tests** run nightly - check results each morning
4. **Benchmarks** on main branches track performance over time
5. **Release** workflow only triggers on version tags

## Troubleshooting

### Workflow Failures

1. Check the specific job that failed in the workflow run
2. Review the logs in the "Actions" tab
3. Download artifacts for detailed analysis
4. Re-run failed jobs if transient issues suspected

### Cache Issues

If caching causes problems:
1. Manually clear cache via GitHub UI
2. Update cache keys in workflow files
3. Caches auto-expire after 7 days of no use

### Resource Limits

- CI timeout: 45 minutes
- Chaos timeout: 60 minutes
- K3d timeout: 45 minutes
- Benchmark timeout: 90 minutes

## Adding New Workflows

1. Create `.yml` file in `.github/workflows/`
2. Follow existing naming conventions
3. Include appropriate timeout
4. Add caching for Rust workflows
5. Upload artifacts for debugging
6. Update this README

## Monitoring

Track workflow health:
- GitHub Actions dashboard
- Status badges in main README
- Email notifications (configure in GitHub settings)
- Slack integration (optional)

## Contributing

When modifying workflows:
1. Test in a fork first
2. Use `workflow_dispatch` for testing
3. Keep workflows focused and modular
4. Document any secrets or configuration needed
5. Update this README with changes
