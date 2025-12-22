//! Anode Storage Engine
//!
//! This crate provides the core storage engine for the Anode distributed object storage system.
//! It implements content-addressed storage with deduplication, integrity verification, and repair capabilities.
//!
//! # Overview
//!
//! The storage engine is built on several key components:
//!
//! - **Storage Engine** ([`StorageEngine`]): Main interface for object storage operations
//! - **Chunk Management** ([`ChunkManager`]): Handles data chunking and deduplication
//! - **Blob Store** ([`BlobStore`]): Low-level storage of chunk data on disk
//! - **Metadata Store** ([`MetadataStore`]): Tracks object manifests, buckets, and reference counts
//! - **Integrity Verification** ([`IntegrityChecker`]): Scans and verifies data integrity
//! - **Repair** ([`RepairManager`]): Repairs corrupted or missing data from replicas
//! - **Scrubber** ([`Scrubber`]): Background integrity checking and auto-repair
//!
//! # Example
//!
//! ```rust,no_run
//! use anode_storage::{StorageEngine, StorageConfig};
//! use std::path::PathBuf;
//! use std::collections::BTreeMap;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create storage engine
//! let config = StorageConfig::new(PathBuf::from("/data"));
//! let engine = StorageEngine::open(config).await?;
//!
//! // Create a bucket
//! let bucket_config = anode_storage::BucketConfig::new("my-bucket");
//! engine.create_bucket("my-bucket", bucket_config).await?;
//!
//! // Store an object
//! let data = b"Hello, world!";
//! let metadata = BTreeMap::new();
//! let manifest = engine.put_object("my-bucket", "my-key", data, metadata).await?;
//!
//! // Retrieve an object
//! if let Some((manifest, data)) = engine.get_object("my-bucket", "my-key").await? {
//!     println!("Retrieved {} bytes", data.len());
//! }
//! # Ok(())
//! # }
//! ```

pub mod blobs;
pub mod chunks;
pub mod engine;
pub mod error;
pub mod integrity;
pub mod metadata;
pub mod repair;
pub mod scrubber;

pub use blobs::{BlobStore, BlobStoreStats};
pub use chunks::{
    Chunk, ChunkId as StorageChunkId, ChunkManager, GcStats, CHUNK_SIZE, MAX_CHUNK_SIZE,
};
pub use engine::{ListResult, StorageConfig, StorageEngine, StorageStats};
pub use error::{Result as StorageResult, StorageError};
pub use metadata::{BucketConfig, BucketUsage, MetadataStore};

// Export new integrity verification and repair modules
pub use integrity::{
    CorruptedChunk, IntegrityChecker, IntegrityReport as DetailedIntegrityReport, MissingChunk,
    RefCountError,
};
pub use repair::{
    ClusterClient, MockClusterClient, RepairError, RepairManager,
    RepairResult as DetailedRepairResult,
};
pub use scrubber::{ScrubProgress, ScrubResult, ScrubStatus, Scrubber, ScrubberConfig};

// Keep old types for backwards compatibility
pub use chunks::IntegrityReport as ChunkIntegrityReport;
pub use engine::RepairResult as EngineRepairResult;

// Re-export commonly used protocol types
pub use anode_protocol::{ChunkId, ObjectKey, ObjectManifest};

// Re-export the Storage trait from the old module for compatibility
use bytes::Bytes;
use std::collections::BTreeMap;

/// Storage trait for object operations.
///
/// This trait provides a simplified interface for object storage operations,
/// abstracting over the underlying storage engine implementation.
#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    /// Store an object in the storage system.
    ///
    /// # Arguments
    ///
    /// * `key` - The object key including bucket and key path
    /// * `data` - The object data as bytes
    /// * `metadata` - Optional custom metadata as key-value pairs
    ///
    /// # Returns
    ///
    /// Returns the object manifest containing chunk IDs and checksums on success.
    async fn put_object(
        &self,
        key: ObjectKey,
        data: Bytes,
        metadata: Option<BTreeMap<String, String>>,
    ) -> StorageResult<ObjectManifest>;

    /// Retrieve an object from the storage system.
    ///
    /// # Arguments
    ///
    /// * `key` - The object key to retrieve
    ///
    /// # Returns
    ///
    /// Returns the object data as bytes on success. Returns error if object not found.
    async fn get_object(&self, key: &ObjectKey) -> StorageResult<Bytes>;

    /// Delete an object from the storage system.
    ///
    /// # Arguments
    ///
    /// * `key` - The object key to delete
    async fn delete_object(&self, key: &ObjectKey) -> StorageResult<()>;

    /// List objects in a bucket with optional prefix filter.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The bucket name
    /// * `prefix` - Optional prefix filter for object keys
    ///
    /// # Returns
    ///
    /// Returns a list of object manifests matching the filter criteria.
    async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
    ) -> StorageResult<Vec<ObjectManifest>>;
}

// Implement the Storage trait for StorageEngine
#[async_trait::async_trait]
impl Storage for StorageEngine {
    async fn put_object(
        &self,
        key: ObjectKey,
        data: Bytes,
        metadata: Option<BTreeMap<String, String>>,
    ) -> StorageResult<ObjectManifest> {
        let metadata = metadata.unwrap_or_default();
        self.put_object(&key.bucket, &key.key, &data, metadata)
            .await
    }

    async fn get_object(&self, key: &ObjectKey) -> StorageResult<Bytes> {
        match self.get_object(&key.bucket, &key.key).await? {
            Some((_, data)) => Ok(Bytes::from(data)),
            None => Err(StorageError::ObjectNotFound {
                bucket: key.bucket.clone(),
                key: key.key.clone(),
            }),
        }
    }

    async fn delete_object(&self, key: &ObjectKey) -> StorageResult<()> {
        self.delete_object(&key.bucket, &key.key).await?;
        Ok(())
    }

    async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
    ) -> StorageResult<Vec<ObjectManifest>> {
        let prefix = prefix.unwrap_or("");
        let result = self.list_objects(bucket, prefix, 1000, None).await?;
        Ok(result.objects)
    }
}
