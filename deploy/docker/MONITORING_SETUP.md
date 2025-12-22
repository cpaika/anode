# Anode Monitoring Setup

## Overview

The Anode monitoring stack consists of:
- **Prometheus** - Metrics collection and storage
- **Grafana** - Visualization and dashboards
- **Alertmanager** - Alert management and notifications
- **Node Exporter** - System-level metrics
- **cAdvisor** - Container metrics
- **Pushgateway** - Batch job metrics

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Anode Cluster                           │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
│  │ anode-1  │  │ anode-2  │  │ anode-3  │                  │
│  │ :8081    │  │ :8081    │  │ :8081    │                  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                  │
│       │             │             │                         │
│       └─────────────┴─────────────┘                         │
│                     │ /metrics                              │
└─────────────────────┼───────────────────────────────────────┘
                      │
                      ▼ scrape
            ┌──────────────────┐
            │   Prometheus     │
            │   :9090          │
            └────────┬─────────┘
                     │
         ┌───────────┼───────────┐
         │           │           │
         ▼           ▼           ▼
    ┌─────────┐ ┌─────────┐ ┌──────────────┐
    │ Grafana │ │ Alerts  │ │ Alertmanager │
    │  :3000  │ │         │ │    :9093     │
    └─────────┘ └─────────┘ └──────────────┘
```

## Quick Start

### 1. Start the Anode Cluster

```bash
cd deploy/docker
docker-compose up -d
```

Wait for all nodes to be healthy:
```bash
docker-compose ps
```

### 2. Start the Monitoring Stack

```bash
docker-compose -f docker-compose.monitoring.yml up -d
```

### 3. Verify Services

Check all monitoring services are running:
```bash
docker-compose -f docker-compose.monitoring.yml ps
```

Expected output:
```
NAME                     STATUS    PORTS
anode-prometheus         Up        0.0.0.0:9090->9090/tcp
anode-grafana            Up        0.0.0.0:3000->3000/tcp
anode-alertmanager       Up        0.0.0.0:9093->9093/tcp
anode-node-exporter      Up        0.0.0.0:9100->9100/tcp
anode-cadvisor           Up        0.0.0.0:8080->8080/tcp
anode-pushgateway        Up        0.0.0.0:9091->9091/tcp
```

### 4. Check Prometheus Targets

Open http://localhost:9090/targets and verify:
- All 3 anode nodes show as "UP"
- Each node is being scraped from `anode-X:8081/metrics`

### 5. Access Grafana

1. Open http://localhost:3000
2. Login:
   - Username: `admin`
   - Password: `admin`
3. Change the password when prompted
4. Navigate to Dashboards → Browse → "Anode Monitoring Dashboard"

## Configuration

### Metrics Endpoint

Anode exposes Prometheus metrics on the admin port (8081):
```
http://anode-1:8081/metrics
http://anode-2:8081/metrics
http://anode-3:8081/metrics
```

### Prometheus Configuration

The Prometheus scrape configuration is in `prometheus/prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'anode'
    scrape_interval: 10s
    scrape_timeout: 5s
    metrics_path: '/metrics'
    static_configs:
      - targets:
          - 'anode-1:8081'
          - 'anode-2:8081'
          - 'anode-3:8081'
```

### Network Setup

The monitoring stack connects to the anode cluster via the `deploy_anode-net` network. This allows Prometheus to reach the anode nodes by their container names.

## Available Dashboards

### Anode Monitoring Dashboard

Pre-configured dashboard showing:
- Cluster overview (nodes up, Raft status)
- Request metrics (rate, errors, latency)
- Raft consensus metrics
- Storage operations
- Parquet cache performance
- System resources

## Useful Queries

### Request Rate
```promql
sum(rate(anode_requests_total[5m])) by (node)
```

### Error Rate
```promql
sum(rate(anode_requests_total{status=~"5.."}[5m])) /
sum(rate(anode_requests_total[5m]))
```

### P95 Latency
```promql
histogram_quantile(0.95,
  rate(anode_request_duration_seconds_bucket[5m])
)
```

### Storage Usage
```promql
sum(anode_storage_bytes_total) by (bucket)
```

### Raft Leader
```promql
anode_raft_is_leader == 1
```

## Accessing Services

| Service | URL | Credentials |
|---------|-----|-------------|
| Grafana | http://localhost:3000 | admin/admin |
| Prometheus | http://localhost:9090 | None |
| Alertmanager | http://localhost:9093 | None |
| Node Exporter | http://localhost:9100 | None |
| cAdvisor | http://localhost:8080 | None |
| Pushgateway | http://localhost:9091 | None |

## Managing the Stack

### Start
```bash
docker-compose -f docker-compose.monitoring.yml up -d
```

### Stop
```bash
docker-compose -f docker-compose.monitoring.yml down
```

### View Logs
```bash
docker-compose -f docker-compose.monitoring.yml logs -f prometheus
docker-compose -f docker-compose.monitoring.yml logs -f grafana
```

### Restart a Service
```bash
docker-compose -f docker-compose.monitoring.yml restart prometheus
```

### Reload Prometheus Configuration
```bash
curl -X POST http://localhost:9090/-/reload
```

## Troubleshooting

### Prometheus Can't Scrape Anode Nodes

**Symptom**: Targets show as "DOWN" in http://localhost:9090/targets

**Solution**:
1. Verify anode nodes are running:
   ```bash
   docker-compose ps
   ```

2. Check if metrics endpoint is accessible:
   ```bash
   docker exec anode-prometheus curl -s http://anode-1:8081/metrics | head
   ```

3. Verify the network connection:
   ```bash
   docker network inspect deploy_anode-net
   ```

### Grafana Shows No Data

**Symptom**: Dashboard panels show "No data"

**Solution**:
1. Check Prometheus datasource in Grafana (Configuration → Data Sources)
2. Verify Prometheus is collecting metrics (http://localhost:9090/targets)
3. Test a simple query in Prometheus UI:
   ```promql
   up{job="anode"}
   ```

### Alerts Not Firing

**Symptom**: No alerts in Alertmanager

**Solution**:
1. Check alert rules in Prometheus (http://localhost:9090/alerts)
2. Verify Alertmanager is reachable:
   ```bash
   curl http://localhost:9093/-/healthy
   ```
3. Check Alertmanager configuration:
   ```bash
   docker exec anode-alertmanager cat /etc/alertmanager/config.yml
   ```

## Data Retention

### Prometheus
- Default: 30 days or 50GB
- Configure in `docker-compose.monitoring.yml`:
  ```yaml
  - '--storage.tsdb.retention.time=30d'
  - '--storage.tsdb.retention.size=50GB'
  ```

### Grafana
- Dashboards: Persisted in `grafana-data` volume
- Settings: Persisted in database

## Security Considerations

### Production Deployment

1. **Change Default Passwords**
   - Grafana admin password
   - Add authentication to Prometheus/Alertmanager

2. **Enable HTTPS**
   - Configure TLS for all web interfaces
   - Use reverse proxy (nginx, traefik)

3. **Network Isolation**
   - Use separate networks for monitoring and application
   - Restrict port access with firewall rules

4. **Resource Limits**
   - Set memory/CPU limits in docker-compose.yml
   - Monitor disk space for Prometheus data

## Next Steps

- Configure alert notifications (Slack, email, PagerDuty)
- Set up long-term storage with remote write
- Create custom dashboards for your use case
- Configure recording rules for complex queries
- Set up backup strategy for Grafana dashboards

## Additional Resources

- [Full Monitoring Guide](MONITORING.md)
- [Prometheus Configuration](prometheus/README.md)
- [Alert Rules](prometheus/alerts.yml)
- [Recording Rules](prometheus/recording-rules.yml)
