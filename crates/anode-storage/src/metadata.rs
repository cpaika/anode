//! Metadata storage for objects, buckets, and chunk reference counts.
//!
//! This module provides the metadata store using redb (embedded database) to track:
//! - Object manifests (chunk lists, checksums, metadata)
//! - Bucket configurations
//! - Chunk reference counts for garbage collection

use crate::chunks::ChunkId;
use crate::error::{Result, StorageError};
use anode_protocol::ObjectManifest;
use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use tracing::{debug, info, warn};

const OBJECTS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("objects");
const BUCKETS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("buckets");
const CHUNKS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("chunks");
const BUCKET_USAGE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("bucket_usage");

/// Configuration for a storage bucket.
///
/// Contains bucket settings including name, creation time, versioning, quota, and custom metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketConfig {
    pub name: String,
    pub created_at: u64,
    pub versioning_enabled: bool,
    pub metadata: BTreeMap<String, String>,
    /// Storage quota in bytes (None means unlimited)
    pub quota_bytes: Option<u64>,
}

impl BucketConfig {
    /// Create a new bucket configuration.
    ///
    /// # Arguments
    ///
    /// * `name` - The bucket name
    ///
    /// # Returns
    ///
    /// A new `BucketConfig` with default settings (versioning disabled, empty metadata).
    pub fn new(name: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            name: name.into(),
            created_at: now,
            versioning_enabled: false,
            metadata: BTreeMap::new(),
            quota_bytes: None, // Unlimited by default
        }
    }

    /// Set the storage quota for this bucket in bytes.
    pub fn with_quota(mut self, quota_bytes: u64) -> Self {
        self.quota_bytes = Some(quota_bytes);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChunkMetadata {
    ref_count: u32,
    size: u64,
    created_at: u64,
}

impl ChunkMetadata {
    fn new(size: u64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            ref_count: 1,
            size,
            created_at: now,
        }
    }
}

/// Tracks storage usage for a bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketUsage {
    pub bucket_name: String,
    pub bytes_used: u64,
    pub object_count: u64,
    pub last_updated: u64,
}

impl BucketUsage {
    fn new(bucket_name: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            bucket_name: bucket_name.into(),
            bytes_used: 0,
            object_count: 0,
            last_updated: now,
        }
    }

    fn update_timestamp(&mut self) {
        self.last_updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

/// Metadata store for objects, buckets, and chunk reference counts.
///
/// The `MetadataStore` uses an embedded database (redb) to store:
/// - Object manifests (mapping object keys to chunk lists)
/// - Bucket configurations
/// - Chunk reference counts for garbage collection
///
/// All operations are ACID-compliant through the underlying database.
pub struct MetadataStore {
    db: Database,
}

impl MetadataStore {
    /// Open or create a metadata store at the given path.
    ///
    /// Creates the database file and initializes tables if they don't exist.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database file
    ///
    /// # Returns
    ///
    /// A new `MetadataStore` instance.
    ///
    /// # Errors
    ///
    /// Returns error if the database cannot be created or opened.
    pub fn open(path: &Path) -> Result<Self> {
        info!("Opening metadata store at {:?}", path);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let db = Database::create(path)?;

        // Initialize tables
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(OBJECTS_TABLE)?;
            let _ = write_txn.open_table(BUCKETS_TABLE)?;
            let _ = write_txn.open_table(CHUNKS_TABLE)?;
            let _ = write_txn.open_table(BUCKET_USAGE_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self { db })
    }

    pub fn put_object(&self, manifest: &ObjectManifest) -> Result<()> {
        let object_key = format!("{}/{}", manifest.key.bucket, manifest.key.key);
        debug!("Storing object metadata: {}", object_key);

        let data = serde_json::to_vec(manifest)?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(OBJECTS_TABLE)?;
            table.insert(object_key.as_str(), data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    pub fn get_object(&self, bucket: &str, key: &str) -> Result<Option<ObjectManifest>> {
        let object_key = format!("{}/{}", bucket, key);
        debug!("Retrieving object metadata: {}", object_key);

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(OBJECTS_TABLE)?;

        match table.get(object_key.as_str())? {
            Some(data) => {
                let manifest: ObjectManifest = serde_json::from_slice(data.value())?;
                Ok(Some(manifest))
            },
            None => Ok(None),
        }
    }

    pub fn delete_object(&self, bucket: &str, key: &str) -> Result<bool> {
        let object_key = format!("{}/{}", bucket, key);
        debug!("Deleting object metadata: {}", object_key);

        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(OBJECTS_TABLE)?;
            let result = table.remove(object_key.as_str())?.is_some();
            result
        };
        write_txn.commit()?;

        Ok(removed)
    }

    /// Store a version-specific object manifest.
    pub fn put_object_versioned(&self, version_key: &str, manifest: &ObjectManifest) -> Result<()> {
        debug!("Storing versioned object metadata: {}", version_key);

        let data = serde_json::to_vec(manifest)?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(OBJECTS_TABLE)?;
            table.insert(version_key, data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    /// Get a version-specific object manifest.
    pub fn get_object_versioned(&self, version_key: &str) -> Result<Option<ObjectManifest>> {
        debug!("Retrieving versioned object metadata: {}", version_key);

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(OBJECTS_TABLE)?;

        match table.get(version_key)? {
            Some(data) => {
                let manifest: ObjectManifest = serde_json::from_slice(data.value())?;
                Ok(Some(manifest))
            },
            None => Ok(None),
        }
    }

    /// Delete a version-specific object manifest.
    pub fn delete_object_versioned(&self, version_key: &str) -> Result<bool> {
        debug!("Deleting versioned object metadata: {}", version_key);

        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(OBJECTS_TABLE)?;
            let result = table.remove(version_key)?.is_some();
            result
        };
        write_txn.commit()?;

        Ok(removed)
    }

    pub fn list_objects(
        &self,
        bucket: &str,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<ObjectManifest>> {
        debug!(
            "Listing objects in bucket: {}, prefix: {}, limit: {}",
            bucket, prefix, limit
        );

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(OBJECTS_TABLE)?;

        let search_prefix = if prefix.is_empty() {
            format!("{}/", bucket)
        } else {
            format!("{}/{}", bucket, prefix)
        };

        let mut results = Vec::new();
        let mut iter = table.iter()?;

        while let Some(Ok((key, value))) = iter.next() {
            let key_str = key.value();
            if key_str.starts_with(&search_prefix) {
                let manifest: ObjectManifest = serde_json::from_slice(value.value())?;
                results.push(manifest);

                if results.len() >= limit {
                    break;
                }
            }
        }

        Ok(results)
    }

    /// List all version-specific manifests for a given object key.
    ///
    /// This returns all stored versions (including delete markers) for a specific object.
    pub fn list_object_versions(&self, bucket: &str, key: &str) -> Result<Vec<ObjectManifest>> {
        debug!("Listing versions for object: {}/{}", bucket, key);

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(OBJECTS_TABLE)?;

        // Version keys are stored as "{bucket}/{key}_v{version_id}"
        let version_prefix = format!("{}/{}_v", bucket, key);

        let mut results = Vec::new();
        let mut iter = table.iter()?;

        while let Some(Ok((stored_key, value))) = iter.next() {
            let key_str = stored_key.value();
            if key_str.starts_with(&version_prefix) {
                let manifest: ObjectManifest = serde_json::from_slice(value.value())?;
                results.push(manifest);
            }
        }

        Ok(results)
    }

    /// List all versions of all objects in a bucket with pagination support.
    ///
    /// This returns all versions (including delete markers and current versions) for all objects matching the prefix.
    /// For buckets without versioning, it returns the current versions.
    /// Supports pagination using key-marker and version-id-marker.
    pub fn list_all_object_versions(
        &self,
        bucket: &str,
        prefix: &str,
        max_keys: usize,
        key_marker: Option<&str>,
        version_id_marker: Option<&str>,
    ) -> Result<Vec<ObjectManifest>> {
        debug!(
            "Listing all versions in bucket: {}, prefix: {}, max_keys: {}",
            bucket, prefix, max_keys
        );

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(OBJECTS_TABLE)?;

        let search_prefix = if prefix.is_empty() {
            format!("{}/", bucket)
        } else {
            format!("{}/{}", bucket, prefix)
        };

        let mut results = Vec::new();
        let mut iter = table.iter()?;
        let mut should_skip = key_marker.is_some();

        while let Some(Ok((stored_key, value))) = iter.next() {
            let key_str = stored_key.value();

            // Skip entries that don't match our bucket/prefix
            if !key_str.starts_with(&search_prefix) {
                continue;
            }

            // Include both versioned objects (_v suffix) and current versions (no _v suffix)
            // This allows ListObjectVersions to work for both versioned and non-versioned buckets

            let manifest: ObjectManifest = serde_json::from_slice(value.value())?;

            // Handle pagination: skip until we pass the marker
            if should_skip {
                if let Some(marker_key) = key_marker {
                    // Check if we've passed the key marker
                    if manifest.key.key.as_str() > marker_key {
                        should_skip = false;
                    } else if manifest.key.key.as_str() == marker_key {
                        // Same key, check version_id_marker
                        if let Some(marker_version) = version_id_marker {
                            if manifest.version_id.as_str() > marker_version {
                                should_skip = false;
                            }
                        } else {
                            // No version marker, start from next key
                            continue;
                        }
                    } else {
                        // Still before the marker
                        continue;
                    }
                }
            }

            if !should_skip {
                results.push(manifest);
                if results.len() >= max_keys {
                    break;
                }
            }
        }

        Ok(results)
    }

    pub fn create_bucket(&self, config: &BucketConfig) -> Result<()> {
        info!("Creating bucket: {}", config.name);

        let data = serde_json::to_vec(config)?;

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(BUCKETS_TABLE)?;

            // Check if bucket already exists
            if table.get(config.name.as_str())?.is_some() {
                return Err(StorageError::BucketAlreadyExists(config.name.clone()));
            }

            table.insert(config.name.as_str(), data.as_slice())?;

            // Initialize bucket usage
            let usage = BucketUsage::new(&config.name);
            let usage_data = serde_json::to_vec(&usage)?;
            let mut usage_table = write_txn.open_table(BUCKET_USAGE_TABLE)?;
            usage_table.insert(config.name.as_str(), usage_data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    pub fn get_bucket(&self, name: &str) -> Result<Option<BucketConfig>> {
        debug!("Retrieving bucket: {}", name);

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BUCKETS_TABLE)?;

        match table.get(name)? {
            Some(data) => {
                let config: BucketConfig = serde_json::from_slice(data.value())?;
                Ok(Some(config))
            },
            None => Ok(None),
        }
    }

    pub fn delete_bucket(&self, name: &str) -> Result<bool> {
        info!("Deleting bucket: {}", name);

        // Check if bucket has any objects
        let objects = self.list_objects(name, "", 1)?;
        if !objects.is_empty() {
            return Err(StorageError::BucketNotEmpty(name.to_string()));
        }

        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(BUCKETS_TABLE)?;
            let result = table.remove(name)?.is_some();
            result
        };
        write_txn.commit()?;

        Ok(removed)
    }

    pub fn list_buckets(&self) -> Result<Vec<BucketConfig>> {
        debug!("Listing all buckets");

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BUCKETS_TABLE)?;

        let mut results = Vec::new();
        let mut iter = table.iter()?;

        while let Some(Ok((_, value))) = iter.next() {
            let config: BucketConfig = serde_json::from_slice(value.value())?;
            results.push(config);
        }

        Ok(results)
    }

    /// Get bucket usage statistics.
    pub fn get_bucket_usage(&self, bucket: &str) -> Result<Option<BucketUsage>> {
        debug!("Retrieving bucket usage: {}", bucket);

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BUCKET_USAGE_TABLE)?;

        match table.get(bucket)? {
            Some(data) => {
                let usage: BucketUsage = serde_json::from_slice(data.value())?;
                Ok(Some(usage))
            },
            None => Ok(None),
        }
    }

    /// Update bucket quota.
    pub fn set_bucket_quota(&self, bucket: &str, quota_bytes: Option<u64>) -> Result<()> {
        info!("Setting bucket quota: {} to {:?}", bucket, quota_bytes);

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(BUCKETS_TABLE)?;

            // Get existing config
            let mut config: BucketConfig = match table.get(bucket)? {
                Some(data) => serde_json::from_slice(data.value())?,
                None => return Err(StorageError::BucketNotFound(bucket.to_string())),
            };

            // Update quota
            config.quota_bytes = quota_bytes;

            // Store updated config
            let data = serde_json::to_vec(&config)?;
            table.insert(bucket, data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    /// Increment bucket usage when an object is added.
    /// Returns the new usage amount and whether it would exceed quota.
    pub fn increment_bucket_usage(&self, bucket: &str, size: u64) -> Result<(u64, bool)> {
        debug!("Incrementing bucket usage: {} by {} bytes", bucket, size);

        let write_txn = self.db.begin_write()?;
        let (new_usage, exceeds_quota) = {
            let mut usage_table = write_txn.open_table(BUCKET_USAGE_TABLE)?;

            let mut usage = match usage_table.get(bucket)? {
                Some(data) => serde_json::from_slice(data.value())?,
                None => BucketUsage::new(bucket),
            };

            usage.bytes_used += size;
            usage.object_count += 1;
            usage.update_timestamp();

            // Check quota
            let bucket_table = write_txn.open_table(BUCKETS_TABLE)?;
            let exceeds = if let Some(config_data) = bucket_table.get(bucket)? {
                let config: BucketConfig = serde_json::from_slice(config_data.value())?;
                if let Some(quota) = config.quota_bytes {
                    usage.bytes_used > quota
                } else {
                    false
                }
            } else {
                false
            };

            let data = serde_json::to_vec(&usage)?;
            usage_table.insert(bucket, data.as_slice())?;

            (usage.bytes_used, exceeds)
        };
        write_txn.commit()?;

        Ok((new_usage, exceeds_quota))
    }

    /// Update bucket usage bytes without changing object count (for object replacement).
    pub fn update_bucket_bytes(&self, bucket: &str, delta: i64) -> Result<()> {
        debug!("Updating bucket bytes: {} by {} bytes", bucket, delta);

        let write_txn = self.db.begin_write()?;
        {
            let mut usage_table = write_txn.open_table(BUCKET_USAGE_TABLE)?;

            let mut usage: BucketUsage = match usage_table.get(bucket)? {
                Some(data) => serde_json::from_slice(data.value())?,
                None => BucketUsage::new(bucket),
            };

            if delta > 0 {
                usage.bytes_used += delta as u64;
            } else {
                usage.bytes_used = usage.bytes_used.saturating_sub((-delta) as u64);
            }
            usage.update_timestamp();

            let data = serde_json::to_vec(&usage)?;
            usage_table.insert(bucket, data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    /// Decrement bucket usage when an object is deleted.
    pub fn decrement_bucket_usage(&self, bucket: &str, size: u64) -> Result<()> {
        debug!("Decrementing bucket usage: {} by {} bytes", bucket, size);

        let write_txn = self.db.begin_write()?;
        {
            let mut usage_table = write_txn.open_table(BUCKET_USAGE_TABLE)?;

            let mut usage: BucketUsage = match usage_table.get(bucket)? {
                Some(data) => serde_json::from_slice(data.value())?,
                None => {
                    warn!("Bucket usage not found for: {}", bucket);
                    return Ok(());
                },
            };

            usage.bytes_used = usage.bytes_used.saturating_sub(size);
            usage.object_count = usage.object_count.saturating_sub(1);
            usage.update_timestamp();

            let data = serde_json::to_vec(&usage)?;
            usage_table.insert(bucket, data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }

    /// Check if adding size bytes would exceed the bucket quota.
    pub fn check_quota(&self, bucket: &str, size: u64) -> Result<bool> {
        debug!("Checking quota for bucket: {} size: {}", bucket, size);

        let read_txn = self.db.begin_read()?;

        // Get bucket config for quota
        let bucket_table = read_txn.open_table(BUCKETS_TABLE)?;
        let config: BucketConfig = match bucket_table.get(bucket)? {
            Some(data) => serde_json::from_slice(data.value())?,
            None => return Err(StorageError::BucketNotFound(bucket.to_string())),
        };

        // If no quota, never exceeds
        let quota = match config.quota_bytes {
            Some(q) => q,
            None => return Ok(false),
        };

        // Get current usage
        let usage_table = read_txn.open_table(BUCKET_USAGE_TABLE)?;
        let current_usage = match usage_table.get(bucket)? {
            Some(data) => {
                let usage: BucketUsage = serde_json::from_slice(data.value())?;
                usage.bytes_used
            },
            None => 0,
        };

        // Check if new size would exceed quota
        Ok(current_usage + size > quota)
    }

    pub fn increment_chunk_ref(&self, chunk_id: &ChunkId) -> Result<u32> {
        let chunk_key = chunk_id.0.as_bytes();
        debug!("Incrementing ref count for chunk: {}", chunk_id);

        let write_txn = self.db.begin_write()?;
        let new_count = {
            let mut table = write_txn.open_table(CHUNKS_TABLE)?;

            let metadata = match table.get(chunk_key)? {
                Some(data) => {
                    let mut meta: ChunkMetadata = serde_json::from_slice(data.value())?;
                    meta.ref_count += 1;
                    meta
                },
                None => {
                    // First reference to this chunk
                    ChunkMetadata::new(0)
                },
            };

            let data = serde_json::to_vec(&metadata)?;
            table.insert(chunk_key, data.as_slice())?;
            metadata.ref_count
        };
        write_txn.commit()?;

        Ok(new_count)
    }

    pub fn decrement_chunk_ref(&self, chunk_id: &ChunkId) -> Result<u32> {
        let chunk_key = chunk_id.0.as_bytes();
        debug!("Decrementing ref count for chunk: {}", chunk_id);

        let write_txn = self.db.begin_write()?;
        let new_count = {
            let mut table = write_txn.open_table(CHUNKS_TABLE)?;

            let metadata = match table.get(chunk_key)? {
                Some(data) => {
                    let mut meta: ChunkMetadata = serde_json::from_slice(data.value())?;
                    if meta.ref_count == 0 {
                        warn!(
                            "Attempting to decrement zero ref count for chunk: {}",
                            chunk_id
                        );
                        return Err(StorageError::RefCountError(
                            "Cannot decrement zero ref count".to_string(),
                        ));
                    }
                    meta.ref_count -= 1;
                    meta
                },
                None => {
                    return Err(StorageError::NotFound(format!(
                        "Chunk not found: {}",
                        chunk_id
                    )));
                },
            };

            if metadata.ref_count == 0 {
                // Remove the chunk metadata when ref count reaches zero
                table.remove(chunk_key)?;
            } else {
                let data = serde_json::to_vec(&metadata)?;
                table.insert(chunk_key, data.as_slice())?;
            }

            metadata.ref_count
        };
        write_txn.commit()?;

        Ok(new_count)
    }

    pub fn get_chunk_ref_count(&self, chunk_id: &ChunkId) -> Result<u32> {
        let chunk_key = chunk_id.0.as_bytes();

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CHUNKS_TABLE)?;

        match table.get(chunk_key)? {
            Some(data) => {
                let meta: ChunkMetadata = serde_json::from_slice(data.value())?;
                Ok(meta.ref_count)
            },
            None => Ok(0),
        }
    }

    pub fn list_all_chunks(&self) -> Result<Vec<ChunkId>> {
        debug!("Listing all chunks");

        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(CHUNKS_TABLE)?;

        let mut results = Vec::new();
        let mut iter = table.iter()?;

        while let Some(Ok((key, _))) = iter.next() {
            let chunk_id_str = String::from_utf8_lossy(key.value()).to_string();
            results.push(ChunkId(chunk_id_str));
        }

        Ok(results)
    }

    pub fn set_chunk_size(&self, chunk_id: &ChunkId, size: u64) -> Result<()> {
        let chunk_key = chunk_id.0.as_bytes();

        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(CHUNKS_TABLE)?;

            let mut metadata = match table.get(chunk_key)? {
                Some(data) => serde_json::from_slice(data.value())?,
                None => ChunkMetadata::new(size),
            };

            metadata.size = size;
            let data = serde_json::to_vec(&metadata)?;
            table.insert(chunk_key, data.as_slice())?;
        }
        write_txn.commit()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ObjectKey;
    use tempfile::TempDir;

    fn create_test_store() -> (MetadataStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("metadata.db");
        let store = MetadataStore::open(&db_path).unwrap();
        (store, temp_dir)
    }

    #[test]
    fn test_bucket_operations() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket");
        store.create_bucket(&config).unwrap();

        let retrieved = store.get_bucket("test-bucket").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-bucket");

        let buckets = store.list_buckets().unwrap();
        assert_eq!(buckets.len(), 1);

        let removed = store.delete_bucket("test-bucket").unwrap();
        assert!(removed);

        let retrieved = store.get_bucket("test-bucket").unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_bucket_already_exists() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket");
        store.create_bucket(&config).unwrap();

        let result = store.create_bucket(&config);
        assert!(matches!(result, Err(StorageError::BucketAlreadyExists(_))));
    }

    #[test]
    fn test_object_operations() {
        let (store, _temp) = create_test_store();

        let object_key = ObjectKey::new("bucket1", "key1");
        let manifest = ObjectManifest::new(object_key.clone(), 1024, [0u8; 32], vec![]);

        store.put_object(&manifest).unwrap();

        let retrieved = store.get_object("bucket1", "key1").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().total_size, 1024);

        let removed = store.delete_object("bucket1", "key1").unwrap();
        assert!(removed);

        let retrieved = store.get_object("bucket1", "key1").unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_list_objects() {
        let (store, _temp) = create_test_store();

        for i in 0..5 {
            let object_key = ObjectKey::new("bucket1", format!("prefix/key{}", i));
            let manifest = ObjectManifest::new(object_key, i * 100, [0u8; 32], vec![]);
            store.put_object(&manifest).unwrap();
        }

        let objects = store.list_objects("bucket1", "prefix/", 10).unwrap();
        assert_eq!(objects.len(), 5);

        let limited = store.list_objects("bucket1", "prefix/", 3).unwrap();
        assert_eq!(limited.len(), 3);
    }

    #[test]
    fn test_chunk_ref_counting() {
        let (store, _temp) = create_test_store();

        let chunk_id = ChunkId("test-chunk-123".to_string());

        let count = store.increment_chunk_ref(&chunk_id).unwrap();
        assert_eq!(count, 1);

        let count = store.increment_chunk_ref(&chunk_id).unwrap();
        assert_eq!(count, 2);

        let count = store.decrement_chunk_ref(&chunk_id).unwrap();
        assert_eq!(count, 1);

        let count = store.decrement_chunk_ref(&chunk_id).unwrap();
        assert_eq!(count, 0);

        let retrieved_count = store.get_chunk_ref_count(&chunk_id).unwrap();
        assert_eq!(retrieved_count, 0);
    }

    #[test]
    fn test_chunk_ref_underflow() {
        let (store, _temp) = create_test_store();

        let chunk_id = ChunkId("test-chunk-456".to_string());

        let result = store.decrement_chunk_ref(&chunk_id);
        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[test]
    fn test_metadata_store_open_creates_database() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("metadata.db");

        // Verify the database file doesn't exist yet
        assert!(!db_path.exists());

        // Create the MetadataStore
        let _store = MetadataStore::open(&db_path).unwrap();

        // Verify the database file was created
        assert!(db_path.exists());
        assert!(db_path.is_file());
    }

    #[test]
    fn test_delete_bucket_non_existent() {
        let (store, _temp) = create_test_store();

        // Try to delete a bucket that doesn't exist
        let result = store.delete_bucket("non-existent-bucket").unwrap();

        // Should return false indicating nothing was deleted
        assert!(!result);
    }

    #[test]
    fn test_list_buckets_returns_all_buckets() {
        let (store, _temp) = create_test_store();

        // Verify initially empty
        let buckets = store.list_buckets().unwrap();
        assert_eq!(buckets.len(), 0);

        // Create multiple buckets with different configurations
        let bucket1 = BucketConfig::new("bucket-alpha");
        let mut bucket2 = BucketConfig::new("bucket-beta");
        bucket2.versioning_enabled = true;
        let mut bucket3 = BucketConfig::new("bucket-gamma");
        bucket3
            .metadata
            .insert("key1".to_string(), "value1".to_string());

        store.create_bucket(&bucket1).unwrap();
        store.create_bucket(&bucket2).unwrap();
        store.create_bucket(&bucket3).unwrap();

        // List all buckets
        let buckets = store.list_buckets().unwrap();
        assert_eq!(buckets.len(), 3);

        // Verify all created buckets are present
        let bucket_names: Vec<String> = buckets.iter().map(|b| b.name.clone()).collect();
        assert!(bucket_names.contains(&"bucket-alpha".to_string()));
        assert!(bucket_names.contains(&"bucket-beta".to_string()));
        assert!(bucket_names.contains(&"bucket-gamma".to_string()));

        // Verify configurations are preserved
        let beta_bucket = buckets.iter().find(|b| b.name == "bucket-beta").unwrap();
        assert!(beta_bucket.versioning_enabled);

        let gamma_bucket = buckets.iter().find(|b| b.name == "bucket-gamma").unwrap();
        assert_eq!(gamma_bucket.metadata.get("key1").unwrap(), "value1");
    }

    #[test]
    fn test_get_object_non_existent() {
        let (store, _temp) = create_test_store();

        // Call get_object for a key that was never stored
        let retrieved = store.get_object("test-bucket", "non-existent-key").unwrap();

        // Verify that None is returned
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_list_objects_returns_all_objects() {
        let (store, _temp) = create_test_store();

        // Create objects with different keys in the same bucket
        let keys = vec![
            "file1.txt",
            "file2.txt",
            "dir/file3.txt",
            "another/path/file4.txt",
            "test.dat",
        ];

        for (i, key) in keys.iter().enumerate() {
            let object_key = ObjectKey::new("test-bucket", *key);
            let manifest = ObjectManifest::new(object_key, (i as u64 + 1) * 100, [0u8; 32], vec![]);
            store.put_object(&manifest).unwrap();
        }

        // List all objects with empty prefix and high limit
        let objects = store.list_objects("test-bucket", "", 100).unwrap();

        // Verify all objects are returned
        assert_eq!(objects.len(), 5);

        // Verify all expected keys are present
        let returned_keys: Vec<String> = objects.iter().map(|obj| obj.key.key.clone()).collect();
        for key in &keys {
            assert!(
                returned_keys.contains(&key.to_string()),
                "Missing key: {}",
                key
            );
        }

        // Verify the objects have the correct sizes
        for obj in objects.iter() {
            assert!(obj.total_size >= 100 && obj.total_size <= 500);
        }
    }

    #[test]
    fn test_list_objects_with_prefix_filter() {
        let (store, _temp) = create_test_store();

        // Create objects with different prefixes in the same bucket
        let objects_data = vec![
            ("logs/2024/app.log", 100),
            ("logs/2024/error.log", 200),
            ("logs/2025/app.log", 300),
            ("data/users.csv", 400),
            ("data/transactions.csv", 500),
            ("config.json", 600),
        ];

        for (key, size) in &objects_data {
            let object_key = ObjectKey::new("test-bucket", *key);
            let manifest = ObjectManifest::new(object_key, *size, [0u8; 32], vec![]);
            store.put_object(&manifest).unwrap();
        }

        // Test 1: List objects with "logs/2024/" prefix
        let logs_2024 = store.list_objects("test-bucket", "logs/2024/", 10).unwrap();
        assert_eq!(logs_2024.len(), 2);
        let keys: Vec<String> = logs_2024.iter().map(|obj| obj.key.key.clone()).collect();
        assert!(keys.contains(&"logs/2024/app.log".to_string()));
        assert!(keys.contains(&"logs/2024/error.log".to_string()));

        // Test 2: List objects with "logs/" prefix (should include both 2024 and 2025)
        let all_logs = store.list_objects("test-bucket", "logs/", 10).unwrap();
        assert_eq!(all_logs.len(), 3);
        let keys: Vec<String> = all_logs.iter().map(|obj| obj.key.key.clone()).collect();
        assert!(keys.contains(&"logs/2024/app.log".to_string()));
        assert!(keys.contains(&"logs/2024/error.log".to_string()));
        assert!(keys.contains(&"logs/2025/app.log".to_string()));

        // Test 3: List objects with "data/" prefix
        let data_objects = store.list_objects("test-bucket", "data/", 10).unwrap();
        assert_eq!(data_objects.len(), 2);
        let keys: Vec<String> = data_objects.iter().map(|obj| obj.key.key.clone()).collect();
        assert!(keys.contains(&"data/users.csv".to_string()));
        assert!(keys.contains(&"data/transactions.csv".to_string()));

        // Test 4: List objects with "config" prefix (partial match)
        let config_objects = store.list_objects("test-bucket", "config", 10).unwrap();
        assert_eq!(config_objects.len(), 1);
        assert_eq!(config_objects[0].key.key, "config.json");

        // Test 5: List objects with non-matching prefix
        let no_match = store
            .list_objects("test-bucket", "nonexistent/", 10)
            .unwrap();
        assert_eq!(no_match.len(), 0);

        // Test 6: Verify prefix filtering with limit
        let limited_logs = store.list_objects("test-bucket", "logs/", 2).unwrap();
        assert_eq!(limited_logs.len(), 2);
        // All returned objects should have the "logs/" prefix
        for obj in &limited_logs {
            assert!(obj.key.key.starts_with("logs/"));
        }
    }

    #[test]
    fn test_bucket_quota_configuration() {
        let (store, _temp) = create_test_store();

        // Create bucket with quota
        let config = BucketConfig::new("quota-bucket").with_quota(1024 * 1024); // 1MB
        store.create_bucket(&config).unwrap();

        // Verify quota was set
        let retrieved = store.get_bucket("quota-bucket").unwrap().unwrap();
        assert_eq!(retrieved.quota_bytes, Some(1024 * 1024));

        // Create bucket without quota
        let unlimited_config = BucketConfig::new("unlimited-bucket");
        store.create_bucket(&unlimited_config).unwrap();

        let retrieved_unlimited = store.get_bucket("unlimited-bucket").unwrap().unwrap();
        assert_eq!(retrieved_unlimited.quota_bytes, None);
    }

    #[test]
    fn test_set_bucket_quota() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket");
        store.create_bucket(&config).unwrap();

        // Initially no quota
        let bucket = store.get_bucket("test-bucket").unwrap().unwrap();
        assert_eq!(bucket.quota_bytes, None);

        // Set quota
        store.set_bucket_quota("test-bucket", Some(5000)).unwrap();
        let bucket = store.get_bucket("test-bucket").unwrap().unwrap();
        assert_eq!(bucket.quota_bytes, Some(5000));

        // Update quota
        store.set_bucket_quota("test-bucket", Some(10000)).unwrap();
        let bucket = store.get_bucket("test-bucket").unwrap().unwrap();
        assert_eq!(bucket.quota_bytes, Some(10000));

        // Remove quota
        store.set_bucket_quota("test-bucket", None).unwrap();
        let bucket = store.get_bucket("test-bucket").unwrap().unwrap();
        assert_eq!(bucket.quota_bytes, None);
    }

    #[test]
    fn test_bucket_usage_initialization() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket");
        store.create_bucket(&config).unwrap();

        // Usage should be initialized to zero
        let usage = store.get_bucket_usage("test-bucket").unwrap().unwrap();
        assert_eq!(usage.bytes_used, 0);
        assert_eq!(usage.object_count, 0);
        assert_eq!(usage.bucket_name, "test-bucket");
    }

    #[test]
    fn test_increment_bucket_usage() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket");
        store.create_bucket(&config).unwrap();

        // Increment usage
        let (new_usage, exceeds) = store.increment_bucket_usage("test-bucket", 1024).unwrap();
        assert_eq!(new_usage, 1024);
        assert!(!exceeds); // No quota set

        let usage = store.get_bucket_usage("test-bucket").unwrap().unwrap();
        assert_eq!(usage.bytes_used, 1024);
        assert_eq!(usage.object_count, 1);

        // Increment again
        let (new_usage, exceeds) = store.increment_bucket_usage("test-bucket", 512).unwrap();
        assert_eq!(new_usage, 1536);
        assert!(!exceeds);

        let usage = store.get_bucket_usage("test-bucket").unwrap().unwrap();
        assert_eq!(usage.bytes_used, 1536);
        assert_eq!(usage.object_count, 2);
    }

    #[test]
    fn test_decrement_bucket_usage() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket");
        store.create_bucket(&config).unwrap();

        // Add some usage first
        store.increment_bucket_usage("test-bucket", 2048).unwrap();
        store.increment_bucket_usage("test-bucket", 1024).unwrap();

        let usage = store.get_bucket_usage("test-bucket").unwrap().unwrap();
        assert_eq!(usage.bytes_used, 3072);
        assert_eq!(usage.object_count, 2);

        // Decrement usage
        store.decrement_bucket_usage("test-bucket", 1024).unwrap();

        let usage = store.get_bucket_usage("test-bucket").unwrap().unwrap();
        assert_eq!(usage.bytes_used, 2048);
        assert_eq!(usage.object_count, 1);

        // Decrement again
        store.decrement_bucket_usage("test-bucket", 2048).unwrap();

        let usage = store.get_bucket_usage("test-bucket").unwrap().unwrap();
        assert_eq!(usage.bytes_used, 0);
        assert_eq!(usage.object_count, 0);
    }

    #[test]
    fn test_check_quota_no_quota_set() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket");
        store.create_bucket(&config).unwrap();

        // Add usage
        store.increment_bucket_usage("test-bucket", 5000).unwrap();

        // Should never exceed when no quota is set
        let exceeds = store.check_quota("test-bucket", 10000).unwrap();
        assert!(!exceeds);
    }

    #[test]
    fn test_check_quota_within_limit() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket").with_quota(10000);
        store.create_bucket(&config).unwrap();

        // Add some usage
        store.increment_bucket_usage("test-bucket", 3000).unwrap();

        // Check if adding more would exceed
        let exceeds = store.check_quota("test-bucket", 5000).unwrap();
        assert!(!exceeds); // 3000 + 5000 = 8000, which is <= 10000
    }

    #[test]
    fn test_check_quota_exceeds_limit() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket").with_quota(10000);
        store.create_bucket(&config).unwrap();

        // Add usage close to limit
        store.increment_bucket_usage("test-bucket", 7000).unwrap();

        // Check if adding more would exceed
        let exceeds = store.check_quota("test-bucket", 5000).unwrap();
        assert!(exceeds); // 7000 + 5000 = 12000, which is > 10000
    }

    #[test]
    fn test_check_quota_exact_limit() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket").with_quota(10000);
        store.create_bucket(&config).unwrap();

        // Add usage up to limit
        store.increment_bucket_usage("test-bucket", 6000).unwrap();

        // Adding exactly to the limit should not exceed
        let exceeds = store.check_quota("test-bucket", 4000).unwrap();
        assert!(!exceeds); // 6000 + 4000 = 10000, which equals quota

        // Adding one more byte should exceed
        let exceeds = store.check_quota("test-bucket", 4001).unwrap();
        assert!(exceeds); // 6000 + 4001 = 10001, which is > 10000
    }

    #[test]
    fn test_increment_usage_tracks_quota_exceeded() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket").with_quota(5000);
        store.create_bucket(&config).unwrap();

        // Add usage within quota
        let (usage, exceeds) = store.increment_bucket_usage("test-bucket", 3000).unwrap();
        assert_eq!(usage, 3000);
        assert!(!exceeds);

        // Add more that exceeds quota (increment still happens)
        let (usage, exceeds) = store.increment_bucket_usage("test-bucket", 3000).unwrap();
        assert_eq!(usage, 6000);
        assert!(exceeds); // Now over quota
    }

    #[test]
    fn test_usage_decrement_saturating() {
        let (store, _temp) = create_test_store();

        let config = BucketConfig::new("test-bucket");
        store.create_bucket(&config).unwrap();

        // Try to decrement more than exists (should saturate at 0)
        store.decrement_bucket_usage("test-bucket", 5000).unwrap();

        let usage = store.get_bucket_usage("test-bucket").unwrap().unwrap();
        assert_eq!(usage.bytes_used, 0);
        assert_eq!(usage.object_count, 0);
    }

    #[test]
    fn test_set_quota_on_nonexistent_bucket() {
        let (store, _temp) = create_test_store();

        let result = store.set_bucket_quota("nonexistent", Some(1000));
        assert!(matches!(result, Err(StorageError::BucketNotFound(_))));
    }

    #[test]
    fn test_check_quota_on_nonexistent_bucket() {
        let (store, _temp) = create_test_store();

        let result = store.check_quota("nonexistent", 1000);
        assert!(matches!(result, Err(StorageError::BucketNotFound(_))));
    }
}
