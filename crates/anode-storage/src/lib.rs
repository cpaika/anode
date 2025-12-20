pub mod blobs;
pub mod chunks;
pub mod engine;
pub mod error;
pub mod integrity;
pub mod metadata;
pub mod repair;
pub mod scrubber;

pub use blobs::{BlobStore, BlobStoreStats};
pub use chunks::{Chunk, ChunkManager, GcStats, CHUNK_SIZE, MAX_CHUNK_SIZE};
pub use engine::{ListResult, StorageConfig, StorageEngine, StorageStats};
pub use error::{Result as StorageResult, StorageError};
pub use metadata::{BucketConfig, MetadataStore};

// Export new integrity verification and repair modules
pub use integrity::{
    CorruptedChunk, IntegrityChecker, IntegrityReport as DetailedIntegrityReport, MissingChunk,
    RefCountError,
};
pub use repair::{ClusterClient, MockClusterClient, RepairError, RepairManager, RepairResult as DetailedRepairResult};
pub use scrubber::{ScrubProgress, ScrubResult, ScrubStatus, Scrubber, ScrubberConfig};

// Keep old types for backwards compatibility
pub use chunks::IntegrityReport as ChunkIntegrityReport;
pub use engine::RepairResult as EngineRepairResult;

// Re-export commonly used protocol types
pub use anode_protocol::{ChunkId, ObjectKey, ObjectManifest};

// Re-export the Storage trait from the old module for compatibility
use std::collections::HashMap;
use bytes::Bytes;

#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    async fn put_object(
        &self,
        key: ObjectKey,
        data: Bytes,
        metadata: Option<HashMap<String, String>>,
    ) -> StorageResult<ObjectManifest>;

    async fn get_object(&self, key: &ObjectKey) -> StorageResult<Bytes>;

    async fn delete_object(&self, key: &ObjectKey) -> StorageResult<()>;

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
        metadata: Option<HashMap<String, String>>,
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
