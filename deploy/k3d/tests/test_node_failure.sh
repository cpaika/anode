#!/bin/bash
# Test recovery from node failure
set -euo pipefail

NAMESPACE="${NAMESPACE:-anode}"
RELEASE_NAME="${RELEASE_NAME:-anode}"

# Get cluster info
echo "[INFO] Getting cluster information..."
NODES=$(kubectl get nodes -o jsonpath='{.items[*].metadata.name}')
NODE_COUNT=$(echo "$NODES" | wc -w)

echo "  Cluster has ${NODE_COUNT} node(s)"

if [ "$NODE_COUNT" -lt 2 ]; then
    echo "[SKIP] Test requires at least 2 nodes, but cluster has ${NODE_COUNT}"
    exit 0
fi

# Get initial pod distribution
echo "[INFO] Initial pod distribution:"
kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
    -o custom-columns=NAME:.metadata.name,NODE:.spec.nodeName --no-headers

# Find a node with anode pods
echo "[TEST] Finding node to cordon..."
TARGET_NODE=""

for NODE in $NODES; do
    POD_COUNT=$(kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
        --field-selector spec.nodeName="${NODE}" --no-headers 2>/dev/null | wc -l)

    if [ "$POD_COUNT" -gt 0 ]; then
        # Don't target the control plane node
        if [[ ! "$NODE" =~ server ]]; then
            TARGET_NODE="$NODE"
            echo "  Found node with ${POD_COUNT} pod(s): ${TARGET_NODE}"
            break
        fi
    fi
done

if [ -z "$TARGET_NODE" ]; then
    echo "[SKIP] Could not find a suitable worker node to test"
    exit 0
fi

# Get pods on target node before cordoning
PODS_ON_NODE=$(kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
    --field-selector spec.nodeName="${TARGET_NODE}" \
    -o jsonpath='{.items[*].metadata.name}')

echo "  Pods on ${TARGET_NODE}: ${PODS_ON_NODE}"

# Cordon the node
echo "[TEST] Cordoning node ${TARGET_NODE}..."
if ! kubectl cordon "${TARGET_NODE}"; then
    echo "[FAIL] Failed to cordon node"
    exit 1
fi

# Verify node is cordoned
NODE_STATUS=$(kubectl get node "${TARGET_NODE}" \
    -o jsonpath='{.spec.unschedulable}')

if [ "$NODE_STATUS" != "true" ]; then
    echo "[FAIL] Node was not successfully cordoned"
    kubectl uncordon "${TARGET_NODE}"
    exit 1
fi

echo "[PASS] Node successfully cordoned"

# Delete a pod on the cordoned node to simulate failure
if [ -n "$PODS_ON_NODE" ]; then
    TEST_POD=$(echo "$PODS_ON_NODE" | awk '{print $1}')
    echo "[TEST] Deleting pod ${TEST_POD} to simulate failure..."

    kubectl delete pod -n "${NAMESPACE}" "${TEST_POD}" --grace-period=0 --force

    # Wait for pod to be recreated
    echo "  Waiting for pod to be recreated..."
    sleep 5

    # Check if pod was recreated on a different node
    TIMEOUT=120
    ELAPSED=0
    INTERVAL=5

    while [ $ELAPSED -lt $TIMEOUT ]; do
        if kubectl get pod -n "${NAMESPACE}" "${TEST_POD}" &> /dev/null; then
            POD_PHASE=$(kubectl get pod -n "${NAMESPACE}" "${TEST_POD}" \
                -o jsonpath='{.status.phase}')
            POD_NODE=$(kubectl get pod -n "${NAMESPACE}" "${TEST_POD}" \
                -o jsonpath='{.spec.nodeName}')
            POD_READY=$(kubectl get pod -n "${NAMESPACE}" "${TEST_POD}" \
                -o jsonpath='{.status.containerStatuses[0].ready}' 2>/dev/null || echo "false")

            echo "  Pod status - Phase: ${POD_PHASE}, Node: ${POD_NODE}, Ready: ${POD_READY}"

            if [ "$POD_PHASE" = "Running" ] && [ "$POD_READY" = "true" ]; then
                if [ "$POD_NODE" != "$TARGET_NODE" ]; then
                    echo "[PASS] Pod recreated on different node: ${POD_NODE}"
                    break
                else
                    echo "[WARN] Pod recreated on same cordoned node"
                fi
            fi
        fi

        sleep $INTERVAL
        ELAPSED=$((ELAPSED + INTERVAL))
    done

    if [ $ELAPSED -ge $TIMEOUT ]; then
        echo "[FAIL] Pod did not recover in time"
        kubectl describe pod -n "${NAMESPACE}" "${TEST_POD}"
        kubectl uncordon "${TARGET_NODE}"
        exit 1
    fi
fi

# Verify cluster health
echo "[TEST] Verifying cluster health after node failure..."

DESIRED_REPLICAS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.spec.replicas}')
READY_REPLICAS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.status.readyReplicas}' || echo "0")

if [ "$READY_REPLICAS" -ne "$DESIRED_REPLICAS" ]; then
    echo "[FAIL] Not all replicas are ready after node failure"
    kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode"
    kubectl uncordon "${TARGET_NODE}"
    exit 1
fi

echo "[PASS] All replicas are healthy"

# Uncordon the node
echo "[TEST] Uncordoning node ${TARGET_NODE}..."
kubectl uncordon "${TARGET_NODE}"

NODE_STATUS=$(kubectl get node "${TARGET_NODE}" \
    -o jsonpath='{.spec.unschedulable}')

if [ "$NODE_STATUS" = "true" ]; then
    echo "[FAIL] Failed to uncordon node"
    exit 1
fi

echo "[PASS] Node successfully uncordoned"

# Display final pod distribution
echo "[INFO] Final pod distribution:"
kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
    -o custom-columns=NAME:.metadata.name,NODE:.spec.nodeName --no-headers

echo ""
echo "=========================================="
echo "Node failure test completed successfully!"
echo "=========================================="
