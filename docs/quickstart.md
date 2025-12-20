# Quickstart Guide

Get Anode up and running in minutes.

## Prerequisites

Before you begin, ensure you have the following installed:

- **Rust 1.75+** - [Install Rust](https://rustup.rs/)
- **Docker 24.0+** - [Install Docker](https://docs.docker.com/get-docker/)
- **Docker Compose 2.0+** - Usually included with Docker Desktop
- **kubectl 1.28+** (optional, for Kubernetes deployment)
- **Helm 3.12+** (optional, for Kubernetes deployment)

## Quick Start (Docker Compose)

The fastest way to try Anode is using Docker Compose:

### 1. Clone the Repository

```bash
git clone https://github.com/christopherpaika/anode
cd anode
```

### 2. Start a 3-Node Cluster

```bash
cd deploy/docker
docker-compose up -d
```

This starts three Anode nodes:
- **anode-1** (bootstrap node): Ports 8080 (S3), 8081 (admin), 9090 (gRPC)
- **anode-2**: Ports 8082 (S3), 8083 (admin), 9091 (gRPC)
- **anode-3**: Ports 8084 (S3), 8085 (admin), 9092 (gRPC)

### 3. Verify Cluster Health

```bash
# Check if all nodes are healthy
docker-compose ps

# Check cluster status
curl http://localhost:8081/health

# View logs
docker-compose logs -f anode-1
```

Expected output:
```json
{
  "status": "healthy",
  "node_id": 1,
  "role": "leader",
  "cluster_size": 3
}
```

## Using the S3 API

### Configure AWS CLI

Install the AWS CLI if you haven't already:

```bash
# macOS
brew install awscli

# Linux
pip install awscli

# Windows
# Download from https://aws.amazon.com/cli/
```

Configure credentials (use any values for local testing):

```bash
aws configure
# AWS Access Key ID: minioadmin
# AWS Secret Access Key: minioadmin
# Default region name: us-east-1
# Default output format: json
```

### Create a Bucket

```bash
aws --endpoint-url http://localhost:8080 s3 mb s3://mybucket
```

Expected output:
```
make_bucket: mybucket
```

### Upload a File

```bash
# Create a test file
echo "Hello, Anode!" > test.txt

# Upload to bucket
aws --endpoint-url http://localhost:8080 s3 cp test.txt s3://mybucket/
```

Expected output:
```
upload: ./test.txt to s3://mybucket/test.txt
```

### List Objects

```bash
aws --endpoint-url http://localhost:8080 s3 ls s3://mybucket/
```

Expected output:
```
2023-12-19 10:30:45         14 test.txt
```

### Download a File

```bash
aws --endpoint-url http://localhost:8080 s3 cp s3://mybucket/test.txt downloaded.txt
cat downloaded.txt
```

Expected output:
```
download: s3://mybucket/test.txt to ./downloaded.txt
Hello, Anode!
```

### Delete a File

```bash
aws --endpoint-url http://localhost:8080 s3 rm s3://mybucket/test.txt
```

### Delete a Bucket

```bash
# Bucket must be empty
aws --endpoint-url http://localhost:8080 s3 rb s3://mybucket
```

## Advanced S3 Operations

### Multipart Upload

For large files (>100MB), use multipart upload:

```bash
# Create a large test file (100MB)
dd if=/dev/urandom of=largefile.bin bs=1M count=100

# Upload using multipart
aws --endpoint-url http://localhost:8080 s3 cp largefile.bin s3://mybucket/ \
    --metadata "description=Large test file"
```

### Presigned URLs

Generate temporary download URLs:

```bash
aws --endpoint-url http://localhost:8080 s3 presign \
    s3://mybucket/test.txt \
    --expires-in 3600
```

Use the presigned URL:

```bash
curl "<presigned-url>" -o downloaded.txt
```

### Object Metadata

Set custom metadata:

```bash
aws --endpoint-url http://localhost:8080 s3 cp test.txt s3://mybucket/test.txt \
    --metadata "author=John,version=1"

# Retrieve metadata
aws --endpoint-url http://localhost:8080 s3api head-object \
    --bucket mybucket \
    --key test.txt
```

### Range Requests

Download partial objects:

```bash
# Download first 100 bytes
aws --endpoint-url http://localhost:8080 s3api get-object \
    --bucket mybucket \
    --key largefile.bin \
    --range "bytes=0-99" \
    partial.bin
```

## Using the Rust Client

For programmatic access from Rust applications:

### Add Dependency

```toml
[dependencies]
anode-client = "0.1"
tokio = { version = "1.40", features = ["full"] }
```

### Example Code

```rust
use anode_client::{Client, ClientConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client
    let config = ClientConfig {
        endpoint: "http://localhost:8080".to_string(),
        access_key: "minioadmin".to_string(),
        secret_key: "minioadmin".to_string(),
        region: "us-east-1".to_string(),
    };

    let client = Client::new(config).await?;

    // Create bucket
    client.create_bucket("my-rust-bucket").await?;

    // Upload object
    let data = b"Hello from Rust!";
    client.put_object(
        "my-rust-bucket",
        "hello.txt",
        data.to_vec(),
        None,
    ).await?;

    // Download object
    let downloaded = client.get_object("my-rust-bucket", "hello.txt").await?;
    println!("Downloaded: {}", String::from_utf8_lossy(&downloaded));

    // List objects
    let objects = client.list_objects("my-rust-bucket", None).await?;
    println!("Objects: {:?}", objects);

    Ok(())
}
```

## Building from Source

If you want to build Anode yourself:

### 1. Install Dependencies

```bash
# Install protobuf compiler
# macOS
brew install protobuf

# Ubuntu/Debian
sudo apt-get install protobuf-compiler libprotobuf-dev

# Fedora
sudo dnf install protobuf-compiler protobuf-devel
```

### 2. Clone and Build

```bash
git clone https://github.com/christopherpaika/anode
cd anode

# Build release binary
cargo build --release

# Binary will be at: target/release/anode
```

### 3. Run Single Node

```bash
# Create data directory
mkdir -p /tmp/anode/data

# Run node
./target/release/anode \
    --node-id 1 \
    --s3-addr 0.0.0.0:8080 \
    --grpc-addr 0.0.0.0:9090 \
    --admin-addr 0.0.0.0:8081 \
    --data-dir /tmp/anode/data \
    --bootstrap
```

### 4. Run 3-Node Cluster Manually

Terminal 1 (Node 1 - Bootstrap):
```bash
./target/release/anode \
    --node-id 1 \
    --s3-addr 127.0.0.1:8080 \
    --grpc-addr 127.0.0.1:9090 \
    --admin-addr 127.0.0.1:8081 \
    --data-dir /tmp/anode/node1 \
    --bootstrap
```

Terminal 2 (Node 2):
```bash
./target/release/anode \
    --node-id 2 \
    --s3-addr 127.0.0.1:8082 \
    --grpc-addr 127.0.0.1:9091 \
    --admin-addr 127.0.0.1:8083 \
    --data-dir /tmp/anode/node2 \
    --join 127.0.0.1:9090
```

Terminal 3 (Node 3):
```bash
./target/release/anode \
    --node-id 3 \
    --s3-addr 127.0.0.1:8084 \
    --grpc-addr 127.0.0.1:9092 \
    --admin-addr 127.0.0.1:8085 \
    --data-dir /tmp/anode/node3 \
    --join 127.0.0.1:9090
```

## Deploying to Kubernetes

For production deployments, use Kubernetes with Helm:

### 1. Install Helm Chart

```bash
# Add Anode Helm repository
helm repo add anode https://christopherpaika.github.io/anode/helm
helm repo update

# Install with default values (3-node cluster)
helm install my-anode anode/anode

# Or install with custom values
helm install my-anode anode/anode \
    --set replicaCount=5 \
    --set persistence.size=100Gi \
    --set resources.requests.memory=4Gi
```

### 2. Access the Cluster

```bash
# Get service endpoint
kubectl get service my-anode

# Port-forward for local access
kubectl port-forward service/my-anode 8080:8080

# Now use AWS CLI with endpoint http://localhost:8080
```

### 3. Monitor the Cluster

```bash
# Check pod status
kubectl get pods -l app=anode

# View logs
kubectl logs -l app=anode -f

# Check metrics
kubectl port-forward service/my-anode-admin 8081:8081
curl http://localhost:8081/metrics
```

See [Kubernetes Deployment Guide](./kubernetes.md) for more details.

## Local Development with K3d

For local Kubernetes testing, use K3d (lightweight Kubernetes in Docker):

### 1. Install K3d

```bash
# macOS
brew install k3d

# Linux
curl -s https://raw.githubusercontent.com/k3d-io/k3d/main/install.sh | bash
```

### 2. Create Local Cluster

```bash
cd deploy/k3d

# Create k3d cluster with registry
./scripts/create-cluster.sh

# This creates:
# - 3-node Kubernetes cluster
# - Local Docker registry
# - LoadBalancer support
```

### 3. Deploy Anode

```bash
# Build and push image to local registry
./scripts/build-and-push.sh

# Deploy using Helm
helm install anode ../helm/anode \
    --set image.repository=localhost:5000/anode \
    --set image.tag=latest

# Wait for pods to be ready
kubectl wait --for=condition=ready pod -l app=anode --timeout=300s
```

### 4. Test the Cluster

```bash
# Get LoadBalancer IP
export ANODE_ENDPOINT=$(kubectl get svc anode -o jsonpath='{.status.loadBalancer.ingress[0].ip}')

# Test S3 API
aws --endpoint-url http://${ANODE_ENDPOINT}:8080 s3 mb s3://k3d-test
aws --endpoint-url http://${ANODE_ENDPOINT}:8080 s3 cp test.txt s3://k3d-test/
```

### 5. Cleanup

```bash
# Delete Helm release
helm uninstall anode

# Delete k3d cluster
k3d cluster delete anode-dev
```

## Using with Python (boto3)

For Python applications:

### Install boto3

```bash
pip install boto3
```

### Example Code

```python
import boto3
from botocore.client import Config

# Create S3 client
s3 = boto3.client(
    's3',
    endpoint_url='http://localhost:8080',
    aws_access_key_id='minioadmin',
    aws_secret_access_key='minioadmin',
    config=Config(signature_version='s3v4'),
    region_name='us-east-1'
)

# Create bucket
s3.create_bucket(Bucket='my-python-bucket')

# Upload file
with open('test.txt', 'rb') as f:
    s3.put_object(Bucket='my-python-bucket', Key='test.txt', Body=f)

# Download file
response = s3.get_object(Bucket='my-python-bucket', Key='test.txt')
data = response['Body'].read()
print(data.decode('utf-8'))

# List objects
response = s3.list_objects_v2(Bucket='my-python-bucket')
for obj in response.get('Contents', []):
    print(f"{obj['Key']} - {obj['Size']} bytes")
```

## Performance Testing

Test cluster performance with sample workloads:

### Small Objects (1KB)

```bash
# Upload 1000 small objects
for i in {1..1000}; do
    echo "Object $i" | aws --endpoint-url http://localhost:8080 \
        s3 cp - s3://mybucket/small-$i.txt
done
```

### Large Objects (100MB)

```bash
# Create 100MB file
dd if=/dev/urandom of=100mb.bin bs=1M count=100

# Upload
time aws --endpoint-url http://localhost:8080 s3 cp 100mb.bin s3://mybucket/

# Download
time aws --endpoint-url http://localhost:8080 s3 cp s3://mybucket/100mb.bin downloaded.bin
```

### Concurrent Operations

```bash
# Install parallel
brew install parallel  # macOS
# or
sudo apt-get install parallel  # Ubuntu

# Upload 100 files in parallel
seq 1 100 | parallel -j 10 \
    "echo 'Test {}' | aws --endpoint-url http://localhost:8080 s3 cp - s3://mybucket/parallel-{}.txt"
```

## Monitoring

### Prometheus Metrics

Anode exposes Prometheus metrics on the admin port:

```bash
# View metrics
curl http://localhost:8081/metrics
```

Key metrics:
- `anode_requests_total` - Total requests
- `anode_request_duration_seconds` - Request latency
- `anode_storage_bytes_total` - Storage usage
- `anode_objects_total` - Object count
- `anode_raft_leader` - Raft leader status

### Grafana Dashboard

Import the provided Grafana dashboard:

```bash
# Start Grafana (via Docker Compose)
cd deploy/docker
docker-compose -f docker-compose.dev.yml up -d grafana

# Access Grafana at http://localhost:3000
# Default credentials: admin/admin

# Import dashboard from: deploy/grafana/dashboard.json
```

## Troubleshooting

### Cluster Won't Start

```bash
# Check logs
docker-compose logs anode-1

# Common issues:
# 1. Port already in use - change ports in docker-compose.yml
# 2. Data directory permissions - check volume mounts
# 3. Network issues - check Docker network
```

### Node Can't Join Cluster

```bash
# Verify bootstrap node is running
curl http://localhost:8081/health

# Check gRPC port is accessible
telnet localhost 9090

# View join logs
docker-compose logs anode-2 | grep -i "join"
```

### S3 Operations Failing

```bash
# Test connectivity
curl -v http://localhost:8080/

# Check authentication
aws --endpoint-url http://localhost:8080 --debug s3 ls 2>&1 | grep -i auth

# Verify bucket exists
aws --endpoint-url http://localhost:8080 s3 ls
```

### Performance Issues

```bash
# Check resource usage
docker stats

# View slow query logs
docker-compose logs anode-1 | grep -i "slow"

# Check Raft state
curl http://localhost:8081/admin/raft
```

## Next Steps

- Read the [Architecture Documentation](./architecture.md) to understand how Anode works
- See [Configuration Reference](./configuration.md) for all configuration options
- Check [Operations Guide](./operations.md) for production best practices
- Review [S3 API Reference](./s3-api.md) for supported operations
- Learn about [Performance Tuning](./performance.md)

## Getting Help

- **Documentation**: https://github.com/christopherpaika/anode/docs
- **Issues**: https://github.com/christopherpaika/anode/issues
- **Discussions**: https://github.com/christopherpaika/anode/discussions

## Clean Up

When you're done testing:

```bash
# Stop cluster
cd deploy/docker
docker-compose down

# Remove volumes (deletes all data)
docker-compose down -v

# Or for k3d
k3d cluster delete anode-dev
```
