# Anode Helm Chart

![Version: 0.1.0](https://img.shields.io/badge/Version-0.1.0-informational?style=flat-square)
![Type: application](https://img.shields.io/badge/Type-application-informational?style=flat-square)
![AppVersion: 0.1.0](https://img.shields.io/badge/AppVersion-0.1.0-informational?style=flat-square)

Distributed object storage for small clusters with S3 API compatibility, Raft consensus, and Parquet-aware optimizations.

## Features

- **S3-Compatible API**: Drop-in replacement for basic S3 operations
- **Distributed Consensus**: Raft-based replication for high availability
- **Parquet-Aware**: Intelligent metadata caching for analytical workloads
- **Production-Ready**: Comprehensive monitoring, health checks, and operational tooling
- **Kubernetes-Native**: Designed for cloud-native deployments

## Prerequisites

- Kubernetes 1.19+
- Helm 3.2.0+
- PV provisioner support in the underlying infrastructure (if persistence is enabled)

## Installation

### Method 1: GitHub Release (Recommended for quick start)

Install directly from a GitHub release:

```bash
# Install from specific version
helm install anode https://github.com/anode/anode/releases/download/v0.1.0/anode-0.1.0.tgz \
  -n anode --create-namespace

# Install with custom values
helm install anode https://github.com/anode/anode/releases/download/v0.1.0/anode-0.1.0.tgz \
  -n anode --create-namespace -f custom-values.yaml
```

### Method 2: OCI Registry (Recommended for production)

Pull from GitHub Container Registry (available for releases v0.1.0+):

```bash
# Install from OCI registry
helm install anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  -n anode --create-namespace

# Install with custom values
helm install anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  -n anode --create-namespace \
  -f custom-values.yaml
```

### Method 3: Local Development

Install from local chart directory:

```bash
# Install from local chart
helm install anode ./deploy/helm/anode -n anode --create-namespace

# Install with custom values
helm install anode ./deploy/helm/anode -n anode --create-namespace -f custom-values.yaml
```

## Configuration

### Basic Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `replicaCount` | Number of anode replicas (should be odd: 3, 5, 7) | `3` |
| `image.repository` | Docker image repository | `anode/anode` |
| `image.tag` | Docker image tag | `""` (uses appVersion) |
| `image.pullPolicy` | Image pull policy | `IfNotPresent` |

### Service Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `service.type` | Kubernetes service type | `ClusterIP` |
| `service.s3Port` | S3 API port | `8080` |
| `service.adminPort` | Admin API port | `8081` |
| `service.grpcPort` | gRPC port for inter-node communication | `9090` |

### Storage Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `persistence.enabled` | Enable persistent storage | `true` |
| `persistence.storageClass` | Storage class name | `""` (default) |
| `persistence.size` | Size of persistent volume | `10Gi` |
| `persistence.accessModes` | Access modes for PVC | `["ReadWriteOnce"]` |

### Resource Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `resources.requests.cpu` | CPU request | `250m` |
| `resources.requests.memory` | Memory request | `512Mi` |
| `resources.limits.cpu` | CPU limit | `1000m` |
| `resources.limits.memory` | Memory limit | `2Gi` |

### Anode Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `config.cluster.placementGroups` | Number of placement groups | `16` |
| `config.cluster.replicationFactor` | Replication factor | `3` |
| `config.storage.chunkSize` | Chunk size for object splitting | `4MB` |
| `config.storage.dataDir` | Data directory path | `/data` |
| `config.parquet.cacheEnabled` | Enable parquet metadata caching | `true` |
| `config.parquet.cacheSizeMb` | Parquet cache size in MB | `256` |

### Monitoring Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `monitoring.enabled` | Enable metrics export | `true` |
| `monitoring.serviceMonitor.enabled` | Create Prometheus ServiceMonitor | `false` |
| `monitoring.prometheusRule.enabled` | Create Prometheus alerting rules | `false` |

### Ingress Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `ingress.enabled` | Enable ingress | `false` |
| `ingress.className` | Ingress class name | `""` |
| `ingress.hosts` | Ingress hosts configuration | `[]` |
| `ingress.tls` | Ingress TLS configuration | `[]` |

## Examples

### Minimal 3-Node Cluster

```yaml
# minimal-values.yaml
replicaCount: 3

persistence:
  enabled: true
  size: 10Gi

resources:
  requests:
    cpu: 250m
    memory: 512Mi
  limits:
    cpu: 1000m
    memory: 2Gi
```

```bash
helm install anode ./deploy/helm/anode -f minimal-values.yaml -n anode --create-namespace
```

### High-Availability 5-Node Cluster

```yaml
# ha-values.yaml
replicaCount: 5

persistence:
  enabled: true
  storageClass: "fast-ssd"
  size: 50Gi

resources:
  requests:
    cpu: 500m
    memory: 1Gi
  limits:
    cpu: 2000m
    memory: 4Gi

config:
  cluster:
    replicationFactor: 3
    placementGroups: 32
  parquet:
    cacheSizeMb: 512

monitoring:
  serviceMonitor:
    enabled: true
  prometheusRule:
    enabled: true

podDisruptionBudget:
  enabled: true
  minAvailable: 3

affinity:
  podAntiAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
    - labelSelector:
        matchLabels:
          app.kubernetes.io/name: anode
      topologyKey: kubernetes.io/hostname
```

```bash
helm install anode ./deploy/helm/anode -f ha-values.yaml -n anode --create-namespace
```

### With Ingress and TLS

```yaml
# ingress-values.yaml
replicaCount: 3

ingress:
  enabled: true
  className: nginx
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/proxy-body-size: "0"
    nginx.ingress.kubernetes.io/proxy-request-buffering: "off"
  hosts:
  - host: s3.example.com
    paths:
    - path: /
      pathType: Prefix
  tls:
  - secretName: anode-tls
    hosts:
    - s3.example.com

tls:
  enabled: true
  certManager:
    enabled: true
    issuerName: letsencrypt-prod
    issuerKind: ClusterIssuer
```

```bash
helm install anode ./deploy/helm/anode -f ingress-values.yaml -n anode --create-namespace
```

### Development/Testing (No Persistence)

```yaml
# dev-values.yaml
replicaCount: 3

persistence:
  enabled: false

resources:
  requests:
    cpu: 100m
    memory: 256Mi
  limits:
    cpu: 500m
    memory: 1Gi

config:
  logging:
    level: debug
```

```bash
helm install anode ./deploy/helm/anode -f dev-values.yaml -n anode --create-namespace
```

## Usage

### Accessing the S3 API

#### Port Forward (Development)

```bash
kubectl port-forward -n anode svc/anode 8080:8080
```

#### Get Admin Credentials

```bash
export ACCESS_KEY=$(kubectl get secret -n anode anode-secret -o jsonpath="{.data.s3-access-key}" | base64 -d)
export SECRET_KEY=$(kubectl get secret -n anode anode-secret -o jsonpath="{.data.s3-secret-key}" | base64 -d)

echo "Access Key: $ACCESS_KEY"
echo "Secret Key: $SECRET_KEY"
```

### Using AWS CLI

```bash
# Configure AWS CLI
aws configure set aws_access_key_id $ACCESS_KEY
aws configure set aws_secret_access_key $SECRET_KEY

# Create bucket
aws --endpoint-url http://localhost:8080 s3 mb s3://my-bucket

# Upload object
echo "Hello, Anode!" > test.txt
aws --endpoint-url http://localhost:8080 s3 cp test.txt s3://my-bucket/

# List objects
aws --endpoint-url http://localhost:8080 s3 ls s3://my-bucket/

# Download object
aws --endpoint-url http://localhost:8080 s3 cp s3://my-bucket/test.txt downloaded.txt
```

### Monitoring

#### Access Metrics

```bash
# Port forward to admin port
kubectl port-forward -n anode svc/anode 8081:8081

# Get metrics
curl http://localhost:8081/metrics
```

#### Check Cluster Health

```bash
# Check pod status
kubectl get pods -n anode -l app.kubernetes.io/name=anode

# Check logs
kubectl logs -n anode -l app.kubernetes.io/name=anode --tail=100 -f

# Check specific pod
kubectl logs -n anode anode-0 -f
```

## Operations

### Scaling

```bash
# Scale to 5 nodes
helm upgrade anode ./deploy/helm/anode -n anode --set replicaCount=5

# Or edit values.yaml and upgrade
helm upgrade anode ./deploy/helm/anode -n anode -f values.yaml
```

### Upgrading

```bash
# Update to new version
helm upgrade anode ./deploy/helm/anode -n anode --set image.tag=0.2.0

# Rollback if needed
helm rollback anode -n anode
```

### Backup and Restore

```bash
# Backup persistent volumes (example using Velero)
velero backup create anode-backup --include-namespaces anode

# Restore from backup
velero restore create --from-backup anode-backup
```

### Testing

```bash
# Run Helm tests
helm test anode -n anode

# Manual health check
kubectl run -it --rm debug --image=curlimages/curl --restart=Never -- \
  curl -f http://anode:8081/health/ready
```

## Troubleshooting

### Pods Not Starting

```bash
# Check pod events
kubectl describe pod -n anode anode-0

# Check logs
kubectl logs -n anode anode-0 --previous

# Check PVC status
kubectl get pvc -n anode
```

### Cluster Not Forming Quorum

```bash
# Check network connectivity between pods
kubectl exec -n anode anode-0 -- ping anode-1.anode-headless

# Check gRPC port accessibility
kubectl exec -n anode anode-0 -- nc -zv anode-1.anode-headless 9090

# Check logs for Raft errors
kubectl logs -n anode -l app.kubernetes.io/name=anode | grep -i raft
```

### High Memory Usage

```bash
# Check resource usage
kubectl top pods -n anode

# Reduce parquet cache size
helm upgrade anode ./deploy/helm/anode -n anode \
  --set config.parquet.cacheSizeMb=128

# Increase memory limits
helm upgrade anode ./deploy/helm/anode -n anode \
  --set resources.limits.memory=4Gi
```

## Uninstallation

```bash
# Uninstall release
helm uninstall anode -n anode

# Delete PVCs (if you want to delete data)
kubectl delete pvc -n anode -l app.kubernetes.io/name=anode

# Delete namespace
kubectl delete namespace anode
```

## Values Reference

See [values.yaml](values.yaml) for complete configuration options.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](https://github.com/anode/anode/blob/main/CONTRIBUTING.md) for details.

## License

This chart is licensed under Apache 2.0 / MIT dual license.

## Support

- Documentation: https://anode.io/docs
- Issues: https://github.com/anode/anode/issues
- Discussions: https://github.com/anode/anode/discussions
