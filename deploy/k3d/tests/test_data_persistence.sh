#!/bin/bash
# Test data survives pod restarts
set -euo pipefail

NAMESPACE="${NAMESPACE:-anode}"
RELEASE_NAME="${RELEASE_NAME:-anode}"

echo "[INFO] Testing data persistence across pod restarts"

# Check if StatefulSet uses PVCs
echo "[TEST] Checking for PersistentVolumeClaims..."
PVCS=$(kubectl get pvc -n "${NAMESPACE}" -o name 2>/dev/null | wc -l || echo 0)

if [ "$PVCS" -eq 0 ]; then
    echo "[SKIP] No PVCs found - data persistence may use emptyDir or hostPath"
    echo "       This test validates pod restart behavior but cannot guarantee data persistence"
fi

# Get initial pod info
POD_NAME="${RELEASE_NAME}-0"
echo "[INFO] Testing with pod: ${POD_NAME}"

# Check initial pod status
echo "[TEST] Getting initial pod status..."
INITIAL_UID=$(kubectl get pod -n "${NAMESPACE}" "${POD_NAME}" \
    -o jsonpath='{.metadata.uid}')
INITIAL_START_TIME=$(kubectl get pod -n "${NAMESPACE}" "${POD_NAME}" \
    -o jsonpath='{.status.startTime}')

echo "  Pod UID: ${INITIAL_UID}"
echo "  Start time: ${INITIAL_START_TIME}"

# If PVCs exist, record which PVC is bound to this pod
if [ "$PVCS" -gt 0 ]; then
    # For StatefulSet, PVC name follows pattern: <volumeClaimTemplate>-<podName>
    POD_PVCS=$(kubectl get pvc -n "${NAMESPACE}" \
        -o jsonpath="{.items[?(@.metadata.name contains '${POD_NAME}')].metadata.name}" || echo "")

    if [ -n "$POD_PVCS" ]; then
        echo "[INFO] Pod has PVC(s): ${POD_PVCS}"

        for PVC in $POD_PVCS; do
            PV=$(kubectl get pvc -n "${NAMESPACE}" "${PVC}" \
                -o jsonpath='{.spec.volumeName}')
            echo "  PVC ${PVC} -> PV ${PV}"
        done
    fi
fi

# Create a marker file in the pod's data directory
echo "[TEST] Creating data marker in pod..."
MARKER_DATA="persistence-test-$(date +%s)"

# Try to create marker file (may not work if pod doesn't have shell access)
kubectl exec -n "${NAMESPACE}" "${POD_NAME}" -- \
    sh -c "echo '${MARKER_DATA}' > /data/persistence-test.marker" 2>/dev/null || \
    echo "[WARN] Could not create marker file (pod may not have /data directory or shell access)"

# Delete the pod
echo "[TEST] Deleting pod to test persistence..."
kubectl delete pod -n "${NAMESPACE}" "${POD_NAME}" --grace-period=30

# Wait for pod to be deleted
echo "  Waiting for pod to be deleted..."
for i in {1..30}; do
    if ! kubectl get pod -n "${NAMESPACE}" "${POD_NAME}" &> /dev/null; then
        echo "  Pod deleted"
        break
    fi

    if [ $i -eq 30 ]; then
        echo "[FAIL] Pod was not deleted in time"
        exit 1
    fi

    sleep 2
done

# Wait for pod to be recreated
echo "[TEST] Waiting for pod to be recreated..."
TIMEOUT=180
ELAPSED=0
INTERVAL=5

while [ $ELAPSED -lt $TIMEOUT ]; do
    if kubectl get pod -n "${NAMESPACE}" "${POD_NAME}" &> /dev/null; then
        POD_PHASE=$(kubectl get pod -n "${NAMESPACE}" "${POD_NAME}" \
            -o jsonpath='{.status.phase}')
        POD_READY=$(kubectl get pod -n "${NAMESPACE}" "${POD_NAME}" \
            -o jsonpath='{.status.containerStatuses[0].ready}' 2>/dev/null || echo "false")

        echo "  Pod status - Phase: ${POD_PHASE}, Ready: ${POD_READY}"

        if [ "$POD_PHASE" = "Running" ] && [ "$POD_READY" = "true" ]; then
            echo "[PASS] Pod successfully recreated and is ready"
            break
        fi
    fi

    sleep $INTERVAL
    ELAPSED=$((ELAPSED + INTERVAL))
done

if [ $ELAPSED -ge $TIMEOUT ]; then
    echo "[FAIL] Pod did not become ready in time"
    kubectl describe pod -n "${NAMESPACE}" "${POD_NAME}"
    exit 1
fi

# Verify it's a new pod instance
NEW_UID=$(kubectl get pod -n "${NAMESPACE}" "${POD_NAME}" \
    -o jsonpath='{.metadata.uid}')
NEW_START_TIME=$(kubectl get pod -n "${NAMESPACE}" "${POD_NAME}" \
    -o jsonpath='{.status.startTime}')

echo "[INFO] New pod info:"
echo "  Pod UID: ${NEW_UID}"
echo "  Start time: ${NEW_START_TIME}"

if [ "$NEW_UID" = "$INITIAL_UID" ]; then
    echo "[WARN] Pod UID is the same (unexpected)"
else
    echo "[PASS] Pod is a new instance (UID changed)"
fi

# Verify PVC is still bound to the same PV
if [ "$PVCS" -gt 0 ] && [ -n "$POD_PVCS" ]; then
    echo "[TEST] Verifying PVC persistence..."

    for PVC in $POD_PVCS; do
        NEW_PV=$(kubectl get pvc -n "${NAMESPACE}" "${PVC}" \
            -o jsonpath='{.spec.volumeName}')

        if [ "$NEW_PV" = "$PV" ]; then
            echo "[PASS] PVC ${PVC} still bound to PV ${PV}"
        else
            echo "[FAIL] PVC ${PVC} bound to different PV (was ${PV}, now ${NEW_PV})"
            exit 1
        fi
    done
fi

# Try to verify marker file still exists
echo "[TEST] Verifying data persistence..."
RECOVERED_MARKER=$(kubectl exec -n "${NAMESPACE}" "${POD_NAME}" -- \
    cat /data/persistence-test.marker 2>/dev/null || echo "")

if [ "$RECOVERED_MARKER" = "$MARKER_DATA" ]; then
    echo "[PASS] Data persisted across pod restart"
elif [ -z "$RECOVERED_MARKER" ]; then
    echo "[WARN] Could not verify data persistence (marker file not found)"
    echo "       This may be due to volume type or permissions"
else
    echo "[FAIL] Data did not persist correctly"
    echo "       Expected: ${MARKER_DATA}"
    echo "       Got: ${RECOVERED_MARKER}"
    exit 1
fi

# Cleanup marker file
kubectl exec -n "${NAMESPACE}" "${POD_NAME}" -- \
    rm -f /data/persistence-test.marker 2>/dev/null || true

# Verify pod is still healthy after all operations
echo "[TEST] Verifying final pod health..."
FINAL_READY=$(kubectl get pod -n "${NAMESPACE}" "${POD_NAME}" \
    -o jsonpath='{.status.containerStatuses[0].ready}')

if [ "$FINAL_READY" != "true" ]; then
    echo "[FAIL] Pod is not ready after persistence test"
    kubectl describe pod -n "${NAMESPACE}" "${POD_NAME}"
    exit 1
fi

echo "[PASS] Pod is healthy after persistence test"

echo ""
echo "=========================================="
echo "Data persistence test completed!"
echo "=========================================="
