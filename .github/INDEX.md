# GitHub Configuration Index

Quick navigation for all GitHub configuration files in the Anode project.

## Quick Links

- **Get Started**: [QUICK_REFERENCE.md](QUICK_REFERENCE.md)
- **Overview**: [README.md](README.md)
- **Complete Guide**: [WORKFLOWS_SUMMARY.md](WORKFLOWS_SUMMARY.md)
- **Workflow Details**: [workflows/README.md](workflows/README.md)

## Workflows

### CI/CD
- [ci.yml](workflows/ci.yml) - Main continuous integration pipeline
- [release.yml](workflows/release.yml) - Automated release process
- [quality.yml](workflows/quality.yml) - Code quality enforcement

### Testing
- [bench.yml](workflows/bench.yml) - Performance benchmarks
- [chaos.yml](workflows/chaos.yml) - Chaos engineering tests
- [k3d.yml](workflows/k3d.yml) - Kubernetes integration tests

### Security & Docs
- [security.yml](workflows/security.yml) - Security vulnerability scanning
- [docs.yml](workflows/docs.yml) - Documentation building and deployment

### Automation
- [stale.yml](workflows/stale.yml) - Stale issue/PR management
- [labeler.yml](workflows/labeler.yml) - Automatic PR labeling

## Templates

### Issues
- [bug_report.md](ISSUE_TEMPLATE/bug_report.md) - Bug report template
- [feature_request.md](ISSUE_TEMPLATE/feature_request.md) - Feature request template

### Pull Requests
- [PULL_REQUEST_TEMPLATE.md](PULL_REQUEST_TEMPLATE.md) - PR template

## Configuration

### Dependencies & Labels
- [dependabot.yml](dependabot.yml) - Automated dependency updates
- [labeler.yml](labeler.yml) - Automatic labeling rules

### Documentation Tools
- [markdown-link-check.json](markdown-link-check.json) - Link validation
- [cspell.json](cspell.json) - Spell checking

## Documentation

### Guides
1. [QUICK_REFERENCE.md](QUICK_REFERENCE.md) - Fast lookup guide (start here!)
2. [README.md](README.md) - GitHub directory overview
3. [workflows/README.md](workflows/README.md) - Detailed workflow documentation
4. [WORKFLOWS_SUMMARY.md](WORKFLOWS_SUMMARY.md) - Complete implementation guide
5. [INDEX.md](INDEX.md) - This file

## File Count

- **Total**: 22 files
- **Workflows**: 10 YAML files
- **Templates**: 3 Markdown files
- **Configuration**: 4 files (YAML/JSON)
- **Documentation**: 5 Markdown files

## Common Tasks

### Running Workflows Manually
```bash
# Main workflows
gh workflow run ci.yml
gh workflow run bench.yml
gh workflow run k3d.yml
gh workflow run security.yml

# Chaos testing (with options)
gh workflow run chaos.yml -f duration=60 -f scenarios="network-partition"
```

### Viewing Workflow Status
```bash
gh run list --workflow=ci.yml
gh run view <run-id>
gh run view <run-id> --log
```

### Creating Issues
- Bug: Use [bug_report.md](ISSUE_TEMPLATE/bug_report.md) template
- Feature: Use [feature_request.md](ISSUE_TEMPLATE/feature_request.md) template

### Creating Pull Requests
- Follow [PULL_REQUEST_TEMPLATE.md](PULL_REQUEST_TEMPLATE.md) structure
- PRs automatically labeled by [labeler.yml](labeler.yml)

## Workflow Trigger Reference

| Workflow | Auto Trigger | Manual | Schedule |
|----------|-------------|--------|----------|
| CI | Push/PR → main | ✓ | - |
| Release | Tag v* | - | - |
| Chaos | - | ✓ | Nightly 2 AM |
| Benchmarks | Push/PR → main | ✓ | - |
| K3d | Push/PR → main | ✓ | - |
| Quality | Push/PR → main | ✓ | - |
| Security | Deps changed | ✓ | Daily 3 AM |
| Docs | Docs changed | ✓ | - |
| Stale | - | ✓ | Daily midnight |
| Labeler | PR events | - | - |

## Required Secrets

- `CODECOV_TOKEN` - For code coverage uploads (optional)
- `GITHUB_TOKEN` - Automatically provided by GitHub

## Support

For help with workflows:
1. Check [QUICK_REFERENCE.md](QUICK_REFERENCE.md) for common commands
2. Read [workflows/README.md](workflows/README.md) for detailed docs
3. Review [WORKFLOWS_SUMMARY.md](WORKFLOWS_SUMMARY.md) for complete guide
4. Search existing issues
5. Create issue with `area/ci-cd` label

## Maintenance

### Daily
- Security scans (3 AM UTC)
- Stale management (midnight UTC)

### Weekly
- Dependency updates (Monday 9 AM UTC via Dependabot)

### Nightly
- Chaos tests (2 AM UTC)

### On-Demand
- Benchmarks (push to main)
- Integration tests (PRs)
- Quality checks (PRs)

## Recent Updates

**2025-12-19**: Initial comprehensive workflow implementation
- 10 production-ready workflows
- 3 issue/PR templates
- 4 configuration files
- 5 documentation files

---

**Navigation**: [↑ Top](#github-configuration-index) | [Workflows](#workflows) | [Templates](#templates) | [Configuration](#configuration) | [Documentation](#documentation)
