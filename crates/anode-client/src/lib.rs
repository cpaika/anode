//! # Anode Client SDK
//!
//! A comprehensive Rust client SDK for interacting with Anode object storage.
//!
//! ## Features
//!
//! - Full S3-compatible API support
//! - Async/await with Tokio
//! - AWS SigV4 request signing
//! - Automatic retry with exponential backoff
//! - Streaming uploads and downloads
//! - Multipart upload support
//! - Range requests
//! - Presigned URLs
//! - Fluent builder APIs
//!
//! ## Quick Start
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//! use bytes::Bytes;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a client
//!     let config = ClientConfig::new("http://localhost:8080")
//!         .with_credentials("access_key", "secret_key");
//!     let client = AnodeClient::new(config)?;
//!
//!     // Create a bucket
//!     client.create_bucket("my-bucket").await?;
//!
//!     // Upload an object
//!     let data = Bytes::from("Hello, World!");
//!     client.put_object("my-bucket", "hello.txt", data).await?;
//!
//!     // Download an object
//!     let result = client.get_object("my-bucket", "hello.txt").await?;
//!     println!("Downloaded: {:?}", result.body);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Configuration
//!
//! ### Basic Configuration
//!
//! ```no_run
//! use anode_client::ClientConfig;
//!
//! let config = ClientConfig::new("http://localhost:8080")
//!     .with_credentials("my_access_key", "my_secret_key");
//! ```
//!
//! ### Advanced Configuration
//!
//! ```no_run
//! use anode_client::ClientConfig;
//! use std::time::Duration;
//!
//! let config = ClientConfig::new("http://localhost:8080")
//!     .with_credentials("access_key", "secret_key")
//!     .with_region("us-west-2")
//!     .with_timeout(Duration::from_secs(60))
//!     .with_retries(5)
//!     .with_checksum_verification(true)
//!     .with_path_style(true);
//! ```
//!
//! ### Environment Variables
//!
//! ```no_run
//! use anode_client::ClientConfig;
//!
//! // Reads from environment variables:
//! // - ANODE_ENDPOINT
//! // - ANODE_ACCESS_KEY_ID
//! // - ANODE_SECRET_ACCESS_KEY
//! // - ANODE_REGION (optional)
//! // - ANODE_TIMEOUT (optional)
//! let config = ClientConfig::from_env()?;
//! # Ok::<(), anode_client::ClientError>(())
//! ```
//!
//! ## Fluent API
//!
//! The client provides a fluent API for more expressive code:
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//! use anode_client::bucket::BucketExt;
//! use anode_client::object::ObjectExt;
//! use bytes::Bytes;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ClientConfig::new("http://localhost:8080");
//!     let client = AnodeClient::new(config)?;
//!
//!     // Using fluent bucket API
//!     let bucket = client.bucket("my-bucket");
//!     bucket.create().await?;
//!
//!     let exists = bucket.exists().await?;
//!     println!("Bucket exists: {}", exists);
//!
//!     // Using fluent object API
//!     let object = client.object("my-bucket", "hello.txt");
//!     object.put(Bytes::from("Hello!")).await?;
//!
//!     let data = object.get().await?;
//!     println!("Data: {:?}", data.body);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Error Handling
//!
//! ### Pattern Matching on Errors
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient, ClientError};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = AnodeClient::new(ClientConfig::default())?;
//! match client.get_object("my-bucket", "file.txt").await {
//!     Ok(result) => {
//!         println!("Downloaded {} bytes", result.body.len());
//!     }
//!     Err(ClientError::ObjectNotFound { bucket, key }) => {
//!         println!("Object {}/{} not found", bucket, key);
//!     }
//!     Err(ClientError::BucketNotFound(bucket)) => {
//!         println!("Bucket {} does not exist", bucket);
//!     }
//!     Err(e) => {
//!         eprintln!("Error: {}", e);
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Using Error Helper Methods
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = AnodeClient::new(ClientConfig::default())?;
//! match client.get_object("my-bucket", "file.txt").await {
//!     Ok(result) => println!("Success: {} bytes", result.body.len()),
//!     Err(e) if e.is_not_found() => println!("Not found"),
//!     Err(e) if e.is_retriable() => println!("Temporary error, retrying..."),
//!     Err(e) => eprintln!("Permanent error: {}", e),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Bucket Operations
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = AnodeClient::new(ClientConfig::default())?;
//! // Create a bucket
//! client.create_bucket("my-bucket").await?;
//!
//! // Check if bucket exists
//! if client.head_bucket("my-bucket").await? {
//!     println!("Bucket exists");
//! }
//!
//! // List all buckets
//! let buckets = client.list_buckets().await?;
//! for bucket in buckets {
//!     println!("Bucket: {} (created: {})", bucket.name, bucket.creation_date);
//! }
//!
//! // Delete bucket (must be empty)
//! client.delete_bucket("my-bucket").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Object Operations
//!
//! ### Upload and Download
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//! use bytes::Bytes;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = AnodeClient::new(ClientConfig::default())?;
//! // Upload
//! let result = client.put_object("my-bucket", "file.txt", Bytes::from("data")).await?;
//! println!("Uploaded with ETag: {}", result.etag);
//!
//! // Download
//! let result = client.get_object("my-bucket", "file.txt").await?;
//! println!("Downloaded {} bytes", result.body.len());
//!
//! // Get metadata only
//! let metadata = client.head_object("my-bucket", "file.txt").await?;
//! println!("Size: {} bytes, ETag: {}", metadata.size, metadata.etag);
//!
//! // Delete
//! client.delete_object("my-bucket", "file.txt").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### List Objects
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = AnodeClient::new(ClientConfig::default())?;
//! // List all objects
//! let result = client.list_objects("my-bucket", None, None).await?;
//!
//! // List with prefix and limit
//! let result = client.list_objects("my-bucket", Some("photos/"), Some(100)).await?;
//! for obj in result.objects {
//!     println!("{}: {} bytes", obj.key, obj.size);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Server-Side Copy
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = AnodeClient::new(ClientConfig::default())?;
//! // Copy within same bucket
//! client.copy_object("my-bucket", "original.txt", "my-bucket", "copy.txt").await?;
//!
//! // Copy to different bucket
//! client.copy_object("source-bucket", "file.txt", "dest-bucket", "file.txt").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Streaming Operations
//!
//! ### Streaming Download
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//! use futures::StreamExt;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = AnodeClient::new(ClientConfig::default())?;
//! let mut stream = client.get_object_stream("my-bucket", "large_file.bin").await?;
//!
//! while let Some(chunk) = stream.next().await {
//!     let bytes = chunk?;
//!     println!("Received {} bytes", bytes.len());
//!     // Process chunk...
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Streaming Upload
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//! use tokio::fs::File;
//! use tokio_util::io::ReaderStream;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = AnodeClient::new(ClientConfig::default())?;
//! let file = File::open("large_file.bin").await?;
//! let metadata = file.metadata().await?;
//! let stream = ReaderStream::new(file);
//!
//! client.put_object_stream("my-bucket", "large_file.bin", stream, metadata.len()).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Range Requests
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = AnodeClient::new(ClientConfig::default())?;
//! // Download first 1024 bytes
//! let chunk = client.get_object_range("my-bucket", "file.bin", 0, 1023).await?;
//! println!("Downloaded {} bytes", chunk.len());
//!
//! // Download specific range (useful for Parquet files)
//! let footer = client.get_object_range("my-bucket", "data.parquet", 1000, 2000).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Multipart Upload
//!
//! For large files (> 5MB), use multipart uploads:
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//! use bytes::Bytes;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = ClientConfig::new("http://localhost:8080");
//!     let client = AnodeClient::new(config)?;
//!
//!     let mut upload = client.multipart_upload("my-bucket", "large-file.bin");
//!
//!     // Initialize
//!     upload.init().await?;
//!
//!     // Upload parts (minimum 5MB each, except last part)
//!     upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
//!     upload.upload_part(2, Bytes::from(vec![1u8; 5_242_880])).await?;
//!     upload.upload_part(3, Bytes::from(vec![2u8; 1_048_576])).await?;
//!
//!     // Complete the upload
//!     let result = upload.complete().await?;
//!     println!("Upload complete: {}", result.etag);
//!
//!     Ok(())
//! }
//! ```
//!
//! ### Aborting Multipart Upload
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//! use bytes::Bytes;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let client = AnodeClient::new(ClientConfig::default())?;
//! let mut upload = client.multipart_upload("my-bucket", "file.bin");
//! upload.init().await?;
//! upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
//!
//! // Something went wrong, abort the upload
//! upload.abort().await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Presigned URLs
//!
//! Generate temporary URLs for sharing access:
//!
//! ```no_run
//! use anode_client::{ClientConfig, AnodeClient};
//! use std::time::Duration;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = ClientConfig::new("http://localhost:8080")
//!     .with_credentials("access_key", "secret_key");
//! let client = AnodeClient::new(config)?;
//!
//! // Generate download URL (valid for 1 hour)
//! let download_url = client.presign_get("my-bucket", "file.txt", Duration::from_secs(3600))?;
//! println!("Download URL: {}", download_url);
//!
//! // Generate upload URL (valid for 30 minutes)
//! let upload_url = client.presign_put("my-bucket", "upload.txt", Duration::from_secs(1800))?;
//! println!("Upload URL: {}", upload_url);
//! # Ok(())
//! # }
//! ```

pub mod bucket;
pub mod client;
pub mod config;
pub mod error;
pub mod multipart;
pub mod object;

// Re-export main types
pub use client::{
    AnodeClient, BucketInfo, CopyObjectOutput, GetObjectOutput, ListObjectsOutput, ObjectMetadata,
    PutObjectOutput,
};
pub use config::ClientConfig;
pub use error::{ClientError, Result};
pub use multipart::{CompleteMultipartOutput, CompletedPart, MultipartUpload};

// Re-export extension traits for fluent API
pub use bucket::{BucketExt, BucketOperations, ListObjectsBuilder};
pub use object::{ObjectExt, ObjectOperations};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = ClientConfig::default();
        let client = AnodeClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_with_credentials() {
        let config =
            ClientConfig::new("http://localhost:8080").with_credentials("test_key", "test_secret");
        let client = AnodeClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_invalid_endpoint() {
        let config = ClientConfig::new("");
        let client = AnodeClient::new(config);
        assert!(client.is_err());
    }
}
