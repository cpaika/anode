# Anode Documentation

Welcome to the Anode documentation. This guide will help you navigate all available documentation.

## Getting Started

**New to Anode?** Start here:

1. **[Quickstart Guide](quickstart.md)** - Get Anode running in 5 minutes
   - Docker Compose setup
   - Basic S3 operations
   - First cluster deployment

2. **[Architecture Overview](architecture.md)** - Understand how Anode works
   - System components
   - Data flow
   - Consensus and replication

## User Guides

### Setup and Configuration

- **[Configuration Reference](configuration.md)** - Complete configuration options
  - All configuration parameters
  - Environment variables
  - Command-line arguments
  - Production examples

### Deployment

- **[Standalone Deployment](deployment-standalone.md)** - Single-node deployments
  - Building from source
  - Binary installation
  - systemd service setup
  - Basic configuration

- **[Docker Deployment](deployment-docker.md)** - Docker and Docker Compose deployments
  - Single-node Docker setup
  - Multi-node clusters with Docker Compose
  - Monitoring stack (Prometheus, Grafana)
  - Volume management and backups
  - Troubleshooting Docker issues

### Operations

- **[Operations Guide](operations.md)** - Running Anode in production
  - Cluster setup
  - Monitoring and metrics
  - Backup and restore
  - Scaling and upgrades
  - Troubleshooting
  - Disaster recovery

- **[Performance Tuning](performance.md)** - Optimize for your workload
  - Benchmarking
  - Storage optimization
  - Network tuning
  - Workload-specific configuration
  - Hardware recommendations

### API and Integration

- **[S3 API Reference](s3-api.md)** - S3-compatible operations
  - Supported operations
  - Authentication (AWS SigV4)
  - Multipart uploads
  - Presigned URLs
  - Error responses
  - Examples in multiple languages

## Development

### Testing

- **[Testing Guide](testing.md)** - Running and writing tests
  - Unit tests
  - Integration tests
  - Correctness tests
  - Chaos testing
  - Benchmarking
  - Test harness usage

### Contributing

- **[Development Guide](development.md)** - Contributing to Anode
  - Development setup
  - Project structure
  - Code guidelines
  - Release process
  - Architecture decisions

## Security

- **[Security Guide](security.md)** - Security best practices
  - Authentication and authorization
  - Network security
  - Data encryption
  - Audit logging
  - Security hardening
  - Compliance

## Documentation by Audience

### For Operators

If you're deploying and running Anode:
1. Start with [Quickstart Guide](quickstart.md)
2. Choose deployment method:
   - [Docker Deployment](deployment-docker.md) for containerized setups
   - [Standalone Deployment](deployment-standalone.md) for bare metal/VM
3. Read [Configuration Reference](configuration.md)
4. Study [Operations Guide](operations.md)
5. Review [Security Guide](security.md)
6. Understand [Performance Tuning](performance.md)

### For Developers

If you're building applications that use Anode:
1. Check [Quickstart Guide](quickstart.md) to set up test cluster
2. Read [S3 API Reference](s3-api.md) for API details
3. Review [Configuration Reference](configuration.md) for client setup
4. See [Architecture Overview](architecture.md) to understand system behavior

### For Contributors

If you're contributing to Anode:
1. Follow [Development Guide](development.md) for setup
2. Read [Architecture Overview](architecture.md) for design
3. Check [Testing Guide](testing.md) for test requirements
4. Review existing code and documentation

## Quick Reference

### Common Tasks

**Start a cluster:**
```bash
cd deploy/docker
docker-compose up -d
```

**Upload a file:**
```bash
aws --endpoint-url http://localhost:8080 s3 cp file.txt s3://bucket/
```

**Check cluster health:**
```bash
curl http://localhost:8081/health
```

**View metrics:**
```bash
curl http://localhost:8081/metrics
```

**Backup data:**
```bash
tar -czf backup.tar.gz /var/lib/anode/data/
```

### Configuration Examples

**Development:**
```toml
[cluster]
replication_factor = 1  # No replication
[storage]
sync_writes = false     # Performance over durability
[logging]
level = "debug"         # Verbose logging
```

**Production:**
```toml
[cluster]
replication_factor = 3  # Triple replication
[storage]
sync_writes = true      # Durability
[logging]
level = "info"
json_format = true      # Structured logs
```

### Architecture Diagrams

```
Write Path:
Client → Node (any) → Leader → Replicas → Success

Read Path:
Client → Node (any) → Local Storage → Response

Replication:
Leader → [Split into chunks] → [Raft log] → Followers → [Write chunks]
```

## Documentation Structure

```
docs/
├── README.md              # This file
├── quickstart.md          # Getting started (5 min)
├── architecture.md        # System design (30 min)
├── configuration.md       # Config reference (reference)
├── deployment-docker.md   # Docker deployment (45 min)
├── deployment-standalone.md # Standalone deployment (40 min)
├── operations.md          # Production ops (60 min)
├── s3-api.md             # API reference (reference)
├── admin-api.md          # Admin API reference (reference)
├── performance.md        # Performance tuning (30 min)
├── security.md           # Security guide (30 min)
├── testing.md            # Testing guide (20 min)
├── monitoring.md         # Monitoring guide (25 min)
└── development.md        # Developer guide (40 min)
```

## External Resources

### Community

- **GitHub**: https://github.com/christopherpaika/anode
- **Issues**: https://github.com/christopherpaika/anode/issues
- **Discussions**: https://github.com/christopherpaika/anode/discussions

### Related Documentation

- [Rust Documentation](https://doc.rust-lang.org/)
- [OpenRaft Guide](https://github.com/datafuselabs/openraft)
- [AWS S3 API](https://docs.aws.amazon.com/s3/)
- [Parquet Format](https://parquet.apache.org/docs/)

### Tools

- [AWS CLI](https://aws.amazon.com/cli/)
- [boto3 (Python)](https://boto3.amazonaws.com/v1/documentation/api/latest/index.html)
- [s3cmd](https://s3tools.org/s3cmd)
- [MinIO Client](https://min.io/docs/minio/linux/reference/minio-mc.html)

## Learning Path

### Beginner

1. **Day 1**: [Quickstart Guide](quickstart.md) - Set up and basic operations
2. **Day 2**: [S3 API Reference](s3-api.md) - Learn S3 operations
3. **Day 3**: [Configuration Reference](configuration.md) - Understand configuration

### Intermediate

1. **Week 1**: [Architecture Overview](architecture.md) - Deep dive into design
2. **Week 2**: [Operations Guide](operations.md) - Production deployment
3. **Week 3**: [Performance Tuning](performance.md) - Optimization

### Advanced

1. **Month 1**: [Security Guide](security.md) - Comprehensive security
2. **Month 2**: [Testing Guide](testing.md) + [Development Guide](development.md) - Contributing
3. **Month 3**: Source code - Implementation details

## Getting Help

### Documentation Not Clear?

- Open an issue: https://github.com/christopherpaika/anode/issues/new
- Start a discussion: https://github.com/christopherpaika/anode/discussions/new

### Found a Bug?

- Check existing issues
- Create a bug report with steps to reproduce

### Want to Contribute?

- Read [Development Guide](development.md)
- Look for `good-first-issue` label
- Join the discussion

## Feedback

We're constantly improving our documentation. If you have suggestions:

1. Open an issue with `documentation` label
2. Submit a PR with improvements
3. Share your experience in discussions

## License

Documentation is licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
Code examples in documentation are licensed under MIT or Apache 2.0.

---

**Last Updated**: December 2023
**Version**: 0.1.0
