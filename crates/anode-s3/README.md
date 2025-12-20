# anode-s3

S3-compatible API layer for the anode distributed object storage system.

## Overview

This crate provides a complete S3-compatible HTTP API implementation using Axum, allowing anode to serve as a drop-in replacement for Amazon S3 in many scenarios.

## Features

### Implemented S3 Operations

**Bucket Operations:**
- `CreateBucket` (PUT /bucket)
- `DeleteBucket` (DELETE /bucket)
- `HeadBucket` (HEAD /bucket)
- `ListBuckets` (GET /)

**Object Operations:**
- `PutObject` (PUT /bucket/key)
- `GetObject` (GET /bucket/key)
- `HeadObject` (HEAD /bucket/key)
- `DeleteObject` (DELETE /bucket/key)
- `CopyObject` (PUT /bucket/key with x-amz-copy-source header)
- `ListObjectsV2` (GET /bucket?list-type=2)
- `DeleteObjects` (POST /bucket?delete)

**Multipart Upload Operations:**
- `CreateMultipartUpload` (POST /bucket/key?uploads)
- `UploadPart` (PUT /bucket/key?partNumber=N&uploadId=ID)
- `CompleteMultipartUpload` (POST /bucket/key?uploadId=ID)
- `AbortMultipartUpload` (DELETE /bucket/key?uploadId=ID)

### Authentication

- **AWS Signature Version 4** authentication support
- Configurable access keys
- Optional authentication (can be disabled for development)

### Architecture

The crate is organized into several modules:

- **error.rs**: S3-compatible error types with automatic XML serialization
- **auth.rs**: AWS Signature V4 authentication implementation
- **xml.rs**: XML request/response serialization for S3 API
- **handlers.rs**: Request handlers for all S3 operations
- **router.rs**: Axum router configuration and middleware

## Usage

### Basic Server Setup

```rust
use anode_s3::{AuthConfig, S3Router};
use anode_storage::StorageEngine;
use std::sync::Arc;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    // Create storage engine
    let storage = Arc::new(
        StorageEngine::new(PathBuf::from("./data")).unwrap()
    );

    // Create S3 router (no auth for simplicity)
    let app = S3Router::new(storage, None).into_router();

    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

### With Authentication

```rust
use std::collections::HashMap;

let mut access_keys = HashMap::new();
access_keys.insert(
    "AKIAIOSFODNN7EXAMPLE".to_string(),
    "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
);

let auth = AuthConfig::enabled(access_keys);
let app = S3Router::new(storage, Some(auth)).into_router();
```

### Using with AWS CLI

Once the server is running, you can use the standard AWS CLI:

```bash
# List buckets
aws --endpoint-url http://localhost:8080 s3 ls

# Create a bucket
aws --endpoint-url http://localhost:8080 s3 mb s3://my-bucket

# Upload a file
aws --endpoint-url http://localhost:8080 s3 cp file.txt s3://my-bucket/

# List objects
aws --endpoint-url http://localhost:8080 s3 ls s3://my-bucket

# Download a file
aws --endpoint-url http://localhost:8080 s3 cp s3://my-bucket/file.txt ./downloaded.txt

# Delete a file
aws --endpoint-url http://localhost:8080 s3 rm s3://my-bucket/file.txt

# Delete a bucket
aws --endpoint-url http://localhost:8080 s3 rb s3://my-bucket
```

### Using with AWS SDKs

The S3 API is compatible with AWS SDKs in various languages. Here's an example using the Rust AWS SDK:

```rust
use aws_sdk_s3::Client;
use aws_config::BehaviorVersion;

#[tokio::main]
async fn main() {
    let config = aws_config::defaults(BehaviorVersion::latest())
        .endpoint_url("http://localhost:8080")
        .load()
        .await;

    let client = Client::new(&config);

    // Create bucket
    client.create_bucket()
        .bucket("my-bucket")
        .send()
        .await
        .unwrap();

    // Put object
    client.put_object()
        .bucket("my-bucket")
        .key("test.txt")
        .body("Hello, World!".into())
        .send()
        .await
        .unwrap();
}
```

## Path-Style vs Virtual-Hosted Style URLs

The server supports both URL styles:

- **Path-style**: `http://localhost:8080/bucket/key`
- **Virtual-hosted style**: `http://bucket.localhost:8080/key` (requires DNS configuration)

Most operations work with path-style URLs by default.

## Implementation Details

### Error Handling

All S3 errors are returned as XML responses following the S3 API specification:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>NoSuchBucket</Code>
    <Message>The specified bucket does not exist</Message>
</Error>
```

### State Management

The router maintains in-memory state for:
- Bucket metadata (name, creation date)
- Multipart upload sessions
- Storage engine reference

### Multipart Uploads

Multipart upload parts are stored in memory until the upload is completed or aborted. For production use, consider implementing persistent storage for multipart upload state.

### Bucket Name Validation

Bucket names are validated according to S3 rules:
- Between 3 and 63 characters
- Lowercase letters, numbers, hyphens, and periods only
- Cannot start or end with a hyphen
- Cannot use IP address format

## Limitations

Current implementation has some limitations:

1. **No versioning support** - Object versioning is not implemented
2. **No ACLs** - Access control lists are not supported
3. **Limited metadata** - Some S3 metadata features are not implemented
4. **No range requests** - Partial content requests are not supported yet
5. **No server-side encryption** - SSE is not implemented
6. **No lifecycle policies** - Automatic object expiration is not supported
7. **Storage engine incomplete** - The underlying storage engine has placeholder implementations

## Testing

Run the test suite:

```bash
cargo test -p anode-s3
```

Run the example server:

```bash
cargo run -p anode-s3 --example simple_server
```

## Future Enhancements

Planned features:
- Range request support (partial content downloads)
- Conditional requests (If-Match, If-None-Match, etc.)
- Pre-signed URLs
- CORS support
- Server-side encryption (SSE-S3, SSE-KMS)
- Object versioning
- Lifecycle policies
- Proper ISO 8601 timestamp formatting
- Pagination for large object listings
- Virtual-hosted style URL support with DNS
- Access control lists (ACLs)
- Bucket policies

## License

See the main anode project for license information.
