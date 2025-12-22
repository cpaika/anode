# Anode Monitoring Guide

This guide provides comprehensive information about monitoring Anode distributed object storage using Prometheus and Grafana.

## Table of Contents

- [Overview](#overview)
- [Metrics Architecture](#metrics-architecture)
- [Available Metrics](#available-metrics)
- [Deployment](#deployment)
  - [Docker Compose](#docker-compose)
  - [Kubernetes with Prometheus Operator](#kubernetes-with-prometheus-operator)
- [Grafana Dashboards](#grafana-dashboards)
- [Alerting](#alerting)
- [Recording Rules](#recording-rules)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## Overview

Anode exposes comprehensive Prometheus metrics that provide visibility into:

- **Request Performance**: Request rates, latencies, and error rates
- **Raft Consensus**: Leader status, term, commit index, and proposal metrics
- **Storage Operations**: Object counts, storage usage, and operation latencies
- **Parquet Cache**: Cache hit rates, evictions, and query performance
- **Cluster Health**: Node availability, replication lag, and peer connectivity
- **System Resources**: CPU, memory, disk usage, and network traffic

All metrics are exposed at the `/metrics` endpoint on the admin port (default: 9000).

## Metrics Architecture

### Metrics Collection

```
┌─────────────┐
│   Anode     │
│   Nodes     │──── /metrics endpoint (port 9000)
└─────────────┘
       │
       ├──── Prometheus (scrapes metrics every 15s)
       │
       └──── Recording Rules (pre-compute aggregations)
              │
              └──── Grafana Dashboards
              │
              └──── Alertmanager (sends alerts)
```

### Metric Types

1. **Counters**: Monotonically increasing values (requests_total, errors_total)
2. **Gauges**: Values that can go up or down (memory_usage, active_connections)
3. **Histograms**: Distribution of values (request_duration_seconds, operation_duration_seconds)

## Available Metrics

### Request Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `anode_requests_total` | Counter | method, endpoint, status | Total HTTP requests |
| `anode_request_duration_seconds` | Histogram | method, endpoint | Request duration distribution |
| `anode_request_size_bytes` | Histogram | method, endpoint | Request size distribution |
| `anode_response_size_bytes` | Histogram | method, endpoint | Response size distribution |
| `anode_active_requests` | Gauge | - | Currently active requests |

### Storage Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `anode_objects_total` | Gauge | bucket | Total number of objects per bucket |
| `anode_storage_bytes_total` | Gauge | bucket | Total storage in bytes per bucket |
| `anode_chunks_total` | Gauge | bucket | Total number of chunks per bucket |
| `anode_operations_total` | Counter | operation | Total storage operations |
| `anode_operation_errors_total` | Counter | operation, error_type | Total operation errors |
| `anode_operation_duration_seconds` | Histogram | operation | Storage operation latency |

### Raft Consensus Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `anode_raft_term` | Gauge | - | Current Raft term |
| `anode_raft_is_leader` | Gauge | - | Whether node is leader (1) or not (0) |
| `anode_raft_commit_index` | Gauge | - | Raft commit index |
| `anode_raft_last_applied` | Gauge | - | Last applied index |
| `anode_raft_proposals_total` | Counter | - | Total Raft proposals |
| `anode_raft_proposal_failures_total` | Counter | - | Total proposal failures |
| `anode_raft_leader_changes_total` | Counter | - | Total leader changes |
| `anode_raft_snapshot_creating` | Gauge | - | Whether snapshot is being created |
| `anode_raft_snapshot_last_index` | Gauge | - | Last snapshot index |

### Cluster Health Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `anode_cluster_nodes_total` | Gauge | - | Total nodes in cluster |
| `anode_cluster_healthy_nodes` | Gauge | - | Number of healthy nodes |
| `anode_cluster_replication_lag_seconds` | Gauge | peer_id | Replication lag per peer |
| `anode_cluster_peer_rtt_seconds` | Gauge | peer_id | Round-trip time to peers |

### Parquet Cache Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `anode_parquet_cache_hits_total` | Counter | - | Cache hits |
| `anode_parquet_cache_misses_total` | Counter | - | Cache misses |
| `anode_parquet_cache_size_bytes` | Gauge | - | Current cache size |
| `anode_parquet_cache_items` | Gauge | - | Number of cached items |
| `anode_parquet_cache_evictions_total` | Counter | - | Total evictions |
| `anode_parquet_queries_total` | Counter | query_type | Total Parquet queries |
| `anode_parquet_query_duration_seconds` | Histogram | query_type | Query latency |
| `anode_parquet_files_total` | Gauge | - | Total Parquet files |
| `anode_parquet_row_groups_scanned` | Counter | file | Row groups scanned |

### System Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `anode_uptime_seconds` | Gauge | - | Server uptime |
| `anode_memory_usage_bytes` | Gauge | - | Memory usage |
| `anode_cpu_usage_percent` | Gauge | - | CPU usage percentage |
| `anode_disk_usage_bytes` | Gauge | path | Disk usage per path |
| `anode_disk_available_bytes` | Gauge | path | Available disk space |
| `anode_open_file_descriptors` | Gauge | - | Open file descriptors |

### Network Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `anode_network_bytes_sent_total` | Counter | protocol | Bytes sent over network |
| `anode_network_bytes_received_total` | Counter | protocol | Bytes received |
| `anode_active_connections` | Gauge | protocol | Active connections |

### Multipart Upload Metrics

| Metric Name | Type | Labels | Description |
|-------------|------|--------|-------------|
| `anode_multipart_uploads_active` | Gauge | - | Active multipart uploads |
| `anode_multipart_parts_total` | Counter | status | Multipart parts count |
| `anode_multipart_upload_duration_seconds` | Histogram | status | Upload duration |

## Deployment

### Docker Compose

The easiest way to get started with monitoring is using Docker Compose:

```bash
# Navigate to the docker deployment directory
cd deploy/docker

# Start the monitoring stack
docker-compose -f docker-compose.monitoring.yml up -d

# Verify services are running
docker-compose -f docker-compose.monitoring.yml ps
```

This will start:
- **Prometheus** on port 9090
- **Grafana** on port 3000 (admin/admin)
- **Alertmanager** on port 9093
- **Node Exporter** on port 9100 (system metrics)
- **cAdvisor** on port 8080 (container metrics)

#### Accessing Services

- Prometheus: http://localhost:9090
- Grafana: http://localhost:3000 (username: admin, password: admin)
- Alertmanager: http://localhost:9093

#### Configuration Files

Customize the monitoring stack by editing:
- `prometheus/prometheus.yml` - Prometheus configuration
- `prometheus/alerts.yml` - Alert rules
- `prometheus/recording-rules.yml` - Recording rules
- `grafana/provisioning/` - Grafana datasources and dashboards
- `alertmanager/config.yml` - Alertmanager configuration

### Kubernetes with Prometheus Operator

For Kubernetes deployments with Prometheus Operator:

1. **Install Prometheus Operator** (if not already installed):

```bash
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo update
helm install prometheus prometheus-community/kube-prometheus-stack \
  --namespace monitoring --create-namespace
```

2. **Deploy Anode with monitoring enabled**:

```bash
helm install anode ./deploy/helm/anode \
  --set monitoring.enabled=true \
  --set monitoring.serviceMonitor.enabled=true \
  --set monitoring.prometheusRule.enabled=true
```

The Helm chart will automatically create:
- **ServiceMonitor**: Configures Prometheus to scrape Anode metrics
- **PrometheusRule**: Defines alerting rules

3. **Access Grafana**:

```bash
# Port-forward to Grafana
kubectl port-forward -n monitoring svc/prometheus-grafana 3000:80

# Import the Anode dashboard from deploy/docker/grafana/provisioning/dashboards/anode.json
```

## Grafana Dashboards

The pre-built Anode dashboard provides comprehensive visualization:

### Dashboard Sections

1. **Cluster Overview**
   - Nodes up/down
   - Raft leader status
   - Total objects and storage

2. **Request Metrics**
   - Request rate by instance
   - Error rate
   - Latency percentiles (p50, p95, p99)

3. **Raft Consensus**
   - Current term
   - Commit index vs applied index
   - Proposal rate and failures

4. **Storage Metrics**
   - Operation rate by type
   - Storage operation latency
   - Storage growth trends

5. **Parquet Cache**
   - Cache hit rate gauge
   - Cache size over time
   - Query rate by type

6. **Cluster Health**
   - Replication lag per peer
   - Peer round-trip time
   - Network partition detection

7. **System Resources**
   - Memory usage per node
   - CPU usage
   - Disk usage percentage

### Importing the Dashboard

**Via Provisioning (automatic)**:
The dashboard is automatically loaded if using the Docker Compose setup.

**Manual Import**:
1. Log in to Grafana
2. Navigate to Dashboards → Import
3. Upload `deploy/docker/grafana/provisioning/dashboards/anode.json`
4. Select the Prometheus datasource

## Alerting

### Critical Alerts

These alerts require immediate attention:

| Alert Name | Condition | Description |
|------------|-----------|-------------|
| `AnodeNodeDown` | Node unreachable for >1min | Cluster node is down |
| `AnodeClusterQuorumLost` | <2 nodes available | Cluster lost quorum |
| `AnodeNoLeader` | No leader for >30s | Raft leader election failed |
| `AnodeMultipleLeaders` | >1 leader detected | Split-brain scenario |
| `AnodeReplicationLagCritical` | Lag >60s for >2min | Critical replication delay |
| `AnodeDiskSpaceCritical` | <10% disk space | Immediate action required |

### Warning Alerts

These alerts indicate degraded performance:

| Alert Name | Condition | Description |
|------------|-----------|-------------|
| `AnodeHighRequestLatency` | p95 >1s for >5min | Request performance degraded |
| `AnodeReplicationLagHigh` | Lag >10s for >5min | Replication delay detected |
| `AnodeDiskSpaceLow` | <20% disk space | Disk space running low |
| `AnodeHighErrorRate` | >0.05 errors/s | Elevated error rate |
| `AnodeParquetCacheHitRateLow` | <50% hit rate | Cache inefficiency |

### Configuring Alertmanager

Edit `deploy/docker/alertmanager/config.yml` to configure notification channels:

**Slack Integration**:
```yaml
receivers:
  - name: 'critical'
    slack_configs:
      - api_url: 'https://hooks.slack.com/services/YOUR/SLACK/WEBHOOK'
        channel: '#alerts-critical'
        title: '[CRITICAL] {{ .GroupLabels.alertname }}'
```

**Email Integration**:
```yaml
global:
  smtp_smarthost: 'smtp.gmail.com:587'
  smtp_from: 'alertmanager@example.com'
  smtp_auth_username: 'your-email@example.com'
  smtp_auth_password: 'your-password'

receivers:
  - name: 'critical'
    email_configs:
      - to: 'oncall@example.com'
```

**PagerDuty Integration**:
```yaml
receivers:
  - name: 'critical'
    pagerduty_configs:
      - routing_key: 'YOUR_PAGERDUTY_ROUTING_KEY'
        severity: 'critical'
```

## Recording Rules

Recording rules pre-compute frequently used queries to improve dashboard performance:

### Request Aggregations

```promql
# Request rate (5min average)
anode:requests:rate5m = rate(anode_requests_total[5m])

# Error rate
anode:requests:error_rate5m = sum(rate(anode_requests_total{status=~"[45].."}[5m]))

# Latency percentiles
anode:request_duration:p95 = histogram_quantile(0.95, rate(anode_request_duration_seconds_bucket[5m]))
```

### Storage Aggregations

```promql
# Total storage across cluster
anode:storage:total_bytes = sum(anode_storage_bytes_total)

# Storage per bucket
anode:storage:bytes_by_bucket = sum(anode_storage_bytes_total) by (bucket)
```

### Parquet Cache Aggregations

```promql
# Cache hit rate
anode:parquet_cache:hit_rate5m =
  rate(anode_parquet_cache_hits_total[5m]) /
  (rate(anode_parquet_cache_hits_total[5m]) + rate(anode_parquet_cache_misses_total[5m]))
```

## Best Practices

### Metric Retention

Configure retention based on your needs:

```yaml
# prometheus.yml
storage:
  tsdb:
    retention.time: 30d      # Keep metrics for 30 days
    retention.size: 50GB     # Maximum storage size
```

For long-term storage, consider using remote write to systems like:
- Cortex
- Thanos
- Victoria Metrics
- Mimir

### Query Optimization

1. **Use recording rules** for frequently used queries
2. **Limit time ranges** in dashboards to reduce load
3. **Use metric relabeling** to drop unnecessary labels
4. **Aggregate before querying** when possible

### Dashboard Performance

1. Set appropriate refresh intervals (10s-30s)
2. Use query caching in Grafana
3. Limit the number of series per panel
4. Use template variables to filter data

### Alert Tuning

1. **Avoid alert fatigue**: Set appropriate thresholds and durations
2. **Use inhibition rules**: Prevent duplicate alerts
3. **Group related alerts**: Reduce notification spam
4. **Test alert rules**: Use `promtool` to validate

```bash
# Validate alert rules
promtool check rules prometheus/alerts.yml
```

## Troubleshooting

### Metrics Not Appearing

1. **Check Prometheus targets**:
   - Navigate to http://localhost:9090/targets
   - Verify Anode endpoints are "UP"

2. **Check network connectivity**:
   ```bash
   curl http://anode-instance:9000/metrics
   ```

3. **Check Prometheus logs**:
   ```bash
   docker logs anode-prometheus
   ```

### High Cardinality Issues

If you experience high cardinality (too many unique label combinations):

1. **Identify high cardinality metrics**:
   ```promql
   topk(10, count by (__name__)({__name__=~".+"}))
   ```

2. **Use metric relabeling** to drop unnecessary labels:
   ```yaml
   metric_relabel_configs:
     - source_labels: [high_cardinality_label]
       action: labeldrop
   ```

### Slow Dashboard Loading

1. Reduce time range in queries
2. Increase `scrape_interval` if real-time data isn't critical
3. Use recording rules for complex queries
4. Enable Grafana query caching

### Missing Alerts

1. **Check Alertmanager**:
   - Navigate to http://localhost:9093
   - Verify alerts are being received

2. **Check routing configuration**:
   ```bash
   promtool check config alertmanager/config.yml
   ```

3. **Test notification channels**:
   ```bash
   # Send test alert
   curl -X POST http://localhost:9093/api/v1/alerts -d '[...]'
   ```

## Example Queries

### Useful PromQL Queries

**Top 5 slowest endpoints**:
```promql
topk(5, histogram_quantile(0.95,
  rate(anode_request_duration_seconds_bucket[5m]))) by (endpoint)
```

**Storage growth rate**:
```promql
deriv(anode_storage_bytes_total[1h])
```

**Predict disk full**:
```promql
predict_linear(anode_disk_usage_bytes[1h], 24*3600) >
  anode_disk_usage_bytes + anode_disk_available_bytes
```

**Cache efficiency**:
```promql
sum(rate(anode_parquet_cache_hits_total[5m])) /
  sum(rate(anode_parquet_cache_hits_total[5m]) +
      rate(anode_parquet_cache_misses_total[5m]))
```

**Cluster availability**:
```promql
avg(up{job="anode"}) * 100
```

## Additional Resources

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Alertmanager Documentation](https://prometheus.io/docs/alerting/latest/alertmanager/)
- [PromQL Basics](https://prometheus.io/docs/prometheus/latest/querying/basics/)

## Support

For issues or questions about monitoring:
- GitHub Issues: https://github.com/your-org/anode/issues
- Documentation: https://docs.anode.io
