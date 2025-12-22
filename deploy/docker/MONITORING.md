# Anode Monitoring Quick Start

This guide will help you get the Anode monitoring stack up and running in minutes.

## Prerequisites

- Docker and Docker Compose installed
- Anode cluster running and accessible
- Ports available: 3000 (Grafana), 9090 (Prometheus), 9093 (Alertmanager)

## Quick Start

### 1. Start the Monitoring Stack

```bash
cd deploy/docker
docker-compose -f docker-compose.monitoring.yml up -d
```

This starts:
- Prometheus (metrics collection)
- Grafana (visualization)
- Alertmanager (alert management)
- Node Exporter (system metrics)
- cAdvisor (container metrics)

### 2. Verify Services

```bash
docker-compose -f docker-compose.monitoring.yml ps
```

All services should show "Up" status.

### 3. Configure Anode Targets

The Prometheus configuration is pre-configured to scrape metrics from anode nodes on port 8081 (admin port).

If you need to customize the targets, edit `prometheus/prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'anode'
    static_configs:
      - targets:
          - 'anode-1:8081'    # Anode admin port (where /metrics is exposed)
          - 'anode-2:8081'
          - 'anode-3:8081'
```

Reload Prometheus configuration:

```bash
curl -X POST http://localhost:9090/-/reload
```

### 4. Access Grafana

1. Open http://localhost:3000
2. Login with:
   - Username: `admin`
   - Password: `admin`
3. Change password when prompted
4. Navigate to Dashboards → Browse
5. Open "Anode Monitoring Dashboard"

## What You'll See

### Grafana Dashboard

The pre-configured dashboard shows:

1. **Cluster Overview** (top row)
   - Nodes Up count
   - Raft Leader status
   - Total Objects
   - Total Storage

2. **Request Metrics**
   - Request rate per node
   - Error rate
   - Latency percentiles (p50, p95, p99)

3. **Raft Consensus**
   - Current term
   - Commit index vs applied index
   - Proposal rate and failures

4. **Storage Metrics**
   - Operation rates by type
   - Operation latency
   - Objects and storage growth

5. **Parquet Cache**
   - Cache hit rate
   - Cache size
   - Query performance

6. **Cluster Health**
   - Replication lag
   - Peer connectivity
   - Network RTT

7. **System Resources**
   - Memory usage
   - CPU usage
   - Disk usage

### Prometheus

Access Prometheus at http://localhost:9090 to:
- Run custom PromQL queries
- View active alerts
- Check scrape targets status
- Explore metrics

Example queries:
```promql
# Total request rate
sum(rate(anode_requests_total[5m]))

# Cache hit rate
rate(anode_parquet_cache_hits_total[5m]) /
  (rate(anode_parquet_cache_hits_total[5m]) +
   rate(anode_parquet_cache_misses_total[5m]))

# Storage per bucket
sum(anode_storage_bytes_total) by (bucket)
```

### Alertmanager

Access Alertmanager at http://localhost:9093 to:
- View active alerts
- Silence alerts
- Test notification channels

## Configuration

### Customize Alert Thresholds

Edit `prometheus/alerts.yml`:

```yaml
- alert: AnodeHighRequestLatency
  expr: histogram_quantile(0.95, rate(anode_request_duration_seconds_bucket[5m])) > 1.0
  for: 5m
  labels:
    severity: warning
```

Adjust the threshold (1.0 seconds) or duration (5m) as needed.

### Configure Notifications

Edit `alertmanager/config.yml` to add Slack, email, or PagerDuty:

**Slack Example**:
```yaml
receivers:
  - name: 'critical'
    slack_configs:
      - api_url: 'https://hooks.slack.com/services/YOUR/WEBHOOK'
        channel: '#alerts'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'
```

**Email Example**:
```yaml
global:
  smtp_smarthost: 'smtp.gmail.com:587'
  smtp_from: 'alerts@example.com'
  smtp_auth_username: 'your-email@gmail.com'
  smtp_auth_password: 'your-app-password'

receivers:
  - name: 'critical'
    email_configs:
      - to: 'oncall@example.com'
```

Reload Alertmanager:
```bash
curl -X POST http://localhost:9093/-/reload
```

### Adjust Retention

Edit `docker-compose.monitoring.yml`:

```yaml
services:
  prometheus:
    command:
      - '--storage.tsdb.retention.time=90d'  # Keep data for 90 days
      - '--storage.tsdb.retention.size=100GB'  # Or limit by size
```

Restart Prometheus:
```bash
docker-compose -f docker-compose.monitoring.yml restart prometheus
```

## Common Tasks

### Add a New Anode Node

1. Edit `prometheus/prometheus.yml`:
   ```yaml
   - targets:
       - 'anode-4:8081'  # Add new node (port 8081 is the admin port)
   ```

2. Reload Prometheus:
   ```bash
   curl -X POST http://localhost:9090/-/reload
   ```

3. Verify in Prometheus UI: http://localhost:9090/targets

### View Logs

```bash
# Prometheus logs
docker logs anode-prometheus

# Grafana logs
docker logs anode-grafana

# Alertmanager logs
docker logs anode-alertmanager
```

### Backup Grafana Dashboards

```bash
# Export dashboard JSON
docker exec anode-grafana grafana-cli admin export-dashboard "Anode Monitoring Dashboard" > backup.json
```

### Backup Prometheus Data

```bash
# Create snapshot
docker exec anode-prometheus promtool tsdb create-blocks-from snapshot /prometheus /prometheus-snapshot

# Copy snapshot
docker cp anode-prometheus:/prometheus-snapshot ./prometheus-backup
```

### Stop Monitoring Stack

```bash
docker-compose -f docker-compose.monitoring.yml down
```

To remove data volumes as well:
```bash
docker-compose -f docker-compose.monitoring.yml down -v
```

## Troubleshooting

### No Metrics in Grafana

1. Check Prometheus targets: http://localhost:9090/targets
2. Verify Anode nodes are accessible:
   ```bash
   curl http://anode-1:8081/metrics
   ```
3. Check Grafana datasource configuration

### Alerts Not Firing

1. Check alert rules in Prometheus: http://localhost:9090/alerts
2. Verify Alertmanager config:
   ```bash
   promtool check config alertmanager/config.yml
   ```
3. Check Alertmanager logs:
   ```bash
   docker logs anode-alertmanager
   ```

### High Memory Usage

1. Reduce retention time
2. Increase scrape interval
3. Use recording rules for complex queries
4. Drop unnecessary metrics with relabeling

### Dashboard Not Loading

1. Check browser console for errors
2. Verify Prometheus is reachable from Grafana
3. Clear Grafana cache:
   ```bash
   docker exec anode-grafana rm -rf /var/lib/grafana/cache/*
   docker-compose -f docker-compose.monitoring.yml restart grafana
   ```

## Production Checklist

Before deploying to production:

- [ ] Change default Grafana password
- [ ] Configure authentication (LDAP, OAuth, etc.)
- [ ] Set up TLS/SSL for all services
- [ ] Configure backup strategy for Prometheus data
- [ ] Set up remote write for long-term storage
- [ ] Configure proper alert routing and notifications
- [ ] Set resource limits in docker-compose.yml
- [ ] Document runbooks for common alerts
- [ ] Test disaster recovery procedures
- [ ] Enable audit logging
- [ ] Configure rate limiting
- [ ] Set up monitoring for the monitoring stack itself

## Next Steps

- Read the full [Monitoring Guide](../../docs/monitoring.md)
- Explore [Prometheus queries](prometheus/README.md)
- Customize [alert rules](prometheus/alerts.yml)
- Configure [recording rules](prometheus/recording-rules.yml)
- Set up [long-term storage](../../docs/monitoring.md#remote-storage)

## Support

For issues or questions:
- GitHub Issues: https://github.com/your-org/anode/issues
- Documentation: ../../docs/monitoring.md
- Prometheus: https://prometheus.io/docs/
- Grafana: https://grafana.com/docs/
