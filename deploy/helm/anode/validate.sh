#!/bin/bash
# Anode Helm Chart Validation Script
# This script validates the Anode installation

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
NAMESPACE="anode"
RELEASE_NAME="anode"
VERBOSE=false

# Counters
PASSED=0
FAILED=0
WARNINGS=0

# Print colored output
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
    ((WARNINGS++))
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
    ((FAILED++))
}

print_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((PASSED++))
}

print_step() {
    echo -e "${BLUE}[CHECK]${NC} $1"
}

# Print usage
usage() {
    cat <<EOF
Anode Helm Chart Validation Script

Usage: $0 [OPTIONS]

Options:
    -n, --namespace NAME        Kubernetes namespace (default: anode)
    -r, --release NAME          Helm release name (default: anode)
    -v, --verbose               Verbose output
    -h, --help                  Show this help message

Examples:
    # Validate default installation
    $0

    # Validate custom installation
    $0 -n my-namespace -r my-release

    # Verbose output
    $0 -v

EOF
    exit 1
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -n|--namespace)
            NAMESPACE="$2"
            shift 2
            ;;
        -r|--release)
            RELEASE_NAME="$2"
            shift 2
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            print_error "Unknown option: $1"
            usage
            ;;
    esac
done

echo "=========================================="
echo "Anode Validation Script"
echo "=========================================="
echo ""
echo "Release: $RELEASE_NAME"
echo "Namespace: $NAMESPACE"
echo ""

# Check 1: Helm release exists
print_step "Checking if Helm release exists..."
if helm status "$RELEASE_NAME" -n "$NAMESPACE" &> /dev/null; then
    print_pass "Helm release '$RELEASE_NAME' found in namespace '$NAMESPACE'"
else
    print_error "Helm release '$RELEASE_NAME' not found in namespace '$NAMESPACE'"
    exit 1
fi

# Check 2: All pods are running
print_step "Checking pod status..."
POD_COUNT=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=anode --no-headers 2>/dev/null | wc -l | tr -d ' ')
READY_COUNT=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=anode -o jsonpath='{.items[*].status.conditions[?(@.type=="Ready")].status}' 2>/dev/null | grep -o "True" | wc -l | tr -d ' ')

if [ "$POD_COUNT" -eq 0 ]; then
    print_error "No anode pods found"
elif [ "$READY_COUNT" -eq "$POD_COUNT" ]; then
    print_pass "All $POD_COUNT pods are ready"
else
    print_error "Only $READY_COUNT out of $POD_COUNT pods are ready"
fi

# Check 3: StatefulSet exists and is healthy
print_step "Checking StatefulSet..."
if kubectl get statefulset "$RELEASE_NAME" -n "$NAMESPACE" &> /dev/null; then
    DESIRED=$(kubectl get statefulset "$RELEASE_NAME" -n "$NAMESPACE" -o jsonpath='{.spec.replicas}')
    CURRENT=$(kubectl get statefulset "$RELEASE_NAME" -n "$NAMESPACE" -o jsonpath='{.status.readyReplicas}')

    if [ "$DESIRED" -eq "$CURRENT" ]; then
        print_pass "StatefulSet has $CURRENT/$DESIRED replicas ready"
    else
        print_error "StatefulSet has $CURRENT/$DESIRED replicas ready"
    fi
else
    print_error "StatefulSet not found"
fi

# Check 4: Services exist
print_step "Checking Services..."
if kubectl get service "$RELEASE_NAME" -n "$NAMESPACE" &> /dev/null; then
    print_pass "Service '$RELEASE_NAME' exists"
else
    print_error "Service '$RELEASE_NAME' not found"
fi

if kubectl get service "${RELEASE_NAME}-headless" -n "$NAMESPACE" &> /dev/null; then
    print_pass "Headless service '${RELEASE_NAME}-headless' exists"
else
    print_error "Headless service '${RELEASE_NAME}-headless' not found"
fi

# Check 5: ConfigMap exists
print_step "Checking ConfigMap..."
if kubectl get configmap "${RELEASE_NAME}-config" -n "$NAMESPACE" &> /dev/null; then
    print_pass "ConfigMap '${RELEASE_NAME}-config' exists"
else
    print_error "ConfigMap '${RELEASE_NAME}-config' not found"
fi

# Check 6: Secret exists
print_step "Checking Secret..."
if kubectl get secret "${RELEASE_NAME}-secret" -n "$NAMESPACE" &> /dev/null; then
    print_pass "Secret '${RELEASE_NAME}-secret' exists"
else
    print_error "Secret '${RELEASE_NAME}-secret' not found"
fi

# Check 7: PVCs exist (if persistence is enabled)
print_step "Checking Persistent Volume Claims..."
PVC_COUNT=$(kubectl get pvc -n "$NAMESPACE" -l app.kubernetes.io/name=anode --no-headers 2>/dev/null | wc -l | tr -d ' ')
if [ "$PVC_COUNT" -gt 0 ]; then
    BOUND_COUNT=$(kubectl get pvc -n "$NAMESPACE" -l app.kubernetes.io/name=anode -o jsonpath='{.items[*].status.phase}' 2>/dev/null | grep -o "Bound" | wc -l | tr -d ' ')
    if [ "$BOUND_COUNT" -eq "$PVC_COUNT" ]; then
        print_pass "All $PVC_COUNT PVCs are bound"
    else
        print_error "Only $BOUND_COUNT out of $PVC_COUNT PVCs are bound"
    fi
else
    print_warn "No PVCs found (persistence might be disabled)"
fi

# Check 8: Health endpoints
print_step "Checking health endpoints..."
for POD in $(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=anode -o jsonpath='{.items[*].metadata.name}'); do
    if kubectl exec -n "$NAMESPACE" "$POD" -- curl -sf http://localhost:8081/health/ready &> /dev/null; then
        print_pass "Pod $POD health endpoint is responding"
    else
        print_error "Pod $POD health endpoint is not responding"
    fi
done

# Check 9: Metrics endpoint
print_step "Checking metrics endpoints..."
for POD in $(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=anode -o jsonpath='{.items[*].metadata.name}'); do
    if kubectl exec -n "$NAMESPACE" "$POD" -- curl -sf http://localhost:8081/metrics &> /dev/null; then
        print_pass "Pod $POD metrics endpoint is responding"
    else
        print_warn "Pod $POD metrics endpoint is not responding"
    fi
done

# Check 10: Resource usage
print_step "Checking resource usage..."
if command -v kubectl-top &> /dev/null || kubectl top pods -n "$NAMESPACE" &> /dev/null; then
    for POD in $(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=anode -o jsonpath='{.items[*].metadata.name}'); do
        USAGE=$(kubectl top pod -n "$NAMESPACE" "$POD" --no-headers 2>/dev/null || echo "N/A")
        if [ "$VERBOSE" = true ]; then
            echo "  $POD: $USAGE"
        fi
    done
    print_pass "Resource metrics available"
else
    print_warn "Metrics server not available, skipping resource check"
fi

# Check 11: Pod restarts
print_step "Checking pod restarts..."
HIGH_RESTART=false
for POD in $(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=anode -o jsonpath='{.items[*].metadata.name}'); do
    RESTARTS=$(kubectl get pod -n "$NAMESPACE" "$POD" -o jsonpath='{.status.containerStatuses[0].restartCount}')
    if [ "$RESTARTS" -gt 5 ]; then
        print_warn "Pod $POD has restarted $RESTARTS times"
        HIGH_RESTART=true
    elif [ "$VERBOSE" = true ]; then
        echo "  $POD: $RESTARTS restarts"
    fi
done

if [ "$HIGH_RESTART" = false ]; then
    print_pass "No excessive pod restarts detected"
fi

# Check 12: Pod events
print_step "Checking for pod warnings/errors..."
ERROR_COUNT=$(kubectl get events -n "$NAMESPACE" --field-selector involvedObject.kind=Pod,type=Warning --no-headers 2>/dev/null | wc -l | tr -d ' ')
if [ "$ERROR_COUNT" -eq 0 ]; then
    print_pass "No recent warning events"
else
    print_warn "$ERROR_COUNT warning events found"
    if [ "$VERBOSE" = true ]; then
        kubectl get events -n "$NAMESPACE" --field-selector involvedObject.kind=Pod,type=Warning --sort-by='.lastTimestamp' | tail -n 5
    fi
fi

# Check 13: Connectivity between pods
print_step "Checking inter-pod connectivity..."
FIRST_POD=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=anode -o jsonpath='{.items[0].metadata.name}')
if [ -n "$FIRST_POD" ]; then
    if kubectl exec -n "$NAMESPACE" "$FIRST_POD" -- nc -zv "${RELEASE_NAME}-headless" 9090 &> /dev/null; then
        print_pass "Inter-pod connectivity working"
    else
        print_error "Inter-pod connectivity check failed"
    fi
fi

# Check 14: ServiceMonitor (if enabled)
print_step "Checking ServiceMonitor..."
if kubectl get servicemonitor -n "$NAMESPACE" "$RELEASE_NAME" &> /dev/null 2>&1; then
    print_pass "ServiceMonitor exists"
elif kubectl api-resources | grep -q servicemonitors; then
    print_warn "ServiceMonitor not found (might not be enabled)"
else
    print_warn "ServiceMonitor CRD not available (Prometheus Operator not installed?)"
fi

# Check 15: PodDisruptionBudget
print_step "Checking PodDisruptionBudget..."
if kubectl get poddisruptionbudget -n "$NAMESPACE" "$RELEASE_NAME" &> /dev/null 2>&1; then
    print_pass "PodDisruptionBudget exists"
else
    print_warn "PodDisruptionBudget not found (might not be enabled)"
fi

# Summary
echo ""
echo "=========================================="
echo "Validation Summary"
echo "=========================================="
echo -e "${GREEN}Passed: $PASSED${NC}"
echo -e "${YELLOW}Warnings: $WARNINGS${NC}"
echo -e "${RED}Failed: $FAILED${NC}"
echo ""

if [ "$FAILED" -eq 0 ]; then
    echo -e "${GREEN}✓ All critical checks passed!${NC}"
    exit 0
else
    echo -e "${RED}✗ Some checks failed. Please review the errors above.${NC}"
    exit 1
fi
