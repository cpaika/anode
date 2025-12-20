#!/bin/bash
# Script to start the Anode development cluster

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(cd "$DOCKER_DIR/../.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
CLUSTER_SIZE=3
COMPOSE_FILE="docker-compose.yml"
WITH_DEV=false
BUILD=false
CLEAN=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -n|--nodes)
            CLUSTER_SIZE="$2"
            shift 2
            ;;
        -d|--dev)
            WITH_DEV=true
            shift
            ;;
        -b|--build)
            BUILD=true
            shift
            ;;
        -c|--clean)
            CLEAN=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -n, --nodes N      Number of nodes (default: 3)"
            echo "  -d, --dev          Include development services (Prometheus, Grafana, Jaeger)"
            echo "  -b, --build        Rebuild Docker images"
            echo "  -c, --clean        Clean volumes and containers before starting"
            echo "  -h, --help         Show this help message"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

cd "$DOCKER_DIR"

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Anode Cluster Startup${NC}"
echo -e "${GREEN}========================================${NC}"
echo "Cluster size: $CLUSTER_SIZE nodes"
echo "Development mode: $WITH_DEV"
echo "Build images: $BUILD"
echo "Clean start: $CLEAN"
echo -e "${GREEN}========================================${NC}"
echo ""

# Clean if requested
if [ "$CLEAN" = true ]; then
    echo -e "${YELLOW}Cleaning existing containers and volumes...${NC}"

    # Stop and remove containers
    docker-compose -f docker-compose.yml down -v 2>/dev/null || true
    if [ "$WITH_DEV" = true ]; then
        docker-compose -f docker-compose.yml -f docker-compose.dev.yml down -v 2>/dev/null || true
    fi

    # Remove volumes
    docker volume ls -q | grep anode | xargs -r docker volume rm 2>/dev/null || true

    echo -e "${GREEN}Cleanup complete${NC}"
    echo ""
fi

# Build if requested
if [ "$BUILD" = true ]; then
    echo -e "${YELLOW}Building Docker images...${NC}"

    BUILD_ARGS=""
    if [ "$WITH_DEV" = true ]; then
        BUILD_ARGS="-f docker-compose.yml -f docker-compose.dev.yml"
    else
        BUILD_ARGS="-f docker-compose.yml"
    fi

    docker-compose $BUILD_ARGS build

    echo -e "${GREEN}Build complete${NC}"
    echo ""
fi

# Generate config file if it doesn't exist
if [ ! -f "$DOCKER_DIR/config/anode.toml" ]; then
    echo -e "${YELLOW}Config file not found, using default configuration${NC}"
fi

# Start the cluster
echo -e "${YELLOW}Starting Anode cluster...${NC}"

if [ "$WITH_DEV" = true ]; then
    docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d
else
    docker-compose -f docker-compose.yml up -d
fi

echo ""
echo -e "${GREEN}Waiting for cluster to be ready...${NC}"

# Wait for nodes to be healthy
MAX_WAIT=60
WAIT_COUNT=0
ALL_HEALTHY=false

while [ $WAIT_COUNT -lt $MAX_WAIT ]; do
    HEALTHY_COUNT=0

    for i in $(seq 1 $CLUSTER_SIZE); do
        PORT=$((8080 + (i - 1) * 2 + 1))
        if curl -sf http://localhost:$PORT/health > /dev/null 2>&1; then
            HEALTHY_COUNT=$((HEALTHY_COUNT + 1))
        fi
    done

    echo -e "${YELLOW}Health check: $HEALTHY_COUNT/$CLUSTER_SIZE nodes ready${NC}"

    if [ $HEALTHY_COUNT -eq $CLUSTER_SIZE ]; then
        ALL_HEALTHY=true
        break
    fi

    sleep 2
    WAIT_COUNT=$((WAIT_COUNT + 2))
done

echo ""

if [ "$ALL_HEALTHY" = true ]; then
    echo -e "${GREEN}========================================${NC}"
    echo -e "${GREEN}Cluster is ready!${NC}"
    echo -e "${GREEN}========================================${NC}"
    echo ""
    echo "Node endpoints:"
    for i in $(seq 1 $CLUSTER_SIZE); do
        S3_PORT=$((8080 + (i - 1) * 2))
        ADMIN_PORT=$((8080 + (i - 1) * 2 + 1))
        echo "  Node $i:"
        echo "    S3 API:    http://localhost:$S3_PORT"
        echo "    Admin API: http://localhost:$ADMIN_PORT"
    done

    if [ "$WITH_DEV" = true ]; then
        echo ""
        echo "Development services:"
        echo "  Prometheus: http://localhost:9091"
        echo "  Grafana:    http://localhost:3000 (admin/admin)"
        echo "  Jaeger:     http://localhost:16686"
        echo "  nginx:      http://localhost:80"
    fi

    echo ""
    echo -e "${GREEN}========================================${NC}"
    echo "Quick start commands:"
    echo ""
    echo "# Configure AWS CLI (or MinIO client)"
    echo "aws configure set aws_access_key_id minioadmin"
    echo "aws configure set aws_secret_access_key minioadmin"
    echo ""
    echo "# Create a bucket"
    echo "aws --endpoint-url http://localhost:8080 s3 mb s3://test-bucket"
    echo ""
    echo "# Upload a file"
    echo "echo 'Hello, Anode!' > test.txt"
    echo "aws --endpoint-url http://localhost:8080 s3 cp test.txt s3://test-bucket/"
    echo ""
    echo "# List objects"
    echo "aws --endpoint-url http://localhost:8080 s3 ls s3://test-bucket/"
    echo ""
    echo "# View logs"
    echo "docker-compose logs -f"
    echo ""
    echo "# Stop cluster"
    echo "./scripts/stop-cluster.sh"
    echo -e "${GREEN}========================================${NC}"
else
    echo -e "${RED}========================================${NC}"
    echo -e "${RED}Cluster failed to start properly${NC}"
    echo -e "${RED}========================================${NC}"
    echo ""
    echo "Check logs with:"
    echo "  docker-compose logs"
    echo ""
    echo "Stop cluster with:"
    echo "  docker-compose down"
    exit 1
fi
