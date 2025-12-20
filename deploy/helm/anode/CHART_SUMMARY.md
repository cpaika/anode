# Anode Helm Chart - Implementation Summary

## Overview

This is a production-ready Helm chart for deploying Anode, a distributed object storage system with S3 API compatibility, Raft-based consensus, and Parquet-aware optimizations.

## Chart Structure

```
anode/
├── Chart.yaml                      # Chart metadata
├── values.yaml                     # Default configuration values
├── values.schema.json              # JSON schema for values validation
├── values-production.yaml          # Production configuration example
├── values-development.yaml         # Development configuration example
├── values-minimal.yaml             # Minimal configuration example
├── README.md                       # Comprehensive documentation
├── QUICKSTART.md                   # Quick start guide
├── CHANGELOG.md                    # Version history
├── CHART_SUMMARY.md               # This file
├── .helmignore                     # Files to ignore when packaging
├── install.sh                      # Installation helper script
├── upgrade.sh                      # Upgrade helper script
├── validate.sh                     # Validation helper script
└── templates/
    ├── _helpers.tpl                # Template helpers
    ├── statefulset.yaml            # Main StatefulSet definition
    ├── service.yaml                # Services (headless + load-balancing)
    ├── configmap.yaml              # Configuration (anode.toml)
    ├── secret.yaml                 # Secrets (S3 credentials)
    ├── serviceaccount.yaml         # Service account
    ├── poddisruptionbudget.yaml    # PDB for high availability
    ├── ingress.yaml                # Ingress for external access
    ├── networkpolicy.yaml          # Network policies for security
    ├── servicemonitor.yaml         # Prometheus ServiceMonitor
    ├── prometheusrule.yaml         # Prometheus alerting rules
    ├── NOTES.txt                   # Post-install instructions
    └── tests/
        └── test-connection.yaml    # Helm test pod
```

## Key Features

### 1. Production-Ready Deployment
- **StatefulSet**: Ordered deployment with stable network identities
- **Persistent Storage**: Configurable PVCs with storage class support
- **High Availability**: Pod anti-affinity, PodDisruptionBudget
- **Resource Management**: CPU/memory requests and limits
- **Security**: Non-root containers, read-only root filesystem, dropped capabilities

### 2. Comprehensive Configuration
- **Cluster Settings**: Placement groups, replication factor, Raft tuning
- **Storage Settings**: Chunk size, sync policies, direct I/O
- **S3 API**: Virtual/path-style hosting, multipart uploads
- **Parquet**: Metadata caching, auto-detection
- **Logging & Metrics**: Structured logging, Prometheus metrics

### 3. Kubernetes-Native
- **Services**: Both headless (for StatefulSet DNS) and load-balancing
- **ConfigMaps**: Templated anode.toml configuration
- **Secrets**: Secure credential management
- **Health Checks**: Startup, liveness, and readiness probes
- **Graceful Shutdown**: PreStop hooks, termination grace period

### 4. Monitoring & Observability
- **Metrics Endpoint**: Prometheus-compatible metrics on admin port
- **ServiceMonitor**: Automatic integration with Prometheus Operator
- **Alerting Rules**: Pre-configured alerts for common issues
- **Health Endpoints**: /health/live, /health/ready, /health/startup

### 5. Network & Security
- **Network Policies**: Restrict ingress/egress traffic
- **Ingress**: External access with TLS support
- **Service Types**: ClusterIP, NodePort, LoadBalancer
- **Pod Security**: Security context, service account, RBAC

### 6. Operational Tools
- **Helm Tests**: Automated connection tests
- **Install Script**: Interactive installation with validation
- **Upgrade Script**: Safe upgrades with health checks
- **Validate Script**: Post-deployment validation suite

## Template Helpers (_helpers.tpl)

### Standard Helpers
- `anode.name`: Chart name
- `anode.fullname`: Full release name
- `anode.chart`: Chart name and version
- `anode.labels`: Common labels
- `anode.selectorLabels`: Pod selector labels
- `anode.serviceAccountName`: Service account name
- `anode.image`: Full image reference

### Custom Helpers
- `anode.headlessServiceName`: Headless service name
- `anode.configMapName`: ConfigMap name
- `anode.secretName`: Secret name
- `anode.imageTag`: Image tag with fallback
- `anode.s3SecretKey`: Generate S3 secret key if not provided
- `anode.clusterId`: Generate cluster ID if not provided
- `anode.nodeAddress`: Node DNS address
- `anode.podAntiAffinity`: Default pod anti-affinity rules

## StatefulSet Configuration

### Key Features
- **Ordered Pod Management**: `podManagementPolicy: OrderedReady`
- **Rolling Updates**: Configurable partition-based updates
- **Init Container**: Data directory initialization
- **Volume Mounts**: Data, config, secrets, tmp
- **Environment Variables**: Pod metadata, config paths
- **Probes**: Comprehensive health checking
- **Lifecycle Hooks**: Graceful shutdown with sleep delay

### Volumes
1. **data**: Persistent volume (from PVC) or emptyDir
2. **config**: ConfigMap with anode.toml
3. **secret**: S3 credentials
4. **tmp**: EmptyDir for temporary files

## Service Configuration

### Headless Service
- **Purpose**: StatefulSet DNS resolution
- **ClusterIP**: None
- **publishNotReadyAddresses**: true
- **Ports**: S3 (8080), Admin (8081), gRPC (9090)

### Load-Balancing Service
- **Purpose**: External access and load balancing
- **Type**: Configurable (ClusterIP, NodePort, LoadBalancer)
- **Ports**: S3 (8080), Admin (8081)
- **Session Affinity**: Optional ClientIP

## ConfigMap (anode.toml)

### Generated Configuration Sections
- **[cluster]**: Cluster ID, placement groups, replication, Raft settings
- **[node]**: Node addresses, bootstrap nodes (auto-generated from StatefulSet)
- **[storage]**: Data directory, chunk size, I/O settings
- **[s3]**: S3 API settings, multipart configuration
- **[parquet]**: Caching configuration
- **[logging]**: Log level, format, output
- **[metrics]**: Metrics configuration
- **[tls]**: Optional TLS configuration

### Dynamic Values
- Bootstrap nodes automatically generated from replica count
- Node ID from pod name (ANODE_NODE_ID)
- Cluster ID auto-generated or from values

## Secrets Management

### S3 Credentials
- **s3-access-key**: Admin access key (configurable)
- **s3-secret-key**: Auto-generated if not provided (32 chars)
- **credentials.json**: JSON format for internal use

### Best Practices
- Secret mounted as read-only volume
- Default mode 0400 (read-only for owner)
- Generated secrets use randAlphaNum

## Health Checks

### Startup Probe
- **Path**: /health/startup
- **Initial Delay**: 0s
- **Period**: 5s
- **Failure Threshold**: 30 (150s max)
- **Purpose**: Wait for slow-starting containers

### Liveness Probe
- **Path**: /health/live
- **Initial Delay**: 30s
- **Period**: 10s
- **Failure Threshold**: 3
- **Purpose**: Restart unhealthy containers

### Readiness Probe
- **Path**: /health/ready
- **Initial Delay**: 10s
- **Period**: 5s
- **Failure Threshold**: 3
- **Purpose**: Remove from service endpoints when not ready

## High Availability

### Pod Disruption Budget
- **Default**: minAvailable: 2
- **Purpose**: Maintain quorum during voluntary disruptions
- **Configurable**: Can set maxUnavailable instead

### Pod Anti-Affinity
- **Preferred**: Spread across nodes (weight: 100)
- **Preferred**: Spread across zones (weight: 50)
- **Customizable**: Override with custom affinity rules

### Topology Spread Constraints
- **Optional**: Configurable max-skew for zones/nodes
- **Purpose**: Ensure even distribution

## Monitoring Integration

### ServiceMonitor
- **CRD**: monitoring.coreos.com/v1
- **Scrape Interval**: 30s (configurable)
- **Scrape Timeout**: 10s
- **Endpoint**: Admin port (/metrics)

### PrometheusRule
- **Default Alerts**:
  - AnodeNodeDown: Node unavailable for 5 minutes
  - AnodeHighMemoryUsage: >90% memory for 5 minutes
  - AnodeHighCPUUsage: >90% CPU for 10 minutes
  - AnodeClusterNotHealthy: Lost quorum
- **Customizable**: Add custom rules via values

## Network Policies

### Ingress Rules
1. gRPC port (9090) from anode pods only
2. S3/Admin ports (8080, 8081) from anywhere

### Egress Rules
1. To anode pods (cluster communication)
2. DNS (port 53 TCP/UDP)

## Ingress Configuration

### Features
- **TLS Support**: With cert-manager integration
- **Annotations**: Customizable (nginx, body size, buffering)
- **Virtual Hosts**: Multiple hosts supported
- **Path-based Routing**: Flexible path configuration

### Common Annotations
- `cert-manager.io/cluster-issuer`: Automatic TLS
- `nginx.ingress.kubernetes.io/proxy-body-size: "0"`: Unlimited upload
- `nginx.ingress.kubernetes.io/proxy-request-buffering: "off"`: Streaming

## Values Validation

### JSON Schema (values.schema.json)
- **Type Checking**: Validate data types
- **Constraints**: Min/max values, patterns, enums
- **Required Fields**: Ensure critical values present
- **Documentation**: Inline descriptions

### Validated Fields
- replicaCount (integer, minimum: 1)
- image.pullPolicy (enum)
- service.type (enum)
- ports (1-65535 range)
- persistence.size (pattern validation)
- resources (pattern validation)

## Helm Tests

### Connection Test (test-connection.yaml)
Three test containers:
1. **test-s3-health**: Check S3 API health endpoint
2. **test-admin-health**: Check Admin API readiness endpoint
3. **test-metrics**: Verify metrics endpoint responding

### Running Tests
```bash
helm test anode -n anode
```

## Helper Scripts

### install.sh
- Prerequisites checking (kubectl, helm)
- Values file selection (dev/prod/minimal)
- Namespace creation
- Chart linting
- Dry-run support
- Installation with wait
- Post-install instructions

### upgrade.sh
- Release existence check
- Cluster health verification
- Chart linting
- Diff preview (if helm-diff installed)
- Confirmation prompt
- Rolling update
- Rollout status monitoring

### validate.sh
- 15 comprehensive checks:
  1. Helm release exists
  2. All pods running
  3. StatefulSet healthy
  4. Services exist
  5. ConfigMap exists
  6. Secret exists
  7. PVCs bound
  8. Health endpoints responding
  9. Metrics endpoints responding
  10. Resource usage check
  11. Pod restart check
  12. Event warnings
  13. Inter-pod connectivity
  14. ServiceMonitor (if applicable)
  15. PodDisruptionBudget (if applicable)

## Configuration Examples

### Minimal (1 node, 5Gi)
```bash
helm install anode ./anode -f values-minimal.yaml
```

### Development (3 nodes, no persistence)
```bash
helm install anode ./anode -f values-development.yaml
```

### Production (5 nodes, 100Gi, monitoring)
```bash
helm install anode ./anode -f values-production.yaml
```

### Custom
```yaml
replicaCount: 7
persistence:
  storageClass: "fast-nvme"
  size: 500Gi
resources:
  requests:
    cpu: 2000m
    memory: 4Gi
config:
  cluster:
    placementGroups: 64
    replicationFactor: 5
  parquet:
    cacheSizeMb: 2048
```

## Best Practices

### Deployment
1. Always use odd replica counts (3, 5, 7) for Raft quorum
2. Enable persistence in production
3. Set appropriate resource limits
4. Use pod anti-affinity for node distribution
5. Enable PodDisruptionBudget

### Security
1. Use network policies in production
2. Enable TLS for external access
3. Rotate S3 credentials regularly
4. Use secrets management (e.g., sealed-secrets, vault)
5. Run as non-root user

### Monitoring
1. Enable ServiceMonitor for Prometheus
2. Configure alerting rules
3. Monitor resource usage
4. Set up log aggregation
5. Track cluster health metrics

### Operations
1. Test upgrades in staging first
2. Use helm diff before upgrading
3. Maintain backup/restore procedures
4. Document customizations
5. Use version control for values files

## Troubleshooting

### Common Issues

**Pods not starting**
```bash
kubectl describe pod -n anode anode-0
kubectl logs -n anode anode-0
```

**PVC not binding**
```bash
kubectl get pvc -n anode
kubectl describe pvc -n anode data-anode-0
```

**Network connectivity**
```bash
kubectl exec -n anode anode-0 -- ping anode-1.anode-headless
```

**Quorum loss**
```bash
# Check pod status
kubectl get pods -n anode
# Check logs for Raft errors
kubectl logs -n anode -l app.kubernetes.io/name=anode | grep -i raft
```

## Maintenance

### Backup
```bash
# Using Velero
velero backup create anode-backup --include-namespaces anode
```

### Restore
```bash
velero restore create --from-backup anode-backup
```

### Upgrade
```bash
./upgrade.sh -f production
```

### Rollback
```bash
helm rollback anode -n anode
```

### Scale
```bash
helm upgrade anode ./anode -n anode --set replicaCount=5
```

## Version History

See [CHANGELOG.md](CHANGELOG.md) for detailed version history.

## Contributing

Contributions welcome! Please ensure:
- Chart passes linting: `helm lint`
- Templates render correctly: `helm template`
- Tests pass: `helm test`
- Documentation updated
- Follow Helm best practices

## License

Apache 2.0 / MIT dual license

## Support

- Documentation: https://anode.io/docs
- Issues: https://github.com/anode/anode/issues
- Discussions: https://github.com/anode/anode/discussions
