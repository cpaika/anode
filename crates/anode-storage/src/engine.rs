use crate::blobs::BlobStore;
use crate::chunks::{Chunk, ChunkManager, GcStats, IntegrityReport};
use crate::error::{Result, StorageError};
use crate::metadata::{BucketConfig, MetadataStore};
use anode_protocol::{ChunkId, ObjectKey, ObjectManifest};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
    pub metadata_dir: PathBuf,
    pub chunk_size: usize,
    pub sync_writes: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data/blobs"),
            metadata_dir: PathBuf::from("./data/metadata"),
            chunk_size: crate::chunks::CHUNK_SIZE,
            sync_writes: true,
        }
    }
}

impl StorageConfig {
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            data_dir: base_dir.join("blobs"),
            metadata_dir: base_dir.join("metadata"),
            chunk_size: crate::chunks::CHUNK_SIZE,
            sync_writes: true,
        }
    }
}

pub struct StorageEngine {
    metadata: Arc<MetadataStore>,
    blobs: Arc<BlobStore>,
    chunks: Arc<ChunkManager>,
    config: StorageConfig,
}

impl StorageEngine {
    pub async fn open(config: StorageConfig) -> Result<Self> {
        info!("Opening storage engine with config: {:?}", config);

        // Create directories
        std::fs::create_dir_all(&config.data_dir)?;
        std::fs::create_dir_all(&config.metadata_dir)?;

        // Initialize blob store
        let blobs = Arc::new(BlobStore::new(config.data_dir.clone())?);

        // Initialize metadata store
        let metadata_path = config.metadata_dir.join("metadata.db");
        let metadata = Arc::new(MetadataStore::open(&metadata_path)?);

        // Initialize chunk manager
        let chunks = Arc::new(ChunkManager::new(blobs.clone(), metadata.clone()));

        Ok(Self {
            metadata,
            blobs,
            chunks,
            config,
        })
    }

    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        data: &[u8],
        metadata: HashMap<String, String>,
    ) -> Result<ObjectManifest> {
        info!("Putting object: {}/{} ({} bytes)", bucket, key, data.len());

        // Verify bucket exists
        if self.metadata.get_bucket(bucket)?.is_none() {
            return Err(StorageError::BucketNotFound(bucket.to_string()));
        }

        // Split data into chunks
        let chunks = ChunkManager::split_into_chunks(data);
        let chunk_count = chunks.len();

        // Store chunks
        let chunk_ids = self.chunks.store_chunks(chunks).await?;

        // Create manifest
        let object_key = ObjectKey::new(bucket, key);
        let mut manifest = ObjectManifest::new(object_key, data.len() as u64, chunk_ids);
        manifest.metadata = metadata;

        // Compute ETag (MD5 hash for compatibility with S3)
        let mut hasher = Sha256::new();
        hasher.update(data);
        manifest.etag = hex::encode(hasher.finalize());

        // Store manifest
        self.metadata.put_object(&manifest)?;

        info!(
            "Stored object {}/{} with {} chunks",
            bucket, key, chunk_count
        );

        Ok(manifest)
    }

    pub async fn get_object(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<Option<(ObjectManifest, Vec<u8>)>> {
        debug!("Getting object: {}/{}", bucket, key);

        // Get manifest
        let manifest = match self.metadata.get_object(bucket, key)? {
            Some(m) => m,
            None => return Ok(None),
        };

        // Retrieve chunks
        let data = self.chunks.retrieve_chunks(&manifest.chunks).await?;

        // Verify size matches
        if data.len() as u64 != manifest.size {
            return Err(StorageError::Corrupted(format!(
                "Object size mismatch: expected {}, got {}",
                manifest.size,
                data.len()
            )));
        }

        Ok(Some((manifest, data)))
    }

    pub async fn delete_object(&self, bucket: &str, key: &str) -> Result<bool> {
        info!("Deleting object: {}/{}", bucket, key);

        // Get manifest
        let manifest = match self.metadata.get_object(bucket, key)? {
            Some(m) => m,
            None => return Ok(false),
        };

        // Delete chunks (decrement ref counts)
        self.chunks.delete_chunks(&manifest.chunks).await?;

        // Delete manifest
        self.metadata.delete_object(bucket, key)?;

        Ok(true)
    }

    pub async fn head_object(&self, bucket: &str, key: &str) -> Result<Option<ObjectManifest>> {
        debug!("Head object: {}/{}", bucket, key);
        self.metadata.get_object(bucket, key)
    }

    pub async fn list_objects(
        &self,
        bucket: &str,
        prefix: &str,
        limit: usize,
        continuation: Option<&str>,
    ) -> Result<ListResult> {
        debug!(
            "Listing objects: bucket={}, prefix={}, limit={}",
            bucket, prefix, limit
        );

        // For now, ignore continuation token (simple implementation)
        let objects = self.metadata.list_objects(bucket, prefix, limit)?;

        Ok(ListResult {
            objects,
            continuation_token: None,
            is_truncated: false,
        })
    }

    pub async fn create_bucket(&self, name: &str, config: BucketConfig) -> Result<()> {
        info!("Creating bucket: {}", name);
        self.metadata.create_bucket(&config)
    }

    pub async fn delete_bucket(&self, name: &str) -> Result<()> {
        info!("Deleting bucket: {}", name);

        let deleted = self.metadata.delete_bucket(name)?;

        if !deleted {
            return Err(StorageError::BucketNotFound(name.to_string()));
        }

        Ok(())
    }

    pub async fn list_buckets(&self) -> Result<Vec<BucketConfig>> {
        debug!("Listing buckets");
        self.metadata.list_buckets()
    }

    pub async fn get_stats(&self) -> Result<StorageStats> {
        debug!("Getting storage stats");

        let buckets = self.metadata.list_buckets()?;
        let blob_stats = self.blobs.get_stats()?;

        let mut total_objects = 0u64;
        let mut total_object_size = 0u64;

        for bucket in &buckets {
            let objects = self.metadata.list_objects(&bucket.name, "", usize::MAX)?;
            total_objects += objects.len() as u64;

            for obj in objects {
                total_object_size += obj.size;
            }
        }

        Ok(StorageStats {
            bucket_count: buckets.len() as u64,
            object_count: total_objects,
            chunk_count: blob_stats.chunk_count,
            total_size: blob_stats.total_size,
            object_size: total_object_size,
        })
    }

    pub async fn verify_integrity(&self) -> Result<IntegrityReport> {
        info!("Verifying storage integrity");
        self.chunks.verify_integrity().await
    }

    pub async fn repair(&self, report: &IntegrityReport) -> Result<RepairResult> {
        info!("Repairing storage issues");

        let mut result = RepairResult {
            chunks_repaired: 0,
            objects_deleted: 0,
            errors: Vec::new(),
        };

        // For now, we can only delete objects with missing or corrupted chunks
        // More sophisticated repair would require replication or backups

        // Find all objects that reference corrupted or missing chunks
        let all_buckets = self.metadata.list_buckets()?;

        for bucket in all_buckets {
            let objects = self.metadata.list_objects(&bucket.name, "", usize::MAX)?;

            for obj in objects {
                let mut has_issues = false;

                for chunk_id in &obj.chunks {
                    if report.corrupted_chunks.contains(chunk_id)
                        || report.missing_chunks.contains(chunk_id)
                    {
                        has_issues = true;
                        break;
                    }
                }

                if has_issues {
                    warn!(
                        "Object {}/{} has corrupted or missing chunks, deleting",
                        obj.object_key.bucket, obj.object_key.key
                    );

                    match self
                        .delete_object(&obj.object_key.bucket, &obj.object_key.key)
                        .await
                    {
                        Ok(_) => {
                            result.objects_deleted += 1;
                        }
                        Err(e) => {
                            result.errors.push(format!(
                                "Failed to delete object {}/{}: {}",
                                obj.object_key.bucket, obj.object_key.key, e
                            ));
                        }
                    }
                }
            }
        }

        // Run garbage collection to clean up orphaned chunks
        let gc_stats = self.chunks.garbage_collect().await?;
        result.chunks_repaired = gc_stats.chunks_deleted;

        Ok(result)
    }

    pub async fn garbage_collect(&self) -> Result<GcStats> {
        info!("Running garbage collection");
        self.chunks.garbage_collect().await
    }

    pub fn metadata(&self) -> &MetadataStore {
        &self.metadata
    }

    pub fn blobs(&self) -> &BlobStore {
        &self.blobs
    }

    pub fn chunks(&self) -> &ChunkManager {
        &self.chunks
    }
}

#[derive(Debug, Clone)]
pub struct ListResult {
    pub objects: Vec<ObjectManifest>,
    pub continuation_token: Option<String>,
    pub is_truncated: bool,
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub bucket_count: u64,
    pub object_count: u64,
    pub chunk_count: u64,
    pub total_size: u64,
    pub object_size: u64,
}

#[derive(Debug, Clone)]
pub struct RepairResult {
    pub chunks_repaired: u64,
    pub objects_deleted: u64,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_engine() -> (StorageEngine, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig::new(temp_dir.path().to_path_buf());
        let engine = StorageEngine::open(config).await.unwrap();
        (engine, temp_dir)
    }

    #[tokio::test]
    async fn test_bucket_lifecycle() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("test-bucket");
        engine.create_bucket("test-bucket", config).await.unwrap();

        let buckets = engine.list_buckets().await.unwrap();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].name, "test-bucket");

        engine.delete_bucket("test-bucket").await.unwrap();

        let buckets = engine.list_buckets().await.unwrap();
        assert_eq!(buckets.len(), 0);
    }

    #[tokio::test]
    async fn test_object_lifecycle() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket
        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        // Put object
        let data = b"Hello, world! This is test data.";
        let metadata = HashMap::new();
        let manifest = engine
            .put_object("bucket1", "key1", data, metadata)
            .await
            .unwrap();

        assert_eq!(manifest.object_key.bucket, "bucket1");
        assert_eq!(manifest.object_key.key, "key1");
        assert_eq!(manifest.size, data.len() as u64);

        // Get object
        let result = engine.get_object("bucket1", "key1").await.unwrap();
        assert!(result.is_some());

        let (retrieved_manifest, retrieved_data) = result.unwrap();
        assert_eq!(retrieved_data, data);
        assert_eq!(retrieved_manifest.etag, manifest.etag);

        // Head object
        let head = engine.head_object("bucket1", "key1").await.unwrap();
        assert!(head.is_some());

        // Delete object
        let deleted = engine.delete_object("bucket1", "key1").await.unwrap();
        assert!(deleted);

        // Verify deleted
        let result = engine.get_object("bucket1", "key1").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_list_objects() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        // Put multiple objects
        for i in 0..5 {
            let key = format!("prefix/key{}", i);
            let data = format!("Data {}", i).into_bytes();
            engine
                .put_object("bucket1", &key, &data, HashMap::new())
                .await
                .unwrap();
        }

        // List all
        let result = engine
            .list_objects("bucket1", "prefix/", 10, None)
            .await
            .unwrap();
        assert_eq!(result.objects.len(), 5);

        // List with limit
        let result = engine
            .list_objects("bucket1", "prefix/", 3, None)
            .await
            .unwrap();
        assert_eq!(result.objects.len(), 3);
    }

    #[tokio::test]
    async fn test_large_object() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        // Create large data (larger than chunk size)
        let large_data = vec![42u8; crate::chunks::CHUNK_SIZE * 2 + 1000];

        let manifest = engine
            .put_object("bucket1", "large-object", &large_data, HashMap::new())
            .await
            .unwrap();

        assert!(manifest.chunks.len() > 1);

        let result = engine
            .get_object("bucket1", "large-object")
            .await
            .unwrap();
        assert!(result.is_some());

        let (_, retrieved_data) = result.unwrap();
        assert_eq!(retrieved_data, large_data);
    }

    #[tokio::test]
    async fn test_object_metadata() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), "test".to_string());
        metadata.insert("version".to_string(), "1.0".to_string());

        let data = b"Test data";
        let manifest = engine
            .put_object("bucket1", "key1", data, metadata.clone())
            .await
            .unwrap();

        assert_eq!(manifest.metadata.get("author"), Some(&"test".to_string()));
        assert_eq!(manifest.metadata.get("version"), Some(&"1.0".to_string()));
    }

    #[tokio::test]
    async fn test_get_stats() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        let data1 = b"First object";
        let data2 = b"Second object with more data";

        engine
            .put_object("bucket1", "key1", data1, HashMap::new())
            .await
            .unwrap();
        engine
            .put_object("bucket1", "key2", data2, HashMap::new())
            .await
            .unwrap();

        let stats = engine.get_stats().await.unwrap();

        assert_eq!(stats.bucket_count, 1);
        assert_eq!(stats.object_count, 2);
        assert_eq!(stats.chunk_count, 2);
        assert_eq!(
            stats.object_size,
            (data1.len() + data2.len()) as u64
        );
    }

    #[tokio::test]
    async fn test_chunk_deduplication() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        let data = b"Identical data for deduplication test";

        // Store same data twice
        engine
            .put_object("bucket1", "key1", data, HashMap::new())
            .await
            .unwrap();
        engine
            .put_object("bucket1", "key2", data, HashMap::new())
            .await
            .unwrap();

        let stats = engine.get_stats().await.unwrap();

        // Should have 2 objects but only 1 chunk (deduplicated)
        assert_eq!(stats.object_count, 2);
        assert_eq!(stats.chunk_count, 1);
    }

    #[tokio::test]
    async fn test_verify_integrity() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        let data = b"Test data for integrity check";
        engine
            .put_object("bucket1", "key1", data, HashMap::new())
            .await
            .unwrap();

        let report = engine.verify_integrity().await.unwrap();

        assert!(report.is_healthy());
        assert_eq!(report.verified_chunks, 1);
        assert!(report.corrupted_chunks.is_empty());
        assert!(report.missing_chunks.is_empty());
    }

    #[tokio::test]
    async fn test_garbage_collection() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        // Put and then delete an object
        let data = b"Data to be deleted";
        engine
            .put_object("bucket1", "key1", data, HashMap::new())
            .await
            .unwrap();

        engine.delete_object("bucket1", "key1").await.unwrap();

        // Chunks should already be deleted due to ref counting
        let stats = engine.garbage_collect().await.unwrap();

        // No orphaned chunks should be found
        assert_eq!(stats.chunks_deleted, 0);
    }

    #[tokio::test]
    async fn test_nonexistent_bucket() {
        let (engine, _temp) = create_test_engine().await;

        let result = engine
            .put_object("nonexistent", "key1", b"data", HashMap::new())
            .await;

        assert!(matches!(result, Err(StorageError::BucketNotFound(_))));
    }

    #[tokio::test]
    async fn test_delete_nonexistent_object() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        let deleted = engine.delete_object("bucket1", "nonexistent").await.unwrap();
        assert!(!deleted);
    }
}
