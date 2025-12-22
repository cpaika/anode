//! S3-compatible API implementation for Anode.
//!
//! This crate provides a complete S3-compatible API layer built on top of the Anode storage engine.
//! It includes support for:
//!
//! - Basic bucket operations (create, delete, list)
//! - Object operations (put, get, delete, copy)
//! - Multipart uploads
//! - Batch delete operations
//! - AWS Signature Version 4 authentication
//! - S3-compatible XML responses
//!
//! # Example
//!
//! ```no_run
//! use anode_s3::{S3Router, AuthConfig};
//! use anode_storage::{StorageEngine, StorageConfig};
//! use std::sync::Arc;
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create storage engine
//! let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
//! let storage = Arc::new(StorageEngine::open(config).await?);
//!
//! // Create S3 router with authentication disabled
//! let router = S3Router::new(storage, None).into_router();
//!
//! // Serve the API
//! // axum::Server::bind(&addr).serve(router.into_make_service()).await?;
//! # Ok(())
//! # }
//! ```

pub mod auth;
pub mod batch;
pub mod copy;
pub mod error;
pub mod handlers;
pub mod middleware;
pub mod multipart;
pub mod policy;
pub mod router;
pub mod versioning;
pub mod xml;

pub use auth::{AuthConfig, AwsSignatureV4};
pub use error::{S3Error, S3Result};
pub use router::S3Router;

// Re-export commonly used types
pub use batch::{BatchHandler, DeleteError, DeleteObjectsResult, DeletedObject, ObjectIdentifier};
pub use copy::{CopyHandler, CopyOptions, CopyResult, MetadataDirective};
pub use handlers::{AppState, BucketMetadata};
pub use multipart::{
    CompletePart, ListPartsResult, ListUploadsResult, MultipartConfig, MultipartManager,
    MultipartUpload, MultipartUploadInfo, UploadedPart,
};
pub use policy::{
    ActionList, BucketPolicy, Effect, EvaluationContext, PolicyDecision, PolicyStatement,
    Principal, PrincipalValue, ResourceList,
};
pub use versioning::VersionId;
