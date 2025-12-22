# Release Guide

This document describes the release process for Anode using semantic versioning.

## Overview

The release workflow is fully automated and triggered by pushing a version tag. It handles:

- Creating GitHub releases with auto-generated notes
- Building multi-platform binaries (Linux x64/ARM64, macOS x64/ARM64)
- Building and pushing Docker images to GitHub Container Registry
- Packaging and publishing Helm charts

## Semantic Versioning

Anode follows [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR** version: Incompatible API changes
- **MINOR** version: Backwards-compatible functionality additions
- **PATCH** version: Backwards-compatible bug fixes

### Pre-release Versions

Pre-release versions can be tagged with identifiers:

- `v1.0.0-alpha.1` - Alpha release
- `v1.0.0-beta.1` - Beta release
- `v1.0.0-rc.1` - Release candidate

These will be marked as "pre-release" on GitHub automatically.

## Creating a Release

### 1. Update Version Numbers

First, update the version in the workspace Cargo.toml:

```bash
# Edit Cargo.toml
vim Cargo.toml

# Update the version field:
# [workspace.package]
# version = "X.Y.Z"
```

### 2. Commit Version Changes

```bash
git add Cargo.toml
git commit -m "Bump version to X.Y.Z"
git push origin master
```

### 3. Create and Push Tag

```bash
# Create the tag (must start with 'v')
git tag -a vX.Y.Z -m "Release version X.Y.Z"

# Push the tag to trigger the workflow
git push origin vX.Y.Z
```

### 4. Monitor the Workflow

The release workflow will automatically:

1. Validate the semantic version format
2. Generate a changelog from commits since the last tag
3. Create a GitHub release (draft or pre-release as appropriate)
4. Build binaries for all platforms in parallel
5. Build and push Docker images with appropriate tags
6. Package the Helm chart with the new version
7. Upload all artifacts to the GitHub release

You can monitor progress at:
```
https://github.com/YOUR_USERNAME/anode/actions/workflows/release.yml
```

## Release Artifacts

After a successful release, the following artifacts are available:

### Binaries

- `anode-linux-amd64.tar.gz` - Linux x86_64 binary + SHA256 checksum
- `anode-linux-arm64.tar.gz` - Linux ARM64 binary + SHA256 checksum
- `anode-darwin-amd64.tar.gz` - macOS x86_64 binary + SHA256 checksum
- `anode-darwin-arm64.tar.gz` - macOS ARM64 binary + SHA256 checksum

Each binary includes a SHA256 checksum file for verification:

```bash
# Download binary and checksum
wget https://github.com/YOUR_USERNAME/anode/releases/download/vX.Y.Z/anode-linux-amd64.tar.gz
wget https://github.com/YOUR_USERNAME/anode/releases/download/vX.Y.Z/anode-linux-amd64.tar.gz.sha256

# Verify checksum
shasum -a 256 -c anode-linux-amd64.tar.gz.sha256
```

### Docker Images

Docker images are published to GitHub Container Registry with multiple tags:

- `ghcr.io/YOUR_USERNAME/anode:X.Y.Z` - Full semantic version
- `ghcr.io/YOUR_USERNAME/anode:X.Y` - Major.Minor version
- `ghcr.io/YOUR_USERNAME/anode:X` - Major version only
- `ghcr.io/YOUR_USERNAME/anode:master-HASH` - Git SHA for traceability

Multi-architecture images support:
- `linux/amd64`
- `linux/arm64`

Pull the image:

```bash
docker pull ghcr.io/YOUR_USERNAME/anode:X.Y.Z
```

### Helm Chart

The Helm chart is packaged and uploaded with the release:

- `anode-helm-chart-X.Y.Z.tgz` - Packaged Helm chart
- `index.yaml` - Helm repository index

Install the chart:

```bash
# Add the Helm repository
helm repo add anode https://github.com/YOUR_USERNAME/anode/releases/download/vX.Y.Z

# Install
helm install my-anode anode/anode --version X.Y.Z
```

## Version Tag Format

The workflow validates that tags follow semantic versioning:

- **Valid**: `v1.0.0`, `v2.3.1`, `v1.0.0-alpha.1`, `v2.0.0-beta.2`, `v3.1.0-rc.1`
- **Invalid**: `1.0.0` (missing 'v'), `v1.0` (incomplete), `release-1.0.0` (wrong prefix)

The regex pattern used for validation:
```regex
^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9]+(\.[a-zA-Z0-9]+)*)?$
```

## Troubleshooting

### Workflow Fails with "Invalid semantic version"

Ensure your tag follows the format `vMAJOR.MINOR.PATCH` or `vMAJOR.MINOR.PATCH-PRERELEASE`.

### Binary Build Fails

Check the build logs in the GitHub Actions UI. Common issues:
- Compilation errors (fix and create a new patch version)
- Cross-compilation toolchain missing (workflow handles this automatically)

### Docker Push Fails

Ensure GitHub Actions has write permissions for packages:
- Go to repository Settings → Actions → General → Workflow permissions
- Enable "Read and write permissions"

### Helm Chart Upload Fails

The workflow requires the `create-release` job to complete successfully first. Check that:
- The GitHub release was created
- The version extraction worked correctly

## Best Practices

1. **Test Before Tagging**: Run full test suite before creating a release tag
   ```bash
   cargo test --all
   cargo clippy --all
   ```

2. **Meaningful Commit Messages**: The changelog is generated from commit messages
   - Use conventional commits: `feat:`, `fix:`, `docs:`, etc.
   - Write clear, descriptive messages

3. **Review Changes**: Before tagging, review all changes since the last release
   ```bash
   git log $(git describe --tags --abbrev=0)..HEAD --oneline
   ```

4. **Update Documentation**: Ensure README and docs are up to date with new features

5. **Test Artifacts**: After release, download and test at least one binary artifact

## Emergency Rollback

If a release has critical issues:

1. Create a new patch version with the fix
2. Tag and push the new version
3. Mark the problematic release as "pre-release" on GitHub
4. Add a warning to the release notes

**Never delete tags** - this breaks reproducibility and confuses users.

## Release Checklist

- [ ] All tests passing (`cargo test --all`)
- [ ] Linter clean (`cargo clippy --all`)
- [ ] Code formatted (`cargo fmt --all`)
- [ ] Documentation updated
- [ ] CHANGELOG reviewed (if maintained separately)
- [ ] Version bumped in `Cargo.toml`
- [ ] Changes committed and pushed
- [ ] Tag created with correct format
- [ ] Tag pushed to trigger workflow
- [ ] Workflow completed successfully
- [ ] Artifacts verified on GitHub release page
- [ ] Docker image tested
- [ ] Helm chart installation tested
