# Anode Helm Chart Testing Guide

This guide covers testing strategies for the Anode Helm chart.

## Quick Test

```bash
# Install chart
./install.sh -f development

# Run Helm tests
helm test anode -n anode

# Validate installation
./validate.sh

# Clean up
helm uninstall anode -n anode
```

## Pre-Installation Testing

### 1. Lint the Chart

```bash
# Basic linting
helm lint ./deploy/helm/anode

# Lint with values files
helm lint ./deploy/helm/anode -f ./deploy/helm/anode/values-development.yaml
helm lint ./deploy/helm/anode -f ./deploy/helm/anode/values-production.yaml
helm lint ./deploy/helm/anode -f ./deploy/helm/anode/values-minimal.yaml
```

### 2. Template Rendering

```bash
# Render all templates
helm template anode ./deploy/helm/anode

# Render with specific values
helm template anode ./deploy/helm/anode -f ./deploy/helm/anode/values-production.yaml

# Render and save to file
helm template anode ./deploy/helm/anode > rendered-templates.yaml

# Render specific template
helm template anode ./deploy/helm/anode -s templates/statefulset.yaml
```

### 3. Dry Run Installation

```bash
# Dry run with debug
helm install anode ./deploy/helm/anode -n anode --dry-run --debug

# Dry run with values file
./install.sh -f production -d
```

### 4. Schema Validation

```bash
# Helm 3.4+ automatically validates against values.schema.json
helm install anode ./deploy/helm/anode -n anode --dry-run

# Test with invalid values (should fail)
helm install anode ./deploy/helm/anode -n anode \
  --set replicaCount=-1 --dry-run
```

## Installation Testing

### 1. Minimal Installation

```bash
# Install minimal configuration
./install.sh -f minimal

# Verify
./validate.sh

# Test basic operations
kubectl port-forward -n anode svc/anode 8080:8080 &

# Get credentials
export ACCESS_KEY=$(kubectl get secret -n anode anode-secret -o jsonpath="{.data.s3-access-key}" | base64 -d)
export SECRET_KEY=$(kubectl get secret -n anode anode-secret -o jsonpath="{.data.s3-secret-key}" | base64 -d)

# Test S3 operations
aws --endpoint-url http://localhost:8080 s3 mb s3://test
aws --endpoint-url http://localhost:8080 s3 ls

# Clean up
helm uninstall anode -n anode
kubectl delete pvc -n anode -l app.kubernetes.io/name=anode
```

### 2. Development Installation

```bash
# Install development configuration
./install.sh -f development

# Verify
./validate.sh -v

# Check logs
kubectl logs -n anode -l app.kubernetes.io/name=anode --tail=50

# Run Helm tests
helm test anode -n anode

# Clean up
helm uninstall anode -n anode
```

### 3. Production Installation

```bash
# Install production configuration
./install.sh -f production

# Verify all components
./validate.sh -v

# Check resource usage
kubectl top pods -n anode

# Check PVCs
kubectl get pvc -n anode

# Run comprehensive tests
helm test anode -n anode

# Clean up
helm uninstall anode -n anode
kubectl delete pvc -n anode -l app.kubernetes.io/name=anode
```

## Functional Testing

### 1. S3 API Tests

```bash
# Port forward
kubectl port-forward -n anode svc/anode 8080:8080 &

# Get credentials
export ACCESS_KEY=$(kubectl get secret -n anode anode-secret -o jsonpath="{.data.s3-access-key}" | base64 -d)
export SECRET_KEY=$(kubectl get secret -n anode anode-secret -o jsonpath="{.data.s3-secret-key}" | base64 -d)

# Configure AWS CLI
aws configure set aws_access_key_id $ACCESS_KEY
aws configure set aws_secret_access_key $SECRET_KEY

# Test bucket operations
aws --endpoint-url http://localhost:8080 s3 mb s3://test-bucket
aws --endpoint-url http://localhost:8080 s3 ls

# Test object operations
echo "Hello, Anode!" > test.txt
aws --endpoint-url http://localhost:8080 s3 cp test.txt s3://test-bucket/
aws --endpoint-url http://localhost:8080 s3 ls s3://test-bucket/
aws --endpoint-url http://localhost:8080 s3 cp s3://test-bucket/test.txt downloaded.txt
diff test.txt downloaded.txt

# Test delete operations
aws --endpoint-url http://localhost:8080 s3 rm s3://test-bucket/test.txt
aws --endpoint-url http://localhost:8080 s3 rb s3://test-bucket
```

### 2. Health Check Tests

```bash
# Port forward to admin port
kubectl port-forward -n anode svc/anode 8081:8081 &

# Test liveness
curl -f http://localhost:8081/health/live && echo "✓ Liveness OK"

# Test readiness
curl -f http://localhost:8081/health/ready && echo "✓ Readiness OK"

# Test startup
curl -f http://localhost:8081/health/startup && echo "✓ Startup OK"
```

### 3. Metrics Tests

```bash
# Get metrics
curl http://localhost:8081/metrics

# Check for specific metrics
curl http://localhost:8081/metrics | grep "anode_"

# Verify Prometheus format
curl http://localhost:8081/metrics | head -n 20
```

## Upgrade Testing

### 1. In-Place Upgrade

```bash
# Install initial version
./install.sh -f development

# Make changes to values
cat > custom-values.yaml <<EOF
replicaCount: 5
config:
  parquet:
    cacheSizeMb: 512
EOF

# Upgrade
./upgrade.sh -f custom-values.yaml

# Verify upgrade
./validate.sh
kubectl get pods -n anode
```

### 2. Rolling Update

```bash
# Install
./install.sh

# Update image tag
helm upgrade anode ./deploy/helm/anode -n anode \
  --set image.tag=0.2.0 \
  --reuse-values

# Watch rollout
kubectl rollout status statefulset anode -n anode

# Verify
./validate.sh
```

### 3. Rollback Testing

```bash
# View history
helm history anode -n anode

# Rollback to previous version
helm rollback anode -n anode

# Rollback to specific revision
helm rollback anode 2 -n anode

# Verify
./validate.sh
```

## Scaling Tests

### 1. Scale Up

```bash
# Install with 3 nodes
./install.sh

# Scale to 5 nodes
helm upgrade anode ./deploy/helm/anode -n anode \
  --set replicaCount=5 \
  --reuse-values

# Wait for new pods
kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=anode -n anode --timeout=300s

# Verify
kubectl get pods -n anode
kubectl get pvc -n anode
./validate.sh
```

### 2. Scale Down

```bash
# Scale to 3 nodes
helm upgrade anode ./deploy/helm/anode -n anode \
  --set replicaCount=3 \
  --reuse-values

# Wait for pods to terminate
kubectl get pods -n anode -w

# Verify
./validate.sh
```

## High Availability Tests

### 1. Pod Failure Simulation

```bash
# Delete a pod
kubectl delete pod -n anode anode-0

# Watch recovery
kubectl get pods -n anode -w

# Verify cluster still functional
kubectl exec -n anode anode-1 -- curl -f http://localhost:8081/health/ready

# Validate after recovery
./validate.sh
```

### 2. Node Failure Simulation

```bash
# Cordon a node
NODE=$(kubectl get pod -n anode anode-0 -o jsonpath='{.spec.nodeName}')
kubectl cordon $NODE

# Delete pod on that node
kubectl delete pod -n anode anode-0

# Watch pod reschedule
kubectl get pods -n anode -w

# Uncordon node
kubectl uncordon $NODE

# Verify
./validate.sh
```

### 3. PodDisruptionBudget Test

```bash
# Try to drain node (should respect PDB)
NODE=$(kubectl get pod -n anode anode-0 -o jsonpath='{.spec.nodeName}')
kubectl drain $NODE --ignore-daemonsets --delete-emptydir-data

# Should maintain minAvailable pods
kubectl get pods -n anode

# Uncordon
kubectl uncordon $NODE
```

## Network Tests

### 1. Inter-Pod Connectivity

```bash
# Test DNS resolution
kubectl exec -n anode anode-0 -- nslookup anode-headless

# Test gRPC port connectivity
kubectl exec -n anode anode-0 -- nc -zv anode-1.anode-headless 9090

# Test all pods
for i in 0 1 2; do
  kubectl exec -n anode anode-0 -- nc -zv anode-$i.anode-headless 9090
done
```

### 2. Service Connectivity

```bash
# Test service DNS
kubectl exec -n anode anode-0 -- nslookup anode

# Test service endpoint
kubectl exec -n anode anode-0 -- curl -f http://anode:8080/health
```

### 3. Network Policy Test (if enabled)

```bash
# Install with network policies
helm upgrade anode ./deploy/helm/anode -n anode \
  --set networkPolicy.enabled=true

# Test allowed connections (should work)
kubectl exec -n anode anode-0 -- nc -zv anode-1.anode-headless 9090

# Test from different namespace (should fail if restricted)
kubectl run -it --rm debug --image=nicolaka/netshoot --restart=Never -- \
  nc -zv anode.anode.svc.cluster.local 9090
```

## Storage Tests

### 1. Persistent Volume Test

```bash
# Check PVCs
kubectl get pvc -n anode

# Verify all bound
kubectl get pvc -n anode -o jsonpath='{.items[*].status.phase}' | grep -v Bound && echo "ERROR: Not all PVCs bound" || echo "✓ All PVCs bound"

# Check PV
kubectl get pv | grep anode

# Write data and verify persistence
kubectl exec -n anode anode-0 -- sh -c 'echo "test" > /data/test.txt'
kubectl delete pod -n anode anode-0
kubectl wait --for=condition=ready pod -n anode anode-0 --timeout=120s
kubectl exec -n anode anode-0 -- cat /data/test.txt
```

### 2. Storage Class Test

```bash
# Install with specific storage class
helm install anode ./deploy/helm/anode -n anode \
  --set persistence.storageClass=fast-ssd

# Verify PVC uses correct storage class
kubectl get pvc -n anode -o jsonpath='{.items[*].spec.storageClassName}'
```

## Monitoring Tests

### 1. ServiceMonitor Test

```bash
# Install with ServiceMonitor
helm upgrade anode ./deploy/helm/anode -n anode \
  --set monitoring.serviceMonitor.enabled=true

# Check ServiceMonitor created
kubectl get servicemonitor -n anode

# If Prometheus Operator installed, check targets
kubectl get servicemonitor -n anode anode -o yaml
```

### 2. PrometheusRule Test

```bash
# Install with alerting rules
helm upgrade anode ./deploy/helm/anode -n anode \
  --set monitoring.prometheusRule.enabled=true

# Check rules created
kubectl get prometheusrule -n anode

# View rules
kubectl get prometheusrule -n anode anode -o yaml
```

## Security Tests

### 1. Pod Security Context

```bash
# Verify non-root user
kubectl exec -n anode anode-0 -- id

# Verify read-only root filesystem
kubectl exec -n anode anode-0 -- touch /test && echo "ERROR: Root filesystem is writable" || echo "✓ Root filesystem is read-only"

# Verify capabilities dropped
kubectl get pod -n anode anode-0 -o jsonpath='{.spec.containers[0].securityContext}'
```

### 2. Secret Security

```bash
# Check secret permissions
kubectl exec -n anode anode-0 -- ls -la /etc/anode/secrets/

# Verify secret mounted read-only
kubectl exec -n anode anode-0 -- touch /etc/anode/secrets/test && echo "ERROR: Secrets writable" || echo "✓ Secrets read-only"
```

### 3. Network Policy (if enabled)

```bash
# Test isolation
# Should work from anode pods
kubectl exec -n anode anode-0 -- nc -zv anode-1.anode-headless 9090

# Should fail from other namespaces (if restricted)
kubectl run -it --rm test --image=busybox --restart=Never -- \
  nc -zv anode.anode.svc.cluster.local 9090
```

## Performance Tests

### 1. Resource Usage

```bash
# Monitor resource usage
kubectl top pods -n anode

# Monitor over time
watch kubectl top pods -n anode

# Check against limits
kubectl get pods -n anode -o jsonpath='{range .items[*]}{.metadata.name}{"\t"}{.spec.containers[0].resources}{"\n"}{end}'
```

### 2. Load Testing

```bash
# Port forward
kubectl port-forward -n anode svc/anode 8080:8080 &

# Simple load test with ab (Apache Bench)
echo "test data" > test.txt
ab -n 100 -c 10 -T 'application/octet-stream' \
  -u test.txt http://localhost:8080/test-bucket/test.txt

# Or use s3-benchmark
s3-benchmark -endpoint http://localhost:8080 \
  -access-key $ACCESS_KEY \
  -secret-key $SECRET_KEY \
  -bucket test-bucket \
  -num-clients 10 \
  -num-objects 100
```

## Cleanup

### Full Cleanup

```bash
# Uninstall release
helm uninstall anode -n anode

# Delete PVCs
kubectl delete pvc -n anode -l app.kubernetes.io/name=anode

# Delete namespace
kubectl delete namespace anode
```

### Partial Cleanup (Keep Data)

```bash
# Uninstall release only
helm uninstall anode -n anode

# PVCs and data remain
kubectl get pvc -n anode
```

## Continuous Testing

### Automated Test Script

```bash
#!/bin/bash
# automated-test.sh

set -e

echo "=== Anode Helm Chart Automated Tests ==="

# Lint
echo "1. Linting chart..."
helm lint ./deploy/helm/anode

# Install
echo "2. Installing chart..."
./deploy/helm/anode/install.sh -f development

# Wait for ready
echo "3. Waiting for pods..."
kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=anode -n anode --timeout=300s

# Validate
echo "4. Validating installation..."
./deploy/helm/anode/validate.sh

# Helm test
echo "5. Running Helm tests..."
helm test anode -n anode

# Functional test
echo "6. Running functional tests..."
# Add your functional tests here

# Upgrade test
echo "7. Testing upgrade..."
helm upgrade anode ./deploy/helm/anode -n anode --set config.parquet.cacheSizeMb=512

# Cleanup
echo "8. Cleaning up..."
helm uninstall anode -n anode
kubectl delete pvc -n anode -l app.kubernetes.io/name=anode

echo "=== All tests passed! ==="
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Helm Chart Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3

    - name: Set up Helm
      uses: azure/setup-helm@v3
      with:
        version: v3.12.0

    - name: Lint chart
      run: helm lint ./deploy/helm/anode

    - name: Template chart
      run: helm template anode ./deploy/helm/anode

    - name: Create k8s cluster
      uses: helm/kind-action@v1.5.0

    - name: Install chart
      run: |
        helm install anode ./deploy/helm/anode -n anode --create-namespace
        kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=anode -n anode --timeout=300s

    - name: Run tests
      run: helm test anode -n anode

    - name: Cleanup
      if: always()
      run: helm uninstall anode -n anode
```

## Troubleshooting Tests

If tests fail, check:

1. **Logs**: `kubectl logs -n anode -l app.kubernetes.io/name=anode --tail=100`
2. **Events**: `kubectl get events -n anode --sort-by='.lastTimestamp'`
3. **Pod Status**: `kubectl describe pod -n anode anode-0`
4. **PVC Status**: `kubectl describe pvc -n anode data-anode-0`
5. **Service Endpoints**: `kubectl get endpoints -n anode`

## Next Steps

After testing:
1. Review test results
2. Fix any issues
3. Update documentation
4. Tag release
5. Publish chart

For more information, see [README.md](README.md).
