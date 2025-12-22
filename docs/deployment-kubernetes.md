# Kubernetes Deployment Guide

This guide covers deploying Anode Object Storage on Kubernetes using Helm, from basic single-namespace deployments to production multi-cluster setups with full monitoring integration.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Helm Chart Installation](#helm-chart-installation)
- [Configuration Reference](#configuration-reference)
- [Persistent Volume Setup](#persistent-volume-setup)
- [Service Exposure](#service-exposure)
- [Cluster Sizing Recommendations](#cluster-sizing-recommendations)
- [Monitoring Integration](#monitoring-integration)
- [Upgrading and Rollback](#upgrading-and-rollback)
- [Troubleshooting Kubernetes Deployments](#troubleshooting-kubernetes-deployments)
- [Production Best Practices](#production-best-practices)
- [Advanced Topics](#advanced-topics)

## Prerequisites

### Required Tools

Install the following tools before deploying Anode:

**kubectl (Kubernetes CLI):**

```bash
# macOS
brew install kubectl

# Linux
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
sudo install -o root -g root -m 0755 kubectl /usr/local/bin/kubectl

# Windows (PowerShell)
curl -LO "https://dl.k8s.io/release/v1.28.0/bin/windows/amd64/kubectl.exe"

# Verify installation
kubectl version --client
```

**Helm 3.2.0+:**

```bash
# macOS
brew install helm

# Linux
curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash

# Windows (PowerShell)
choco install kubernetes-helm

# Verify installation
helm version
```

### Kubernetes Cluster Requirements

**Minimum cluster specifications:**

- Kubernetes 1.19+
- 3 worker nodes (for HA deployment)
- 8 CPU cores total (across all nodes)
- 16GB RAM total
- 100GB storage capacity
- Dynamic PersistentVolume provisioning (StorageClass configured)

**Recommended production specifications:**

- Kubernetes 1.24+
- 5+ worker nodes
- 32+ CPU cores total
- 64GB+ RAM total
- 500GB+ SSD/NVMe storage
- Network policy support
- Metrics server installed
- Ingress controller (nginx, traefik, or similar)
- Storage provisioner (CSI driver for cloud, local-path for on-prem)

### Verify Cluster Access

```bash
# Check cluster connectivity
kubectl cluster-info

# Verify node readiness
kubectl get nodes

# Check available resources
kubectl top nodes

# List available storage classes
kubectl get storageclass

# Verify metrics server (optional but recommended)
kubectl top nodes
```

## Quick Start

The fastest way to get Anode running on Kubernetes:

```bash
# Create namespace
kubectl create namespace anode

# Install Anode using Helm (from GitHub release)
helm install anode \
  https://github.com/anode/anode/releases/download/v0.1.0/anode-0.1.0.tgz \
  --namespace anode

# Or install from OCI registry (for v0.1.0+)
helm install anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode

# Watch pods come up
kubectl get pods -n anode -w

# Check deployment status
helm status anode -n anode
```

Once deployed, access the S3 API:

```bash
# Port forward to access locally
kubectl port-forward -n anode svc/anode 8080:8080

# Get credentials
export ACCESS_KEY=$(kubectl get secret -n anode anode-secret -o jsonpath="{.data.s3-access-key}" | base64 -d)
export SECRET_KEY=$(kubectl get secret -n anode anode-secret -o jsonpath="{.data.s3-secret-key}" | base64 -d)

echo "Access Key: $ACCESS_KEY"
echo "Secret Key: $SECRET_KEY"

# Test with AWS CLI
aws --endpoint-url http://localhost:8080 s3 mb s3://test-bucket
echo "Hello, Anode on Kubernetes!" > test.txt
aws --endpoint-url http://localhost:8080 s3 cp test.txt s3://test-bucket/
aws --endpoint-url http://localhost:8080 s3 ls s3://test-bucket/
```

## Helm Chart Installation

### Installation Methods

**Method 1: Install from GitHub Release (Recommended for Production)**

```bash
# Download and install specific version
helm install anode \
  https://github.com/anode/anode/releases/download/v0.1.0/anode-0.1.0.tgz \
  --namespace anode \
  --create-namespace \
  --values custom-values.yaml

# Verify deployment
kubectl get all -n anode
```

**Method 2: Install from OCI Registry (Available for v0.1.0+)**

```bash
# Install latest stable version
helm install anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --create-namespace

# With custom values
helm install anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --create-namespace \
  --values production-values.yaml
```

**Method 3: Install from Local Chart (Development)**

```bash
# Clone repository
git clone https://github.com/anode/anode.git
cd anode

# Install from local chart
helm install anode ./deploy/helm/anode \
  --namespace anode \
  --create-namespace \
  --values custom-values.yaml

# Or for development/testing
helm install anode ./deploy/helm/anode \
  --namespace anode \
  --create-namespace \
  --set replicaCount=3 \
  --set persistence.size=20Gi
```

### Basic Configuration Examples

**Minimal 3-node cluster:**

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
helm install anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --create-namespace \
  --values minimal-values.yaml
```

**Production 5-node cluster with monitoring:**

```yaml
# production-values.yaml
replicaCount: 5

image:
  tag: "0.1.0"

persistence:
  enabled: true
  storageClass: "fast-ssd"
  size: 100Gi

resources:
  requests:
    cpu: 1000m
    memory: 2Gi
  limits:
    cpu: 4000m
    memory: 8Gi

config:
  cluster:
    replicationFactor: 3
    placementGroups: 64
  storage:
    chunkSize: "4MB"
    syncWrites: true
  parquet:
    enableCache: true
    cacheSize: 536870912  # 512MB

monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s
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
helm install anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --create-namespace \
  --values production-values.yaml
```

## Configuration Reference

### Core Configuration Options

**Replica Configuration:**

| Parameter | Description | Default | Recommended |
|-----------|-------------|---------|-------------|
| `replicaCount` | Number of Anode pods (use odd numbers) | `3` | `3` (dev), `5` (prod), `7` (HA) |

**Image Configuration:**

| Parameter | Description | Default |
|-----------|-------------|---------|
| `image.repository` | Docker image repository | `anode/anode` |
| `image.tag` | Image tag (overrides appVersion) | `""` |
| `image.pullPolicy` | Image pull policy | `IfNotPresent` |
| `imagePullSecrets` | Secrets for private registries | `[]` |

**Resource Configuration:**

| Parameter | Description | Default | Production |
|-----------|-------------|---------|------------|
| `resources.requests.cpu` | CPU request | `250m` | `1000m-2000m` |
| `resources.requests.memory` | Memory request | `512Mi` | `2Gi-4Gi` |
| `resources.limits.cpu` | CPU limit | `1000m` | `4000m-8000m` |
| `resources.limits.memory` | Memory limit | `2Gi` | `8Gi-16Gi` |

### Anode-Specific Configuration

**Cluster Settings:**

```yaml
config:
  cluster:
    # Number of placement groups (affects data distribution)
    # Recommended: 16 * number of nodes
    placementGroups: 64

    # Replication factor (number of data copies)
    # Must be ≤ replicaCount
    replicationFactor: 3

    # Cluster ID (auto-generated if empty)
    clusterId: ""

    # Raft election timeout in milliseconds
    electionTimeoutMs: 1000

    # Raft heartbeat interval in milliseconds
    heartbeatIntervalMs: 250
```

**Storage Settings:**

```yaml
config:
  storage:
    # Chunk size for object splitting
    # Options: "1MB", "2MB", "4MB", "8MB", "16MB"
    chunkSize: "4MB"

    # Data directory (must match PVC mount)
    dataDir: /data

    # Fsync on writes (true = durable, false = faster)
    syncWrites: false

    # Metadata cache size in bytes
    metadataCacheSize: 104857600  # 100MB

    # Number of I/O worker threads (0 = auto-detect)
    ioThreads: 0
```

**S3 API Settings:**

```yaml
config:
  s3:
    # Maximum request body size (5GB default)
    maxBodySize: 5368709120

    # Request timeout in seconds
    requestTimeoutSecs: 300

    # Enable multipart uploads
    enableMultipart: true

    # Minimum part size (5MB per S3 spec)
    minPartSize: 5242880

    # Virtual host style vs path style
    virtualHostStyle: false
```

**Parquet Optimizations:**

```yaml
config:
  parquet:
    # Enable Parquet metadata caching
    enableCache: true

    # Cache size in bytes (256MB default)
    cacheSize: 268435456

    # Rows per row group
    rowGroupSize: 100000

    # Enable query optimizations
    enablePredicatePushdown: true
    enableColumnPruning: true
```

**Raft Consensus Settings:**

```yaml
config:
  raft:
    # Snapshot after N log entries
    snapshotInterval: 10000

    # Maximum log entries in memory
    maxLogEntries: 100000

    # Trigger compaction after N entries
    compactionThreshold: 50000

    # Enable log compaction
    enableCompaction: true
```

### Service Configuration

```yaml
service:
  type: ClusterIP  # ClusterIP, NodePort, or LoadBalancer
  s3Port: 8080     # S3 API port
  adminPort: 8081  # Admin/metrics API port
  grpcPort: 9090   # Inter-node gRPC port
  annotations: {}
  # sessionAffinity: ClientIP  # Optional: sticky sessions
```

### Health Check Configuration

```yaml
# Liveness probe - is the container alive?
livenessProbe:
  httpGet:
    path: /health/live
    port: admin
  initialDelaySeconds: 30
  periodSeconds: 10
  timeoutSeconds: 5
  failureThreshold: 3

# Readiness probe - can it serve traffic?
readinessProbe:
  httpGet:
    path: /health/ready
    port: admin
  initialDelaySeconds: 10
  periodSeconds: 5
  timeoutSeconds: 3
  failureThreshold: 3

# Startup probe - for slow-starting containers
startupProbe:
  httpGet:
    path: /health/startup
    port: admin
  initialDelaySeconds: 0
  periodSeconds: 5
  timeoutSeconds: 3
  failureThreshold: 30  # 150 seconds total
```

## Persistent Volume Setup

Anode requires persistent storage to maintain data across pod restarts.

### Storage Class Configuration

**Using Default Storage Class:**

```yaml
# values.yaml
persistence:
  enabled: true
  storageClass: ""  # Uses cluster default
  size: 10Gi
  accessModes:
    - ReadWriteOnce
```

**Using Specific Storage Class:**

```yaml
# values.yaml
persistence:
  enabled: true
  storageClass: "fast-ssd"  # Cloud provider SSD
  size: 100Gi
  accessModes:
    - ReadWriteOnce
```

### Common Storage Classes by Provider

**AWS EKS:**

```yaml
persistence:
  storageClass: "gp3"  # General Purpose SSD v3
  # or "io2" for high-performance IOPS
  size: 100Gi
```

**Google GKE:**

```yaml
persistence:
  storageClass: "standard-rwo"  # Balanced persistent disk
  # or "premium-rwo" for SSD
  size: 100Gi
```

**Azure AKS:**

```yaml
persistence:
  storageClass: "managed-premium"  # Premium SSD
  # or "managed" for standard
  size: 100Gi
```

**On-Premise / Bare Metal:**

```yaml
# Using local-path provisioner
persistence:
  storageClass: "local-path"
  size: 100Gi

# Or using Longhorn
persistence:
  storageClass: "longhorn"
  size: 100Gi

# Or using Rook Ceph
persistence:
  storageClass: "rook-ceph-block"
  size: 100Gi
```

### Verifying Storage Setup

```bash
# List available storage classes
kubectl get storageclass

# Verify PVCs are bound
kubectl get pvc -n anode

# Expected output:
# NAME           STATUS   VOLUME                   CAPACITY   ACCESS MODES   STORAGECLASS
# data-anode-0   Bound    pvc-abc123...            10Gi       RWO            fast-ssd
# data-anode-1   Bound    pvc-def456...            10Gi       RWO            fast-ssd
# data-anode-2   Bound    pvc-ghi789...            10Gi       RWO            fast-ssd

# Check PV details
kubectl get pv

# Describe specific PVC
kubectl describe pvc -n anode data-anode-0
```

### Storage Sizing Guidelines

| Workload Type | Per-Node Size | Total Cluster | Storage Class |
|---------------|---------------|---------------|---------------|
| Development | 10-20Gi | 30-60Gi | Standard/Default |
| Small Production | 50-100Gi | 150-300Gi | SSD (gp3, standard-rwo) |
| Medium Production | 200-500Gi | 600-1500Gi | Premium SSD (io2, premium-rwo) |
| Large Production | 1-5Ti | 3-15Ti | High IOPS SSD |

**Calculation formula:**

```
Total Storage = (Data Size * Replication Factor) + 30% overhead
Per-Node Storage = Total Storage / Number of Nodes
```

### Expanding Persistent Volumes

```bash
# Check if storage class supports expansion
kubectl get storageclass fast-ssd -o yaml | grep allowVolumeExpansion

# If true, edit PVC to increase size
kubectl edit pvc -n anode data-anode-0

# Change spec.resources.requests.storage to new size
# Then restart the pod
kubectl delete pod -n anode anode-0

# Verify expansion
kubectl get pvc -n anode data-anode-0
```

## Service Exposure

Anode can be exposed in several ways depending on your requirements.

### ClusterIP (Default)

Internal cluster access only:

```yaml
# values.yaml
service:
  type: ClusterIP
  s3Port: 8080
  adminPort: 8081
```

**Access via port-forward:**

```bash
# S3 API
kubectl port-forward -n anode svc/anode 8080:8080

# Admin API
kubectl port-forward -n anode svc/anode 8081:8081

# Use from local machine
aws --endpoint-url http://localhost:8080 s3 ls
```

**Access from within cluster:**

```bash
# From other pods
kubectl run -it --rm test --image=curlimages/curl -- sh
curl http://anode.anode.svc.cluster.local:8080
```

### NodePort

Access via any node's IP:

```yaml
# values.yaml
service:
  type: NodePort
  s3Port: 8080
  adminPort: 8081
```

```bash
# Get NodePort assigned
kubectl get svc -n anode anode

# Access via node IP
export NODE_IP=$(kubectl get nodes -o jsonpath='{.items[0].status.addresses[?(@.type=="ExternalIP")].address}')
export NODE_PORT=$(kubectl get svc -n anode anode -o jsonpath='{.spec.ports[?(@.name=="s3")].nodePort}')

echo "S3 API: http://$NODE_IP:$NODE_PORT"
aws --endpoint-url http://$NODE_IP:$NODE_PORT s3 ls
```

### LoadBalancer

Cloud provider load balancer:

```yaml
# values.yaml
service:
  type: LoadBalancer
  s3Port: 8080
  adminPort: 8081
  annotations:
    # AWS
    service.beta.kubernetes.io/aws-load-balancer-type: "nlb"
    # GCP
    cloud.google.com/load-balancer-type: "External"
    # Azure
    service.beta.kubernetes.io/azure-load-balancer-internal: "false"
```

```bash
# Wait for external IP
kubectl get svc -n anode anode -w

# Get external IP/hostname
export LB_IP=$(kubectl get svc -n anode anode -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
export LB_HOSTNAME=$(kubectl get svc -n anode anode -o jsonpath='{.status.loadBalancer.ingress[0].hostname}')

# Access S3 API
aws --endpoint-url http://$LB_IP:8080 s3 ls
# or
aws --endpoint-url http://$LB_HOSTNAME:8080 s3 ls
```

### Ingress

HTTP(S) access with hostname:

```yaml
# values.yaml
ingress:
  enabled: true
  className: nginx  # or traefik, alb, etc.
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/proxy-body-size: "0"
    nginx.ingress.kubernetes.io/proxy-request-buffering: "off"
    nginx.ingress.kubernetes.io/backend-protocol: "HTTP"
  hosts:
    - host: s3.example.com
      paths:
        - path: /
          pathType: Prefix
  tls:
    - secretName: anode-tls
      hosts:
        - s3.example.com

# Optional: TLS with cert-manager
tls:
  enabled: true
  certManager:
    enabled: true
    issuerName: letsencrypt-prod
    issuerKind: ClusterIssuer
```

```bash
# Install ingress
helm install anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --create-namespace \
  --values ingress-values.yaml

# Wait for certificate
kubectl get certificate -n anode

# Test access
curl https://s3.example.com
aws --endpoint-url https://s3.example.com s3 ls
```

**Ingress with AWS ALB Controller:**

```yaml
ingress:
  enabled: true
  className: alb
  annotations:
    alb.ingress.kubernetes.io/scheme: internet-facing
    alb.ingress.kubernetes.io/target-type: ip
    alb.ingress.kubernetes.io/listen-ports: '[{"HTTP": 80}, {"HTTPS": 443}]'
    alb.ingress.kubernetes.io/ssl-redirect: "443"
  hosts:
    - host: s3.example.com
      paths:
        - path: /
          pathType: Prefix
```

**Ingress with Google Cloud Load Balancer:**

```yaml
ingress:
  enabled: true
  className: gce
  annotations:
    kubernetes.io/ingress.global-static-ip-name: "anode-ip"
    networking.gke.io/managed-certificates: "anode-cert"
  hosts:
    - host: s3.example.com
      paths:
        - path: /
          pathType: Prefix
```

## Cluster Sizing Recommendations

### Development / Testing

**Single developer or CI/CD:**

```yaml
replicaCount: 3

resources:
  requests:
    cpu: 100m
    memory: 256Mi
  limits:
    cpu: 500m
    memory: 1Gi

persistence:
  size: 10Gi

config:
  cluster:
    replicationFactor: 2
    placementGroups: 16
```

**Total requirements:**
- 3 nodes (can share with other workloads)
- 1.5 CPU cores
- 3GB RAM
- 30GB storage

### Small Production

**100GB data, 1000 objects/day:**

```yaml
replicaCount: 3

resources:
  requests:
    cpu: 500m
    memory: 1Gi
  limits:
    cpu: 2000m
    memory: 4Gi

persistence:
  size: 50Gi
  storageClass: "fast-ssd"

config:
  cluster:
    replicationFactor: 3
    placementGroups: 32
  storage:
    metadataCacheSize: 268435456  # 256MB
  parquet:
    cacheSize: 536870912  # 512MB
```

**Total requirements:**
- 3 dedicated nodes
- 6 CPU cores
- 12GB RAM
- 150GB SSD storage

### Medium Production

**1TB data, 10,000 objects/day:**

```yaml
replicaCount: 5

resources:
  requests:
    cpu: 1000m
    memory: 2Gi
  limits:
    cpu: 4000m
    memory: 8Gi

persistence:
  size: 250Gi
  storageClass: "premium-ssd"

config:
  cluster:
    replicationFactor: 3
    placementGroups: 64
  storage:
    metadataCacheSize: 536870912  # 512MB
    ioThreads: 8
  parquet:
    cacheSize: 1073741824  # 1GB

podDisruptionBudget:
  enabled: true
  minAvailable: 3
```

**Total requirements:**
- 5 dedicated nodes
- 20 CPU cores
- 40GB RAM
- 1.25TB premium SSD storage

### Large Production

**10TB+ data, 100,000 objects/day:**

```yaml
replicaCount: 7

resources:
  requests:
    cpu: 2000m
    memory: 4Gi
  limits:
    cpu: 8000m
    memory: 16Gi

persistence:
  size: 2Ti
  storageClass: "high-iops-ssd"

config:
  cluster:
    replicationFactor: 3
    placementGroups: 128
  storage:
    metadataCacheSize: 2147483648  # 2GB
    ioThreads: 16
  parquet:
    cacheSize: 4294967296  # 4GB

podDisruptionBudget:
  enabled: true
  minAvailable: 5

affinity:
  podAntiAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
    - labelSelector:
        matchLabels:
          app.kubernetes.io/name: anode
      topologyKey: kubernetes.io/hostname

topologySpreadConstraints:
  - maxSkew: 1
    topologyKey: topology.kubernetes.io/zone
    whenUnsatisfiable: DoNotSchedule
    labelSelector:
      matchLabels:
        app.kubernetes.io/name: anode
```

**Total requirements:**
- 7+ dedicated nodes (spread across 3 availability zones)
- 56+ CPU cores
- 112GB+ RAM
- 14TB high-IOPS SSD storage

## Monitoring Integration

Anode integrates seamlessly with Prometheus and Grafana for comprehensive monitoring.

### Prerequisites

Install Prometheus Operator:

```bash
# Add prometheus-community helm repo
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo update

# Install kube-prometheus-stack
helm install prometheus prometheus-community/kube-prometheus-stack \
  --namespace monitoring \
  --create-namespace \
  --set prometheus.prometheusSpec.serviceMonitorSelectorNilUsesHelmValues=false
```

### Enabling ServiceMonitor

```yaml
# values.yaml
monitoring:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 30s
    scrapeTimeout: 10s
    labels:
      release: prometheus  # Match your Prometheus label selector
    # Optional: scrape from different namespace
    # namespace: monitoring
```

Deploy with monitoring:

```bash
helm install anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --create-namespace \
  --set monitoring.serviceMonitor.enabled=true \
  --set monitoring.serviceMonitor.labels.release=prometheus
```

Verify ServiceMonitor:

```bash
# Check ServiceMonitor created
kubectl get servicemonitor -n anode

# Check Prometheus targets
kubectl port-forward -n monitoring svc/prometheus-kube-prometheus-prometheus 9090:9090

# Open http://localhost:9090/targets
# Look for anode/anode/* targets
```

### Enabling PrometheusRules (Alerts)

```yaml
# values.yaml
monitoring:
  enabled: true
  prometheusRule:
    enabled: true
    labels:
      release: prometheus
    # Optional: add custom alerts
    additionalRules:
      - alert: AnodeHighErrorRate
        expr: rate(anode_http_requests_total{status=~"5.."}[5m]) > 0.05
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High error rate on Anode S3 API"
          description: "Error rate is {{ $value }} requests/sec"
```

Deploy with alerts:

```bash
helm upgrade anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --reuse-values \
  --set monitoring.prometheusRule.enabled=true \
  --set monitoring.prometheusRule.labels.release=prometheus
```

### Built-in Alert Rules

The Anode Helm chart includes these alert rules:

**AnodeDown:**
- Fires when: Anode pod is down for 5 minutes
- Severity: critical

**AnodeLowReplicas:**
- Fires when: Less than minimum replicas available
- Severity: warning

**AnodeHighMemoryUsage:**
- Fires when: Memory usage > 90% for 10 minutes
- Severity: warning

**AnodeHighCPUUsage:**
- Fires when: CPU usage > 80% for 15 minutes
- Severity: warning

**AnodePersistentVolumeNearFull:**
- Fires when: PV usage > 85%
- Severity: warning

**AnodeRaftLeaderChanges:**
- Fires when: More than 3 leader changes in 10 minutes
- Severity: warning

### Key Metrics to Monitor

**Cluster Health:**

```promql
# Number of healthy replicas
count(up{job="anode"} == 1)

# Raft leader status (1 = leader, 0 = follower)
anode_raft_is_leader

# Cluster member count
anode_cluster_members_total
```

**Storage Metrics:**

```promql
# Total objects stored
anode_storage_objects_total

# Total bytes stored
anode_storage_bytes_total

# Storage operations rate
rate(anode_storage_operations_total[5m])
```

**Performance Metrics:**

```promql
# HTTP request rate
rate(anode_http_requests_total[5m])

# Request latency (p95)
histogram_quantile(0.95, rate(anode_http_request_duration_seconds_bucket[5m]))

# Error rate
rate(anode_http_requests_total{status=~"5.."}[5m])
```

**Raft Metrics:**

```promql
# Raft commit latency
anode_raft_commit_duration_seconds

# Raft log size
anode_raft_log_entries

# Leader changes
rate(anode_raft_leader_changes_total[10m])
```

### Grafana Dashboard

Access Grafana:

```bash
# Port forward to Grafana
kubectl port-forward -n monitoring svc/prometheus-grafana 3000:80

# Open http://localhost:3000
# Login: admin / prom-operator (default)
```

Import Anode dashboard (if available) or create custom dashboard with panels for:

1. Cluster status and health
2. Storage capacity and usage
3. Request throughput and latency
4. Error rates
5. Raft consensus status
6. Resource utilization (CPU, memory, disk)

## Upgrading and Rollback

### Upgrading Anode

**Check current version:**

```bash
# Get current chart version
helm list -n anode

# Get current image version
kubectl get statefulset -n anode anode -o jsonpath='{.spec.template.spec.containers[0].image}'
```

**Upgrade to new version:**

```bash
# Fetch latest chart
helm repo update  # if using helm repo

# Upgrade to specific version
helm upgrade anode oci://ghcr.io/anode/charts/anode \
  --version 0.2.0 \
  --namespace anode \
  --reuse-values

# Or upgrade with new values
helm upgrade anode oci://ghcr.io/anode/charts/anode \
  --version 0.2.0 \
  --namespace anode \
  --values production-values.yaml
```

**Monitor upgrade progress:**

```bash
# Watch rollout status
kubectl rollout status statefulset/anode -n anode

# Watch pods
kubectl get pods -n anode -w

# Check logs
kubectl logs -n anode -l app.kubernetes.io/name=anode --tail=50 -f
```

### Rolling Update Strategy

The default update strategy performs zero-downtime rolling updates:

```yaml
# values.yaml
updateStrategy:
  type: RollingUpdate
  rollingUpdate:
    partition: 0  # Update all pods

# Pod management
podManagementPolicy: OrderedReady  # Update one at a time

# Pod disruption budget
podDisruptionBudget:
  enabled: true
  minAvailable: 2  # Keep at least 2 pods available
```

**How rolling updates work:**

1. Update starts from highest ordinal (e.g., anode-2, then anode-1, then anode-0)
2. Pod is gracefully terminated (30s grace period)
3. New pod is created with new image/config
4. New pod must pass readiness probe before next pod updates
5. At least `minAvailable` pods remain healthy during update

### Pausing and Resuming Upgrades

```bash
# Pause upgrade at specific partition
kubectl patch statefulset anode -n anode -p '{"spec":{"updateStrategy":{"rollingUpdate":{"partition":2}}}}'

# Only pods with ordinal >= 2 will be updated
# anode-2 updates, anode-0 and anode-1 remain on old version

# Resume upgrade (update all)
kubectl patch statefulset anode -n anode -p '{"spec":{"updateStrategy":{"rollingUpdate":{"partition":0}}}}'
```

### Rollback to Previous Version

**Using Helm:**

```bash
# View revision history
helm history anode -n anode

# Rollback to previous release
helm rollback anode -n anode

# Rollback to specific revision
helm rollback anode 3 -n anode

# Verify rollback
kubectl get pods -n anode
kubectl get statefulset -n anode anode -o jsonpath='{.spec.template.spec.containers[0].image}'
```

**Manual rollback:**

```bash
# Update image to previous version
kubectl set image statefulset/anode -n anode anode=anode/anode:0.1.0

# Or edit StatefulSet directly
kubectl edit statefulset anode -n anode

# Watch rollout
kubectl rollout status statefulset/anode -n anode
```

### Backup Before Upgrade

Always backup before major upgrades:

```bash
# Create snapshot of PVs (cloud provider specific)
# AWS EBS:
aws ec2 create-snapshot --volume-id vol-xxx --description "anode-pre-upgrade"

# Or backup data using Velero
velero backup create anode-pre-upgrade-backup \
  --include-namespaces anode \
  --wait

# Or export critical data
kubectl exec -n anode anode-0 -- tar czf /tmp/backup.tar.gz /data
kubectl cp anode/anode-0:/tmp/backup.tar.gz ./anode-backup.tar.gz
```

## Troubleshooting Kubernetes Deployments

### Pods Not Starting

**Check pod status:**

```bash
# View pod status
kubectl get pods -n anode

# Describe pod for events
kubectl describe pod -n anode anode-0

# Common issues to look for:
# - ImagePullBackOff: Image doesn't exist or auth failed
# - Pending: Resource constraints or PVC not bound
# - CrashLoopBackOff: Application crashes on startup
```

**Solution for ImagePullBackOff:**

```bash
# Verify image exists
docker pull anode/anode:0.1.0

# Check image pull secrets
kubectl get secrets -n anode

# Add image pull secret if needed
kubectl create secret docker-registry regcred \
  --docker-server=ghcr.io \
  --docker-username=your-username \
  --docker-password=your-token \
  -n anode

# Update values
helm upgrade anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --reuse-values \
  --set imagePullSecrets[0].name=regcred
```

**Solution for Pending pods:**

```bash
# Check node resources
kubectl describe node | grep -A 5 "Allocated resources"

# Check PVC status
kubectl get pvc -n anode

# If PVC pending, check storage class exists
kubectl get storageclass

# Check PV provisioner logs
kubectl logs -n kube-system -l app=<provisioner-name>
```

**Solution for CrashLoopBackOff:**

```bash
# View logs
kubectl logs -n anode anode-0

# View previous crash logs
kubectl logs -n anode anode-0 --previous

# Common causes:
# - Configuration error
# - Permission issues on /data
# - Resource limits too low
# - Failed health checks
```

### Cluster Not Forming Quorum

**Check pod connectivity:**

```bash
# Test DNS resolution
kubectl exec -n anode anode-0 -- nslookup anode-headless

# Test gRPC port connectivity
kubectl exec -n anode anode-1 -- nc -zv anode-0.anode-headless 9090

# Check headless service
kubectl get svc -n anode anode-headless
kubectl get endpoints -n anode anode-headless
```

**Check cluster membership:**

```bash
# Port forward to admin API
kubectl port-forward -n anode anode-0 8081:8081

# Check cluster members
curl http://localhost:8081/admin/cluster/members

# Check Raft status
curl http://localhost:8081/admin/raft/status
```

**Solution - Reset cluster:**

```bash
# Delete all pods (StatefulSet will recreate)
kubectl delete pod -n anode -l app.kubernetes.io/name=anode

# Or delete and recreate StatefulSet
kubectl delete statefulset -n anode anode
helm upgrade anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --reuse-values

# If persistent, delete PVCs (WARNING: DATA LOSS)
kubectl delete pvc -n anode -l app.kubernetes.io/name=anode
```

### High Memory Usage / OOMKilled

**Check resource usage:**

```bash
# Check current usage
kubectl top pod -n anode

# Check for OOM events
kubectl get events -n anode | grep OOM

# View resource limits
kubectl describe pod -n anode anode-0 | grep -A 5 "Limits"
```

**Solution - Increase limits:**

```bash
# Increase memory limits
helm upgrade anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --reuse-values \
  --set resources.limits.memory=4Gi

# Or reduce cache sizes
helm upgrade anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --reuse-values \
  --set config.storage.metadataCacheSize=134217728 \
  --set config.parquet.cacheSize=268435456
```

### PersistentVolume Issues

**PVC stuck in Pending:**

```bash
# Check PVC events
kubectl describe pvc -n anode data-anode-0

# Common issues:
# - No storage class available
# - Storage class doesn't support dynamic provisioning
# - Node selector doesn't match any nodes
# - Insufficient storage capacity
```

**Solution:**

```bash
# Create storage class if missing
kubectl apply -f - <<EOF
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: fast-ssd
provisioner: kubernetes.io/gce-pd  # or appropriate provisioner
parameters:
  type: pd-ssd
  replication-type: regional-pd
allowVolumeExpansion: true
EOF

# Update helm to use storage class
helm upgrade anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --reuse-values \
  --set persistence.storageClass=fast-ssd
```

**PV permission errors:**

```bash
# Check logs for permission denied
kubectl logs -n anode anode-0 | grep "permission denied"

# Solution: init container should fix permissions
# Verify init container ran successfully
kubectl describe pod -n anode anode-0 | grep -A 10 "Init Containers"

# If needed, manually fix permissions
kubectl exec -n anode anode-0 -- chown -R 1000:1000 /data
```

### Network Policy Issues

**Symptoms:**
- Pods can't communicate
- Health checks failing
- Cluster formation problems

```bash
# Check if network policies exist
kubectl get networkpolicies -n anode

# Describe network policy
kubectl describe networkpolicy -n anode

# Test connectivity
kubectl run -it --rm debug --image=nicolaka/netshoot -n anode -- bash
# From debug pod:
curl http://anode-0.anode-headless:8081/health
nc -zv anode-0.anode-headless 9090
```

**Solution:**

```bash
# Disable network policy temporarily
helm upgrade anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --reuse-values \
  --set networkPolicy.enabled=false

# Or adjust network policy rules
kubectl edit networkpolicy -n anode
```

### Service Mesh Compatibility

**Issues with Istio/Linkerd:**

```bash
# Disable mTLS for gRPC port
kubectl annotate pod -n anode anode-0 traffic.sidecar.istio.io/excludeOutboundPorts="9090"

# Or exclude entire namespace
kubectl label namespace anode istio-injection=disabled

# For Linkerd, skip gRPC port
kubectl annotate pod -n anode anode-0 config.linkerd.io/skip-outbound-ports="9090"
```

### Debugging with Ephemeral Containers

```bash
# Add debug container to running pod
kubectl debug -it -n anode anode-0 --image=nicolaka/netshoot --target=anode

# Inside debug container:
ps aux
netstat -tuln
curl http://localhost:8081/health
```

## Production Best Practices

### Security Hardening

**Use Pod Security Standards:**

```yaml
# Apply to namespace
apiVersion: v1
kind: Namespace
metadata:
  name: anode
  labels:
    pod-security.kubernetes.io/enforce: restricted
    pod-security.kubernetes.io/audit: restricted
    pod-security.kubernetes.io/warn: restricted
```

**Rotate credentials regularly:**

```bash
# Generate new S3 credentials
kubectl create secret generic anode-secret-new \
  --from-literal=s3-access-key=$(openssl rand -hex 16) \
  --from-literal=s3-secret-key=$(openssl rand -hex 32) \
  -n anode

# Update secret reference in Helm
helm upgrade anode oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --reuse-values

# Rotate and restart pods
kubectl rollout restart statefulset/anode -n anode
```

**Use NetworkPolicies:**

```yaml
# values.yaml
networkPolicy:
  enabled: true
  ingress:
    # Allow gRPC only from other anode pods
    - from:
      - podSelector:
          matchLabels:
            app.kubernetes.io/name: anode
      ports:
      - protocol: TCP
        port: 9090
    # Allow S3 API from anywhere
    - from: []
      ports:
      - protocol: TCP
        port: 8080
```

### High Availability Configuration

```yaml
# values.yaml
# Minimum 5 nodes for true HA
replicaCount: 5

# Prevent multiple pods on same node
affinity:
  podAntiAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
    - labelSelector:
        matchLabels:
          app.kubernetes.io/name: anode
      topologyKey: kubernetes.io/hostname

# Spread across zones
topologySpreadConstraints:
  - maxSkew: 1
    topologyKey: topology.kubernetes.io/zone
    whenUnsatisfiable: DoNotSchedule
    labelSelector:
      matchLabels:
        app.kubernetes.io/name: anode

# Ensure minimum availability during updates
podDisruptionBudget:
  enabled: true
  minAvailable: 3

# Graceful termination
terminationGracePeriodSeconds: 60

# Lifecycle hooks for graceful shutdown
lifecycle:
  preStop:
    exec:
      command:
        - /bin/sh
        - -c
        - sleep 30  # Allow time for connections to drain
```

### Resource Quotas and Limits

```yaml
# Create resource quota for namespace
apiVersion: v1
kind: ResourceQuota
metadata:
  name: anode-quota
  namespace: anode
spec:
  hard:
    requests.cpu: "20"
    requests.memory: "40Gi"
    requests.storage: "1Ti"
    persistentvolumeclaims: "10"
    pods: "10"
```

### Backup and Disaster Recovery

**Automated backup with CronJob:**

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: anode-backup
  namespace: anode
spec:
  schedule: "0 2 * * *"  # Daily at 2 AM
  jobTemplate:
    spec:
      template:
        spec:
          containers:
          - name: backup
            image: amazon/aws-cli
            command:
            - /bin/sh
            - -c
            - |
              # Backup to S3
              aws s3 sync /data s3://backups/anode/$(date +%Y%m%d)/
            volumeMounts:
            - name: data
              mountPath: /data
              readOnly: true
          restartPolicy: OnFailure
          volumes:
          - name: data
            persistentVolumeClaim:
              claimName: data-anode-0
```

**Using Velero for cluster backups:**

```bash
# Install Velero
velero install \
  --provider aws \
  --plugins velero/velero-plugin-for-aws:v1.8.0 \
  --bucket velero-backups \
  --backup-location-config region=us-east-1

# Create backup schedule
velero schedule create anode-daily \
  --schedule="0 2 * * *" \
  --include-namespaces anode \
  --ttl 168h0m0s

# Manual backup
velero backup create anode-manual --include-namespaces anode

# Restore from backup
velero restore create --from-backup anode-manual
```

## Advanced Topics

### Multi-Cluster Deployments

Deploy Anode across multiple Kubernetes clusters for geo-distribution:

```bash
# Cluster 1 (us-east)
kubectl config use-context us-east-cluster
helm install anode-east oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --create-namespace \
  --set config.cluster.clusterId=us-east

# Cluster 2 (eu-west)
kubectl config use-context eu-west-cluster
helm install anode-west oci://ghcr.io/anode/charts/anode \
  --version 0.1.0 \
  --namespace anode \
  --create-namespace \
  --set config.cluster.clusterId=eu-west
```

### Using Kustomize with Helm

```bash
# Create kustomization
cat <<EOF > kustomization.yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization

helmCharts:
- name: anode
  repo: oci://ghcr.io/anode/charts
  version: 0.1.0
  releaseName: anode
  namespace: anode
  valuesFile: production-values.yaml

patches:
- target:
    kind: StatefulSet
    name: anode
  patch: |-
    - op: add
      path: /spec/template/metadata/annotations/custom
      value: "custom-annotation"
EOF

# Apply with kustomize
kubectl apply -k .
```

### Horizontal Pod Autoscaling

Note: HPA for StatefulSets requires Kubernetes 1.23+

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: anode
  namespace: anode
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: StatefulSet
    name: anode
  minReplicas: 3
  maxReplicas: 9
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 300
      policies:
      - type: Pods
        value: 2
        periodSeconds: 60
    scaleDown:
      stabilizationWindowSeconds: 600
      policies:
      - type: Pods
        value: 1
        periodSeconds: 120
```

### Using with ArgoCD

```yaml
# argocd-application.yaml
apiVersion: argoproj.io/v1alpha1
kind: Application
metadata:
  name: anode
  namespace: argocd
spec:
  project: default
  source:
    chart: anode
    repoURL: oci://ghcr.io/anode/charts
    targetRevision: 0.1.0
    helm:
      valueFiles:
      - values-production.yaml
  destination:
    server: https://kubernetes.default.svc
    namespace: anode
  syncPolicy:
    automated:
      prune: true
      selfHeal: true
    syncOptions:
    - CreateNamespace=true
```

### Integration with External Secrets Operator

```yaml
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: anode-credentials
  namespace: anode
spec:
  refreshInterval: 1h
  secretStoreRef:
    name: aws-secrets-manager
    kind: SecretStore
  target:
    name: anode-secret
    creationPolicy: Owner
  data:
  - secretKey: s3-access-key
    remoteRef:
      key: anode/credentials
      property: access_key
  - secretKey: s3-secret-key
    remoteRef:
      key: anode/credentials
      property: secret_key
```

## Additional Resources

- [Helm Chart README](../deploy/helm/anode/README.md) - Detailed chart documentation
- [Docker Deployment Guide](./deployment-docker.md) - Docker and Docker Compose deployments
- [Standalone Deployment Guide](./deployment-standalone.md) - Non-containerized deployments
- [Operations Guide](./operations.md) - Day-2 operations
- [Monitoring Guide](./monitoring.md) - Advanced monitoring setup
- [Security Guide](./security.md) - Security best practices
- [Kubernetes Documentation](https://kubernetes.io/docs/) - Official Kubernetes docs
- [Helm Documentation](https://helm.sh/docs/) - Official Helm docs

## Getting Help

If you encounter issues:

1. Check pod logs: `kubectl logs -n anode -l app.kubernetes.io/name=anode`
2. Review this troubleshooting guide
3. Check pod events: `kubectl describe pod -n anode anode-0`
4. Search existing issues: https://github.com/anode/anode/issues
5. Join discussions: https://github.com/anode/anode/discussions
6. Open a new issue with:
   - Kubernetes version: `kubectl version`
   - Helm version: `helm version`
   - Helm values used
   - Full error logs and events
   - Steps to reproduce

For security issues, email security@anode.io instead of opening a public issue.
