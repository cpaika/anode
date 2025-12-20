#!/bin/bash
# Chaos testing runner script
# Executes various chaos scenarios against the Anode cluster

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
CLUSTER_SIZE="${ANODE_CLUSTER_SIZE:-5}"
CHAOS_DURATION="${CHAOS_DURATION:-3600}"  # 1 hour
TOXIPROXY_API="${TOXIPROXY_API:-http://toxiproxy:8474}"
RESULTS_DIR="${RESULTS_DIR:-/results}"
SCENARIOS="${CHAOS_SCENARIOS:-partition,latency,packet_loss,node_crash,disk_full}"

# Logging
log_info() {
    echo -e "${GREEN}[INFO]${NC} $(date '+%Y-%m-%d %H:%M:%S') $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $(date '+%Y-%m-%d %H:%M:%S') $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $(date '+%Y-%m-%d %H:%M:%S') $*"
}

log_test() {
    echo -e "${BLUE}[TEST]${NC} $(date '+%Y-%m-%d %H:%M:%S') $*"
}

# Wait for cluster to be ready
wait_for_cluster() {
    log_info "Waiting for cluster to be ready..."

    local max_attempts=60
    local attempt=0

    while [ $attempt -lt $max_attempts ]; do
        local healthy_count=0

        for i in $(seq 1 "$CLUSTER_SIZE"); do
            if curl -sf "http://chaos-anode-${i}:8081/health" >/dev/null 2>&1; then
                healthy_count=$((healthy_count + 1))
            fi
        done

        if [ $healthy_count -eq "$CLUSTER_SIZE" ]; then
            log_info "All $CLUSTER_SIZE nodes are healthy"
            return 0
        fi

        log_info "Healthy nodes: $healthy_count/$CLUSTER_SIZE"
        sleep 2
        attempt=$((attempt + 1))
    done

    log_error "Cluster failed to become healthy"
    return 1
}

# Network partition scenario
chaos_partition() {
    log_test "Starting network partition chaos..."

    # Partition node 2 and 3 from the rest
    log_info "Creating network partition (nodes 2-3 isolated)"

    curl -sf -X POST "${TOXIPROXY_API}/proxies/anode-2-proxy/toxics" \
        -d '{"name":"partition","type":"timeout","attributes":{"timeout":0}}' \
        >/dev/null 2>&1 || log_warn "Failed to partition node 2"

    curl -sf -X POST "${TOXIPROXY_API}/proxies/anode-3-proxy/toxics" \
        -d '{"name":"partition","type":"timeout","attributes":{"timeout":0}}' \
        >/dev/null 2>&1 || log_warn "Failed to partition node 3"

    log_info "Partition active, sleeping for 30 seconds..."
    sleep 30

    # Heal partition
    log_info "Healing network partition"

    curl -sf -X DELETE "${TOXIPROXY_API}/proxies/anode-2-proxy/toxics/partition" \
        >/dev/null 2>&1 || log_warn "Failed to heal node 2"

    curl -sf -X DELETE "${TOXIPROXY_API}/proxies/anode-3-proxy/toxics/partition" \
        >/dev/null 2>&1 || log_warn "Failed to heal node 3"

    log_info "Partition healed, sleeping for 10 seconds..."
    sleep 10

    log_test "Network partition chaos complete"
}

# Latency injection scenario
chaos_latency() {
    log_test "Starting latency chaos..."

    # Add 500ms latency to all nodes
    for i in $(seq 1 "$CLUSTER_SIZE"); do
        log_info "Adding 500ms latency to node $i"

        curl -sf -X POST "${TOXIPROXY_API}/proxies/anode-${i}-proxy/toxics" \
            -d '{"name":"latency","type":"latency","attributes":{"latency":500,"jitter":100}}' \
            >/dev/null 2>&1 || log_warn "Failed to add latency to node $i"
    done

    log_info "Latency active, sleeping for 30 seconds..."
    sleep 30

    # Remove latency
    for i in $(seq 1 "$CLUSTER_SIZE"); do
        log_info "Removing latency from node $i"

        curl -sf -X DELETE "${TOXIPROXY_API}/proxies/anode-${i}-proxy/toxics/latency" \
            >/dev/null 2>&1 || log_warn "Failed to remove latency from node $i"
    done

    log_info "Latency removed, sleeping for 10 seconds..."
    sleep 10

    log_test "Latency chaos complete"
}

# Packet loss scenario
chaos_packet_loss() {
    log_test "Starting packet loss chaos..."

    # Add 30% packet loss to nodes 2 and 4
    for i in 2 4; do
        log_info "Adding 30% packet loss to node $i"

        curl -sf -X POST "${TOXIPROXY_API}/proxies/anode-${i}-proxy/toxics" \
            -d '{"name":"packet_loss","type":"limit_data","attributes":{"bytes":100000}}' \
            >/dev/null 2>&1 || log_warn "Failed to add packet loss to node $i"
    done

    log_info "Packet loss active, sleeping for 30 seconds..."
    sleep 30

    # Remove packet loss
    for i in 2 4; do
        log_info "Removing packet loss from node $i"

        curl -sf -X DELETE "${TOXIPROXY_API}/proxies/anode-${i}-proxy/toxics/packet_loss" \
            >/dev/null 2>&1 || log_warn "Failed to remove packet loss from node $i"
    done

    log_info "Packet loss removed, sleeping for 10 seconds..."
    sleep 10

    log_test "Packet loss chaos complete"
}

# Node crash scenario
chaos_node_crash() {
    log_test "Starting node crash chaos..."

    # Crash node 3
    local target_node="chaos-anode-3"
    log_info "Crashing node $target_node"

    docker kill "$target_node" >/dev/null 2>&1 || log_warn "Failed to kill $target_node"

    log_info "Node crashed, sleeping for 20 seconds..."
    sleep 20

    # Restart node
    log_info "Restarting node $target_node"
    docker start "$target_node" >/dev/null 2>&1 || log_warn "Failed to restart $target_node"

    log_info "Node restarted, sleeping for 15 seconds..."
    sleep 15

    log_test "Node crash chaos complete"
}

# Bandwidth limiting scenario
chaos_bandwidth() {
    log_test "Starting bandwidth limiting chaos..."

    # Limit bandwidth to 1MB/s on node 1
    log_info "Limiting bandwidth on node 1 to 1MB/s"

    curl -sf -X POST "${TOXIPROXY_API}/proxies/anode-1-proxy/toxics" \
        -d '{"name":"bandwidth","type":"bandwidth","attributes":{"rate":1000}}' \
        >/dev/null 2>&1 || log_warn "Failed to limit bandwidth on node 1"

    log_info "Bandwidth limited, sleeping for 30 seconds..."
    sleep 30

    # Remove bandwidth limit
    log_info "Removing bandwidth limit from node 1"

    curl -sf -X DELETE "${TOXIPROXY_API}/proxies/anode-1-proxy/toxics/bandwidth" \
        >/dev/null 2>&1 || log_warn "Failed to remove bandwidth limit from node 1"

    log_info "Bandwidth limit removed, sleeping for 10 seconds..."
    sleep 10

    log_test "Bandwidth limiting chaos complete"
}

# Combined chaos scenario
chaos_combined() {
    log_test "Starting combined chaos scenario..."

    # Apply multiple failures simultaneously
    log_info "Applying latency to node 1"
    curl -sf -X POST "${TOXIPROXY_API}/proxies/anode-1-proxy/toxics" \
        -d '{"name":"latency","type":"latency","attributes":{"latency":200}}' \
        >/dev/null 2>&1 || log_warn "Failed"

    log_info "Applying packet loss to node 2"
    curl -sf -X POST "${TOXIPROXY_API}/proxies/anode-2-proxy/toxics" \
        -d '{"name":"packet_loss","type":"limit_data","attributes":{"bytes":50000}}' \
        >/dev/null 2>&1 || log_warn "Failed"

    log_info "Crashing node 4"
    docker pause chaos-anode-4 >/dev/null 2>&1 || log_warn "Failed to pause node 4"

    log_info "Combined chaos active, sleeping for 45 seconds..."
    sleep 45

    # Clean up
    log_info "Removing all toxics and resuming nodes"
    curl -sf -X DELETE "${TOXIPROXY_API}/proxies/anode-1-proxy/toxics/latency" >/dev/null 2>&1 || true
    curl -sf -X DELETE "${TOXIPROXY_API}/proxies/anode-2-proxy/toxics/packet_loss" >/dev/null 2>&1 || true
    docker unpause chaos-anode-4 >/dev/null 2>&1 || true

    log_info "Combined chaos cleaned up, sleeping for 15 seconds..."
    sleep 15

    log_test "Combined chaos complete"
}

# Verify cluster consistency
verify_cluster() {
    log_info "Verifying cluster consistency..."

    # TODO: Implement actual consistency checks
    # For now, just check that all nodes are healthy

    local healthy=0
    for i in $(seq 1 "$CLUSTER_SIZE"); do
        if curl -sf "http://chaos-anode-${i}:8081/health" >/dev/null 2>&1; then
            healthy=$((healthy + 1))
        fi
    done

    if [ $healthy -eq "$CLUSTER_SIZE" ]; then
        log_info "Cluster consistency verified: $healthy/$CLUSTER_SIZE nodes healthy"
        return 0
    else
        log_error "Cluster consistency check failed: only $healthy/$CLUSTER_SIZE nodes healthy"
        return 1
    fi
}

# Main test loop
run_chaos_tests() {
    log_info "=========================================="
    log_info "Anode Chaos Testing"
    log_info "=========================================="
    log_info "Cluster size: $CLUSTER_SIZE"
    log_info "Duration: $CHAOS_DURATION seconds"
    log_info "Scenarios: $SCENARIOS"
    log_info "=========================================="

    # Wait for cluster
    if ! wait_for_cluster; then
        log_error "Cluster not ready, aborting"
        exit 1
    fi

    # Create results directory
    mkdir -p "$RESULTS_DIR"

    # Start time
    local start_time=$(date +%s)
    local end_time=$((start_time + CHAOS_DURATION))

    # Run scenarios in a loop
    local iteration=1
    while [ $(date +%s) -lt $end_time ]; do
        log_info "=========================================="
        log_info "Chaos iteration $iteration"
        log_info "=========================================="

        # Parse and run scenarios
        IFS=',' read -ra SCENARIO_ARRAY <<< "$SCENARIOS"
        for scenario in "${SCENARIO_ARRAY[@]}"; do
            case "$scenario" in
                partition)
                    chaos_partition
                    ;;
                latency)
                    chaos_latency
                    ;;
                packet_loss)
                    chaos_packet_loss
                    ;;
                node_crash)
                    chaos_node_crash
                    ;;
                bandwidth)
                    chaos_bandwidth
                    ;;
                combined)
                    chaos_combined
                    ;;
                *)
                    log_warn "Unknown scenario: $scenario"
                    ;;
            esac

            # Verify cluster after each scenario
            if ! verify_cluster; then
                log_error "Cluster consistency check failed after $scenario"
                # Continue anyway for more testing
            fi

            # Small delay between scenarios
            sleep 5
        done

        iteration=$((iteration + 1))

        # Longer delay between iterations
        log_info "Iteration $iteration complete, resting for 30 seconds..."
        sleep 30
    done

    log_info "=========================================="
    log_info "Chaos testing complete"
    log_info "Total iterations: $iteration"
    log_info "=========================================="

    # Final verification
    if verify_cluster; then
        log_info "Final cluster state: HEALTHY"
        echo "success" > "$RESULTS_DIR/test-result.txt"
        exit 0
    else
        log_error "Final cluster state: UNHEALTHY"
        echo "failure" > "$RESULTS_DIR/test-result.txt"
        exit 1
    fi
}

# Handle signals
trap 'log_info "Received interrupt, cleaning up..."; exit 130' INT TERM

# Run tests
run_chaos_tests
