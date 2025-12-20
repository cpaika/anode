# Anode GitHub Configuration

This directory contains all GitHub-specific configuration for the Anode project, including CI/CD workflows, issue templates, and automation.

## Directory Structure

```
.github/
├── workflows/           # GitHub Actions workflows
│   ├── ci.yml          # Main CI pipeline
│   ├── release.yml     # Release automation
│   ├── chaos.yml       # Chaos testing
│   ├── bench.yml       # Performance benchmarks
│   ├── k3d.yml         # Kubernetes integration tests
│   ├── quality.yml     # Code quality checks
│   ├── security.yml    # Security auditing
│   ├── docs.yml        # Documentation building
│   ├── stale.yml       # Stale issue management
│   ├── labeler.yml     # Automatic PR labeling
│   └── README.md       # Workflow documentation
├── ISSUE_TEMPLATE/     # Issue templates
│   ├── bug_report.md   # Bug report template
│   └── feature_request.md  # Feature request template
├── PULL_REQUEST_TEMPLATE.md  # PR template
├── dependabot.yml      # Dependency update configuration
├── labeler.yml         # Labeling rules
├── markdown-link-check.json  # Link checker config
├── cspell.json         # Spell checker config
└── README.md           # This file
```

## Workflows Summary

### Production Workflows

| Workflow | Trigger | Purpose | Duration |
|----------|---------|---------|----------|
| **CI** | Push, PR | Comprehensive testing & checks | ~30 min |
| **Release** | Tag (v*) | Build & publish releases | ~60 min |
| **K3d** | Push, PR | Kubernetes integration tests | ~45 min |
| **Security** | Daily, Dependencies | Security audits | ~20 min |

### Development Workflows

| Workflow | Trigger | Purpose | Duration |
|----------|---------|---------|----------|
| **Quality** | PR, Manual | Code quality analysis | ~30 min |
| **Docs** | Docs changes | Build documentation | ~15 min |
| **Benchmarks** | Push to main, Manual | Performance testing | ~90 min |
| **Chaos** | Nightly, Manual | Chaos engineering tests | ~60 min |

### Automation Workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| **Stale** | Daily | Manage inactive issues/PRs |
| **Labeler** | PR events | Auto-label PRs |
| **Dependabot** | Weekly | Update dependencies |

## Issue Templates

### Bug Reports (`ISSUE_TEMPLATE/bug_report.md`)

Standardized bug reporting with sections for:
- Bug description
- Reproduction steps
- Expected vs actual behavior
- Environment details
- Logs and configuration
- Possible solutions

### Feature Requests (`ISSUE_TEMPLATE/feature_request.md`)

Structured feature proposals including:
- Problem statement
- Proposed solution
- Alternative approaches
- Use cases
- Implementation ideas
- Priority level
- Contribution willingness

## Pull Request Template

The PR template (`PULL_REQUEST_TEMPLATE.md`) ensures comprehensive PR descriptions:

### Required Sections
- Description of changes
- Related issues (with auto-close syntax)
- Type of change (bug fix, feature, breaking, etc.)
- Testing performed
- Documentation updates
- Checklist of requirements

### Additional Information
- Breaking changes and migration guide
- Performance impact
- Security considerations
- Deployment notes

## Dependabot Configuration

Automated dependency updates for:

### Cargo Dependencies
- **Schedule**: Weekly (Mondays, 9 AM UTC)
- **Grouping**: Related crates grouped together (tokio, serde, AWS SDK, etc.)
- **Controls**: Major version updates ignored for critical dependencies
- **Limit**: 10 open PRs max

### GitHub Actions
- **Schedule**: Weekly
- **Limit**: 5 open PRs max
- **Auto-labels**: `dependencies`, `github-actions`

### Docker
- **Path**: `deploy/docker`
- **Schedule**: Weekly
- **Limit**: 5 open PRs max

## Labeling System

### Automatic Labels

PRs are automatically labeled based on changed files:

#### Area Labels
- `area/storage` - Storage engine changes
- `area/raft` - Raft consensus changes
- `area/s3` - S3 API changes
- `area/protocol` - Protocol definitions
- `area/parquet` - Parquet integration
- `area/client` - Client SDK
- `area/ci-cd` - CI/CD workflows
- `area/docker` - Docker/containers
- `area/kubernetes` - Kubernetes/Helm
- `area/tests` - Test changes
- `area/chaos` - Chaos testing
- `area/benchmarks` - Performance benchmarks

#### Type Labels
- `documentation` - Documentation changes
- `dependencies` - Dependency updates
- `configuration` - Config file changes

#### Size Labels
- `size/xs` - < 10 lines changed
- `size/s` - < 100 lines changed
- `size/m` - < 500 lines changed
- `size/l` - < 1000 lines changed
- `size/xl` - 1000+ lines changed

## Configuration Files

### `markdown-link-check.json`
Validates links in documentation:
- Ignores localhost URLs
- Custom timeout and retry settings
- GitHub API headers
- Status code configuration

### `cspell.json`
Spell checking configuration:
- Project-specific dictionary (Rust terms, Anode components)
- Ignore patterns (hashes, hex values)
- Excluded paths (target/, node_modules/)
- Regular expressions for technical content

## Secrets and Variables

### Required Secrets
- `CODECOV_TOKEN` - Code coverage uploads (optional but recommended)
- `GITHUB_TOKEN` - Automatically provided by GitHub

### Repository Variables
None currently required. All configuration is in workflow files.

## Best Practices

### For Contributors

1. **Use Issue Templates**: Create issues using provided templates
2. **Fill Out PR Template**: Complete all sections of the PR template
3. **Wait for CI**: All CI checks must pass before merge
4. **Review Labels**: Check that auto-labels are correct
5. **Follow Conventions**: Use conventional commit messages

### For Maintainers

1. **Review Security Alerts**: Check daily security workflow results
2. **Monitor Chaos Tests**: Review nightly chaos test results
3. **Track Performance**: Monitor benchmark trends
4. **Manage Stale Items**: Review and close stale issues appropriately
5. **Update Dependencies**: Review and merge Dependabot PRs regularly

## Workflow Badges

Add these badges to your README.md:

```markdown
[![CI](https://github.com/USERNAME/anode/actions/workflows/ci.yml/badge.svg)](https://github.com/USERNAME/anode/actions/workflows/ci.yml)
[![Security](https://github.com/USERNAME/anode/actions/workflows/security.yml/badge.svg)](https://github.com/USERNAME/anode/actions/workflows/security.yml)
[![Chaos Tests](https://github.com/USERNAME/anode/actions/workflows/chaos.yml/badge.svg)](https://github.com/USERNAME/anode/actions/workflows/chaos.yml)
[![codecov](https://codecov.io/gh/USERNAME/anode/branch/main/graph/badge.svg)](https://codecov.io/gh/USERNAME/anode)
```

## Troubleshooting

### Workflow Not Running

1. Check workflow file syntax (YAML validation)
2. Verify trigger conditions match your action
3. Check repository settings > Actions permissions
4. Review branch protection rules

### Failed Workflows

1. Click on failed job for detailed logs
2. Download artifacts for offline analysis
3. Re-run jobs for transient failures
4. Check for service outages (GitHub, Codecov, etc.)

### Cache Issues

1. Clear cache via repository settings
2. Update cache keys in workflow files
3. Check disk usage on runners
4. Verify cache restore/save logs

### Security Alerts

1. Review Dependabot alerts in Security tab
2. Check workflow run for detailed reports
3. Download SARIF files for offline analysis
4. Prioritize critical/high severity issues

## Customization

### Adding New Workflows

1. Create file in `.github/workflows/`
2. Use descriptive name (lowercase, hyphens)
3. Include:
   - Clear name and description
   - Appropriate triggers
   - Timeout settings
   - Error handling
   - Artifact uploads
   - Cache configuration
4. Test in fork first
5. Update this README

### Modifying Templates

1. Edit files in `ISSUE_TEMPLATE/` or `PULL_REQUEST_TEMPLATE.md`
2. Test with actual issues/PRs
3. Gather feedback from contributors
4. Iterate based on usage

### Adjusting Dependabot

1. Edit `dependabot.yml`
2. Adjust schedules, limits, or grouping
3. Add/remove ecosystems as needed
4. Test with manual trigger

## Monitoring and Metrics

### GitHub Insights

Monitor project health via GitHub Insights:
- Actions usage and trends
- Dependency alerts
- Code frequency
- Contributor activity

### Workflow Metrics

Track these metrics:
- CI success rate
- Average run time
- Queue time
- Resource usage

### Quality Metrics

Monitor code quality:
- Test coverage trends (Codecov)
- Security vulnerability count
- Dependency freshness
- Documentation coverage

## Contributing to Workflows

When contributing workflow improvements:

1. Fork the repository
2. Create a feature branch
3. Test workflows in your fork
4. Submit PR with:
   - Clear description of changes
   - Before/after comparison
   - Test results
   - Updated documentation

## Support

For issues with workflows:
1. Check workflow logs
2. Review this documentation
3. Search existing issues
4. Create new issue with `area/ci-cd` label

## License

All workflow and configuration files are part of the Anode project and follow the same license (Apache 2.0 / MIT dual license).
