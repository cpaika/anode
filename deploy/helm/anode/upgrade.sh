#!/bin/bash
# Anode Helm Chart Upgrade Script
# This script helps automate the upgrade of Anode on Kubernetes

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default values
NAMESPACE="anode"
RELEASE_NAME="anode"
VALUES_FILE=""
WAIT_TIMEOUT="600s"
DRY_RUN=false
FORCE=false
RESET_VALUES=false
REUSE_VALUES=true

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

print_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

# Print usage
usage() {
    cat <<EOF
Anode Helm Chart Upgrade Script

Usage: $0 [OPTIONS]

Options:
    -n, --namespace NAME        Kubernetes namespace (default: anode)
    -r, --release NAME          Helm release name (default: anode)
    -f, --values FILE           Values file to use
    -w, --wait DURATION         Wait timeout for pods (default: 600s)
    -d, --dry-run               Perform a dry run without upgrading
    --force                     Force upgrade (bypass checks)
    --reset-values              Reset values to chart defaults
    --no-reuse-values           Don't reuse previous values
    -h, --help                  Show this help message

Examples:
    # Upgrade with current values
    $0

    # Upgrade with new values file
    $0 -f production

    # Dry run to see what would change
    $0 -f production -d

    # Force upgrade
    $0 --force

    # Reset all values to defaults
    $0 --reset-values

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
        --force)
            FORCE=true
            shift
            ;;
        --reset-values)
            RESET_VALUES=true
            REUSE_VALUES=false
            shift
            ;;
        --no-reuse-values)
            REUSE_VALUES=false
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
print_step "Checking prerequisites..."

if ! command -v kubectl &> /dev/null; then
    print_error "kubectl not found. Please install kubectl."
    exit 1
fi

if ! command -v helm &> /dev/null; then
    print_error "helm not found. Please install Helm 3.2.0+."
    exit 1
fi

# Check if release exists
if ! helm status "$RELEASE_NAME" -n "$NAMESPACE" &> /dev/null; then
    print_error "Release '$RELEASE_NAME' not found in namespace '$NAMESPACE'."
    print_info "Use install.sh to install a new release."
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

# Show current release info
print_step "Current release information:"
helm status "$RELEASE_NAME" -n "$NAMESPACE" | head -n 10

# Check cluster health before upgrade
print_step "Checking cluster health..."
POD_COUNT=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=anode --no-headers 2>/dev/null | wc -l | tr -d ' ')
READY_COUNT=$(kubectl get pods -n "$NAMESPACE" -l app.kubernetes.io/name=anode --no-headers 2>/dev/null | grep -c "Running" || echo "0")

print_info "Total pods: $POD_COUNT"
print_info "Running pods: $READY_COUNT"

if [ "$POD_COUNT" -eq 0 ]; then
    print_error "No anode pods found."
    exit 1
fi

if [ "$READY_COUNT" -lt "$POD_COUNT" ] && [ "$FORCE" = false ]; then
    print_warn "Not all pods are running. Use --force to bypass this check."
    exit 1
fi

# Lint the chart
print_step "Linting Helm chart..."
helm lint "$CHART_DIR" $HELM_VALUES_FLAG

# Show diff if possible
if command -v helm-diff &> /dev/null; then
    print_step "Showing differences..."
    helm diff upgrade "$RELEASE_NAME" "$CHART_DIR" -n "$NAMESPACE" $HELM_VALUES_FLAG || true
else
    print_warn "helm-diff plugin not installed. Skipping diff. Install with: helm plugin install https://github.com/databus23/helm-diff"
fi

# Build upgrade command
HELM_CMD="helm upgrade $RELEASE_NAME $CHART_DIR -n $NAMESPACE $HELM_VALUES_FLAG"

if [ "$REUSE_VALUES" = true ] && [ "$RESET_VALUES" = false ]; then
    HELM_CMD="$HELM_CMD --reuse-values"
fi

if [ "$RESET_VALUES" = true ]; then
    HELM_CMD="$HELM_CMD --reset-values"
fi

if [ "$DRY_RUN" = true ]; then
    print_step "Performing dry run..."
    $HELM_CMD --dry-run --debug
    print_info "Dry run completed successfully."
    exit 0
fi

# Confirm upgrade
if [ "$FORCE" = false ]; then
    print_warn "This will upgrade the Anode cluster. Continue? (y/N)"
    read -r CONFIRM
    if [ "$CONFIRM" != "y" ] && [ "$CONFIRM" != "Y" ]; then
        print_info "Upgrade cancelled."
        exit 0
    fi
fi

# Perform upgrade
print_step "Upgrading Anode Helm chart..."
print_info "Release: $RELEASE_NAME"
print_info "Namespace: $NAMESPACE"
print_info "Chart: $CHART_DIR"

$HELM_CMD

# Wait for rollout to complete
print_step "Waiting for rollout to complete (timeout: $WAIT_TIMEOUT)..."
if kubectl rollout status statefulset -n "$NAMESPACE" "$RELEASE_NAME" --timeout="$WAIT_TIMEOUT"; then
    print_info "Rollout completed successfully!"
else
    print_warn "Timeout waiting for rollout. Check status manually."
fi

# Verify pods are ready
print_step "Verifying pods are ready..."
if kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=anode -n "$NAMESPACE" --timeout=60s; then
    print_info "All pods are ready!"
else
    print_warn "Not all pods are ready. Check pod status."
fi

# Get release status
print_step "Getting release status..."
helm status "$RELEASE_NAME" -n "$NAMESPACE"

# Print summary
print_info ""
print_info "=========================================="
print_info "Anode upgrade completed!"
print_info "=========================================="
print_info ""
print_info "To check pod status, run:"
echo "  kubectl get pods -n $NAMESPACE -l app.kubernetes.io/name=anode"
print_info ""
print_info "To view logs, run:"
echo "  kubectl logs -n $NAMESPACE -l app.kubernetes.io/name=anode --tail=100"
print_info ""
print_info "To rollback if needed, run:"
echo "  helm rollback $RELEASE_NAME -n $NAMESPACE"
print_info ""
print_info "To view history, run:"
echo "  helm history $RELEASE_NAME -n $NAMESPACE"
print_info ""
