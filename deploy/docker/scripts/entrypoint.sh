#!/bin/sh
# Anode Docker Entrypoint Script
# Handles proper signal forwarding and graceful shutdown

set -e

# Function to handle signals
shutdown() {
    echo "Received shutdown signal, terminating anode..."
    if [ -n "$ANODE_PID" ]; then
        kill -TERM "$ANODE_PID" 2>/dev/null || true
        wait "$ANODE_PID"
    fi
    echo "Anode shutdown complete"
    exit 0
}

# Trap SIGTERM and SIGINT
trap shutdown TERM INT

# Print environment information
echo "=========================================="
echo "Anode Object Storage"
echo "=========================================="
echo "Node ID: ${ANODE_NODE_ID:-not set}"
echo "Node Name: ${ANODE_NODE_NAME:-not set}"
echo "Data Directory: ${ANODE_DATA_DIR:-/data}"
echo "Config Directory: ${ANODE_CONFIG_DIR:-/config}"
echo "Log Level: ${ANODE_LOG_LEVEL:-info}"
echo "=========================================="

# Ensure data directories exist
mkdir -p "${ANODE_DATA_DIR}"
mkdir -p "${ANODE_DATA_DIR}/blobs"
mkdir -p "${ANODE_DATA_DIR}/metadata"
mkdir -p "${ANODE_DATA_DIR}/parquet-cache"

# Verify permissions
if [ ! -w "${ANODE_DATA_DIR}" ]; then
    echo "ERROR: Data directory ${ANODE_DATA_DIR} is not writable"
    exit 1
fi

# Check if config file exists
if [ -f "${ANODE_CONFIG_DIR}/anode.toml" ]; then
    echo "Using config file: ${ANODE_CONFIG_DIR}/anode.toml"
else
    echo "WARNING: No config file found at ${ANODE_CONFIG_DIR}/anode.toml"
    echo "Using environment variables and defaults"
fi

# Wait for dependencies (if JOIN_ADDR is set)
if [ -n "$ANODE_JOIN_ADDR" ]; then
    JOIN_HOST=$(echo "$ANODE_JOIN_ADDR" | cut -d: -f1)
    JOIN_PORT=$(echo "$ANODE_JOIN_ADDR" | cut -d: -f2)
    JOIN_PORT=${JOIN_PORT:-9090}

    echo "Waiting for bootstrap node at ${JOIN_HOST}:${JOIN_PORT}..."

    max_attempts=30
    attempt=0
    while [ $attempt -lt $max_attempts ]; do
        if nc -z "$JOIN_HOST" "$JOIN_PORT" 2>/dev/null; then
            echo "Bootstrap node is ready"
            break
        fi
        attempt=$((attempt + 1))
        echo "Attempt $attempt/$max_attempts: Bootstrap node not ready, waiting..."
        sleep 2
    done

    if [ $attempt -eq $max_attempts ]; then
        echo "WARNING: Could not connect to bootstrap node after $max_attempts attempts"
        echo "Proceeding anyway..."
    fi
fi

# Print startup message
echo "=========================================="
echo "Starting Anode with command: $*"
echo "=========================================="

# Execute the main command in the background
"$@" &
ANODE_PID=$!

# Wait for the process
wait $ANODE_PID
EXIT_CODE=$?

echo "Anode exited with code $EXIT_CODE"
exit $EXIT_CODE
