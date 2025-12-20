#!/bin/bash
# Create K3d cluster for anode testing
set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLUSTER_CONFIG="${SCRIPT_DIR}/cluster.yaml"
CLUSTER_NAME="anode-test"
REGISTRY_NAME="anode-registry.localhost"
REGISTRY_PORT="5111"

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

# Check dependencies
check_dependencies() {
    log_info "Checking dependencies..."

    local missing_deps=()

    if ! command -v k3d &> /dev/null; then
        missing_deps+=("k3d")
    fi

    if ! command -v kubectl &> /dev/null; then
        missing_deps+=("kubectl")
    fi

    if ! command -v docker &> /dev/null; then
        missing_deps+=("docker")
    fi

    if [ ${#missing_deps[@]} -ne 0 ]; then
        log_error "Missing required dependencies: ${missing_deps[*]}"
        log_info "Install with:"
        for dep in "${missing_deps[@]}"; do
            case $dep in
                k3d)
                    echo "  brew install k3d"
                    ;;
                kubectl)
                    echo "  brew install kubectl"
                    ;;
                docker)
                    echo "  brew install --cask docker"
                    ;;
            esac
        done
        exit 1
    fi

    log_info "All dependencies found"
}

# Check if Docker is running
check_docker() {
    log_info "Checking Docker daemon..."
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running. Please start Docker Desktop."
        exit 1
    fi
    log_info "Docker daemon is running"
}

# Cleanup on error
cleanup_on_error() {
    log_error "Cluster creation failed. Cleaning up..."
    k3d cluster delete "${CLUSTER_NAME}" 2>/dev/null || true
    k3d registry delete "${REGISTRY_NAME}" 2>/dev/null || true
}

# Trap errors
trap cleanup_on_error ERR

# Check if cluster already exists
check_existing_cluster() {
    if k3d cluster list | grep -q "${CLUSTER_NAME}"; then
        log_warn "Cluster '${CLUSTER_NAME}' already exists"
        read -p "Do you want to delete and recreate it? [y/N] " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            log_info "Deleting existing cluster..."
            k3d cluster delete "${CLUSTER_NAME}"
            # Also delete registry if it exists
            k3d registry delete "${REGISTRY_NAME}" 2>/dev/null || true
        else
            log_info "Using existing cluster"
            return 1
        fi
    fi
    return 0
}

# Create cluster
create_cluster() {
    log_info "Creating K3d cluster '${CLUSTER_NAME}'..."

    if ! k3d cluster create --config "${CLUSTER_CONFIG}"; then
        log_error "Failed to create cluster"
        exit 1
    fi

    log_info "Cluster created successfully"
}

# Wait for cluster to be ready
wait_for_cluster() {
    log_info "Waiting for cluster to be ready..."

    local max_attempts=30
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        if kubectl get nodes &> /dev/null; then
            local ready_nodes=$(kubectl get nodes --no-headers 2>/dev/null | grep -c " Ready " || true)
            local total_nodes=$(kubectl get nodes --no-headers 2>/dev/null | wc -l || echo 0)

            log_info "Ready nodes: ${ready_nodes}/${total_nodes}"

            if [ "${ready_nodes}" -eq "${total_nodes}" ] && [ "${total_nodes}" -gt 0 ]; then
                log_info "All nodes are ready"
                return 0
            fi
        fi

        attempt=$((attempt + 1))
        sleep 5
    done

    log_error "Cluster did not become ready in time"
    return 1
}

# Verify registry
verify_registry() {
    log_info "Verifying registry at localhost:${REGISTRY_PORT}..."

    local max_attempts=10
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        if curl -s "http://localhost:${REGISTRY_PORT}/v2/" > /dev/null 2>&1; then
            log_info "Registry is accessible"
            return 0
        fi

        attempt=$((attempt + 1))
        sleep 2
    done

    log_warn "Registry may not be accessible, but continuing..."
    return 0
}

# Setup kubectl context
setup_kubectl() {
    log_info "Setting up kubectl context..."

    kubectl config use-context "k3d-${CLUSTER_NAME}"

    log_info "Current context: $(kubectl config current-context)"
}

# Display cluster info
display_cluster_info() {
    log_info "Cluster information:"
    echo ""
    echo "  Cluster Name:    ${CLUSTER_NAME}"
    echo "  Registry:        localhost:${REGISTRY_PORT}"
    echo "  Kubeconfig:      ${KUBECONFIG:-~/.kube/config}"
    echo "  Context:         k3d-${CLUSTER_NAME}"
    echo ""
    echo "Nodes:"
    kubectl get nodes -o wide
    echo ""
    echo "Namespaces:"
    kubectl get namespaces
    echo ""
    log_info "Cluster is ready for use!"
    log_info "Build and push images to: localhost:${REGISTRY_PORT}/anode:tag"
}

# Main execution
main() {
    log_info "Starting K3d cluster creation for anode testing"
    echo ""

    check_dependencies
    check_docker

    if check_existing_cluster; then
        create_cluster
        wait_for_cluster
        verify_registry
    fi

    setup_kubectl
    display_cluster_info

    log_info "Cluster creation complete!"
}

main "$@"
