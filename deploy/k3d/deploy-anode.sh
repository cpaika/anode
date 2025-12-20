#!/bin/bash
# Deploy anode to K3d cluster using Helm
set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
HELM_CHART_DIR="${PROJECT_ROOT}/deploy/helm/anode"
NAMESPACE="${NAMESPACE:-anode}"
RELEASE_NAME="${RELEASE_NAME:-anode}"
REGISTRY_PORT="5111"
IMAGE_REPOSITORY="localhost:${REGISTRY_PORT}/anode"
IMAGE_TAG="${IMAGE_TAG:-latest}"

# Logging functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

# Check dependencies
check_dependencies() {
    log_info "Checking dependencies..."

    local missing_deps=()

    if ! command -v kubectl &> /dev/null; then
        missing_deps+=("kubectl")
    fi

    if ! command -v helm &> /dev/null; then
        missing_deps+=("helm")
    fi

    if [ ${#missing_deps[@]} -ne 0 ]; then
        log_error "Missing required dependencies: ${missing_deps[*]}"
        exit 1
    fi

    log_info "All dependencies found"
}

# Check cluster connectivity
check_cluster() {
    log_info "Checking cluster connectivity..."

    if ! kubectl cluster-info &> /dev/null; then
        log_error "Cannot connect to Kubernetes cluster"
        log_info "Make sure cluster is running: ./create-cluster.sh"
        exit 1
    fi

    local context=$(kubectl config current-context)
    log_info "Connected to cluster: ${context}"
}

# Check Helm chart
check_helm_chart() {
    log_info "Checking Helm chart..."

    if ! [ -d "${HELM_CHART_DIR}" ]; then
        log_error "Helm chart not found at ${HELM_CHART_DIR}"
        exit 1
    fi

    if ! [ -f "${HELM_CHART_DIR}/Chart.yaml" ]; then
        log_error "Chart.yaml not found in ${HELM_CHART_DIR}"
        exit 1
    fi

    log_info "Helm chart found"
}

# Create namespace
create_namespace() {
    log_step "Creating namespace: ${NAMESPACE}"

    if kubectl get namespace "${NAMESPACE}" &> /dev/null; then
        log_info "Namespace '${NAMESPACE}' already exists"
    else
        kubectl create namespace "${NAMESPACE}"
        log_info "Namespace '${NAMESPACE}' created"
    fi
}

# Check if image exists in registry
check_image() {
    log_info "Checking if image exists in registry..."

    local manifest_url="http://localhost:${REGISTRY_PORT}/v2/anode/manifests/${IMAGE_TAG}"
    if ! curl -sf -o /dev/null "${manifest_url}"; then
        log_warn "Image ${IMAGE_REPOSITORY}:${IMAGE_TAG} not found in registry"
        log_info "Build image first: ./build-and-load.sh"
        exit 1
    fi

    log_info "Image found in registry"
}

# Deploy or upgrade with Helm
deploy_helm() {
    log_step "Deploying anode with Helm"
    echo ""

    local helm_args=(
        upgrade
        "${RELEASE_NAME}"
        "${HELM_CHART_DIR}"
        --install
        --namespace "${NAMESPACE}"
        --set "image.repository=${IMAGE_REPOSITORY}"
        --set "image.tag=${IMAGE_TAG}"
        --set "image.pullPolicy=Always"
        --wait
        --timeout 5m
    )

    # Add any additional values from environment or file
    if [ -n "${HELM_VALUES_FILE:-}" ] && [ -f "${HELM_VALUES_FILE}" ]; then
        helm_args+=(--values "${HELM_VALUES_FILE}")
        log_info "Using values file: ${HELM_VALUES_FILE}"
    fi

    if helm "${helm_args[@]}"; then
        log_info "Deployment successful"
    else
        log_error "Deployment failed"
        exit 1
    fi
}

# Wait for pods to be ready
wait_for_pods() {
    log_step "Waiting for pods to be ready..."

    local max_wait=300
    local elapsed=0
    local interval=5

    while [ $elapsed -lt $max_wait ]; do
        local ready_pods=$(kubectl get pods -n "${NAMESPACE}" \
            -l "app.kubernetes.io/name=anode" \
            -o jsonpath='{.items[?(@.status.phase=="Running")].metadata.name}' \
            2>/dev/null | wc -w || echo 0)

        local total_pods=$(kubectl get pods -n "${NAMESPACE}" \
            -l "app.kubernetes.io/name=anode" \
            --no-headers 2>/dev/null | wc -l || echo 0)

        log_info "Ready pods: ${ready_pods}/${total_pods}"

        if [ "${ready_pods}" -gt 0 ] && [ "${ready_pods}" -eq "${total_pods}" ]; then
            # Also check if containers are ready
            local not_ready=$(kubectl get pods -n "${NAMESPACE}" \
                -l "app.kubernetes.io/name=anode" \
                -o jsonpath='{.items[?(@.status.containerStatuses[*].ready==false)].metadata.name}' \
                2>/dev/null || echo "")

            if [ -z "$not_ready" ]; then
                log_info "All pods are ready!"
                return 0
            fi
        fi

        sleep $interval
        elapsed=$((elapsed + interval))
    done

    log_error "Pods did not become ready in time"
    kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode"
    return 1
}

# Display deployment status
display_status() {
    echo ""
    log_info "Deployment status:"
    echo ""

    echo "Pods:"
    kubectl get pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" -o wide

    echo ""
    echo "Services:"
    kubectl get services -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode"

    echo ""
    echo "StatefulSets:"
    kubectl get statefulsets -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" 2>/dev/null || echo "  No StatefulSets found"

    echo ""
    echo "PersistentVolumeClaims:"
    kubectl get pvc -n "${NAMESPACE}" 2>/dev/null || echo "  No PVCs found"
}

# Display access information
display_access_info() {
    echo ""
    log_info "Access information:"
    echo ""

    # Get service details
    local s3_port=$(kubectl get svc -n "${NAMESPACE}" "${RELEASE_NAME}" \
        -o jsonpath='{.spec.ports[?(@.name=="s3")].port}' 2>/dev/null || echo "8080")
    local admin_port=$(kubectl get svc -n "${NAMESPACE}" "${RELEASE_NAME}" \
        -o jsonpath='{.spec.ports[?(@.name=="admin")].port}' 2>/dev/null || echo "8081")

    echo "  Namespace:       ${NAMESPACE}"
    echo "  Release:         ${RELEASE_NAME}"
    echo "  Image:           ${IMAGE_REPOSITORY}:${IMAGE_TAG}"
    echo ""
    echo "  S3 API:          kubectl port-forward -n ${NAMESPACE} svc/${RELEASE_NAME} ${s3_port}:${s3_port}"
    echo "  Admin API:       kubectl port-forward -n ${NAMESPACE} svc/${RELEASE_NAME} ${admin_port}:${admin_port}"
    echo ""
    echo "  View logs:       kubectl logs -n ${NAMESPACE} -l app.kubernetes.io/name=anode -f"
    echo "  Shell access:    kubectl exec -n ${NAMESPACE} -it ${RELEASE_NAME}-0 -- /bin/sh"
}

# Main execution
main() {
    log_info "Deploying anode to K3d cluster"
    echo ""

    check_dependencies
    check_cluster
    check_helm_chart
    check_image
    create_namespace
    deploy_helm
    wait_for_pods
    display_status
    display_access_info

    echo ""
    log_info "Deployment complete!"
    log_info "Run tests with: ./run-tests.sh"
}

main "$@"
