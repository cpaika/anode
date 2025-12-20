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
//! ## Fluent API
//!
//! The client also provides a fluent API for more expressive code:
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
//! ## Multipart Upload
//!
//! For large files, use multipart uploads:
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
//!     upload.init().await?;
//!     upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
//!     upload.upload_part(2, Bytes::from(vec![1u8; 5_242_880])).await?;
//!
//!     let result = upload.complete().await?;
//!     println!("Upload complete: {}", result.etag);
//!
//!     Ok(())
//! }
//! ```

pub mod bucket;
pub mod client;
pub mod config;
pub mod error;
pub mod multipart;
pub mod object;

// Re-export main types
pub use client::{
    AnodeClient, BucketInfo, CopyObjectOutput, GetObjectOutput, ListObjectsOutput,
    ObjectMetadata, PutObjectOutput,
};
pub use config::ClientConfig;
pub use error::{ClientError, Result};
pub use multipart::{CompletedPart, CompleteMultipartOutput, MultipartUpload};

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
        let config = ClientConfig::new("http://localhost:8080")
            .with_credentials("test_key", "test_secret");
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
