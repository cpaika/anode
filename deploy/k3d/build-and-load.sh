#!/bin/bash
# Build anode image and load into K3d registry
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
REGISTRY_PORT="5111"
REGISTRY_NAME="localhost:${REGISTRY_PORT}"
IMAGE_NAME="anode"
IMAGE_TAG="${IMAGE_TAG:-latest}"
FULL_IMAGE="${REGISTRY_NAME}/${IMAGE_NAME}:${IMAGE_TAG}"
DOCKERFILE="${PROJECT_ROOT}/deploy/docker/Dockerfile"

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

    if ! command -v docker &> /dev/null; then
        log_error "docker is not installed"
        exit 1
    fi

    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running"
        exit 1
    fi

    if ! [ -f "${DOCKERFILE}" ]; then
        log_error "Dockerfile not found at ${DOCKERFILE}"
        exit 1
    fi

    log_info "All dependencies found"
}

# Check if registry is accessible
check_registry() {
    log_info "Checking registry accessibility..."

    if ! curl -s "http://${REGISTRY_NAME}/v2/" > /dev/null 2>&1; then
        log_error "Registry at ${REGISTRY_NAME} is not accessible"
        log_info "Make sure the K3d cluster is running: ./create-cluster.sh"
        exit 1
    fi

    log_info "Registry is accessible"
}

# Clean up old images
cleanup_old_images() {
    log_info "Cleaning up old images..."

    # Remove dangling images
    local dangling=$(docker images -f "dangling=true" -q | head -n 5)
    if [ -n "$dangling" ]; then
        docker rmi $dangling 2>/dev/null || true
        log_info "Removed dangling images"
    fi
}

# Build Docker image
build_image() {
    log_step "Building Docker image: ${FULL_IMAGE}"
    echo ""

    local build_start=$(date +%s)

    # Build with BuildKit for better caching
    if DOCKER_BUILDKIT=1 docker build \
        --build-arg BUILDKIT_INLINE_CACHE=1 \
        --tag "${FULL_IMAGE}" \
        --file "${DOCKERFILE}" \
        "${PROJECT_ROOT}"; then

        local build_end=$(date +%s)
        local build_duration=$((build_end - build_start))

        log_info "Build completed in ${build_duration}s"
    else
        log_error "Docker build failed"
        exit 1
    fi
}

# Verify image
verify_image() {
    log_info "Verifying image..."

    if ! docker inspect "${FULL_IMAGE}" &> /dev/null; then
        log_error "Image ${FULL_IMAGE} not found after build"
        exit 1
    fi

    local image_size=$(docker inspect -f '{{ .Size }}' "${FULL_IMAGE}" | awk '{printf "%.2f MB", $1/1024/1024}')
    log_info "Image size: ${image_size}"

    # Check if image has required labels/metadata
    log_info "Image created: $(docker inspect -f '{{ .Created }}' "${FULL_IMAGE}")"
}

# Push to registry
push_to_registry() {
    log_step "Pushing image to K3d registry: ${FULL_IMAGE}"
    echo ""

    if docker push "${FULL_IMAGE}"; then
        log_info "Successfully pushed to registry"
    else
        log_error "Failed to push image to registry"
        exit 1
    fi
}

# Verify image in registry
verify_registry_image() {
    log_info "Verifying image in registry..."

    local manifest_url="http://${REGISTRY_NAME}/v2/${IMAGE_NAME}/manifests/${IMAGE_TAG}"
    if curl -sf -o /dev/null "${manifest_url}"; then
        log_info "Image successfully available in registry"
    else
        log_warn "Could not verify image in registry (may still be available)"
    fi
}

# Tag additional versions
tag_additional_versions() {
    if [ "${IMAGE_TAG}" = "latest" ]; then
        # Also tag with git commit hash if in git repo
        if git rev-parse --git-dir > /dev/null 2>&1; then
            local git_hash=$(git rev-parse --short HEAD)
            local git_image="${REGISTRY_NAME}/${IMAGE_NAME}:${git_hash}"

            log_info "Tagging with git hash: ${git_hash}"
            docker tag "${FULL_IMAGE}" "${git_image}"
            docker push "${git_image}"

            log_info "Also available as: ${git_image}"
        fi
    fi
}

# Display summary
display_summary() {
    echo ""
    log_info "Build and load summary:"
    echo "  Image:           ${FULL_IMAGE}"
    echo "  Registry:        ${REGISTRY_NAME}"
    echo "  Dockerfile:      ${DOCKERFILE}"
    echo ""
    log_info "Image is ready for deployment!"
    log_info "Deploy with: ./deploy-anode.sh"
}

# Main execution
main() {
    log_info "Building and loading anode image into K3d registry"
    echo ""

    check_dependencies
    check_registry
    cleanup_old_images
    build_image
    verify_image
    push_to_registry
    verify_registry_image
    tag_additional_versions
    display_summary

    log_info "Build and load complete!"
}

main "$@"
