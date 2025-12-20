#!/bin/bash
# Test that deployment succeeds and pods are healthy
set -euo pipefail

NAMESPACE="${NAMESPACE:-anode}"
RELEASE_NAME="${RELEASE_NAME:-anode}"

# Test: Check StatefulSet exists
echo "[TEST] Checking if StatefulSet exists..."
if ! kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" &> /dev/null; then
    echo "[FAIL] StatefulSet not found"
    exit 1
fi
echo "[PASS] StatefulSet exists"

# Test: Check desired replicas
echo "[TEST] Checking StatefulSet replicas..."
DESIRED_REPLICAS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.spec.replicas}')
READY_REPLICAS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.status.readyReplicas}')

if [ -z "$READY_REPLICAS" ]; then
    READY_REPLICAS=0
fi

echo "  Desired: ${DESIRED_REPLICAS}, Ready: ${READY_REPLICAS}"

if [ "$READY_REPLICAS" -ne "$DESIRED_REPLICAS" ]; then
    echo "[FAIL] Not all replicas are ready"
    kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode"
    exit 1
fi
echo "[PASS] All replicas are ready"

# Test: Check pod status
echo "[TEST] Checking pod status..."
PODS=$(kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
    -o jsonpath='{.items[*].metadata.name}')

for POD in $PODS; do
    echo "  Checking pod: ${POD}"

    # Check pod phase
    PHASE=$(kubectl get pod -n "${NAMESPACE}" "${POD}" -o jsonpath='{.status.phase}')
    if [ "$PHASE" != "Running" ]; then
        echo "[FAIL] Pod ${POD} is not running (phase: ${PHASE})"
        kubectl describe pod -n "${NAMESPACE}" "${POD}"
        exit 1
    fi

    # Check container status
    READY=$(kubectl get pod -n "${NAMESPACE}" "${POD}" \
        -o jsonpath='{.status.containerStatuses[0].ready}')
    if [ "$READY" != "true" ]; then
        echo "[FAIL] Pod ${POD} container is not ready"
        kubectl describe pod -n "${NAMESPACE}" "${POD}"
        exit 1
    fi

    # Check restart count
    RESTARTS=$(kubectl get pod -n "${NAMESPACE}" "${POD}" \
        -o jsonpath='{.status.containerStatuses[0].restartCount}')
    if [ "$RESTARTS" -gt 3 ]; then
        echo "[WARN] Pod ${POD} has restarted ${RESTARTS} times"
    fi

    echo "  Pod ${POD} is healthy (phase: ${PHASE}, ready: ${READY}, restarts: ${RESTARTS})"
done
echo "[PASS] All pods are healthy"

# Test: Check service exists
echo "[TEST] Checking if Service exists..."
if ! kubectl get service -n "${NAMESPACE}" "${RELEASE_NAME}" &> /dev/null; then
    echo "[FAIL] Service not found"
    exit 1
fi
echo "[PASS] Service exists"

# Test: Check service endpoints
echo "[TEST] Checking Service endpoints..."
ENDPOINTS=$(kubectl get endpoints -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.subsets[*].addresses[*].ip}' | wc -w)

if [ "$ENDPOINTS" -eq 0 ]; then
    echo "[FAIL] Service has no endpoints"
    kubectl describe endpoints -n "${NAMESPACE}" "${RELEASE_NAME}"
    exit 1
fi

echo "  Service has ${ENDPOINTS} endpoint(s)"
echo "[PASS] Service endpoints are configured"

# Test: Check PVCs if they exist
echo "[TEST] Checking PersistentVolumeClaims..."
PVCS=$(kubectl get pvc -n "${NAMESPACE}" -o name 2>/dev/null | wc -l || echo 0)

if [ "$PVCS" -gt 0 ]; then
    echo "  Found ${PVCS} PVC(s)"

    # Check if all PVCs are bound
    UNBOUND=$(kubectl get pvc -n "${NAMESPACE}" \
        -o jsonpath='{.items[?(@.status.phase!="Bound")].metadata.name}' || echo "")

    if [ -n "$UNBOUND" ]; then
        echo "[FAIL] Some PVCs are not bound: ${UNBOUND}"
        kubectl get pvc -n "${NAMESPACE}"
        exit 1
    fi

    echo "[PASS] All PVCs are bound"
else
    echo "  No PVCs found (may not be required)"
fi

# Test: Check health endpoint
echo "[TEST] Checking health endpoint..."
POD_NAME="${RELEASE_NAME}-0"

# Port forward in background
kubectl port-forward -n "${NAMESPACE}" "pod/${POD_NAME}" 8081:8081 &
PF_PID=$!
sleep 3

# Cleanup function
cleanup() {
    kill $PF_PID 2>/dev/null || true
}
trap cleanup EXIT

# Check health endpoint
if curl -sf http://localhost:8081/health > /dev/null 2>&1; then
    echo "[PASS] Health endpoint is accessible"
else
    echo "[FAIL] Health endpoint is not accessible"
    cleanup
    exit 1
fi

cleanup
trap - EXIT

echo ""
echo "=========================================="
echo "Deployment test completed successfully!"
echo "=========================================="
