#!/bin/bash
# Test rolling update with zero downtime
set -euo pipefail

NAMESPACE="${NAMESPACE:-anode}"
RELEASE_NAME="${RELEASE_NAME:-anode}"

# Get initial image
INITIAL_IMAGE=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.spec.template.spec.containers[0].image}')

echo "[INFO] Initial image: ${INITIAL_IMAGE}"

# Check if we can update the annotation to trigger a rollout
echo "[TEST] Triggering rolling update..."

# Add annotation to trigger update
kubectl patch statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" -p \
    "{\"spec\":{\"template\":{\"metadata\":{\"annotations\":{\"test-update\":\"$(date +%s)\"}}}}}"

echo "  Waiting for rollout to start..."
sleep 5

# Monitor the rollout
echo "[TEST] Monitoring rolling update..."

# Get number of replicas
REPLICAS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.spec.replicas}')

echo "  StatefulSet has ${REPLICAS} replica(s)"

# Track the update process
UPDATE_TIMEOUT=300
ELAPSED=0
INTERVAL=5

while [ $ELAPSED -lt $UPDATE_TIMEOUT ]; do
    # Check current revision
    CURRENT_REVISION=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
        -o jsonpath='{.status.currentRevision}' || echo "")
    UPDATE_REVISION=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
        -o jsonpath='{.status.updateRevision}' || echo "")

    CURRENT_REPLICAS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
        -o jsonpath='{.status.currentReplicas}' || echo "0")
    UPDATED_REPLICAS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
        -o jsonpath='{.status.updatedReplicas}' || echo "0")
    READY_REPLICAS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
        -o jsonpath='{.status.readyReplicas}' || echo "0")

    echo "  Current replicas: ${CURRENT_REPLICAS}, Updated: ${UPDATED_REPLICAS}, Ready: ${READY_REPLICAS}"

    # Check if update is complete
    if [ "$CURRENT_REVISION" = "$UPDATE_REVISION" ] && \
       [ "$UPDATED_REPLICAS" -eq "$REPLICAS" ] && \
       [ "$READY_REPLICAS" -eq "$REPLICAS" ]; then
        echo "[PASS] Rolling update completed"
        break
    fi

    # Check for pod failures during update
    FAILED_PODS=$(kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
        -o jsonpath='{.items[?(@.status.phase=="Failed")].metadata.name}' || echo "")

    if [ -n "$FAILED_PODS" ]; then
        echo "[FAIL] Pods failed during update: ${FAILED_PODS}"
        kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode"
        exit 1
    fi

    sleep $INTERVAL
    ELAPSED=$((ELAPSED + INTERVAL))
done

if [ $ELAPSED -ge $UPDATE_TIMEOUT ]; then
    echo "[FAIL] Rolling update did not complete in time"
    kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode"
    exit 1
fi

# Verify zero downtime - check that service remained available
echo "[TEST] Verifying service availability during update..."

# Check that all pods are ready
NOT_READY=$(kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
    -o jsonpath='{.items[?(@.status.containerStatuses[0].ready==false)].metadata.name}' || echo "")

if [ -n "$NOT_READY" ]; then
    echo "[FAIL] Some pods are not ready after update: ${NOT_READY}"
    exit 1
fi

echo "[PASS] All pods are ready after update"

# Verify service endpoints
echo "[TEST] Verifying service endpoints..."
ENDPOINTS=$(kubectl get endpoints -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.subsets[*].addresses[*].ip}' | wc -w)

if [ "$ENDPOINTS" -ne "$REPLICAS" ]; then
    echo "[FAIL] Service endpoint count (${ENDPOINTS}) does not match replica count (${REPLICAS})"
    kubectl describe endpoints -n "${NAMESPACE}" "${RELEASE_NAME}"
    exit 1
fi

echo "[PASS] Service endpoints are correct"

# Test pod connectivity
echo "[TEST] Testing pod connectivity..."
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
    echo "[PASS] Pod is responding after update"
else
    echo "[FAIL] Pod is not responding after update"
    cleanup
    exit 1
fi

cleanup
trap - EXIT

echo ""
echo "=========================================="
echo "Rolling update test completed successfully!"
echo "=========================================="
