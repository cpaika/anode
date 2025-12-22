# Prometheus Configuration for Anode

This directory contains Prometheus configuration files for monitoring Anode clusters.

## Files

- **prometheus.yml**: Main Prometheus configuration
  - Defines scrape targets for Anode nodes
  - Configures alerting and recording rules
  - Sets retention policies

- **alerts.yml**: Alerting rules
  - Critical alerts (node down, quorum lost, split brain)
  - Warning alerts (high latency, replication lag)
  - Performance alerts (error rates, cache efficiency)

- **recording-rules.yml**: Pre-computed metric aggregations
  - Request rate and latency percentiles
  - Storage and cache metrics
  - Cluster health indicators
  - SLI (Service Level Indicator) metrics

## Quick Start

### Docker Compose

```bash
cd ../..  # Navigate to deploy/docker
docker-compose -f docker-compose.monitoring.yml up -d
```

Access Prometheus at: http://localhost:9090

### Standalone Prometheus

```bash
# Start Prometheus with this configuration
prometheus --config.file=prometheus.yml \
           --storage.tsdb.path=/data/prometheus \
           --storage.tsdb.retention.time=30d
```

## Configuration

### Adding Anode Nodes

Edit `prometheus.yml` and update the static_configs:

```yaml
scrape_configs:
  - job_name: 'anode'
    static_configs:
      - targets:
          - 'anode-1:9000'
          - 'anode-2:9000'
          - 'anode-3:9000'
```

### Using File-Based Service Discovery

For dynamic node discovery, uncomment the file_sd_configs section:

```yaml
- job_name: 'anode-dynamic'
  file_sd_configs:
    - files:
        - '/etc/prometheus/targets/anode-*.json'
      refresh_interval: 30s
```

Create target files in JSON format:

```json
[
  {
    "targets": ["anode-1:9000", "anode-2:9000"],
    "labels": {
      "cluster": "production",
      "datacenter": "us-east-1"
    }
  }
]
```

## Validating Configuration

Before deploying changes:

```bash
# Check Prometheus configuration
promtool check config prometheus.yml

# Check alert rules
promtool check rules alerts.yml

# Check recording rules
promtool check rules recording-rules.yml
```

## Testing Alert Rules

Test individual alert rules:

```bash
promtool test rules test_alerts.yml
```

Example test file:

```yaml
rule_files:
  - alerts.yml

evaluation_interval: 1m

tests:
  - interval: 1m
    input_series:
      - series: 'up{job="anode",instance="anode-1:9000"}'
        values: '0+0x10'
    alert_rule_test:
      - eval_time: 1m
        alertname: AnodeNodeDown
        exp_alerts:
          - exp_labels:
              severity: critical
              instance: anode-1:9000
```

## Troubleshooting

### No Data in Prometheus

1. Check if targets are reachable:
   ```bash
   curl http://anode-1:9000/metrics
   ```

2. Verify targets in Prometheus UI: http://localhost:9090/targets

3. Check Prometheus logs:
   ```bash
   docker logs anode-prometheus
   ```

### Alerts Not Firing

1. Verify alert evaluation in Prometheus UI: http://localhost:9090/alerts

2. Check Alertmanager connection:
   ```bash
   curl http://localhost:9093/api/v2/status
   ```

3. Review alert rule syntax and thresholds

### High Memory Usage

1. Reduce retention time in prometheus.yml
2. Increase scrape_interval to reduce data points
3. Use metric relabeling to drop unnecessary metrics

## Best Practices

1. **Use recording rules** for frequently used aggregations
2. **Set appropriate retention** based on storage capacity
3. **Monitor Prometheus itself** with node_exporter
4. **Regular backups** of Prometheus data directory
5. **Use remote write** for long-term storage

## Remote Storage

For long-term metrics storage, configure remote write:

```yaml
remote_write:
  - url: 'http://cortex:9009/api/prom/push'
    queue_config:
      capacity: 10000
      max_shards: 50
```

Compatible backends:
- Cortex
- Thanos
- Victoria Metrics
- Mimir
- AWS Timestream
- Google Cloud Monitoring

## Additional Resources

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Recording Rules Best Practices](https://prometheus.io/docs/practices/rules/)
- [Alerting Best Practices](https://prometheus.io/docs/practices/alerting/)
