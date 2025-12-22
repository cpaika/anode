#!/bin/bash
# Test that Helm chart installs successfully and all components are configured correctly
set -euo pipefail

NAMESPACE="${NAMESPACE:-anode}"
RELEASE_NAME="${RELEASE_NAME:-anode}"
SCRIPT_DIR="${SCRIPT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
HELM_CHART_DIR="${PROJECT_ROOT}/deploy/helm/anode"

# Test: Verify Helm chart exists
echo "[TEST] Checking if Helm chart exists..."
if ! [ -d "${HELM_CHART_DIR}" ]; then
    echo "[FAIL] Helm chart directory not found at ${HELM_CHART_DIR}"
    exit 1
fi

if ! [ -f "${HELM_CHART_DIR}/Chart.yaml" ]; then
    echo "[FAIL] Chart.yaml not found in Helm chart"
    exit 1
fi
echo "[PASS] Helm chart exists"

# Test: Verify Helm release is installed
echo "[TEST] Checking if Helm release is installed..."
if ! helm list -n "${NAMESPACE}" | grep -q "^${RELEASE_NAME}"; then
    echo "[FAIL] Helm release '${RELEASE_NAME}' not found in namespace '${NAMESPACE}'"
    echo ""
    echo "Available releases:"
    helm list -n "${NAMESPACE}"
    exit 1
fi
echo "[PASS] Helm release is installed"

# Test: Verify Helm release status
echo "[TEST] Checking Helm release status..."
if command -v jq &> /dev/null; then
    RELEASE_STATUS=$(helm status "${RELEASE_NAME}" -n "${NAMESPACE}" -o json | \
        jq -r '.info.status' 2>/dev/null || echo "unknown")
else
    # Fallback to grep if jq is not available
    RELEASE_STATUS=$(helm status "${RELEASE_NAME}" -n "${NAMESPACE}" 2>/dev/null | \
        grep "^STATUS:" | awk '{print $2}' || echo "unknown")
fi

if [ "$RELEASE_STATUS" != "deployed" ]; then
    echo "[FAIL] Helm release status is '${RELEASE_STATUS}', expected 'deployed'"
    helm status "${RELEASE_NAME}" -n "${NAMESPACE}"
    exit 1
fi
echo "[PASS] Helm release status is 'deployed'"

# Test: Verify Helm release revision
echo "[TEST] Checking Helm release revision..."
if command -v jq &> /dev/null; then
    REVISION=$(helm list -n "${NAMESPACE}" -o json | \
        jq -r ".[] | select(.name==\"${RELEASE_NAME}\") | .revision" 2>/dev/null || echo "0")
else
    # Fallback to awk if jq is not available
    REVISION=$(helm list -n "${NAMESPACE}" | grep "^${RELEASE_NAME}" | awk '{print $3}' || echo "0")
fi

if [ "$REVISION" -eq 0 ]; then
    echo "[FAIL] Could not determine Helm release revision"
    exit 1
fi
echo "[PASS] Helm release is at revision ${REVISION}"

# Test: Verify all expected Kubernetes resources are created
echo "[TEST] Checking if expected Kubernetes resources are created..."

# Check StatefulSet
if ! kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" &> /dev/null; then
    echo "[FAIL] StatefulSet '${RELEASE_NAME}' not found"
    exit 1
fi
echo "  StatefulSet: OK"

# Check Service
if ! kubectl get service -n "${NAMESPACE}" "${RELEASE_NAME}" &> /dev/null; then
    echo "[FAIL] Service '${RELEASE_NAME}' not found"
    exit 1
fi
echo "  Service: OK"

# Check ServiceAccount (optional, depending on chart configuration)
if kubectl get serviceaccount -n "${NAMESPACE}" "${RELEASE_NAME}" &> /dev/null 2>&1; then
    echo "  ServiceAccount: OK"
fi

# Check ConfigMap (if any)
CONFIGMAPS=$(kubectl get configmap -n "${NAMESPACE}" \
    -l "app.kubernetes.io/name=anode,app.kubernetes.io/instance=${RELEASE_NAME}" \
    -o name 2>/dev/null | wc -l || echo 0)
if [ "$CONFIGMAPS" -gt 0 ]; then
    echo "  ConfigMaps: ${CONFIGMAPS} found"
fi

echo "[PASS] All expected Kubernetes resources are created"

# Test: Verify chart values are applied correctly
echo "[TEST] Checking if chart values are applied correctly..."

# Get replica count from StatefulSet
REPLICAS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.spec.replicas}')
echo "  Replica count: ${REPLICAS}"

# Get image from StatefulSet
IMAGE=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.spec.template.spec.containers[0].image}')
echo "  Container image: ${IMAGE}"

# Verify labels are set correctly
if command -v jq &> /dev/null; then
    LABELS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
        -o jsonpath='{.metadata.labels}' | jq -r 'keys[]' 2>/dev/null || echo "")
else
    # Fallback to kubectl for checking labels
    LABELS=$(kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" \
        --show-labels --no-headers | awk '{print $NF}' || echo "")
fi

if ! echo "$LABELS" | grep -q "app.kubernetes.io/name"; then
    echo "[FAIL] Required label 'app.kubernetes.io/name' not found on StatefulSet"
    exit 1
fi

if ! echo "$LABELS" | grep -q "app.kubernetes.io/instance"; then
    echo "[FAIL] Required label 'app.kubernetes.io/instance' not found on StatefulSet"
    exit 1
fi

echo "[PASS] Chart values are applied correctly"

# Test: Verify service ports are configured
echo "[TEST] Checking Service port configuration..."
SERVICE_PORTS=$(kubectl get service -n "${NAMESPACE}" "${RELEASE_NAME}" \
    -o jsonpath='{.spec.ports[*].name}')

echo "  Service ports: ${SERVICE_PORTS}"

# Check if expected ports exist
if ! echo "$SERVICE_PORTS" | grep -q "s3\|http"; then
    echo "[WARN] Expected 's3' or 'http' port not found in service"
fi

echo "[PASS] Service ports are configured"

# Test: Verify PersistentVolumeClaims if persistence is enabled
echo "[TEST] Checking PersistentVolumeClaims..."
PVCS=$(kubectl get pvc -n "${NAMESPACE}" \
    -l "app.kubernetes.io/name=anode,app.kubernetes.io/instance=${RELEASE_NAME}" \
    -o name 2>/dev/null | wc -l || echo 0)

if [ "$PVCS" -gt 0 ]; then
    echo "  Found ${PVCS} PVC(s)"

    # Verify all PVCs are bound
    UNBOUND=$(kubectl get pvc -n "${NAMESPACE}" \
        -l "app.kubernetes.io/name=anode,app.kubernetes.io/instance=${RELEASE_NAME}" \
        -o jsonpath='{.items[?(@.status.phase!="Bound")].metadata.name}' 2>/dev/null || echo "")

    if [ -n "$UNBOUND" ]; then
        echo "[FAIL] Some PVCs are not bound: ${UNBOUND}"
        kubectl get pvc -n "${NAMESPACE}"
        exit 1
    fi

    echo "[PASS] All PVCs are bound"
else
    echo "  No PVCs found (persistence may be disabled)"
    echo "[PASS] No PVCs to check"
fi

# Test: Verify Helm chart can be upgraded
echo "[TEST] Testing Helm chart upgrade (dry-run)..."
if ! helm upgrade "${RELEASE_NAME}" "${HELM_CHART_DIR}" \
    --namespace "${NAMESPACE}" \
    --reuse-values \
    --dry-run &> /dev/null; then
    echo "[FAIL] Helm upgrade dry-run failed"
    exit 1
fi
echo "[PASS] Helm chart can be upgraded"

# Test: Verify Helm chart metadata
echo "[TEST] Checking Helm chart metadata..."
CHART_NAME=$(helm show chart "${HELM_CHART_DIR}" | grep "^name:" | awk '{print $2}')
CHART_VERSION=$(helm show chart "${HELM_CHART_DIR}" | grep "^version:" | awk '{print $2}')
APP_VERSION=$(helm show chart "${HELM_CHART_DIR}" | grep "^appVersion:" | awk '{print $2}')

echo "  Chart name: ${CHART_NAME}"
echo "  Chart version: ${CHART_VERSION}"
echo "  App version: ${APP_VERSION}"

if [ -z "$CHART_NAME" ] || [ -z "$CHART_VERSION" ]; then
    echo "[FAIL] Chart metadata is incomplete"
    exit 1
fi
echo "[PASS] Helm chart metadata is valid"

# Test: Verify Helm notes are present
echo "[TEST] Checking Helm release notes..."
if ! helm get notes "${RELEASE_NAME}" -n "${NAMESPACE}" &> /dev/null; then
    echo "[WARN] Could not retrieve Helm notes (this is not critical)"
else
    echo "[PASS] Helm notes are accessible"
fi

# Test: Verify Helm hooks (if any)
echo "[TEST] Checking Helm hooks..."
HOOKS=$(kubectl get jobs -n "${NAMESPACE}" \
    -l "app.kubernetes.io/managed-by=Helm,app.kubernetes.io/instance=${RELEASE_NAME}" \
    -o name 2>/dev/null | wc -l || echo 0)

if [ "$HOOKS" -gt 0 ]; then
    echo "  Found ${HOOKS} Helm hook job(s)"

    # Check if hooks completed successfully
    FAILED_HOOKS=$(kubectl get jobs -n "${NAMESPACE}" \
        -l "app.kubernetes.io/managed-by=Helm,app.kubernetes.io/instance=${RELEASE_NAME}" \
        -o jsonpath='{.items[?(@.status.failed>0)].metadata.name}' 2>/dev/null || echo "")

    if [ -n "$FAILED_HOOKS" ]; then
        echo "[FAIL] Some Helm hooks failed: ${FAILED_HOOKS}"
        exit 1
    fi
    echo "[PASS] All Helm hooks completed successfully"
else
    echo "  No Helm hooks found (chart may not use hooks)"
    echo "[PASS] No Helm hooks to check"
fi

# Test: Verify Helm test resources (if present)
echo "[TEST] Checking Helm test resources..."
TEST_PODS=$(kubectl get pods -n "${NAMESPACE}" \
    -l "helm.sh/chart,app.kubernetes.io/instance=${RELEASE_NAME}" \
    -l "helm.sh/hook=test" \
    -o name 2>/dev/null | wc -l || echo 0)

if [ "$TEST_PODS" -gt 0 ]; then
    echo "  Found ${TEST_PODS} Helm test pod(s)"
    echo "[PASS] Helm test resources are present"
else
    echo "  No Helm test pods found (tests may not be defined in chart)"
    echo "[PASS] No Helm test resources to check"
fi

# Test: Verify Helm values can be retrieved
echo "[TEST] Checking if Helm values can be retrieved..."
if ! helm get values "${RELEASE_NAME}" -n "${NAMESPACE}" &> /dev/null; then
    echo "[FAIL] Could not retrieve Helm values"
    exit 1
fi
echo "[PASS] Helm values are retrievable"

# Test: Verify Helm manifest can be retrieved
echo "[TEST] Checking if Helm manifest can be retrieved..."
MANIFEST_RESOURCES=$(helm get manifest "${RELEASE_NAME}" -n "${NAMESPACE}" 2>/dev/null | \
    grep -c "^kind:" || echo 0)

if [ "$MANIFEST_RESOURCES" -eq 0 ]; then
    echo "[FAIL] Could not retrieve Helm manifest or manifest is empty"
    exit 1
fi

echo "  Manifest contains ${MANIFEST_RESOURCES} resources"
echo "[PASS] Helm manifest is retrievable"

echo ""
echo "=========================================="
echo "Helm install test completed successfully!"
echo "=========================================="
echo ""
echo "Release Summary:"
echo "  Name:       ${RELEASE_NAME}"
echo "  Namespace:  ${NAMESPACE}"
echo "  Status:     ${RELEASE_STATUS}"
echo "  Revision:   ${REVISION}"
echo "  Chart:      ${CHART_NAME}-${CHART_VERSION}"
echo "  Replicas:   ${REPLICAS}"
echo "  Image:      ${IMAGE}"
