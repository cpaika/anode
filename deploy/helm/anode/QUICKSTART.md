# Anode Quick Start Guide

This guide will help you quickly deploy Anode on Kubernetes using Helm.

## Prerequisites

- Kubernetes cluster (1.19+)
- Helm 3.2.0+
- kubectl configured to access your cluster
- (Optional) Storage class that supports dynamic provisioning

## Quick Installation

### 1. Install with Default Settings

```bash
# Create namespace
kubectl create namespace anode

# Install Anode
helm install anode ./deploy/helm/anode -n anode

# Wait for pods to be ready
kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=anode -n anode --timeout=300s
```

### 2. Get Access Credentials

```bash
# Get S3 credentials
export ACCESS_KEY=$(kubectl get secret -n anode anode-secret -o jsonpath="{.data.s3-access-key}" | base64 -d)
export SECRET_KEY=$(kubectl get secret -n anode anode-secret -o jsonpath="{.data.s3-secret-key}" | base64 -d)

echo "Access Key: $ACCESS_KEY"
echo "Secret Key: $SECRET_KEY"
```

### 3. Access the S3 API

```bash
# Port forward to local machine
kubectl port-forward -n anode svc/anode 8080:8080 &

# Configure AWS CLI
aws configure set aws_access_key_id $ACCESS_KEY
aws configure set aws_secret_access_key $SECRET_KEY
```

### 4. Test the Installation

```bash
# Run Helm tests
helm test anode -n anode

# Create a test bucket
aws --endpoint-url http://localhost:8080 s3 mb s3://test-bucket

# Upload a file
echo "Hello, Anode!" > test.txt
aws --endpoint-url http://localhost:8080 s3 cp test.txt s3://test-bucket/

# List objects
aws --endpoint-url http://localhost:8080 s3 ls s3://test-bucket/

# Download the file
aws --endpoint-url http://localhost:8080 s3 cp s3://test-bucket/test.txt downloaded.txt
cat downloaded.txt
```

## Common Deployment Scenarios

### Development Environment

```bash
helm install anode ./deploy/helm/anode \
  -n anode \
  -f ./deploy/helm/anode/values-development.yaml
```

### Production Environment

```bash
helm install anode ./deploy/helm/anode \
  -n anode \
  -f ./deploy/helm/anode/values-production.yaml \
  --set ingress.hosts[0].host=s3.yourdomain.com
```

### Minimal Single-Node Deployment

```bash
helm install anode ./deploy/helm/anode \
  -n anode \
  -f ./deploy/helm/anode/values-minimal.yaml
```

### High-Availability 5-Node Cluster

```bash
helm install anode ./deploy/helm/anode \
  -n anode \
  --set replicaCount=5 \
  --set persistence.storageClass=fast-ssd \
  --set persistence.size=100Gi \
  --set resources.requests.cpu=500m \
  --set resources.requests.memory=1Gi
```

## Customization Examples

### Change Storage Size

```bash
helm upgrade anode ./deploy/helm/anode \
  -n anode \
  --set persistence.size=50Gi
```

### Enable Ingress

```bash
helm upgrade anode ./deploy/helm/anode \
  -n anode \
  --set ingress.enabled=true \
  --set ingress.hosts[0].host=s3.example.com
```

### Enable Prometheus Monitoring

```bash
helm upgrade anode ./deploy/helm/anode \
  -n anode \
  --set monitoring.serviceMonitor.enabled=true \
  --set monitoring.prometheusRule.enabled=true
```

### Increase Resources

```bash
helm upgrade anode ./deploy/helm/anode \
  -n anode \
  --set resources.requests.cpu=1000m \
  --set resources.requests.memory=2Gi \
  --set resources.limits.cpu=4000m \
  --set resources.limits.memory=8Gi
```

## Monitoring

### Check Cluster Status

```bash
# Get pods
kubectl get pods -n anode

# Get services
kubectl get svc -n anode

# Get persistent volumes
kubectl get pvc -n anode

# Check logs
kubectl logs -n anode -l app.kubernetes.io/name=anode --tail=100
```

### Access Metrics

```bash
# Port forward to metrics endpoint
kubectl port-forward -n anode svc/anode 8081:8081 &

# Get metrics
curl http://localhost:8081/metrics
```

### Health Checks

```bash
# Liveness check
curl http://localhost:8081/health/live

# Readiness check
curl http://localhost:8081/health/ready

# Startup check
curl http://localhost:8081/health/startup
```

## Scaling

### Scale to 5 Nodes

```bash
helm upgrade anode ./deploy/helm/anode \
  -n anode \
  --set replicaCount=5

# Wait for new pods
kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=anode -n anode --timeout=300s
```

### Scale Down

```bash
helm upgrade anode ./deploy/helm/anode \
  -n anode \
  --set replicaCount=3

# Note: Scaling down may require data rebalancing
```

## Backup and Recovery

### Manual Backup (Example)

```bash
# Backup using kubectl cp (for small datasets)
for i in 0 1 2; do
  kubectl cp anode/anode-$i:/data ./backup/anode-$i-data
done

# Or use Velero for production
velero backup create anode-backup \
  --include-namespaces anode \
  --wait
```

### Restore from Backup

```bash
# Using Velero
velero restore create --from-backup anode-backup
```

## Troubleshooting

### Pods Not Starting

```bash
# Check pod status
kubectl describe pod -n anode anode-0

# Check events
kubectl get events -n anode --sort-by='.lastTimestamp'

# Check logs
kubectl logs -n anode anode-0
```

### Storage Issues

```bash
# Check PVC status
kubectl get pvc -n anode

# Describe PVC
kubectl describe pvc -n anode data-anode-0

# Check storage class
kubectl get storageclass
```

### Network Issues

```bash
# Test connectivity between pods
kubectl exec -n anode anode-0 -- ping anode-1.anode-headless

# Check services
kubectl get svc -n anode

# Check endpoints
kubectl get endpoints -n anode
```

### View Configuration

```bash
# View ConfigMap
kubectl get configmap -n anode anode-config -o yaml

# View generated config
kubectl exec -n anode anode-0 -- cat /etc/anode/config.toml
```

## Uninstallation

### Remove Release (Keep Data)

```bash
helm uninstall anode -n anode
```

### Remove Everything (Including Data)

```bash
# Uninstall release
helm uninstall anode -n anode

# Delete PVCs
kubectl delete pvc -n anode -l app.kubernetes.io/name=anode

# Delete namespace
kubectl delete namespace anode
```

## Advanced Configuration

### Custom Values File

Create a file `my-values.yaml`:

```yaml
replicaCount: 5

persistence:
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
    placementGroups: 32
    replicationFactor: 3
  parquet:
    cacheSizeMb: 1024

monitoring:
  serviceMonitor:
    enabled: true
```

Install with custom values:

```bash
helm install anode ./deploy/helm/anode -n anode -f my-values.yaml
```

## Next Steps

- Read the full [README.md](README.md) for detailed configuration options
- Check the [values.yaml](values.yaml) for all available parameters
- Review [CHANGELOG.md](CHANGELOG.md) for version history
- See production examples in [values-production.yaml](values-production.yaml)

## Getting Help

- Issues: https://github.com/anode/anode/issues
- Discussions: https://github.com/anode/anode/discussions
- Documentation: https://anode.io/docs
