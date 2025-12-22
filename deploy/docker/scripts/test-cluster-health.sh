#!/bin/bash
# Test script to verify docker-compose cluster health
# This script:
# 1. Starts a 3-node Anode cluster using docker-compose
# 2. Waits for all nodes to become healthy
# 3. Runs basic S3 operations to verify cluster functionality
# 4. Cleans up resources

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")"
PROJECT_ROOT="$(cd "$DOCKER_DIR/../.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CLUSTER_SIZE=3
MAX_WAIT_TIME=120  # Maximum time to wait for cluster to be healthy (seconds)
TEST_BUCKET="test-cluster-health"
TEST_FILE="test-object.txt"
TEST_CONTENT="Hello from Anode cluster health test!"

# Track if we need cleanup
CLEANUP_NEEDED=false

# Cleanup function
cleanup() {
    if [ "$CLEANUP_NEEDED" = true ]; then
        echo ""
        echo -e "${YELLOW}Cleaning up...${NC}"
        cd "$DOCKER_DIR"

        # Stop and remove containers
        docker-compose -f docker-compose.yml down -v 2>/dev/null || true

        # Remove test file if it exists
        rm -f /tmp/"$TEST_FILE" 2>/dev/null || true

        echo -e "${GREEN}Cleanup complete${NC}"
    fi
}

# Set trap to cleanup on exit
trap cleanup EXIT

# Print header
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Anode Cluster Health Test${NC}"
echo -e "${BLUE}========================================${NC}"
echo "Project root: $PROJECT_ROOT"
echo "Docker dir: $DOCKER_DIR"
echo "Cluster size: $CLUSTER_SIZE nodes"
echo "Max wait time: $MAX_WAIT_TIME seconds"
echo -e "${BLUE}========================================${NC}"
echo ""

# Change to docker directory
cd "$DOCKER_DIR"

# Step 1: Build images
echo -e "${YELLOW}Step 1: Building Docker images...${NC}"
docker-compose -f docker-compose.yml build
echo -e "${GREEN}Build complete${NC}"
echo ""

# Step 2: Start cluster
echo -e "${YELLOW}Step 2: Starting cluster...${NC}"
docker-compose -f docker-compose.yml up -d
CLEANUP_NEEDED=true
echo -e "${GREEN}Cluster started${NC}"
echo ""

# Step 3: Wait for all nodes to be healthy
echo -e "${YELLOW}Step 3: Waiting for all nodes to become healthy...${NC}"
WAIT_COUNT=0
ALL_HEALTHY=false

while [ $WAIT_COUNT -lt $MAX_WAIT_TIME ]; do
    HEALTHY_COUNT=0
    NODE_STATUS=""

    for i in $(seq 1 $CLUSTER_SIZE); do
        # Calculate admin port for each node
        # Node 1: 8081, Node 2: 8083, Node 3: 8085
        ADMIN_PORT=$((8080 + (i - 1) * 2 + 1))

        if curl -sf http://localhost:$ADMIN_PORT/health > /dev/null 2>&1; then
            HEALTHY_COUNT=$((HEALTHY_COUNT + 1))
            NODE_STATUS="${NODE_STATUS}Node $i: ${GREEN}HEALTHY${NC} "
        else
            NODE_STATUS="${NODE_STATUS}Node $i: ${RED}UNHEALTHY${NC} "
        fi
    done

    echo -e "  [$WAIT_COUNT/$MAX_WAIT_TIME] $NODE_STATUS ($HEALTHY_COUNT/$CLUSTER_SIZE ready)"

    if [ $HEALTHY_COUNT -eq $CLUSTER_SIZE ]; then
        ALL_HEALTHY=true
        break
    fi

    sleep 2
    WAIT_COUNT=$((WAIT_COUNT + 2))
done

echo ""

if [ "$ALL_HEALTHY" != true ]; then
    echo -e "${RED}FAILED: Cluster did not become healthy within $MAX_WAIT_TIME seconds${NC}"
    echo ""
    echo -e "${YELLOW}Container status:${NC}"
    docker-compose -f docker-compose.yml ps
    echo ""
    echo -e "${YELLOW}Recent logs:${NC}"
    docker-compose -f docker-compose.yml logs --tail=50
    exit 1
fi

echo -e "${GREEN}All nodes are healthy!${NC}"
echo ""

# Step 4: Verify health endpoints
echo -e "${YELLOW}Step 4: Verifying health endpoints...${NC}"
for i in $(seq 1 $CLUSTER_SIZE); do
    ADMIN_PORT=$((8080 + (i - 1) * 2 + 1))

    echo -e "  Node $i (port $ADMIN_PORT):"

    # Test /health endpoint
    if health_response=$(curl -sf http://localhost:$ADMIN_PORT/health 2>&1); then
        echo -e "    /health: ${GREEN}OK${NC}"
    else
        echo -e "    /health: ${RED}FAILED${NC}"
        echo "    Response: $health_response"
        exit 1
    fi

    # Test /live endpoint
    if curl -sf http://localhost:$ADMIN_PORT/live > /dev/null 2>&1; then
        echo -e "    /live: ${GREEN}OK${NC}"
    else
        echo -e "    /live: ${RED}FAILED${NC}"
        exit 1
    fi

    # Test /ready endpoint
    if curl -sf http://localhost:$ADMIN_PORT/ready > /dev/null 2>&1; then
        echo -e "    /ready: ${GREEN}OK${NC}"
    else
        echo -e "    /ready: ${RED}FAILED${NC}"
        exit 1
    fi
done
echo -e "${GREEN}All health endpoints verified${NC}"
echo ""

# Step 5: Test basic S3 operations (optional, requires AWS CLI)
if command -v aws >/dev/null 2>&1; then
    echo -e "${YELLOW}Step 5: Testing S3 operations...${NC}"

    # Configure AWS CLI for local testing
    export AWS_ACCESS_KEY_ID=minioadmin
    export AWS_SECRET_ACCESS_KEY=minioadmin
    export AWS_REGION=us-east-1

    S3_ENDPOINT="http://localhost:8080"

    # Create test bucket
    echo -e "  Creating bucket '$TEST_BUCKET'..."
    if aws --endpoint-url "$S3_ENDPOINT" s3 mb "s3://$TEST_BUCKET" 2>&1; then
        echo -e "    ${GREEN}OK${NC}"
    else
        echo -e "    ${RED}FAILED${NC}"
        exit 1
    fi

    # Create test file
    echo "$TEST_CONTENT" > /tmp/"$TEST_FILE"

    # Upload object
    echo -e "  Uploading object '$TEST_FILE'..."
    if aws --endpoint-url "$S3_ENDPOINT" s3 cp /tmp/"$TEST_FILE" "s3://$TEST_BUCKET/$TEST_FILE" 2>&1; then
        echo -e "    ${GREEN}OK${NC}"
    else
        echo -e "    ${RED}FAILED${NC}"
        exit 1
    fi

    # List objects
    echo -e "  Listing objects in bucket..."
    if list_output=$(aws --endpoint-url "$S3_ENDPOINT" s3 ls "s3://$TEST_BUCKET/" 2>&1); then
        if echo "$list_output" | grep -q "$TEST_FILE"; then
            echo -e "    ${GREEN}OK${NC} - Found $TEST_FILE"
        else
            echo -e "    ${RED}FAILED${NC} - Object not found in listing"
            exit 1
        fi
    else
        echo -e "    ${RED}FAILED${NC}"
        echo "    Output: $list_output"
        exit 1
    fi

    # Download object
    echo -e "  Downloading object..."
    rm -f /tmp/"$TEST_FILE.downloaded"
    if aws --endpoint-url "$S3_ENDPOINT" s3 cp "s3://$TEST_BUCKET/$TEST_FILE" /tmp/"$TEST_FILE.downloaded" 2>&1; then
        echo -e "    ${GREEN}OK${NC}"
    else
        echo -e "    ${RED}FAILED${NC}"
        exit 1
    fi

    # Verify content
    echo -e "  Verifying downloaded content..."
    if diff /tmp/"$TEST_FILE" /tmp/"$TEST_FILE.downloaded" > /dev/null 2>&1; then
        echo -e "    ${GREEN}OK${NC} - Content matches"
    else
        echo -e "    ${RED}FAILED${NC} - Content mismatch"
        exit 1
    fi

    # Delete object
    echo -e "  Deleting object..."
    if aws --endpoint-url "$S3_ENDPOINT" s3 rm "s3://$TEST_BUCKET/$TEST_FILE" 2>&1; then
        echo -e "    ${GREEN}OK${NC}"
    else
        echo -e "    ${RED}FAILED${NC}"
        exit 1
    fi

    # Delete bucket
    echo -e "  Deleting bucket..."
    if aws --endpoint-url "$S3_ENDPOINT" s3 rb "s3://$TEST_BUCKET" 2>&1; then
        echo -e "    ${GREEN}OK${NC}"
    else
        echo -e "    ${RED}FAILED${NC}"
        exit 1
    fi

    # Cleanup test files
    rm -f /tmp/"$TEST_FILE" /tmp/"$TEST_FILE.downloaded"

    echo -e "${GREEN}S3 operations test passed${NC}"
    echo ""
else
    echo -e "${YELLOW}Step 5: Skipping S3 operations test (AWS CLI not installed)${NC}"
    echo ""
fi

# Step 6: Verify cluster metrics (optional)
echo -e "${YELLOW}Step 6: Checking cluster status...${NC}"
for i in $(seq 1 $CLUSTER_SIZE); do
    ADMIN_PORT=$((8080 + (i - 1) * 2 + 1))
    echo -e "  Node $i status:"

    # Try to get metrics endpoint (if it exists)
    if curl -sf http://localhost:$ADMIN_PORT/metrics > /dev/null 2>&1; then
        echo -e "    Metrics: ${GREEN}Available${NC}"
    else
        echo -e "    Metrics: ${YELLOW}Not available or not exposed${NC}"
    fi
done
echo -e "${GREEN}Cluster status check complete${NC}"
echo ""

# Success!
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}ALL TESTS PASSED${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Summary:"
echo "  - All $CLUSTER_SIZE nodes started successfully"
echo "  - All health endpoints are responding"
if command -v aws >/dev/null 2>&1; then
    echo "  - S3 operations (create, upload, download, delete) working"
fi
echo ""
echo -e "${BLUE}Cluster will be cleaned up automatically${NC}"
echo ""

exit 0
