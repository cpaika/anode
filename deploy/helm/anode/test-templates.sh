#!/bin/bash
# Anode Helm Template Rendering Test Script
# This script validates that Helm templates render correctly with various values configurations

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
PASSED=0
FAILED=0
WARNINGS=0

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHART_DIR="$SCRIPT_DIR"

# Temp directory for rendered templates
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

# Print colored output
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
    ((WARNINGS++))
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
    ((FAILED++))
}

print_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((PASSED++))
}

print_step() {
    echo -e "${BLUE}[TEST]${NC} $1"
}

# Check prerequisites
check_prerequisites() {
    print_step "Checking prerequisites..."

    if ! command -v helm &> /dev/null; then
        print_error "helm is not installed or not in PATH"
        exit 1
    fi

    if ! command -v kubectl &> /dev/null; then
        print_error "kubectl is not installed or not in PATH"
        exit 1
    fi

    print_pass "Prerequisites satisfied (helm, kubectl found)"
}

# Validate YAML syntax
validate_yaml() {
    local file=$1
    local name=$2

    if kubectl apply --dry-run=client -f "$file" &> /dev/null; then
        print_pass "$name: Valid Kubernetes YAML"
        return 0
    else
        print_error "$name: Invalid Kubernetes YAML"
        if kubectl apply --dry-run=client -f "$file" 2>&1 | head -n 5; then
            :
        fi
        return 1
    fi
}

# Test template rendering with specific values
test_template_render() {
    local values_file=$1
    local test_name=$2
    local output_file="${TEMP_DIR}/${test_name}.yaml"

    print_step "Testing template rendering: $test_name"

    if [ -n "$values_file" ]; then
        if helm template test-release "$CHART_DIR" \
            --values "$values_file" \
            --namespace test-namespace \
            > "$output_file" 2>&1; then
            print_pass "$test_name: Template rendered successfully"
        else
            print_error "$test_name: Template rendering failed"
            cat "$output_file" | head -n 10
            return 1
        fi
    else
        if helm template test-release "$CHART_DIR" \
            --namespace test-namespace \
            > "$output_file" 2>&1; then
            print_pass "$test_name: Template rendered successfully"
        else
            print_error "$test_name: Template rendering failed"
            cat "$output_file" | head -n 10
            return 1
        fi
    fi

    # Validate the rendered YAML
    validate_yaml "$output_file" "$test_name"
}

# Test with custom values via --set
test_template_with_set() {
    local set_args=$1
    local test_name=$2
    local output_file="${TEMP_DIR}/${test_name}.yaml"

    print_step "Testing template rendering with --set: $test_name"

    if helm template test-release "$CHART_DIR" \
        $set_args \
        --namespace test-namespace \
        > "$output_file" 2>&1; then
        print_pass "$test_name: Template rendered successfully"
    else
        print_error "$test_name: Template rendering failed"
        cat "$output_file" | head -n 10
        return 1
    fi

    validate_yaml "$output_file" "$test_name"
}

# Verify specific resources are present
verify_resources() {
    local output_file=$1
    local test_name=$2

    print_step "Verifying resources in: $test_name"

    # Check for StatefulSet
    if grep -q "kind: StatefulSet" "$output_file"; then
        print_pass "$test_name: StatefulSet present"
    else
        print_error "$test_name: StatefulSet missing"
    fi

    # Check for Service
    if grep -q "kind: Service" "$output_file"; then
        print_pass "$test_name: Service present"
    else
        print_error "$test_name: Service missing"
    fi

    # Check for ConfigMap
    if grep -q "kind: ConfigMap" "$output_file"; then
        print_pass "$test_name: ConfigMap present"
    else
        print_error "$test_name: ConfigMap missing"
    fi

    # Check for Secret
    if grep -q "kind: Secret" "$output_file"; then
        print_pass "$test_name: Secret present"
    else
        print_error "$test_name: Secret missing"
    fi

    # Check for ServiceAccount
    if grep -q "kind: ServiceAccount" "$output_file"; then
        print_pass "$test_name: ServiceAccount present"
    else
        print_warn "$test_name: ServiceAccount missing (might be disabled)"
    fi
}

# Test Helm lint
test_helm_lint() {
    print_step "Running helm lint on chart..."

    if helm lint "$CHART_DIR" > "${TEMP_DIR}/lint.log" 2>&1; then
        print_pass "Helm lint passed"
    else
        print_error "Helm lint failed"
        cat "${TEMP_DIR}/lint.log"
        return 1
    fi
}

# Test Helm lint with values files
test_helm_lint_with_values() {
    local values_file=$1
    local test_name=$2

    print_step "Running helm lint with $test_name values..."

    if helm lint "$CHART_DIR" --values "$values_file" > "${TEMP_DIR}/lint-${test_name}.log" 2>&1; then
        print_pass "Helm lint with $test_name values passed"
    else
        print_error "Helm lint with $test_name values failed"
        cat "${TEMP_DIR}/lint-${test_name}.log"
        return 1
    fi
}

# Test conditional resources
test_conditional_resources() {
    print_step "Testing conditional resources..."

    # Test with ingress enabled
    local output_file="${TEMP_DIR}/ingress-enabled.yaml"
    helm template test-release "$CHART_DIR" \
        --set ingress.enabled=true \
        --namespace test-namespace \
        > "$output_file" 2>&1

    if grep -q "kind: Ingress" "$output_file"; then
        print_pass "Ingress rendered when enabled"
    else
        print_error "Ingress not rendered when enabled"
    fi

    # Test with ingress disabled
    output_file="${TEMP_DIR}/ingress-disabled.yaml"
    helm template test-release "$CHART_DIR" \
        --set ingress.enabled=false \
        --namespace test-namespace \
        > "$output_file" 2>&1

    if ! grep -q "kind: Ingress" "$output_file"; then
        print_pass "Ingress not rendered when disabled"
    else
        print_error "Ingress rendered when disabled"
    fi

    # Test with ServiceMonitor enabled
    output_file="${TEMP_DIR}/servicemonitor-enabled.yaml"
    helm template test-release "$CHART_DIR" \
        --set monitoring.serviceMonitor.enabled=true \
        --namespace test-namespace \
        > "$output_file" 2>&1

    if grep -q "kind: ServiceMonitor" "$output_file"; then
        print_pass "ServiceMonitor rendered when enabled"
    else
        print_error "ServiceMonitor not rendered when enabled"
    fi

    # Test with NetworkPolicy enabled
    output_file="${TEMP_DIR}/networkpolicy-enabled.yaml"
    helm template test-release "$CHART_DIR" \
        --set networkPolicy.enabled=true \
        --namespace test-namespace \
        > "$output_file" 2>&1

    if grep -q "kind: NetworkPolicy" "$output_file"; then
        print_pass "NetworkPolicy rendered when enabled"
    else
        print_error "NetworkPolicy not rendered when enabled"
    fi

    # Test with PodDisruptionBudget enabled
    output_file="${TEMP_DIR}/pdb-enabled.yaml"
    helm template test-release "$CHART_DIR" \
        --set podDisruptionBudget.enabled=true \
        --namespace test-namespace \
        > "$output_file" 2>&1

    if grep -q "kind: PodDisruptionBudget" "$output_file"; then
        print_pass "PodDisruptionBudget rendered when enabled"
    else
        print_error "PodDisruptionBudget not rendered when enabled"
    fi
}

# Test different replica counts
test_replica_counts() {
    print_step "Testing different replica counts..."

    for replicas in 1 3 5; do
        local output_file="${TEMP_DIR}/replicas-${replicas}.yaml"
        helm template test-release "$CHART_DIR" \
            --set replicaCount=$replicas \
            --namespace test-namespace \
            > "$output_file" 2>&1

        if grep -A 5 "kind: StatefulSet" "$output_file" | grep -q "replicas: $replicas"; then
            print_pass "Replica count $replicas correctly set"
        else
            print_error "Replica count $replicas not correctly set"
        fi
    done
}

# Test values schema validation (if supported)
test_values_schema() {
    print_step "Testing values schema validation..."

    # Check if values.schema.json exists
    if [ -f "$CHART_DIR/values.schema.json" ]; then
        print_info "Found values.schema.json, testing schema validation..."

        # Test with default values (should pass)
        if helm template test-release "$CHART_DIR" \
            --validate \
            --namespace test-namespace \
            > /dev/null 2>&1; then
            print_pass "Default values pass schema validation"
        else
            print_error "Default values fail schema validation"
        fi
    else
        print_warn "No values.schema.json found, skipping schema validation"
    fi
}

# Main test execution
main() {
    echo "=========================================="
    echo "Anode Helm Template Rendering Tests"
    echo "=========================================="
    echo ""
    echo "Chart directory: $CHART_DIR"
    echo "Temp directory: $TEMP_DIR"
    echo ""

    # Check prerequisites
    check_prerequisites
    echo ""

    # Test 1: Helm lint
    test_helm_lint
    echo ""

    # Test 2: Default values
    test_template_render "" "default-values"
    verify_resources "${TEMP_DIR}/default-values.yaml" "default-values"
    echo ""

    # Test 3: Production values
    if [ -f "$CHART_DIR/values-production.yaml" ]; then
        test_template_render "$CHART_DIR/values-production.yaml" "production-values"
        test_helm_lint_with_values "$CHART_DIR/values-production.yaml" "production"
        verify_resources "${TEMP_DIR}/production-values.yaml" "production-values"
        echo ""
    fi

    # Test 4: Development values
    if [ -f "$CHART_DIR/values-development.yaml" ]; then
        test_template_render "$CHART_DIR/values-development.yaml" "development-values"
        test_helm_lint_with_values "$CHART_DIR/values-development.yaml" "development"
        verify_resources "${TEMP_DIR}/development-values.yaml" "development-values"
        echo ""
    fi

    # Test 5: Minimal values
    if [ -f "$CHART_DIR/values-minimal.yaml" ]; then
        test_template_render "$CHART_DIR/values-minimal.yaml" "minimal-values"
        test_helm_lint_with_values "$CHART_DIR/values-minimal.yaml" "minimal"
        verify_resources "${TEMP_DIR}/minimal-values.yaml" "minimal-values"
        echo ""
    fi

    # Test 6: Custom replica counts
    test_replica_counts
    echo ""

    # Test 7: Conditional resources
    test_conditional_resources
    echo ""

    # Test 8: Custom namespace override
    test_template_with_set "--set fullnameOverride=custom-anode" "custom-name"
    echo ""

    # Test 9: Custom image settings
    test_template_with_set "--set image.repository=myrepo/anode --set image.tag=v1.0.0" "custom-image"
    echo ""

    # Test 10: Resource limits
    test_template_with_set "--set resources.limits.cpu=2000m --set resources.limits.memory=4Gi" "custom-resources"
    echo ""

    # Test 11: Persistence disabled
    test_template_with_set "--set persistence.enabled=false" "no-persistence"
    echo ""

    # Test 12: Values schema validation
    test_values_schema
    echo ""

    # Summary
    echo "=========================================="
    echo "Test Summary"
    echo "=========================================="
    echo -e "${GREEN}Passed: $PASSED${NC}"
    echo -e "${YELLOW}Warnings: $WARNINGS${NC}"
    echo -e "${RED}Failed: $FAILED${NC}"
    echo ""

    if [ "$FAILED" -eq 0 ]; then
        echo -e "${GREEN}✓ All template rendering tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}✗ Some tests failed. Please review the errors above.${NC}"
        exit 1
    fi
}

# Run main function
main
