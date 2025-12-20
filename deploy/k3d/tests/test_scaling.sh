#!/bin/bash
# Test scaling up and down
set -euo pipefail

NAMESPACE="${NAMESPACE:-anode}"
RELEASE_NAME="${RELEASE_NAME:-anode}"

# Get initial replica count
INITIAL_REPLICAS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.spec.replicas}')

echo "[INFO] Initial replicas: ${INITIAL_REPLICAS}"

# Test: Scale up
echo "[TEST] Scaling up to $((INITIAL_REPLICAS + 1)) replicas..."
NEW_REPLICAS=$((INITIAL_REPLICAS + 1))

kubectl scale statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" --replicas="${NEW_REPLICAS}"

# Wait for scale up
echo "  Waiting for scale up to complete..."
if ! kubectl wait --for=jsonpath='{.status.readyReplicas}'="${NEW_REPLICAS}" \
    statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" --timeout=120s; then
    echo "[FAIL] Scale up did not complete in time"
    kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode"
    exit 1
fi

# Verify all pods are running
RUNNING_PODS=$(kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
    -o jsonpath='{.items[?(@.status.phase=="Running")].metadata.name}' | wc -w)

if [ "$RUNNING_PODS" -ne "$NEW_REPLICAS" ]; then
    echo "[FAIL] Not all pods are running after scale up"
    kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode"
    exit 1
fi

echo "[PASS] Successfully scaled up to ${NEW_REPLICAS} replicas"

# Verify new pod is healthy
echo "[TEST] Verifying new pod is healthy..."
NEW_POD="${RELEASE_NAME}-${INITIAL_REPLICAS}"

if ! kubectl get pod -n "${NAMESPACE}" "${NEW_POD}" &> /dev/null; then
    echo "[FAIL] New pod ${NEW_POD} not found"
    exit 1
fi

POD_READY=$(kubectl get pod -n "${NAMESPACE}" "${NEW_POD}" \
    -o jsonpath='{.status.containerStatuses[0].ready}')

if [ "$POD_READY" != "true" ]; then
    echo "[FAIL] New pod ${NEW_POD} is not ready"
    kubectl describe pod -n "${NAMESPACE}" "${NEW_POD}"
    exit 1
fi

echo "[PASS] New pod is healthy"

# Test: Scale down
echo "[TEST] Scaling down to ${INITIAL_REPLICAS} replicas..."

kubectl scale statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" --replicas="${INITIAL_REPLICAS}"

# Wait for scale down
echo "  Waiting for scale down to complete..."
if ! kubectl wait --for=jsonpath='{.status.readyReplicas}'="${INITIAL_REPLICAS}" \
    statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" --timeout=120s; then
    echo "[FAIL] Scale down did not complete in time"
    kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode"
    exit 1
fi

# Verify pod is terminated
echo "  Waiting for pod termination..."
for i in {1..30}; do
    if ! kubectl get pod -n "${NAMESPACE}" "${NEW_POD}" &> /dev/null; then
        echo "[PASS] Successfully scaled down to ${INITIAL_REPLICAS} replicas"
        break
    fi

    if [ $i -eq 30 ]; then
        echo "[FAIL] Pod ${NEW_POD} was not terminated"
        exit 1
    fi

    sleep 2
done

# Verify remaining pods are still healthy
echo "[TEST] Verifying remaining pods are healthy..."
RUNNING_PODS=$(kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
    -o jsonpath='{.items[?(@.status.phase=="Running")].metadata.name}' | wc -w)

if [ "$RUNNING_PODS" -ne "$INITIAL_REPLICAS" ]; then
    echo "[FAIL] Unexpected number of running pods after scale down"
    kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode"
    exit 1
fi

# Check all remaining pods are ready
NOT_READY=$(kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
    -o jsonpath='{.items[?(@.status.containerStatuses[0].ready==false)].metadata.name}' || echo "")

if [ -n "$NOT_READY" ]; then
    echo "[FAIL] Some pods are not ready after scale down: ${NOT_READY}"
    exit 1
fi

echo "[PASS] All remaining pods are healthy"

echo ""
echo "=========================================="
echo "Scaling test completed successfully!"
echo "=========================================="
