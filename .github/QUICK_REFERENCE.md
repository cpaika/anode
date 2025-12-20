# GitHub Actions Quick Reference

Fast lookup guide for Anode workflows.

## Workflow Triggers

| Workflow | Auto Trigger | Manual | Schedule |
|----------|-------------|--------|----------|
| CI | Push/PR to main | ✅ | - |
| Release | Tag v* | - | - |
| Chaos | - | ✅ | Nightly 2 AM UTC |
| Benchmarks | Push/PR to main | ✅ | - |
| K3d | Push/PR to main | ✅ | - |
| Quality | Push/PR to main | ✅ | - |
| Security | Dependency changes | ✅ | Daily 3 AM UTC |
| Docs | Doc changes | ✅ | - |
| Stale | - | ✅ | Daily midnight UTC |
| Labeler | PR events | - | - |

## Manual Workflow Execution

```bash
# Run chaos tests for 60 minutes
gh workflow run chaos.yml -f duration=60

# Run specific chaos scenarios
gh workflow run chaos.yml -f scenarios="network-partition,pod-kill"

# Run benchmarks
gh workflow run bench.yml

# Run K3d integration tests
gh workflow run k3d.yml

# Run quality checks
gh workflow run quality.yml

# Run security audit
gh workflow run security.yml

# Rebuild documentation
gh workflow run docs.yml
```

## CI Workflow Status

Required checks for PR merge:
- ✅ Format check (rustfmt)
- ✅ Clippy linting
- ✅ Tests (Ubuntu + macOS)
- ✅ Release build
- ✅ Documentation build
- ✅ Security audit

## Release Process

1. Tag version: `git tag v1.0.0`
2. Push tag: `git push origin v1.0.0`
3. Wait for workflow completion (~60 min)
4. Verify release on GitHub

**Artifacts created:**
- Linux AMD64 binary
- Linux ARM64 binary
- macOS AMD64 binary
- macOS ARM64 binary
- Docker images (ghcr.io)
- Helm chart
- SHA256 checksums
- Changelog

## Quick Diagnostics

### Check workflow status
```bash
gh run list --workflow=ci.yml
gh run view <run-id>
```

### Download artifacts
```bash
gh run download <run-id>
```

### View logs
```bash
gh run view <run-id> --log
```

### Cancel running workflow
```bash
gh run cancel <run-id>
```

### Re-run failed workflow
```bash
gh run rerun <run-id>
```

## Common Issues

### CI Fails - Formatting
```bash
# Fix locally
cargo fmt --all

# Check before push
cargo fmt --all -- --check
```

### CI Fails - Clippy
```bash
# Run clippy locally
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Auto-fix
cargo clippy --fix --workspace --allow-dirty
```

### CI Fails - Tests
```bash
# Run tests locally
cargo test --workspace --all-features

# Run specific test
cargo test <test_name> -- --nocapture
```

### Security Alerts
1. Check Security tab on GitHub
2. Review Dependabot PRs
3. Run locally: `cargo audit`
4. Check dependencies: `cargo deny check`

### Cache Issues
1. Go to Actions > Caches
2. Delete stale caches
3. Re-run workflow

## File Locations

### Workflows
```
.github/workflows/
├── ci.yml          - Main CI
├── release.yml     - Releases
├── chaos.yml       - Chaos tests
├── bench.yml       - Benchmarks
├── k3d.yml         - K8s tests
├── quality.yml     - Code quality
├── security.yml    - Security
├── docs.yml        - Documentation
├── stale.yml       - Issue management
└── labeler.yml     - Auto labeling
```

### Templates
```
.github/
├── ISSUE_TEMPLATE/
│   ├── bug_report.md
│   └── feature_request.md
└── PULL_REQUEST_TEMPLATE.md
```

### Configuration
```
.github/
├── dependabot.yml              - Dependency updates
├── labeler.yml                 - Label rules
├── markdown-link-check.json    - Link validation
└── cspell.json                 - Spell checking
```

## Workflow Dependencies

```
CI ──────────────┐
                 ├─> All checks pass ─> Merge PR
Quality ─────────┤
                 │
Security ────────┘

Tag v* ─> Release ─┬─> Binaries
                   ├─> Docker images
                   └─> Helm chart

Nightly ─> Chaos ─> Test reports

Push to main ─┬─> Benchmarks ─> Performance tracking
              └─> K3d ─────────> Integration validation
```

## Labels

### Auto-applied by area
- `area/storage` - Storage engine
- `area/raft` - Raft consensus
- `area/s3` - S3 API
- `area/protocol` - Protocol defs
- `area/parquet` - Parquet features
- `area/client` - Client SDK
- `area/ci-cd` - CI/CD
- `area/docker` - Docker
- `area/kubernetes` - K8s/Helm
- `area/tests` - Tests
- `area/chaos` - Chaos tests
- `area/benchmarks` - Benchmarks

### Auto-applied by type
- `documentation` - Docs
- `dependencies` - Deps
- `configuration` - Config

### Auto-applied by size
- `size/xs` - < 10 lines
- `size/s` - < 100 lines
- `size/m` - < 500 lines
- `size/l` - < 1000 lines
- `size/xl` - 1000+ lines

## Performance Thresholds

- **Benchmark regression**: 150% of baseline
- **Binary size**: Track in artifacts
- **Code coverage**: Upload to Codecov
- **Build time**: Monitored per workflow

## Security Scans

Daily scans include:
- cargo-audit (vulnerabilities)
- cargo-deny (licenses, bans)
- CodeQL (static analysis)
- Trivy (container scanning)
- Dependency review (PRs)

## Caching

Cache locations:
- `~/.cargo/registry/index` - Cargo registry
- `~/.cargo/git` - Git dependencies
- `target/` - Build artifacts

Cache keys include:
- OS: `${{ runner.os }}`
- Workflow: `<workflow-name>`
- Lock file: `${{ hashFiles('**/Cargo.lock') }}`

Cache expires: 7 days of inactivity

## Timeouts

| Workflow | Timeout | Reason |
|----------|---------|--------|
| CI | 45 min | Comprehensive testing |
| Release | 60 min | Multi-platform builds |
| Chaos | 60 min | Long-running tests |
| Benchmarks | 90 min | Performance analysis |
| K3d | 45 min | Cluster provisioning |
| Quality | 30 min | Multiple checks |
| Security | 30 min | Multiple scans |
| Docs | 30 min | Build + deploy |

## Secrets Required

- `CODECOV_TOKEN` - Code coverage (optional)
- `GITHUB_TOKEN` - Auto-provided by GitHub

## Support Resources

- **Workflow Docs**: `.github/workflows/README.md`
- **GitHub Overview**: `.github/README.md`
- **Complete Guide**: `.github/WORKFLOWS_SUMMARY.md`
- **This File**: `.github/QUICK_REFERENCE.md`

## Emergency Procedures

### Broken main branch
1. Identify failing workflow
2. Review recent commits
3. Revert if necessary: `git revert <commit>`
4. Push fix immediately

### Security vulnerability
1. Check Security tab
2. Review advisory details
3. Update dependency
4. Test thoroughly
5. Release patch version

### Failed release
1. Check release workflow logs
2. Delete failed tag: `git tag -d v1.0.0`
3. Delete remote tag: `git push origin :refs/tags/v1.0.0`
4. Delete draft release on GitHub
5. Fix issue
6. Re-tag and push

### Workflow quota exceeded
1. Check Actions usage in Settings
2. Cancel unnecessary runs
3. Optimize workflows
4. Consider runner retention

## Best Practices

1. **Always run CI locally first**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features
   cargo test --workspace
   ```

2. **Use draft PRs for WIP**
   - Prevents excessive CI runs
   - Marks work as in-progress

3. **Keep PRs small**
   - Easier to review
   - Faster CI execution
   - Less likely to conflict

4. **Tag releases properly**
   - Use semantic versioning
   - Format: v1.2.3
   - Follow existing patterns

5. **Monitor security alerts**
   - Check daily
   - Address promptly
   - Document decisions

## Additional Commands

```bash
# List all workflows
gh workflow list

# View specific workflow
gh workflow view ci.yml

# Enable/disable workflow
gh workflow enable ci.yml
gh workflow disable ci.yml

# Check rate limit
gh api rate_limit

# View repository Actions settings
gh api repos/{owner}/{repo}/actions/permissions
```

## Getting Help

1. Check workflow logs first
2. Review this quick reference
3. Read detailed docs in `.github/workflows/README.md`
4. Search existing issues
5. Create issue with `area/ci-cd` label

---

**Last Updated**: 2025-12-19
**Maintained By**: Anode Project
