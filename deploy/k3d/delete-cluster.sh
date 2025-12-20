#!/bin/bash
# Delete K3d cluster
set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

CLUSTER_NAME="anode-test"
REGISTRY_NAME="anode-registry.localhost"

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

# Parse command line arguments
FORCE=false
while [[ $# -gt 0 ]]; do
    case $1 in
        -f|--force)
            FORCE=true
            shift
            ;;
        *)
            log_error "Unknown option: $1"
            echo "Usage: $0 [-f|--force]"
            exit 1
            ;;
    esac
done

# Check if cluster exists
check_cluster_exists() {
    if ! k3d cluster list | grep -q "${CLUSTER_NAME}"; then
        log_info "Cluster '${CLUSTER_NAME}' does not exist"
        return 1
    fi
    return 0
}

# Confirm deletion
confirm_deletion() {
    if [ "$FORCE" = false ]; then
        log_warn "This will delete the K3d cluster '${CLUSTER_NAME}' and all its resources"
        read -p "Are you sure you want to continue? [y/N] " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            log_info "Deletion cancelled"
            exit 0
        fi
    fi
}

# Delete cluster
delete_cluster() {
    log_info "Deleting K3d cluster '${CLUSTER_NAME}'..."

    if k3d cluster delete "${CLUSTER_NAME}"; then
        log_info "Cluster deleted successfully"
    else
        log_error "Failed to delete cluster"
        return 1
    fi
}

# Delete registry
delete_registry() {
    log_info "Checking for registry '${REGISTRY_NAME}'..."

    if k3d registry list | grep -q "${REGISTRY_NAME}"; then
        log_info "Deleting registry '${REGISTRY_NAME}'..."
        if k3d registry delete "${REGISTRY_NAME}"; then
            log_info "Registry deleted successfully"
        else
            log_warn "Failed to delete registry (it may have been deleted with the cluster)"
        fi
    else
        log_info "Registry not found (may have been deleted with cluster)"
    fi
}

# Cleanup kubectl context
cleanup_kubectl_context() {
    log_info "Cleaning up kubectl context..."

    local context="k3d-${CLUSTER_NAME}"
    if kubectl config get-contexts -o name | grep -q "^${context}$"; then
        kubectl config delete-context "${context}" &> /dev/null || true
        log_info "Kubectl context removed"
    fi
}

# Main execution
main() {
    log_info "Starting K3d cluster deletion"
    echo ""

    if ! command -v k3d &> /dev/null; then
        log_error "k3d is not installed"
        exit 1
    fi

    if ! check_cluster_exists; then
        exit 0
    fi

    confirm_deletion
    delete_cluster
    delete_registry
    cleanup_kubectl_context

    echo ""
    log_info "Cleanup complete!"
}

main "$@"
