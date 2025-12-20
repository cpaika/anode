# Anode Client SDK

A comprehensive Rust client SDK for interacting with Anode object storage.

## Features

- **Full S3-compatible API support** - All standard S3 operations
- **Async/await with Tokio** - Modern async Rust
- **AWS SigV4 request signing** - Secure authentication
- **Automatic retry with exponential backoff** - Resilient to transient failures
- **Streaming uploads and downloads** - Efficient memory usage
- **Multipart upload support** - For large files
- **Range requests** - Partial object downloads
- **Presigned URLs** - Temporary access links
- **Fluent builder APIs** - Expressive and ergonomic

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
anode-client = "0.1.0"
bytes = "1.8"
tokio = { version = "1.42", features = ["full"] }
```

## Quick Start

```rust
use anode_client::{ClientConfig, AnodeClient};
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client
    let config = ClientConfig::new("http://localhost:8080")
        .with_credentials("access_key", "secret_key");
    let client = AnodeClient::new(config)?;

    // Create a bucket
    client.create_bucket("my-bucket").await?;

    // Upload an object
    let data = Bytes::from("Hello, World!");
    client.put_object("my-bucket", "hello.txt", data).await?;

    // Download an object
    let result = client.get_object("my-bucket", "hello.txt").await?;
    println!("Downloaded: {:?}", result.body);

    Ok(())
}
```

## Configuration

### Basic Configuration

```rust
use anode_client::ClientConfig;

let config = ClientConfig::new("http://localhost:8080");
```

### With Credentials

```rust
let config = ClientConfig::new("http://localhost:8080")
    .with_credentials("access_key_id", "secret_access_key")
    .with_region("us-east-1");
```

### Advanced Configuration

```rust
use std::time::Duration;

let config = ClientConfig::new("http://localhost:8080")
    .with_credentials("access_key", "secret_key")
    .with_timeout(Duration::from_secs(60))
    .with_retries(5)
    .with_checksum_verification(true)
    .with_path_style(true);
```

### From Environment Variables

```rust
let config = ClientConfig::from_env()?;
```

Supported environment variables:
- `ANODE_ENDPOINT` - Endpoint URL (default: `http://localhost:8080`)
- `ANODE_ACCESS_KEY_ID` - Access key ID
- `ANODE_SECRET_ACCESS_KEY` - Secret access key
- `ANODE_REGION` - Region (default: `us-east-1`)
- `ANODE_TIMEOUT` - Request timeout in seconds (default: 300)

## Usage Examples

### Bucket Operations

```rust
// Create bucket
client.create_bucket("my-bucket").await?;

// Check if bucket exists
let exists = client.head_bucket("my-bucket").await?;

// List all buckets
let buckets = client.list_buckets().await?;
for bucket in buckets {
    println!("Bucket: {} created at {}", bucket.name, bucket.creation_date);
}

// Delete bucket
client.delete_bucket("my-bucket").await?;
```

### Object Operations

```rust
use bytes::Bytes;

// Put object
let data = Bytes::from("Hello, World!");
let result = client.put_object("my-bucket", "hello.txt", data).await?;
println!("ETag: {}", result.etag);

// Get object
let result = client.get_object("my-bucket", "hello.txt").await?;
println!("Data: {:?}", result.body);
println!("Size: {} bytes", result.metadata.size);

// Head object (get metadata only)
let metadata = client.head_object("my-bucket", "hello.txt").await?;
println!("Size: {}, ETag: {}", metadata.size, metadata.etag);

// Delete object
client.delete_object("my-bucket", "hello.txt").await?;
```

### List Objects

```rust
// List all objects in a bucket
let result = client.list_objects("my-bucket", None, None).await?;
for obj in result.objects {
    println!("{}: {} bytes", obj.key, obj.size);
}

// List with prefix
let result = client.list_objects("my-bucket", Some("docs/"), None).await?;

// List with pagination
let result = client.list_objects("my-bucket", None, Some(100)).await?;
if result.is_truncated {
    // Handle pagination with continuation token
}
```

### Copy Objects

```rust
// Copy within same bucket
client.copy_object(
    "my-bucket", "source.txt",
    "my-bucket", "destination.txt"
).await?;

// Copy to different bucket
client.copy_object(
    "source-bucket", "file.txt",
    "dest-bucket", "file.txt"
).await?;
```

### Range Requests

```rust
// Get bytes 0-99 of an object
let data = client.get_object_range("my-bucket", "large-file.bin", 0, 99).await?;
println!("Downloaded {} bytes", data.len());
```

### Streaming

```rust
use futures::StreamExt;

// Stream download
let mut stream = client.get_object_stream("my-bucket", "large-file.bin").await?;
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    // Process chunk
}

// Stream upload
use tokio::fs::File;
use tokio_util::io::ReaderStream;

let file = File::open("large-file.bin").await?;
let stream = ReaderStream::new(file);
let file_size = tokio::fs::metadata("large-file.bin").await?.len();

client.put_object_stream("my-bucket", "uploaded.bin", stream, file_size).await?;
```

### Multipart Upload

For files larger than 5 MB, use multipart uploads:

```rust
use bytes::Bytes;

let mut upload = client.multipart_upload("my-bucket", "large-file.bin");

// Initialize the upload
upload.init().await?;

// Upload parts (minimum 5 MB per part, except the last)
let part1 = Bytes::from(vec![0u8; 5 * 1024 * 1024]);
let part2 = Bytes::from(vec![1u8; 5 * 1024 * 1024]);

upload.upload_part(1, part1).await?;
upload.upload_part(2, part2).await?;

// Complete the upload
let result = upload.complete().await?;
println!("Upload complete: ETag {}", result.etag);
```

To abort a multipart upload:

```rust
let mut upload = client.multipart_upload("my-bucket", "file.bin");
upload.init().await?;
// ... upload some parts ...
upload.abort().await?;
```

### Presigned URLs

Generate temporary URLs for direct access:

```rust
use std::time::Duration;

// Presigned GET URL (valid for 1 hour)
let url = client.presign_get(
    "my-bucket",
    "private-file.txt",
    Duration::from_secs(3600)
)?;

// Presigned PUT URL
let url = client.presign_put(
    "my-bucket",
    "upload-here.txt",
    Duration::from_secs(3600)
)?;
```

### Fluent API

The client provides a fluent API for more expressive code:

```rust
use anode_client::bucket::BucketExt;
use anode_client::object::ObjectExt;

// Bucket operations
let bucket = client.bucket("my-bucket");
bucket.create().await?;

let exists = bucket.exists().await?;
println!("Bucket exists: {}", exists);

// Object operations
let object = client.object("my-bucket", "hello.txt");
object.put(Bytes::from("Hello!")).await?;

let data = object.get().await?;
println!("Data: {:?}", data.body);

object.delete().await?;
bucket.delete().await?;
```

## Error Handling

All operations return a `Result<T, ClientError>`:

```rust
use anode_client::ClientError;

match client.get_object("my-bucket", "file.txt").await {
    Ok(result) => println!("Got data: {} bytes", result.body.len()),
    Err(ClientError::ObjectNotFound { bucket, key }) => {
        println!("Object {}/{} not found", bucket, key);
    }
    Err(ClientError::BucketNotFound(bucket)) => {
        println!("Bucket {} not found", bucket);
    }
    Err(ClientError::Network(e)) => {
        println!("Network error: {}", e);
    }
    Err(e) => {
        println!("Other error: {}", e);
    }
}
```

## Testing

Integration tests can be run against a live Anode cluster:

```bash
# Set the endpoint
export ANODE_TEST_ENDPOINT=http://localhost:8080

# Optional: Set credentials
export ANODE_TEST_ACCESS_KEY=your_access_key
export ANODE_TEST_SECRET_KEY=your_secret_key

# Run tests
cargo test --test integration_tests
```

If `ANODE_TEST_ENDPOINT` is not set, the tests will be skipped.

## Examples

See the `tests/integration_tests.rs` file for comprehensive examples of all features.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
