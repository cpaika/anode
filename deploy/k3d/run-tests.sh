#!/bin/bash
# Run integration tests against K3d cluster
set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTS_DIR="${SCRIPT_DIR}/tests"
NAMESPACE="${NAMESPACE:-anode}"
RELEASE_NAME="${RELEASE_NAME:-anode}"

# Test results
TESTS_PASSED=0
TESTS_FAILED=0
FAILED_TESTS=()

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

log_test() {
    echo -e "${BLUE}[TEST]${NC} $1"
}

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
}

# Check dependencies
check_dependencies() {
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl is not installed"
        exit 1
    fi
}

# Check cluster and deployment
check_deployment() {
    log_info "Checking deployment..."

    if ! kubectl get namespace "${NAMESPACE}" &> /dev/null; then
        log_error "Namespace '${NAMESPACE}' does not exist"
        log_info "Deploy first: ./deploy-anode.sh"
        exit 1
    fi

    if ! kubectl get statefulset -n "${NAMESPACE}" "${RELEASE_NAME}" &> /dev/null; then
        log_error "Deployment '${RELEASE_NAME}' not found in namespace '${NAMESPACE}'"
        log_info "Deploy first: ./deploy-anode.sh"
        exit 1
    fi

    log_info "Deployment found"
}

# Run a single test
run_test() {
    local test_script=$1
    local test_name=$(basename "${test_script}" .sh)

    log_test "Running: ${test_name}"

    # Export variables for test scripts
    export NAMESPACE
    export RELEASE_NAME
    export SCRIPT_DIR

    if bash "${test_script}"; then
        log_pass "${test_name}"
        TESTS_PASSED=$((TESTS_PASSED + 1))
        return 0
    else
        log_fail "${test_name}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
        FAILED_TESTS+=("${test_name}")
        return 1
    fi
}

# Run all tests
run_all_tests() {
    log_step "Running integration tests"
    echo ""

    # Find all test scripts
    local test_scripts=()
    if [ -d "${TESTS_DIR}" ]; then
        while IFS= read -r -d '' script; do
            test_scripts+=("$script")
        done < <(find "${TESTS_DIR}" -name "test_*.sh" -type f -print0 | sort -z)
    fi

    if [ ${#test_scripts[@]} -eq 0 ]; then
        log_warn "No test scripts found in ${TESTS_DIR}"
        return 0
    fi

    log_info "Found ${#test_scripts[@]} test(s)"
    echo ""

    # Run each test
    for test_script in "${test_scripts[@]}"; do
        run_test "${test_script}"
        echo ""
    done
}

# Display summary
display_summary() {
    echo ""
    echo "=========================================="
    log_info "Test Summary"
    echo "=========================================="
    echo ""
    echo "  Total tests:     $((TESTS_PASSED + TESTS_FAILED))"
    echo "  Passed:          ${TESTS_PASSED}"
    echo "  Failed:          ${TESTS_FAILED}"
    echo ""

    if [ ${#FAILED_TESTS[@]} -gt 0 ]; then
        echo "Failed tests:"
        for test in "${FAILED_TESTS[@]}"; do
            echo "  - ${test}"
        done
        echo ""
    fi

    if [ $TESTS_FAILED -eq 0 ]; then
        log_pass "All tests passed!"
        return 0
    else
        log_fail "Some tests failed"
        return 1
    fi
}

# Collect logs on failure
collect_logs_on_failure() {
    if [ $TESTS_FAILED -gt 0 ]; then
        log_info "Collecting logs for debugging..."
        echo ""

        local log_dir="${SCRIPT_DIR}/test-logs-$(date +%Y%m%d-%H%M%S)"
        mkdir -p "${log_dir}"

        # Collect pod logs
        log_info "Saving pod logs to ${log_dir}"
        kubectl logs -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
            --all-containers --prefix > "${log_dir}/pod-logs.txt" 2>&1 || true

        # Collect pod descriptions
        kubectl describe pods -n "${NAMESPACE}" -l "app.kubernetes.io/name=anode" \
            > "${log_dir}/pod-describe.txt" 2>&1 || true

        # Collect events
        kubectl get events -n "${NAMESPACE}" --sort-by='.lastTimestamp' \
            > "${log_dir}/events.txt" 2>&1 || true

        log_info "Logs saved to: ${log_dir}"
    fi
}

# Main execution
main() {
    log_info "Starting integration tests for anode on K3d"
    echo ""

    check_dependencies
    check_deployment
    run_all_tests

    local exit_code=0
    if ! display_summary; then
        exit_code=1
        collect_logs_on_failure
    fi

    exit $exit_code
}

main "$@"
