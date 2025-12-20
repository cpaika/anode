# K3d Testing Infrastructure for Anode

This directory contains a complete K3d-based testing infrastructure for the Anode distributed object storage system. It provides a local Kubernetes environment for integration testing, development, and validation.

## Overview

The K3d testing infrastructure allows you to:

- Create a local multi-node Kubernetes cluster using K3d
- Build and deploy Anode to the cluster
- Run comprehensive integration tests
- Test cluster resilience, scaling, and failover scenarios
- Validate S3 API compatibility
- Test data persistence and recovery

## Prerequisites

Install the following tools:

```bash
# macOS
brew install k3d kubectl helm docker

# Linux
# Install Docker first, then:
curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
```

Ensure Docker Desktop (macOS/Windows) or Docker daemon (Linux) is running.

## Quick Start

### 1. Create Cluster and Deploy

```bash
# Create cluster, build image, and deploy
make deploy

# Or run steps individually:
make cluster        # Create K3d cluster
make rebuild        # Build and deploy without recreating cluster
```

### 2. Run Tests

```bash
# Run all integration tests
make test

# Or run individual tests:
make test-deployment       # Test deployment health
make test-scaling          # Test scaling up/down
make test-rolling-update   # Test zero-downtime updates
make test-node-failure     # Test node failure recovery
make test-s3               # Test S3 API operations
make test-persistence      # Test data persistence
```

### 3. Check Status

```bash
# View cluster and deployment status
make status

# View logs
make logs

# Watch pods
make watch-pods
```

### 4. Clean Up

```bash
# Delete cluster and all resources
make clean
```

## Architecture

### Cluster Configuration

The K3d cluster is configured with:

- **1 server node** (control plane)
- **3 agent nodes** (workers)
- **Local Docker registry** on port 5111
- **Load balancer** exposing ports 8080 (HTTP) and 8443 (HTTPS)
- **Traefik disabled** (to avoid conflicts)

Configuration file: `cluster.yaml`

### Scripts

| Script | Purpose |
|--------|---------|
| `create-cluster.sh` | Create K3d cluster with registry |
| `delete-cluster.sh` | Delete cluster and cleanup |
| `build-and-load.sh` | Build Docker image and push to registry |
| `deploy-anode.sh` | Deploy Anode using Helm |
| `run-tests.sh` | Run all integration tests |

### Tests

| Test | Description |
|------|-------------|
| `test_deployment.sh` | Verifies deployment, pod health, and service endpoints |
| `test_scaling.sh` | Tests scaling up and down |
| `test_rolling_update.sh` | Tests zero-downtime rolling updates |
| `test_node_failure.sh` | Tests recovery from node failure |
| `test_s3_operations.sh` | Tests S3 API operations (CRUD) |
| `test_data_persistence.sh` | Tests data survives pod restarts |

## Usage

### Basic Commands

```bash
# Show all available commands
make help

# Create cluster only
make cluster

# Build and deploy without tests
make rebuild

# Run all tests
make test

# Reset everything (delete and recreate)
make reset
```

### Development Workflow

```bash
# Make code changes, then:
make dev              # Rebuild and deploy

# View logs
make logs

# Open shell in pod
make shell

# Test your changes
make test
```

### Port Forwarding

Access Anode services locally:

```bash
# S3 API (port 8080)
make port-forward-s3

# Admin API (port 8081)
make port-forward-admin

# Or manually:
kubectl port-forward -n anode svc/anode 8080:8080
```

### Debugging

```bash
# View pod details
make describe-pod

# View recent events
make events

# Get logs
make logs

# Shell access
make shell

# Check status
make status
```

## Environment Variables

Customize the deployment:

```bash
# Change namespace
NAMESPACE=my-namespace make deploy

# Change Helm release name
RELEASE_NAME=my-anode make deploy

# Use specific image tag
IMAGE_TAG=v1.2.3 make deploy

# Use custom Helm values
HELM_VALUES_FILE=custom-values.yaml make deploy
```

## Test Details

### Deployment Test

Validates:
- StatefulSet creation and replica count
- Pod health and readiness
- Service configuration and endpoints
- PersistentVolumeClaim binding
- Health endpoint accessibility

### Scaling Test

Validates:
- Scaling up increases replicas
- New pods are scheduled and become ready
- Scaling down removes pods correctly
- Remaining pods stay healthy

### Rolling Update Test

Validates:
- Updates are applied gradually
- Pods are updated one at a time
- Service remains available during update
- No failed pods during rollout

### Node Failure Test

Validates:
- Pods are rescheduled when node fails
- Cluster recovers automatically
- All replicas remain available
- No data loss during recovery

### S3 Operations Test

Validates:
- S3 API endpoint accessibility
- Bucket operations (create, list, delete)
- Object operations (put, get, delete)
- Multi-part upload endpoints

### Data Persistence Test

Validates:
- Data survives pod restarts
- PVCs remain bound to same PVs
- Pods can access previous data
- StatefulSet maintains identity

## Cluster Information

### Default Configuration

- Cluster name: `anode-test`
- Namespace: `anode`
- Release name: `anode`
- Registry: `localhost:5111`
- S3 API port: `8080`
- Admin API port: `8081`
- gRPC port: `9090`

### Resource Requirements

Minimum system requirements:
- CPU: 4 cores
- Memory: 8GB RAM
- Disk: 20GB free space
- Docker with sufficient resources allocated

### Registry

The local Docker registry allows:
- Fast image push/pull (no external network)
- Consistent image versions
- Development iteration speed
- Works without internet connection

Access the registry:
```bash
# List images
curl http://localhost:5111/v2/_catalog

# List tags for anode
curl http://localhost:5111/v2/anode/tags/list
```

## Troubleshooting

### Cluster Creation Fails

```bash
# Check Docker is running
docker info

# Check if ports are available
lsof -i :5111  # Registry port
lsof -i :8080  # HTTP port
lsof -i :8443  # HTTPS port

# Delete and recreate
make reset
```

### Deployment Fails

```bash
# Check cluster is running
kubectl cluster-info

# Check image exists in registry
curl http://localhost:5111/v2/anode/tags/list

# View pod events
kubectl describe pod -n anode anode-0

# Check logs
make logs
```

### Tests Fail

```bash
# Run individual test for debugging
./tests/test_deployment.sh

# Check pod status
make status

# Collect logs
make logs > test-debug.log

# Check events
make events
```

### Image Build Fails

```bash
# Check Docker has enough resources
docker system df

# Clean up old images
docker system prune -a

# Rebuild
make rebuild
```

## Advanced Usage

### Custom Cluster Configuration

Edit `cluster.yaml` to customize:
- Number of nodes
- Kubernetes version
- Port mappings
- Registry configuration
- K3s arguments

Then recreate the cluster:
```bash
make reset
```

### Custom Test Values

Create a custom Helm values file:

```yaml
# custom-values.yaml
replicaCount: 5
resources:
  requests:
    memory: "2Gi"
    cpu: "1000m"
```

Deploy with custom values:
```bash
HELM_VALUES_FILE=custom-values.yaml make deploy
```

### CI/CD Integration

The scripts support non-interactive mode:

```bash
# Force deletion without confirmation
./delete-cluster.sh --force

# Set timeout for tests
TEST_TIMEOUT=600 ./run-tests.sh
```

### Multi-Cluster Testing

Run multiple clusters for parallel testing:

```bash
# Edit cluster.yaml to change cluster name
# Then:
./create-cluster.sh
NAMESPACE=anode-cluster-2 ./deploy-anode.sh
```

## Integration with Other Tools

### kubectl

All standard kubectl commands work:

```bash
kubectl get pods -n anode
kubectl describe statefulset -n anode anode
kubectl exec -it -n anode anode-0 -- /bin/sh
```

### Helm

Manage deployment directly:

```bash
helm list -n anode
helm upgrade anode ./deploy/helm/anode -n anode
helm rollback anode -n anode
```

### K9s

Use K9s for interactive cluster management:

```bash
brew install k9s
k9s -n anode
```

## Best Practices

1. **Always clean up** after testing: `make clean`
2. **Check status** before deploying: `make status`
3. **Review logs** when tests fail: `make logs`
4. **Test incrementally** during development
5. **Use port-forwarding** for API access
6. **Monitor resources** in Docker Desktop
7. **Keep cluster configuration** in version control

## Contributing

When adding new tests:

1. Create test script in `tests/` directory
2. Name it `test_<feature>.sh`
3. Follow existing test structure
4. Add to `Makefile` with target
5. Update this README
6. Ensure proper cleanup on failure

## License

Same as Anode project (MIT OR Apache-2.0)

## Support

For issues:
1. Check this README
2. Review test logs
3. Check cluster status
4. Open GitHub issue with details

---

Happy testing! 🚀
