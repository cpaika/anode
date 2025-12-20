# Changelog

All notable changes to the Anode Helm chart will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-12-19

### Added
- Initial release of Anode Helm chart
- StatefulSet deployment with configurable replica count
- Headless and load-balancing services
- ConfigMap for anode.toml configuration
- Secret management for S3 credentials
- Persistent volume support with configurable storage class
- Pod disruption budget for high availability
- Network policies for security
- Ingress support with TLS
- Prometheus ServiceMonitor integration
- Prometheus alerting rules
- Comprehensive health checks (liveness, readiness, startup)
- Init container for data directory setup
- Pod anti-affinity for node distribution
- Resource requests and limits
- Configurable logging and metrics
- S3 API, Admin API, and gRPC service ports
- Helm tests for connection verification
- Values schema validation
- Comprehensive documentation and examples
- Support for custom environment variables
- Support for extra volumes and volume mounts
- Support for init containers and sidecars
- Lifecycle hooks with graceful shutdown
- Service account with optional RBAC
- Topology spread constraints support
- Priority class support

### Features
- S3-compatible object storage
- Raft-based distributed consensus
- Parquet-aware metadata caching
- Configurable placement groups and replication factor
- Multi-node cluster support (3, 5, 7+ nodes)
- Automatic cluster bootstrap
- Rolling updates with configurable strategy
- Production-ready monitoring and alerting
- TLS/SSL support with cert-manager integration

### Security
- Non-root container execution
- Read-only root filesystem
- Dropped all capabilities
- Pod security context enforcement
- Secret-based credential management
- Network policy support

[0.1.0]: https://github.com/anode/anode/releases/tag/chart-v0.1.0
