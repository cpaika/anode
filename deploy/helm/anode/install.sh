#!/bin/bash
# Anode Helm Chart Installation Script
# This script helps automate the installation of Anode on Kubernetes

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
NAMESPACE="anode"
RELEASE_NAME="anode"
VALUES_FILE=""
WAIT_TIMEOUT="600s"
DRY_RUN=false

# Print colored output
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Print usage
usage() {
    cat <<EOF
Anode Helm Chart Installation Script

Usage: $0 [OPTIONS]

Options:
    -n, --namespace NAME        Kubernetes namespace (default: anode)
    -r, --release NAME          Helm release name (default: anode)
    -f, --values FILE           Values file to use (development, production, minimal, or custom path)
    -w, --wait DURATION         Wait timeout for pods (default: 600s)
    -d, --dry-run               Perform a dry run without installing
    -h, --help                  Show this help message

Examples:
    # Install with default values
    $0

    # Install with production values
    $0 -f production

    # Install with custom values file
    $0 -f /path/to/my-values.yaml

    # Install in custom namespace
    $0 -n my-namespace -r my-release

    # Dry run to see what would be installed
    $0 -f production -d

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
        -f|--values)
            VALUES_FILE="$2"
            shift 2
            ;;
        -w|--wait)
            WAIT_TIMEOUT="$2"
            shift 2
            ;;
        -d|--dry-run)
            DRY_RUN=true
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

# Check prerequisites
print_info "Checking prerequisites..."

if ! command -v kubectl &> /dev/null; then
    print_error "kubectl not found. Please install kubectl."
    exit 1
fi

if ! command -v helm &> /dev/null; then
    print_error "helm not found. Please install Helm 3.2.0+."
    exit 1
fi

# Check kubectl access
if ! kubectl cluster-info &> /dev/null; then
    print_error "Cannot connect to Kubernetes cluster. Please check your kubeconfig."
    exit 1
fi

# Get chart directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHART_DIR="$SCRIPT_DIR"

# Determine values file
HELM_VALUES_FLAG=""
if [ -n "$VALUES_FILE" ]; then
    case "$VALUES_FILE" in
        development|dev)
            VALUES_FILE="$CHART_DIR/values-development.yaml"
            ;;
        production|prod)
            VALUES_FILE="$CHART_DIR/values-production.yaml"
            ;;
        minimal|min)
            VALUES_FILE="$CHART_DIR/values-minimal.yaml"
            ;;
    esac

    if [ ! -f "$VALUES_FILE" ]; then
        print_error "Values file not found: $VALUES_FILE"
        exit 1
    fi

    HELM_VALUES_FLAG="-f $VALUES_FILE"
    print_info "Using values file: $VALUES_FILE"
fi

# Create namespace if it doesn't exist
if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
    print_info "Creating namespace: $NAMESPACE"
    if [ "$DRY_RUN" = false ]; then
        kubectl create namespace "$NAMESPACE"
    fi
else
    print_info "Namespace already exists: $NAMESPACE"
fi

# Lint the chart
print_info "Linting Helm chart..."
helm lint "$CHART_DIR" $HELM_VALUES_FLAG

# Build dry-run command
HELM_CMD="helm install $RELEASE_NAME $CHART_DIR -n $NAMESPACE $HELM_VALUES_FLAG"

if [ "$DRY_RUN" = true ]; then
    print_info "Performing dry run..."
    $HELM_CMD --dry-run --debug
    print_info "Dry run completed successfully."
    exit 0
fi

# Install the chart
print_info "Installing Anode Helm chart..."
print_info "Release: $RELEASE_NAME"
print_info "Namespace: $NAMESPACE"
print_info "Chart: $CHART_DIR"

$HELM_CMD

# Wait for pods to be ready
print_info "Waiting for pods to be ready (timeout: $WAIT_TIMEOUT)..."
if kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=anode -n "$NAMESPACE" --timeout="$WAIT_TIMEOUT"; then
    print_info "All pods are ready!"
else
    print_warn "Timeout waiting for pods to be ready. Check pod status manually."
fi

# Get release status
print_info "Getting release status..."
helm status "$RELEASE_NAME" -n "$NAMESPACE"

# Print access information
print_info ""
print_info "=========================================="
print_info "Anode installation completed!"
print_info "=========================================="
print_info ""
print_info "To get S3 credentials, run:"
echo "  export ACCESS_KEY=\$(kubectl get secret -n $NAMESPACE ${RELEASE_NAME}-secret -o jsonpath=\"{.data.s3-access-key}\" | base64 -d)"
echo "  export SECRET_KEY=\$(kubectl get secret -n $NAMESPACE ${RELEASE_NAME}-secret -o jsonpath=\"{.data.s3-secret-key}\" | base64 -d)"
print_info ""
print_info "To access the S3 API, run:"
echo "  kubectl port-forward -n $NAMESPACE svc/$RELEASE_NAME 8080:8080"
print_info ""
print_info "To check pod status, run:"
echo "  kubectl get pods -n $NAMESPACE -l app.kubernetes.io/name=anode"
print_info ""
print_info "To view logs, run:"
echo "  kubectl logs -n $NAMESPACE -l app.kubernetes.io/name=anode --tail=100"
print_info ""
print_info "To run Helm tests, run:"
echo "  helm test $RELEASE_NAME -n $NAMESPACE"
print_info ""
print_info "For more information, see:"
echo "  helm status $RELEASE_NAME -n $NAMESPACE"
print_info ""
