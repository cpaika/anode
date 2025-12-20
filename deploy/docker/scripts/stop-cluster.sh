#!/bin/bash
# Script to stop the Anode development cluster

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOCKER_DIR="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
REMOVE_VOLUMES=false
REMOVE_IMAGES=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--volumes)
            REMOVE_VOLUMES=true
            shift
            ;;
        -i|--images)
            REMOVE_IMAGES=true
            shift
            ;;
        -a|--all)
            REMOVE_VOLUMES=true
            REMOVE_IMAGES=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -v, --volumes      Remove volumes (data will be lost)"
            echo "  -i, --images       Remove Docker images"
            echo "  -a, --all          Remove volumes and images"
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
echo -e "${GREEN}Anode Cluster Shutdown${NC}"
echo -e "${GREEN}========================================${NC}"
echo "Remove volumes: $REMOVE_VOLUMES"
echo "Remove images: $REMOVE_IMAGES"
echo -e "${GREEN}========================================${NC}"
echo ""

# Stop containers
echo -e "${YELLOW}Stopping containers...${NC}"

# Try to stop all possible compose configurations
docker-compose -f docker-compose.yml down 2>/dev/null || true
docker-compose -f docker-compose.yml -f docker-compose.dev.yml down 2>/dev/null || true
docker-compose -f docker-compose.chaos.yml down 2>/dev/null || true

echo -e "${GREEN}Containers stopped${NC}"
echo ""

# Remove volumes if requested
if [ "$REMOVE_VOLUMES" = true ]; then
    echo -e "${YELLOW}Removing volumes...${NC}"
    echo -e "${RED}WARNING: This will delete all data!${NC}"
    read -p "Are you sure? (yes/no): " -r
    if [[ $REPLY =~ ^[Yy][Ee][Ss]$ ]]; then
        docker volume ls -q | grep -E 'anode|chaos' | xargs -r docker volume rm 2>/dev/null || true
        echo -e "${GREEN}Volumes removed${NC}"
    else
        echo -e "${YELLOW}Volume removal cancelled${NC}"
    fi
    echo ""
fi

# Remove images if requested
if [ "$REMOVE_IMAGES" = true ]; then
    echo -e "${YELLOW}Removing images...${NC}"

    # Get image name from docker-compose
    IMAGE_NAME=$(docker-compose -f docker-compose.yml config | grep 'image:' | head -1 | awk '{print $2}')

    if [ -n "$IMAGE_NAME" ]; then
        docker rmi "$IMAGE_NAME" 2>/dev/null || true
    fi

    # Also try to remove by pattern
    docker images | grep anode | awk '{print $3}' | xargs -r docker rmi -f 2>/dev/null || true

    echo -e "${GREEN}Images removed${NC}"
    echo ""
fi

# Show remaining resources
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Remaining Anode resources:${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

CONTAINERS=$(docker ps -a | grep anode | wc -l)
VOLUMES=$(docker volume ls | grep -E 'anode|chaos' | wc -l)
IMAGES=$(docker images | grep anode | wc -l)

echo "Containers: $CONTAINERS"
echo "Volumes: $VOLUMES"
echo "Images: $IMAGES"
echo ""

if [ "$CONTAINERS" -eq 0 ] && [ "$VOLUMES" -eq 0 ] && [ "$IMAGES" -eq 0 ]; then
    echo -e "${GREEN}All Anode resources have been cleaned up${NC}"
else
    echo -e "${YELLOW}Some resources remain${NC}"
    echo ""
    echo "To remove everything:"
    echo "  $0 --all"
fi

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Shutdown complete${NC}"
echo -e "${GREEN}========================================${NC}"
