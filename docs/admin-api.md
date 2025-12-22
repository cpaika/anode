# Admin API Reference

Comprehensive reference documentation for Anode's Admin API endpoints used for health checks, monitoring, and cluster management.

## Table of Contents

- [Overview](#overview)
- [Endpoint Summary](#endpoint-summary)
- [Health Endpoints](#health-endpoints)
  - [GET /health](#get-health)
  - [GET /health/live](#get-healthlive)
  - [GET /health/ready](#get-healthready)
  - [GET /health/detailed](#get-healthdetailed)
  - [GET /live](#get-live)
  - [GET /ready](#get-ready)
- [Metrics Endpoint](#metrics-endpoint)
  - [GET /metrics](#get-metrics)
- [Cluster Management Endpoints](#cluster-management-endpoints)
  - [GET /cluster/status](#get-clusterstatus)
  - [GET /node/status](#get-nodestatus)
- [Kubernetes Integration](#kubernetes-integration)
- [Monitoring Integration](#monitoring-integration)

## Overview

The Admin API provides endpoints for:

- **Health Checks**: Liveness and readiness probes for Kubernetes and load balancers
- **Metrics**: Prometheus-compatible metrics for monitoring
- **Cluster Status**: Information about cluster health and membership
- **Node Status**: Raft consensus state and node-level metrics

All Admin API endpoints are exposed on the admin port (default: **9000**) and do not require authentication.

**Base URL**: `http://localhost:9000`

## Endpoint Summary

| Endpoint | Method | Purpose | Use Case |
|----------|--------|---------|----------|
| `/health` | GET | Basic readiness check (JSON) | Application health monitoring |
| `/health/live` | GET | Liveness check (JSON) | Kubernetes liveness probe (JSON response) |
| `/health/ready` | GET | Readiness check (JSON) | Kubernetes readiness probe (JSON response) |
| `/health/detailed` | GET | Detailed health status | Debugging and troubleshooting |
| `/live` | GET | Liveness probe (text) | Kubernetes liveness probe (simple response) |
| `/ready` | GET | Readiness probe (text) | Kubernetes readiness probe (simple response) |
| `/metrics` | GET | Prometheus metrics | Monitoring and alerting |
| `/cluster/status` | GET | Cluster status information | Cluster health monitoring |
| `/node/status` | GET | Node Raft status | Raft consensus monitoring |

## Health Endpoints

### GET /health

Returns basic readiness status in JSON format. This is equivalent to `/health/ready`.

**Method:** `GET`
**Path:** `/health`
**Authentication:** None required

**Response:**
- **Status:** `200 OK`
- **Content-Type:** `application/json`
- **Body:** Health status object

**Response Format:**
```json
{
  "healthy": true,
  "message": "Ready"
}
```

**Response Fields:**
- `healthy` (boolean): Whether the node is ready to serve requests
- `message` (string): Human-readable status message

**Health States:**

| State | `healthy` | `message` | Meaning |
|-------|-----------|-----------|---------|
| Ready | `true` | `"Ready"` | Node has a Raft leader and can serve requests |
| Not Ready | `false` | `"No Raft leader elected"` | Cluster is electing a leader or has lost quorum |

**Example (curl):**
```bash
curl http://localhost:9000/health
```

**Example Response:**
```json
{
  "healthy": true,
  "message": "Ready"
}
```

**Use Cases:**
- Application-level health monitoring
- Load balancer health checks (when JSON parsing is supported)
- Monitoring dashboards

---

### GET /health/live

Returns liveness status in JSON format. A node is considered "live" if the process is running and responsive.

**Method:** `GET`
**Path:** `/health/live`
**Authentication:** None required

**Response:**
- **Status:** `200 OK`
- **Content-Type:** `application/json`
- **Body:** Health status object

**Response Format:**
```json
{
  "healthy": true,
  "message": "Alive"
}
```

**Liveness Check Logic:**
- **Always returns `true`** if the process is running
- Does not check storage, Raft, or cluster health
- Only indicates the HTTP server is responsive

**Example (curl):**
```bash
curl http://localhost:9000/health/live
```

**Example Response:**
```json
{
  "healthy": true,
  "message": "Alive"
}
```

**Use Cases:**
- Kubernetes liveness probe (with JSON parsing)
- Detecting completely hung processes
- Basic process monitoring

---

### GET /health/ready

Returns readiness status in JSON format. A node is "ready" when it can serve S3 API requests.

**Method:** `GET`
**Path:** `/health/ready`
**Authentication:** None required

**Response:**
- **Status:** `200 OK`
- **Content-Type:** `application/json`
- **Body:** Health status object

**Response Format:**
```json
{
  "healthy": true,
  "message": "Ready"
}
```

**Readiness Check Logic:**
1. **Raft Leader Check**: Verifies that a Raft leader is elected
2. **Storage Check**: Confirms storage engine is initialized (future enhancement)

**When Not Ready:**
```json
{
  "healthy": false,
  "message": "No Raft leader elected"
}
```

**Example (curl):**
```bash
curl http://localhost:9000/health/ready
```

**Example Response (Ready):**
```json
{
  "healthy": true,
  "message": "Ready"
}
```

**Example Response (Not Ready):**
```json
{
  "healthy": false,
  "message": "No Raft leader elected"
}
```

**Use Cases:**
- Kubernetes readiness probe (with JSON parsing)
- Load balancer backend health checks
- Determining when to route traffic to a node

---

### GET /health/detailed

Returns detailed health information for all system components.

**Method:** `GET`
**Path:** `/health/detailed`
**Authentication:** None required

**Response:**
- **Status:** `200 OK`
- **Content-Type:** `application/json`
- **Body:** Detailed health object

**Response Format:**
```json
{
  "storage": {
    "healthy": true,
    "message": "Storage engine operational",
    "details": {
      "engine": "operational"
    }
  },
  "raft": {
    "healthy": true,
    "message": "Leader",
    "details": {
      "node_id": 1,
      "current_term": 5,
      "current_leader": 1,
      "is_leader": true,
      "membership": "{voters:[1,2,3]}"
    }
  },
  "cluster": {
    "healthy": true,
    "message": "3/3 nodes healthy",
    "details": {
      "total_nodes": 3,
      "healthy_nodes": 3,
      "leader_id": 1,
      "current_term": 5
    }
  },
  "overall": {
    "healthy": true,
    "message": "All systems operational"
  }
}
```

**Component Health:**

**Storage Component:**
- `healthy`: Always `true` (storage health checks planned)
- `message`: Operational status
- `details`: Storage engine information

**Raft Component:**
- `healthy`: `true` if a leader is elected
- `message`: Role description (e.g., "Leader", "Follower (leader: 2)")
- `details`:
  - `node_id`: This node's Raft ID
  - `current_term`: Current Raft term
  - `current_leader`: Node ID of current leader (or `null`)
  - `is_leader`: Whether this node is the leader
  - `membership`: Cluster membership configuration

**Cluster Component:**
- `healthy`: `true` if leader exists and nodes > 0
- `message`: Node count summary (e.g., "3/3 nodes healthy")
- `details`:
  - `total_nodes`: Total nodes in cluster
  - `healthy_nodes`: Number of healthy nodes
  - `leader_id`: Current leader node ID
  - `current_term`: Current Raft term

**Overall Status:**
- `healthy`: `true` only if all components are healthy
- `message`: Summary message

**Example (curl):**
```bash
curl http://localhost:9000/health/detailed
```

**Example Response (Follower Node):**
```json
{
  "storage": {
    "healthy": true,
    "message": "Storage engine operational",
    "details": {
      "engine": "operational"
    }
  },
  "raft": {
    "healthy": true,
    "message": "Follower (leader: 1)",
    "details": {
      "node_id": 2,
      "current_term": 5,
      "current_leader": 1,
      "is_leader": false,
      "membership": "{voters:[1,2,3]}"
    }
  },
  "cluster": {
    "healthy": true,
    "message": "3/3 nodes healthy",
    "details": {
      "total_nodes": 3,
      "healthy_nodes": 3,
      "leader_id": 1,
      "current_term": 5
    }
  },
  "overall": {
    "healthy": true,
    "message": "All systems operational"
  }
}
```

**Example Response (No Leader):**
```json
{
  "storage": {
    "healthy": true,
    "message": "Storage engine operational",
    "details": {
      "engine": "operational"
    }
  },
  "raft": {
    "healthy": false,
    "message": "No leader elected",
    "details": {
      "node_id": 2,
      "current_term": 5,
      "current_leader": null,
      "is_leader": false,
      "membership": "{voters:[1,2,3]}"
    }
  },
  "cluster": {
    "healthy": false,
    "message": "0/3 nodes healthy",
    "details": {
      "total_nodes": 3,
      "healthy_nodes": 0,
      "leader_id": null,
      "current_term": 5
    }
  },
  "overall": {
    "healthy": false,
    "message": "One or more systems degraded"
  }
}
```

**Use Cases:**
- Troubleshooting cluster issues
- Understanding why a node is unhealthy
- Monitoring dashboards with component-level detail
- Debugging Raft consensus problems

---

### GET /live

Simple text-based liveness probe endpoint for Kubernetes.

**Method:** `GET`
**Path:** `/live`
**Authentication:** None required

**Response (Alive):**
- **Status:** `200 OK`
- **Content-Type:** `text/plain`
- **Body:** `alive`

**Response (Not Alive):**
- **Status:** `503 Service Unavailable`
- **Content-Type:** `text/plain`
- **Body:** `not alive`

**Liveness Check Logic:**
- **Always returns `200 OK`** with body `alive` if the process is running
- Returns `503` only in exceptional cases (process hung)

**Example (curl):**
```bash
curl http://localhost:9000/live
```

**Example Response:**
```
alive
```

**Use Cases:**
- **Kubernetes liveness probe** (recommended - simpler than JSON endpoints)
- Load balancers that don't support JSON parsing
- Simple monitoring scripts

**Kubernetes Liveness Probe Configuration:**
```yaml
livenessProbe:
  httpGet:
    path: /live
    port: 9000
  initialDelaySeconds: 10
  periodSeconds: 10
  timeoutSeconds: 5
  failureThreshold: 3
```

---

### GET /ready

Simple text-based readiness probe endpoint for Kubernetes.

**Method:** `GET`
**Path:** `/ready`
**Authentication:** None required

**Response (Ready):**
- **Status:** `200 OK`
- **Content-Type:** `text/plain`
- **Body:** `ready`

**Response (Not Ready):**
- **Status:** `503 Service Unavailable`
- **Content-Type:** `text/plain`
- **Body:** `not ready`

**Readiness Check Logic:**
1. Checks if Raft has elected a leader
2. Returns `200 OK` if ready, `503` if not ready

**Example (curl):**
```bash
# Check if node is ready
curl http://localhost:9000/ready
echo $?  # 0 if ready (200 OK), non-zero if not ready (503)
```

**Example Response (Ready):**
```
ready
```

**Example Response (Not Ready):**
```
not ready
```

**Use Cases:**
- **Kubernetes readiness probe** (recommended - simpler than JSON endpoints)
- Load balancer health checks
- Deployment scripts waiting for node readiness

**Kubernetes Readiness Probe Configuration:**
```yaml
readinessProbe:
  httpGet:
    path: /ready
    port: 9000
  initialDelaySeconds: 5
  periodSeconds: 5
  timeoutSeconds: 3
  failureThreshold: 3
```

---

## Metrics Endpoint

### GET /metrics

Returns Prometheus-compatible metrics for monitoring and alerting.

**Method:** `GET`
**Path:** `/metrics`
**Authentication:** None required

**Response:**
- **Status:** `200 OK`
- **Content-Type:** `text/plain; version=0.0.4`
- **Body:** Prometheus text format metrics

**Metric Categories:**

1. **Request Metrics**: HTTP request rates, latencies, and errors
2. **Storage Metrics**: Object counts, storage usage, operation latencies
3. **Raft Metrics**: Consensus state, proposals, leader changes
4. **Cluster Metrics**: Node health, replication lag, peer connectivity
5. **Parquet Metrics**: Cache hit rates, query performance
6. **System Metrics**: CPU, memory, disk usage, network traffic
7. **Multipart Metrics**: Active uploads, part counts, upload duration

**Example (curl):**
```bash
curl http://localhost:9000/metrics
```

**Example Response (partial):**
```prometheus
# HELP anode_requests_total Total number of requests
# TYPE anode_requests_total counter
anode_requests_total{method="GET",endpoint="/health",status="200"} 42

# HELP anode_request_duration_seconds Request duration in seconds
# TYPE anode_request_duration_seconds histogram
anode_request_duration_seconds_bucket{method="GET",endpoint="/health",le="0.001"} 35
anode_request_duration_seconds_bucket{method="GET",endpoint="/health",le="0.005"} 42
anode_request_duration_seconds_sum{method="GET",endpoint="/health"} 0.123
anode_request_duration_seconds_count{method="GET",endpoint="/health"} 42

# HELP anode_uptime_seconds Server uptime in seconds
# TYPE anode_uptime_seconds gauge
anode_uptime_seconds 3600.5

# HELP anode_active_requests Number of currently active requests
# TYPE anode_active_requests gauge
anode_active_requests 2

# HELP anode_raft_term Current Raft term
# TYPE anode_raft_term gauge
anode_raft_term 5

# HELP anode_raft_is_leader Whether this node is leader (1) or not (0)
# TYPE anode_raft_is_leader gauge
anode_raft_is_leader 1

# HELP anode_raft_commit_index Raft commit index
# TYPE anode_raft_commit_index gauge
anode_raft_commit_index 1234

# HELP anode_cluster_nodes_total Total number of nodes in cluster
# TYPE anode_cluster_nodes_total gauge
anode_cluster_nodes_total 3

# HELP anode_cluster_healthy_nodes Number of healthy nodes in cluster
# TYPE anode_cluster_healthy_nodes gauge
anode_cluster_healthy_nodes 3

# HELP anode_storage_objects_total Total number of objects in storage
# TYPE anode_storage_objects_total gauge
anode_storage_objects_total 1000

# HELP anode_parquet_cache_hits_total Total Parquet cache hits
# TYPE anode_parquet_cache_hits_total counter
anode_parquet_cache_hits_total 500

# HELP anode_parquet_cache_misses_total Total Parquet cache misses
# TYPE anode_parquet_cache_misses_total counter
anode_parquet_cache_misses_total 100
```

**Key Metrics for Monitoring:**

**Availability:**
- `up` (provided by Prometheus): 1 if scrape successful, 0 if failed
- `anode_raft_is_leader`: 1 if this node is leader
- `anode_cluster_healthy_nodes`: Number of healthy cluster nodes

**Performance:**
- `anode_request_duration_seconds`: Request latency histogram
- `anode_operation_duration_seconds`: Storage operation latency
- `anode_parquet_query_duration_seconds`: Parquet query latency

**Errors:**
- `anode_operation_errors_total`: Storage operation errors
- `anode_raft_proposal_failures_total`: Failed Raft proposals

**Resource Usage:**
- `anode_memory_usage_bytes`: Memory consumption
- `anode_cpu_usage_percent`: CPU utilization
- `anode_disk_usage_bytes`: Disk space used
- `anode_disk_available_bytes`: Available disk space

**Parquet Cache Efficiency:**
- `anode_parquet_cache_hits_total`: Cache hits
- `anode_parquet_cache_misses_total`: Cache misses
- `anode_parquet_cache_size_bytes`: Current cache size
- `anode_parquet_cache_evictions_total`: Cache evictions

**Use Cases:**
- **Prometheus scraping**: Primary use case
- Monitoring dashboards (Grafana)
- Alerting rules based on metric thresholds
- Capacity planning and trend analysis

**Prometheus Configuration:**
```yaml
scrape_configs:
  - job_name: 'anode'
    static_configs:
      - targets: ['localhost:9000']
    scrape_interval: 15s
    metrics_path: /metrics
```

**See Also:**
- [Monitoring Guide](monitoring.md) - Complete metrics reference and Grafana dashboards

---

## Cluster Management Endpoints

### GET /cluster/status

Returns cluster-level status information including all nodes.

**Method:** `GET`
**Path:** `/cluster/status`
**Authentication:** None required

**Response:**
- **Status:** `200 OK`
- **Content-Type:** `application/json`
- **Body:** Cluster status object

**Response Format:**
```json
{
  "cluster_name": "anode-cluster",
  "total_nodes": 3,
  "healthy_nodes": 3,
  "leader_id": 1,
  "current_term": 5,
  "nodes": [
    {
      "node_id": 1,
      "address": "10.0.1.10:8080",
      "is_leader": true,
      "is_healthy": true,
      "last_seen": "2023-12-19T10:30:00Z"
    },
    {
      "node_id": 2,
      "address": "10.0.1.11:8080",
      "is_leader": false,
      "is_healthy": true,
      "last_seen": "2023-12-19T10:30:01Z"
    },
    {
      "node_id": 3,
      "address": "10.0.1.12:8080",
      "is_leader": false,
      "is_healthy": true,
      "last_seen": "2023-12-19T10:30:02Z"
    }
  ]
}
```

**Response Fields:**
- `cluster_name` (string): Cluster identifier
- `total_nodes` (integer): Total nodes in the cluster
- `healthy_nodes` (integer): Number of healthy nodes
- `leader_id` (integer|null): Node ID of the current Raft leader
- `current_term` (integer): Current Raft term
- `nodes` (array): List of node information objects
  - `node_id` (integer): Unique node identifier
  - `address` (string): Node network address
  - `is_leader` (boolean): Whether this node is the leader
  - `is_healthy` (boolean): Whether this node is healthy
  - `last_seen` (string): ISO 8601 timestamp of last heartbeat

**Example (curl):**
```bash
curl http://localhost:9000/cluster/status
```

**Example Response (Healthy Cluster):**
```json
{
  "cluster_name": "production-cluster",
  "total_nodes": 3,
  "healthy_nodes": 3,
  "leader_id": 1,
  "current_term": 12,
  "nodes": [
    {
      "node_id": 1,
      "address": "anode-0.anode.default.svc.cluster.local:8080",
      "is_leader": true,
      "is_healthy": true,
      "last_seen": "2023-12-19T10:30:00Z"
    },
    {
      "node_id": 2,
      "address": "anode-1.anode.default.svc.cluster.local:8080",
      "is_leader": false,
      "is_healthy": true,
      "last_seen": "2023-12-19T10:30:01Z"
    },
    {
      "node_id": 3,
      "address": "anode-2.anode.default.svc.cluster.local:8080",
      "is_leader": false,
      "is_healthy": true,
      "last_seen": "2023-12-19T10:30:02Z"
    }
  ]
}
```

**Example Response (Degraded Cluster - No Leader):**
```json
{
  "cluster_name": "production-cluster",
  "total_nodes": 3,
  "healthy_nodes": 2,
  "leader_id": null,
  "current_term": 13,
  "nodes": [
    {
      "node_id": 1,
      "address": "anode-0.anode.default.svc.cluster.local:8080",
      "is_leader": false,
      "is_healthy": true,
      "last_seen": "2023-12-19T10:30:00Z"
    },
    {
      "node_id": 2,
      "address": "anode-1.anode.default.svc.cluster.local:8080",
      "is_leader": false,
      "is_healthy": true,
      "last_seen": "2023-12-19T10:30:01Z"
    },
    {
      "node_id": 3,
      "address": "anode-2.anode.default.svc.cluster.local:8080",
      "is_leader": false,
      "is_healthy": false,
      "last_seen": "2023-12-19T10:25:00Z"
    }
  ]
}
```

**Use Cases:**
- Monitoring cluster membership
- Identifying the current leader
- Detecting unhealthy nodes
- Capacity planning
- Troubleshooting cluster issues

**Example (Using jq to find leader):**
```bash
curl -s http://localhost:9000/cluster/status | \
  jq '.nodes[] | select(.is_leader == true) | .address'
```

---

### GET /node/status

Returns Raft consensus status for this specific node.

**Method:** `GET`
**Path:** `/node/status`
**Authentication:** None required

**Response:**
- **Status:** `200 OK`
- **Content-Type:** `application/json`
- **Body:** Node status object

**Response Format:**
```json
{
  "node_id": 1,
  "is_leader": true,
  "current_term": 5,
  "current_leader": 1,
  "uptime_seconds": 3600.5
}
```

**Response Fields:**
- `node_id` (integer): This node's unique Raft ID
- `is_leader` (boolean): Whether this node is the Raft leader
- `current_term` (integer): Current Raft term number
- `current_leader` (integer|null): Node ID of current leader (or `null` if no leader)
- `uptime_seconds` (float): Node uptime in seconds

**Example (curl):**
```bash
curl http://localhost:9000/node/status
```

**Example Response (Leader Node):**
```json
{
  "node_id": 1,
  "is_leader": true,
  "current_term": 12,
  "current_leader": 1,
  "uptime_seconds": 86400.25
}
```

**Example Response (Follower Node):**
```json
{
  "node_id": 2,
  "is_leader": false,
  "current_term": 12,
  "current_leader": 1,
  "uptime_seconds": 3600.5
}
```

**Example Response (No Leader):**
```json
{
  "node_id": 2,
  "is_leader": false,
  "current_term": 13,
  "current_leader": null,
  "uptime_seconds": 3650.75
}
```

**Use Cases:**
- Determining if a specific node is the leader
- Monitoring Raft term progression
- Detecting leader election issues
- Node-level monitoring dashboards
- Troubleshooting consensus problems

**Example (Check if node is leader):**
```bash
if curl -s http://localhost:9000/node/status | jq -e '.is_leader == true' > /dev/null; then
  echo "This node is the leader"
else
  echo "This node is a follower"
fi
```

---

## Kubernetes Integration

The Admin API is designed for seamless integration with Kubernetes health checking.

### Liveness Probe

Use `/live` to detect when a pod should be restarted:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: anode
spec:
  containers:
  - name: anode
    image: anode:latest
    ports:
    - containerPort: 8080
      name: s3-api
    - containerPort: 9000
      name: admin
    livenessProbe:
      httpGet:
        path: /live
        port: 9000
      initialDelaySeconds: 10
      periodSeconds: 10
      timeoutSeconds: 5
      failureThreshold: 3
```

**Configuration Recommendations:**
- `initialDelaySeconds: 10` - Allow time for startup
- `periodSeconds: 10` - Check every 10 seconds
- `timeoutSeconds: 5` - 5 second timeout
- `failureThreshold: 3` - Restart after 3 consecutive failures

### Readiness Probe

Use `/ready` to detect when a pod can serve traffic:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: anode
spec:
  containers:
  - name: anode
    image: anode:latest
    ports:
    - containerPort: 8080
      name: s3-api
    - containerPort: 9000
      name: admin
    readinessProbe:
      httpGet:
        path: /ready
        port: 9000
      initialDelaySeconds: 5
      periodSeconds: 5
      timeoutSeconds: 3
      failureThreshold: 3
      successThreshold: 1
```

**Configuration Recommendations:**
- `initialDelaySeconds: 5` - Start checking readiness early
- `periodSeconds: 5` - Check frequently during startup
- `timeoutSeconds: 3` - Short timeout
- `failureThreshold: 3` - Remove from service after 3 failures
- `successThreshold: 1` - Add back after 1 success

### StatefulSet Example

Complete StatefulSet configuration with health checks:

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: anode
spec:
  serviceName: anode
  replicas: 3
  selector:
    matchLabels:
      app: anode
  template:
    metadata:
      labels:
        app: anode
    spec:
      containers:
      - name: anode
        image: anode:latest
        ports:
        - containerPort: 8080
          name: s3-api
        - containerPort: 9000
          name: admin
        - containerPort: 7000
          name: raft
        env:
        - name: ANODE_NODE_ID
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        livenessProbe:
          httpGet:
            path: /live
            port: 9000
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /ready
            port: 9000
          initialDelaySeconds: 5
          periodSeconds: 5
          timeoutSeconds: 3
          failureThreshold: 3
        volumeMounts:
        - name: data
          mountPath: /data
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 100Gi
```

### Service with Health Check

Expose the admin port for health checks:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: anode-admin
spec:
  selector:
    app: anode
  ports:
  - port: 9000
    targetPort: 9000
    name: admin
  type: ClusterIP
```

---

## Monitoring Integration

### Prometheus ServiceMonitor

Automatically configure Prometheus to scrape metrics:

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: anode
  labels:
    app: anode
spec:
  selector:
    matchLabels:
      app: anode
  endpoints:
  - port: admin
    path: /metrics
    interval: 15s
    scrapeTimeout: 10s
```

### Prometheus Scrape Config

For non-Operator Prometheus deployments:

```yaml
scrape_configs:
  - job_name: 'anode'
    static_configs:
      - targets:
        - 'anode-0.anode.default.svc.cluster.local:9000'
        - 'anode-1.anode.default.svc.cluster.local:9000'
        - 'anode-2.anode.default.svc.cluster.local:9000'
    scrape_interval: 15s
    metrics_path: /metrics
```

### Alert Rules

Example Prometheus alert rules:

```yaml
groups:
- name: anode
  interval: 30s
  rules:
  # Critical: Node is down
  - alert: AnodeNodeDown
    expr: up{job="anode"} == 0
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "Anode node {{ $labels.instance }} is down"
      description: "Node has been unreachable for more than 1 minute"

  # Critical: No leader elected
  - alert: AnodeNoLeader
    expr: sum(anode_raft_is_leader) == 0
    for: 30s
    labels:
      severity: critical
    annotations:
      summary: "Anode cluster has no leader"
      description: "No Raft leader elected for 30 seconds"

  # Critical: Multiple leaders (split brain)
  - alert: AnodeMultipleLeaders
    expr: sum(anode_raft_is_leader) > 1
    for: 10s
    labels:
      severity: critical
    annotations:
      summary: "Multiple Anode leaders detected (split brain)"
      description: "{{ $value }} leaders detected - potential split brain"

  # Warning: High request latency
  - alert: AnodeHighRequestLatency
    expr: histogram_quantile(0.95, rate(anode_request_duration_seconds_bucket[5m])) > 1
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "High request latency on {{ $labels.instance }}"
      description: "P95 latency is {{ $value }}s (threshold: 1s)"

  # Warning: Low disk space
  - alert: AnodeDiskSpaceLow
    expr: (anode_disk_available_bytes / (anode_disk_usage_bytes + anode_disk_available_bytes)) < 0.2
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "Low disk space on {{ $labels.instance }}"
      description: "Only {{ $value | humanizePercentage }} disk space remaining"

  # Critical: Very low disk space
  - alert: AnodeDiskSpaceCritical
    expr: (anode_disk_available_bytes / (anode_disk_usage_bytes + anode_disk_available_bytes)) < 0.1
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "Critical disk space on {{ $labels.instance }}"
      description: "Only {{ $value | humanizePercentage }} disk space remaining"

  # Warning: Cluster quorum at risk
  - alert: AnodeClusterQuorumAtRisk
    expr: anode_cluster_healthy_nodes < 2
    for: 1m
    labels:
      severity: warning
    annotations:
      summary: "Anode cluster quorum at risk"
      description: "Only {{ $value }} healthy nodes (need 2 for quorum)"
```

### Grafana Dashboard Queries

Example PromQL queries for Grafana dashboards:

**Request Rate:**
```promql
sum(rate(anode_requests_total[5m])) by (instance)
```

**Request Latency (P95):**
```promql
histogram_quantile(0.95, sum(rate(anode_request_duration_seconds_bucket[5m])) by (le, instance))
```

**Error Rate:**
```promql
sum(rate(anode_requests_total{status=~"[45].."}[5m])) / sum(rate(anode_requests_total[5m]))
```

**Cluster Health:**
```promql
anode_cluster_healthy_nodes / anode_cluster_nodes_total
```

**Raft Leader:**
```promql
anode_raft_is_leader
```

**Storage Usage:**
```promql
sum(anode_storage_bytes_total) by (bucket)
```

**Parquet Cache Hit Rate:**
```promql
rate(anode_parquet_cache_hits_total[5m]) /
  (rate(anode_parquet_cache_hits_total[5m]) + rate(anode_parquet_cache_misses_total[5m]))
```

---

## Troubleshooting

### Common Issues

**Problem: `/ready` returns 503**

**Diagnosis:**
```bash
# Check detailed health
curl http://localhost:9000/health/detailed | jq

# Check node status
curl http://localhost:9000/node/status | jq

# Check cluster status
curl http://localhost:9000/cluster/status | jq
```

**Possible Causes:**
1. No Raft leader elected (cluster startup or network partition)
2. Cluster has lost quorum (majority of nodes down)
3. Node is isolated from cluster

**Solution:**
1. Wait for leader election to complete (typically < 30 seconds)
2. Check network connectivity between nodes
3. Verify at least N/2 + 1 nodes are running
4. Check Raft logs for election issues

---

**Problem: Metrics endpoint returns empty or incomplete data**

**Diagnosis:**
```bash
# Fetch metrics
curl http://localhost:9000/metrics

# Check for specific metric
curl -s http://localhost:9000/metrics | grep anode_raft_term
```

**Possible Causes:**
1. Prometheus scrape timing (some metrics only appear after activity)
2. Metrics not being updated (check application logs)

**Solution:**
1. Verify metrics are being exported: `curl http://localhost:9000/metrics | grep "# TYPE"`
2. Generate activity (make S3 requests) to populate request metrics
3. Check application logs for errors

---

**Problem: Prometheus not scraping metrics**

**Diagnosis:**
```bash
# Check Prometheus targets
# Navigate to http://prometheus:9090/targets

# Test manual scrape
curl http://anode-pod:9000/metrics
```

**Possible Causes:**
1. ServiceMonitor not created or incorrect label selector
2. Network policy blocking Prometheus access
3. Admin port not exposed in Service
4. Firewall rules blocking port 9000

**Solution:**
1. Verify ServiceMonitor matches Service labels
2. Check network policies allow Prometheus namespace to scrape
3. Ensure Service exposes port 9000
4. Verify pod is listening on 0.0.0.0:9000, not 127.0.0.1:9000

---

**Problem: Kubernetes readiness probe failing but node appears healthy**

**Diagnosis:**
```bash
# Check pod events
kubectl describe pod anode-0

# Test readiness endpoint manually
kubectl exec anode-0 -- curl http://localhost:9000/ready

# Check detailed health
kubectl exec anode-0 -- curl http://localhost:9000/health/detailed | jq
```

**Possible Causes:**
1. Timeout too short for slow clusters
2. Raft leader election taking longer than expected
3. DNS or network issues within pod

**Solution:**
1. Increase `timeoutSeconds` and `initialDelaySeconds` in readiness probe
2. Check Raft logs for election issues
3. Verify node can communicate with other cluster members

---

## Performance Considerations

### Health Check Frequency

- **Liveness probes**: Check every 10-30 seconds
- **Readiness probes**: Check every 5-10 seconds
- **Prometheus scrapes**: Every 15-30 seconds

**Recommendation**: Avoid checking more frequently than every 5 seconds to reduce overhead.

### Metrics Cardinality

Anode metrics are designed with low cardinality:

- Fixed label sets (method, endpoint, status)
- No unbounded labels (e.g., bucket names, object keys)
- Efficient encoding for Prometheus

**Current Cardinality**: ~500 unique time series per node (safe for Prometheus)

### Admin Port Isolation

The admin port (9000) is separate from the S3 API port (8080) for:

1. **Security**: Health checks don't require S3 authentication
2. **Performance**: Health checks don't compete with S3 traffic
3. **Reliability**: Admin endpoints remain available even under heavy S3 load

---

## Security Considerations

### No Authentication Required

Admin API endpoints do **not** require authentication:

- **Rationale**: Health checks and metrics need to work without credentials
- **Risk**: Information disclosure (cluster topology, metrics)
- **Mitigation**: Restrict admin port access via network policies or firewalls

### Network Access Control

**Kubernetes Network Policy Example:**

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: anode-admin-access
spec:
  podSelector:
    matchLabels:
      app: anode
  policyTypes:
  - Ingress
  ingress:
  # Allow Prometheus to scrape metrics
  - from:
    - namespaceSelector:
        matchLabels:
          name: monitoring
    ports:
    - protocol: TCP
      port: 9000
  # Allow Kubernetes API server for health checks
  - from:
    - namespaceSelector:
        matchLabels:
          name: kube-system
    ports:
    - protocol: TCP
      port: 9000
  # Allow internal cluster access
  - from:
    - podSelector:
        matchLabels:
          app: anode
    ports:
    - protocol: TCP
      port: 9000
```

### Firewall Rules

For non-Kubernetes deployments:

```bash
# Allow Prometheus server
iptables -A INPUT -p tcp --dport 9000 -s 10.0.1.100 -j ACCEPT

# Allow Kubernetes nodes
iptables -A INPUT -p tcp --dport 9000 -s 10.0.0.0/16 -j ACCEPT

# Deny all other access to admin port
iptables -A INPUT -p tcp --dport 9000 -j DROP
```

---

## See Also

- [Monitoring Guide](monitoring.md) - Complete metrics reference, Grafana dashboards, and alert rules
- [Operations Guide](operations.md) - Cluster operations and troubleshooting
- [Configuration Guide](configuration.md) - Admin port configuration options
- [Security Guide](security.md) - Security best practices
