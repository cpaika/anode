# Anode Docker Build Guide

## Overview

This directory contains an optimized multi-stage Dockerfile for building and running Anode in production or development environments.

## Dockerfile Features

### Multi-Stage Build
1. **Chef Planner Stage**: Analyzes dependencies using cargo-chef
2. **Builder Stage**: Compiles dependencies and application with optimization
3. **Runtime Stage (Default)**: Minimal distroless image for production
4. **Alternative Runtime Stage**: Debian-based image for debugging

### Optimizations

#### Build Speed
- **cargo-chef**: Intelligent dependency caching that survives source code changes
- **Layer Optimization**: Dependencies are built separately and cached
- **Incremental Builds**: Only rebuilds changed code, not entire dependency tree

#### Image Size
- **Distroless Base**: Minimal runtime image (~20-30MB vs ~80MB for debian-slim)
- **Stripped Binary**: Debug symbols removed for smaller size
- **Multi-stage**: Build artifacts don't pollute final image

#### Security
- **Distroless**: No shell, package manager, or unnecessary utilities
- **Non-root User**: Runs as uid=65532 (nonroot) by default
- **Minimal Attack Surface**: Only runtime libraries and application binary

## Building the Image

### Production Build (Distroless)
```bash
# Build with default distroless runtime
docker build -t anode:latest -f deploy/docker/Dockerfile .

# Build with explicit distroless target
docker build -t anode:distroless --target gcr.io/distroless/cc-debian12:nonroot -f deploy/docker/Dockerfile .
```

### Development Build (Debian-based)
```bash
# Build with debian runtime for debugging
docker build -t anode:debug --target runtime-debian -f deploy/docker/Dockerfile .
```

### Build Arguments
You can customize the build with arguments:
```bash
# Use specific Rust version
docker build --build-arg RUST_VERSION=1.83 -t anode:latest -f deploy/docker/Dockerfile .
```

## Running the Container

### Basic Run
```bash
docker run -d \
  --name anode \
  -p 8080:8080 \
  -p 8081:8081 \
  -p 9090:9090 \
  -v $(pwd)/data:/data \
  -v $(pwd)/config:/config \
  anode:latest
```

### With Environment Variables
```bash
docker run -d \
  --name anode \
  -p 8080:8080 \
  -e ANODE_LOG_LEVEL=debug \
  -e ANODE_DATA_DIR=/data \
  -v $(pwd)/data:/data \
  -v $(pwd)/config:/config \
  anode:latest
```

### Using Docker Compose
See `docker-compose.yml` for full cluster setup:
```bash
docker-compose up -d
```

## Image Comparison

### Distroless (Default)
- **Size**: ~50-70MB
- **Security**: Highest (no shell, minimal packages)
- **Debugging**: Limited (no shell access)
- **Use Case**: Production deployments

### Debian Slim (Alternative)
- **Size**: ~120-150MB
- **Security**: Good (can be hardened)
- **Debugging**: Full (shell, curl, standard tools)
- **Use Case**: Development, debugging, troubleshooting

## Ports

| Port | Service | Description |
|------|---------|-------------|
| 8080 | S3 API | HTTP API for S3-compatible operations |
| 8081 | Admin | Health checks and admin endpoints |
| 9090 | gRPC | Inter-node communication for clustering |

## Volume Mounts

| Path | Purpose |
|------|---------|
| `/data` | Persistent storage for object data |
| `/config` | Configuration files (anode.toml) |

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ANODE_DATA_DIR` | `/data` | Directory for object storage |
| `ANODE_CONFIG_DIR` | `/config` | Configuration directory |
| `ANODE_LOG_LEVEL` | `info` | Log level (trace, debug, info, warn, error) |
| `RUST_BACKTRACE` | `1` | Enable backtraces on panic |

## Troubleshooting

### Debugging with Distroless
Since distroless has no shell, debugging requires alternative approaches:

```bash
# View logs
docker logs anode

# Copy files out
docker cp anode:/data/debug.log ./

# Use debug build instead
docker build -t anode:debug --target runtime-debian -f deploy/docker/Dockerfile .
docker run --rm -it anode:debug /bin/sh
```

### Permission Issues
If you encounter permission errors with volumes:

```bash
# Check ownership
docker run --rm -v $(pwd)/data:/data anode:debug ls -la /data

# For distroless (uid=65532)
chown -R 65532:65532 ./data ./config

# For debian runtime (uid=1000)
chown -R 1000:1000 ./data ./config
```

### Build Cache Issues
If builds are slow or using old dependencies:

```bash
# Clear build cache
docker builder prune -a

# Build without cache
docker build --no-cache -t anode:latest -f deploy/docker/Dockerfile .
```

## Advanced Usage

### Multi-Architecture Builds
```bash
# Build for multiple platforms
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t anode:latest \
  -f deploy/docker/Dockerfile \
  --push .
```

### Layer Analysis
```bash
# Analyze image layers
docker history anode:latest

# Check image size
docker images anode:latest
```

### Registry Push
```bash
# Tag for registry
docker tag anode:latest registry.example.com/anode:v1.0.0

# Push to registry
docker push registry.example.com/anode:v1.0.0
```

## Performance Tips

1. **Use BuildKit**: Enable Docker BuildKit for faster builds
   ```bash
   DOCKER_BUILDKIT=1 docker build -t anode:latest -f deploy/docker/Dockerfile .
   ```

2. **Leverage Cache**: Don't modify Cargo.toml unnecessarily

3. **Resource Limits**: Set appropriate memory/CPU limits
   ```bash
   docker run -d --memory=2g --cpus=2 anode:latest
   ```

## Development Workflow

1. **Code Changes**: Modify source code
2. **Rebuild**: `docker build` will only rebuild changed code
3. **Test**: Run container with your changes
4. **Iterate**: cargo-chef ensures fast incremental builds

## Further Reading

- [Docker Multi-stage Builds](https://docs.docker.com/build/building/multi-stage/)
- [Distroless Images](https://github.com/GoogleContainerTools/distroless)
- [cargo-chef](https://github.com/LukeMathWalker/cargo-chef)
