#!/bin/sh
# Health check script for Anode containers
# This script is used by Docker's HEALTHCHECK directive and can also be run manually

set -e

# Configuration
ADMIN_PORT="${ANODE_ADMIN_PORT:-8081}"
HEALTH_ENDPOINT="${ANODE_HEALTH_ENDPOINT:-/health}"
TIMEOUT="${ANODE_HEALTH_TIMEOUT:-5}"

# Check if curl is available
if ! command -v curl >/dev/null 2>&1; then
    echo "ERROR: curl is not available"
    exit 1
fi

# Perform health check
if ! response=$(curl -sf -m "$TIMEOUT" "http://localhost:${ADMIN_PORT}${HEALTH_ENDPOINT}" 2>&1); then
    echo "UNHEALTHY: Failed to connect to health endpoint"
    echo "Response: $response"
    exit 1
fi

# Parse response (expecting JSON)
if ! echo "$response" | grep -q '"status".*"healthy"'; then
    echo "UNHEALTHY: Service reports unhealthy status"
    echo "Response: $response"
    exit 1
fi

# All checks passed
echo "HEALTHY"
exit 0
