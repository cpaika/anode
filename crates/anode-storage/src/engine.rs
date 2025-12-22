//! Storage engine implementation.
//!
//! This module provides the main [`StorageEngine`] which coordinates object storage operations
//! across the blob store, metadata store, and chunk manager.

use crate::blobs::BlobStore;
use crate::chunks::{ChunkId, ChunkManager, GcStats, IntegrityReport};
use crate::error::{Result, StorageError};
use crate::metadata::{BucketConfig, MetadataStore};
use anode_protocol::{ChunkId as ProtocolChunkId, ObjectKey, ObjectManifest};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

/// Configuration for the storage engine.
///
/// Defines paths and settings for the storage engine including data directories,
/// chunk size, and write synchronization behavior.
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
    /// Create a new storage configuration from a base directory.
    ///
    /// # Arguments
    ///
    /// * `base_dir` - Base directory for storage data
    ///
    /// # Returns
    ///
    /// A new `StorageConfig` with blob and metadata subdirectories under `base_dir`.
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            data_dir: base_dir.join("blobs"),
            metadata_dir: base_dir.join("metadata"),
            chunk_size: crate::chunks::CHUNK_SIZE,
            sync_writes: true,
        }
    }
}

/// Main storage engine coordinating all storage operations.
///
/// The `StorageEngine` provides a high-level interface for object storage operations
/// including put, get, delete, and list. It coordinates between the blob store
/// (for chunk data), metadata store (for manifests and ref counts), and chunk manager
/// (for chunking and deduplication).
///
/// # Examples
///
/// ## Creating a Storage Engine
///
/// ```rust,no_run
/// use anode_storage::{StorageEngine, StorageConfig};
/// use std::path::PathBuf;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create storage engine with default configuration
/// let config = StorageConfig::new(PathBuf::from("/var/lib/anode"));
/// let engine = StorageEngine::open(config).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Storing and Retrieving Objects
///
/// ```rust,no_run
/// use anode_storage::{StorageEngine, StorageConfig, BucketConfig};
/// use std::path::PathBuf;
/// use std::collections::BTreeMap;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
/// # let engine = StorageEngine::open(config).await?;
/// // Create a bucket first
/// engine.create_bucket("documents", BucketConfig::new("documents")).await?;
///
/// // Store an object with custom metadata
/// let data = b"Hello, world! This is my document.";
/// let mut metadata = BTreeMap::new();
/// metadata.insert("content-type".to_string(), "text/plain".to_string());
/// metadata.insert("author".to_string(), "Alice".to_string());
///
/// let manifest = engine
///     .put_object("documents", "hello.txt", data, metadata)
///     .await?;
/// println!("Stored object with {} chunks", manifest.chunks.len());
///
/// // Retrieve the object
/// if let Some((manifest, retrieved_data)) = engine.get_object("documents", "hello.txt").await? {
///     println!("Retrieved {} bytes", retrieved_data.len());
///     assert_eq!(retrieved_data, data);
///     println!("Author: {}", manifest.metadata.get("author").unwrap());
/// }
/// # Ok(())
/// # }
/// ```
///
/// ## Listing Objects
///
/// ```rust,no_run
/// use anode_storage::{StorageEngine, StorageConfig, BucketConfig};
/// use std::path::PathBuf;
/// use std::collections::BTreeMap;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
/// # let engine = StorageEngine::open(config).await?;
/// # engine.create_bucket("photos", BucketConfig::new("photos")).await?;
/// // Store multiple objects
/// engine.put_object("photos", "2024/photo1.jpg", b"...", BTreeMap::new()).await?;
/// engine.put_object("photos", "2024/photo2.jpg", b"...", BTreeMap::new()).await?;
/// engine.put_object("photos", "2025/photo3.jpg", b"...", BTreeMap::new()).await?;
///
/// // List all objects with "2024/" prefix
/// let result = engine.list_objects("photos", "2024/", 100, None).await?;
/// for obj in result.objects {
///     println!("Found: {} ({} bytes)", obj.key.key, obj.total_size);
/// }
/// # Ok(())
/// # }
/// ```
///
/// ## Deleting Objects
///
/// ```rust,no_run
/// use anode_storage::{StorageEngine, StorageConfig, BucketConfig};
/// use std::path::PathBuf;
/// use std::collections::BTreeMap;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
/// # let engine = StorageEngine::open(config).await?;
/// # engine.create_bucket("temp", BucketConfig::new("temp")).await?;
/// # engine.put_object("temp", "old-file.txt", b"data", BTreeMap::new()).await?;
/// // Delete an object
/// let deleted = engine.delete_object("temp", "old-file.txt").await?;
/// if deleted {
///     println!("Object deleted successfully");
/// } else {
///     println!("Object did not exist");
/// }
///
/// // Verify deletion
/// let result = engine.get_object("temp", "old-file.txt").await?;
/// assert!(result.is_none());
/// # Ok(())
/// # }
/// ```
///
/// ## Working with Large Objects and Deduplication
///
/// ```rust,no_run
/// use anode_storage::{StorageEngine, StorageConfig, BucketConfig};
/// use std::path::PathBuf;
/// use std::collections::BTreeMap;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
/// # let engine = StorageEngine::open(config).await?;
/// # engine.create_bucket("backups", BucketConfig::new("backups")).await?;
/// // Store a large object (will be automatically chunked)
/// let large_data = vec![0u8; 10 * 1024 * 1024]; // 10 MB
/// engine.put_object("backups", "backup-v1.tar", &large_data, BTreeMap::new()).await?;
///
/// // Store the same data again (chunks will be deduplicated)
/// engine.put_object("backups", "backup-v2.tar", &large_data, BTreeMap::new()).await?;
///
/// // Check statistics
/// let stats = engine.get_stats().await?;
/// println!("Objects: {}", stats.object_count); // 2 objects
/// println!("Unique chunks: {}", stats.chunk_count); // Shared chunks due to dedup
/// println!("Total object size: {}", stats.object_size);
/// println!("Actual storage used: {}", stats.total_size); // Less due to dedup
/// # Ok(())
/// # }
/// ```
pub struct StorageEngine {
    metadata: Arc<MetadataStore>,
    blobs: Arc<BlobStore>,
    chunks: Arc<ChunkManager>,
    #[allow(dead_code)]
    config: StorageConfig,
}

impl StorageEngine {
    /// Open a storage engine with the given configuration.
    ///
    /// Creates necessary directories and initializes the blob store, metadata store,
    /// and chunk manager.
    ///
    /// # Arguments
    ///
    /// * `config` - Storage configuration including data directories and settings
    ///
    /// # Returns
    ///
    /// Returns a new `StorageEngine` instance on success.
    ///
    /// # Errors
    ///
    /// Returns error if directories cannot be created or stores cannot be initialized.
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

    /// Store an object in the storage engine.
    ///
    /// Chunks the data, stores chunks with deduplication, computes checksums,
    /// and stores the object manifest.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Bucket name where the object will be stored
    /// * `key` - Object key (path) within the bucket
    /// * `data` - Object data as bytes
    /// * `metadata` - Custom metadata as key-value pairs
    ///
    /// # Returns
    ///
    /// Returns the object manifest containing chunk IDs and checksum.
    ///
    /// # Errors
    ///
    /// Returns error if the bucket doesn't exist or if storage operations fail.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```rust,no_run
    /// # use anode_storage::{StorageEngine, StorageConfig, BucketConfig};
    /// # use std::path::PathBuf;
    /// # use std::collections::BTreeMap;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
    /// # let engine = StorageEngine::open(config).await?;
    /// # engine.create_bucket("mybucket", BucketConfig::new("mybucket")).await?;
    /// let data = b"Hello, world!";
    /// let manifest = engine
    ///     .put_object("mybucket", "greeting.txt", data, BTreeMap::new())
    ///     .await?;
    /// println!("Stored {} bytes in {} chunks", manifest.total_size, manifest.chunks.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// With metadata:
    ///
    /// ```rust,no_run
    /// # use anode_storage::{StorageEngine, StorageConfig, BucketConfig};
    /// # use std::path::PathBuf;
    /// # use std::collections::BTreeMap;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
    /// # let engine = StorageEngine::open(config).await?;
    /// # engine.create_bucket("images", BucketConfig::new("images")).await?;
    /// let image_data = vec![0u8; 1024]; // Placeholder image data
    /// let mut metadata = BTreeMap::new();
    /// metadata.insert("content-type".to_string(), "image/png".to_string());
    /// metadata.insert("width".to_string(), "800".to_string());
    /// metadata.insert("height".to_string(), "600".to_string());
    ///
    /// let manifest = engine
    ///     .put_object("images", "photo.png", &image_data, metadata)
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        data: &[u8],
        metadata: BTreeMap<String, String>,
    ) -> Result<ObjectManifest> {
        info!("Putting object: {}/{} ({} bytes)", bucket, key, data.len());

        // Get bucket configuration to check versioning status
        let bucket_config = self
            .metadata
            .get_bucket(bucket)?
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        // Check if we're replacing an existing object (for quota calculation)
        let existing_size = if let Some(existing) = self.metadata.get_object(bucket, key)? {
            existing.total_size
        } else {
            0
        };

        // Check quota before storing
        // We need to account for the net change in storage: new size - existing size
        let size_delta = data.len() as u64;
        let net_increase = if existing_size > 0 {
            // Replacing existing object - only check if new object is larger
            if size_delta > existing_size {
                size_delta - existing_size
            } else {
                0 // No net increase, quota check not needed
            }
        } else {
            size_delta // New object, full size counts
        };

        if net_increase > 0 && self.metadata.check_quota(bucket, net_increase)? {
            // Get current usage for error message
            let usage = self.metadata.get_bucket_usage(bucket)?.unwrap_or_else(|| {
                crate::metadata::BucketUsage {
                    bucket_name: bucket.to_string(),
                    bytes_used: 0,
                    object_count: 0,
                    last_updated: 0,
                }
            });

            return Err(StorageError::QuotaExceeded {
                bucket: bucket.to_string(),
                quota: bucket_config.quota_bytes.unwrap_or(0),
                current_usage: usage.bytes_used,
                requested: net_increase,
            });
        }

        // Split data into chunks
        let chunks = ChunkManager::split_into_chunks(data);
        let chunk_count = chunks.len();

        // Store chunks
        let chunk_ids = self.chunks.store_chunks(chunks).await?;

        // Compute checksum (SHA-256)
        let mut hasher = Sha256::new();
        hasher.update(data);
        let checksum_bytes = hasher.finalize();
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&checksum_bytes);

        // Convert storage ChunkIds to protocol ChunkIds
        let protocol_chunk_ids: Vec<ProtocolChunkId> = chunk_ids
            .iter()
            .map(|id| ProtocolChunkId::from_hex(&id.0).unwrap())
            .collect();

        // Create manifest
        let object_key = ObjectKey::new(bucket, key);
        let mut manifest =
            ObjectManifest::new(object_key, data.len() as u64, checksum, protocol_chunk_ids);
        manifest.metadata = metadata;

        // Generate version ID if versioning is enabled
        if bucket_config.versioning_enabled {
            manifest.version_id = generate_version_id();
        } else {
            manifest.version_id = "null".to_string();
        }

        // Store manifest
        // For versioned buckets, store both the current version and the version-specific manifest
        if bucket_config.versioning_enabled {
            // Store version-specific manifest with version-qualified key
            let version_key = format!("{}/{}_v{}", bucket, key, manifest.version_id);
            self.metadata
                .put_object_versioned(&version_key, &manifest)?;
        }

        // Always store the current version with the regular key (for non-versioned GET)
        self.metadata.put_object(&manifest)?;

        // Update bucket usage tracking
        if existing_size > 0 {
            // Replacing object - adjust usage by the difference (don't change object count)
            let delta = size_delta as i64 - existing_size as i64;
            if delta != 0 {
                self.metadata.update_bucket_bytes(bucket, delta)?;
            }
        } else {
            // New object - increment by full size and count
            self.metadata.increment_bucket_usage(bucket, size_delta)?;
        }

        info!(
            "Stored object {}/{} with {} chunks (version: {})",
            bucket, key, chunk_count, manifest.version_id
        );

        Ok(manifest)
    }

    /// Retrieve an object from the storage engine.
    ///
    /// Fetches the object manifest and reconstructs the object data from its chunks.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Bucket name containing the object
    /// * `key` - Object key to retrieve
    ///
    /// # Returns
    ///
    /// Returns `Some((manifest, data))` if the object exists, or `None` if not found.
    ///
    /// # Errors
    ///
    /// Returns error if chunks are missing/corrupted or if retrieval fails.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use anode_storage::{StorageEngine, StorageConfig, BucketConfig};
    /// # use std::path::PathBuf;
    /// # use std::collections::BTreeMap;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
    /// # let engine = StorageEngine::open(config).await?;
    /// # engine.create_bucket("docs", BucketConfig::new("docs")).await?;
    /// # engine.put_object("docs", "readme.md", b"# README", BTreeMap::new()).await?;
    /// match engine.get_object("docs", "readme.md").await? {
    ///     Some((manifest, data)) => {
    ///         println!("Found object: {} bytes", data.len());
    ///         println!("Chunks: {}", manifest.chunks.len());
    ///         if let Some(content_type) = manifest.metadata.get("content-type") {
    ///             println!("Content-Type: {}", content_type);
    ///         }
    ///     }
    ///     None => println!("Object not found"),
    /// }
    /// # Ok(())
    /// # }
    /// ```
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

        // Convert protocol ChunkIds to storage ChunkIds
        let chunk_ids: Vec<ChunkId> = manifest
            .chunks
            .iter()
            .map(|id| ChunkId(id.to_hex()))
            .collect();

        // Retrieve chunks
        let data = self.chunks.retrieve_chunks(&chunk_ids).await?;

        // Verify size matches
        if data.len() as u64 != manifest.total_size {
            return Err(StorageError::Corrupted(format!(
                "Object size mismatch: expected {}, got {}",
                manifest.total_size,
                data.len()
            )));
        }

        Ok(Some((manifest, data)))
    }

    /// Get a specific version of an object.
    ///
    /// Retrieves a specific version of an object by its version ID. This method looks up
    /// the object manifest using the version-qualified key format: `{bucket}/{key}@{version_id}`.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Bucket name containing the object
    /// * `key` - Object key
    /// * `version_id` - The specific version ID to retrieve
    ///
    /// # Returns
    ///
    /// Returns the object manifest and data for the specified version, or None if not found.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use anode_storage::{StorageEngine, StorageConfig};
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
    /// # let engine = StorageEngine::open(config).await?;
    /// // Retrieve a specific version
    /// if let Some((manifest, data)) = engine
    ///     .get_object_version("my-bucket", "document.txt", "AAABBBCCCDDDEEEFFFGGGHHHiii")
    ///     .await?
    /// {
    ///     println!("Retrieved version: {}", manifest.version_id);
    ///     println!("Data: {} bytes", data.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_object_version(
        &self,
        bucket: &str,
        key: &str,
        version_id: &str,
    ) -> Result<Option<(ObjectManifest, Vec<u8>)>> {
        debug!("Getting object version: {}/{}@{}", bucket, key, version_id);

        // Construct version-qualified key
        let version_key = format!("{}/{}_v{}", bucket, key, version_id);

        // Get manifest using version-qualified key
        let manifest = match self.metadata.get_object_versioned(&version_key)? {
            Some(m) => m,
            None => return Ok(None),
        };

        // Verify the version ID matches (safety check)
        if manifest.version_id != version_id {
            warn!(
                "Version ID mismatch: expected {}, got {}",
                version_id, manifest.version_id
            );
            return Ok(None);
        }

        // Convert protocol ChunkIds to storage ChunkIds
        let chunk_ids: Vec<ChunkId> = manifest
            .chunks
            .iter()
            .map(|id| ChunkId(id.to_hex()))
            .collect();

        // Retrieve chunks
        let data = self.chunks.retrieve_chunks(&chunk_ids).await?;

        // Verify size matches
        if data.len() as u64 != manifest.total_size {
            return Err(StorageError::Corrupted(format!(
                "Object size mismatch: expected {}, got {}",
                manifest.total_size,
                data.len()
            )));
        }

        Ok(Some((manifest, data)))
    }

    /// Delete an object from the storage engine.
    ///
    /// Deletes the object manifest and decrements reference counts on all chunks.
    /// Chunks with zero references are automatically deleted.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Bucket name containing the object
    /// * `key` - Object key to delete
    ///
    /// # Returns
    ///
    /// Returns `true` if the object was deleted, `false` if it didn't exist.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use anode_storage::{StorageEngine, StorageConfig, BucketConfig};
    /// # use std::path::PathBuf;
    /// # use std::collections::BTreeMap;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
    /// # let engine = StorageEngine::open(config).await?;
    /// # engine.create_bucket("temp", BucketConfig::new("temp")).await?;
    /// # engine.put_object("temp", "old.txt", b"data", BTreeMap::new()).await?;
    /// // Delete an object
    /// if engine.delete_object("temp", "old.txt").await? {
    ///     println!("Object deleted successfully");
    /// } else {
    ///     println!("Object did not exist");
    /// }
    ///
    /// // Deleting again returns false (idempotent operation)
    /// assert!(!engine.delete_object("temp", "old.txt").await?);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_object(&self, bucket: &str, key: &str) -> Result<bool> {
        info!("Deleting object: {}/{}", bucket, key);

        // Get bucket configuration to check versioning status
        let bucket_config = self
            .metadata
            .get_bucket(bucket)?
            .ok_or_else(|| StorageError::BucketNotFound(bucket.to_string()))?;

        // For versioned buckets, create a delete marker instead of actually deleting
        if bucket_config.versioning_enabled {
            // This is a non-versioned delete on a versioned bucket - create delete marker
            return self.create_delete_marker(bucket, key).await;
        }

        // For non-versioned buckets, perform actual deletion
        // Get manifest
        let manifest = match self.metadata.get_object(bucket, key)? {
            Some(m) => m,
            None => return Ok(false),
        };

        // Convert protocol ChunkIds to storage ChunkIds
        let chunk_ids: Vec<ChunkId> = manifest
            .chunks
            .iter()
            .map(|id| ChunkId(id.to_hex()))
            .collect();

        // Delete chunks (decrement ref counts)
        self.chunks.delete_chunks(&chunk_ids).await?;

        // Delete manifest
        self.metadata.delete_object(bucket, key)?;

        // Update bucket usage
        self.metadata
            .decrement_bucket_usage(bucket, manifest.total_size)?;

        Ok(true)
    }

    /// Create a delete marker for a versioned bucket.
    ///
    /// Delete markers are special manifest entries that indicate an object has been deleted
    /// but preserve all previous versions.
    async fn create_delete_marker(&self, bucket: &str, key: &str) -> Result<bool> {
        info!("Creating delete marker for: {}/{}", bucket, key);

        // Create a delete marker manifest (empty object with is_delete_marker = true)
        let object_key = ObjectKey::new(bucket, key);
        let mut manifest = ObjectManifest::new(object_key, 0, [0u8; 32], vec![]);
        manifest.is_delete_marker = true;
        manifest.version_id = generate_version_id();

        // Store the delete marker as the current version
        self.metadata.put_object(&manifest)?;

        // Also store the version-specific delete marker
        let version_key = format!("{}/{}_v{}", bucket, key, manifest.version_id);
        let versioned_manifest = manifest.clone();
        self.metadata
            .put_object_versioned(&version_key, &versioned_manifest)?;

        Ok(true)
    }

    /// Delete a specific version of an object (permanent deletion).
    ///
    /// This permanently deletes the specified version, including delete markers.
    pub async fn delete_object_version(
        &self,
        bucket: &str,
        key: &str,
        version_id: &str,
    ) -> Result<Option<ObjectManifest>> {
        info!(
            "Permanently deleting version: {}/{} (version: {})",
            bucket, key, version_id
        );

        // Get the version-specific manifest
        let version_key = format!("{}/{}_v{}", bucket, key, version_id);
        let manifest = match self.metadata.get_object_versioned(&version_key)? {
            Some(m) => m,
            None => return Ok(None),
        };

        // If it's a delete marker, just remove the manifest (no chunks to delete)
        if manifest.is_delete_marker {
            self.metadata.delete_object_versioned(&version_key)?;

            // Check if this was the current version
            if let Some(current) = self.metadata.get_object(bucket, key)? {
                if current.version_id == version_id {
                    // Find and restore the previous version as current
                    let mut versions = self.metadata.list_object_versions(bucket, key)?;

                    // Filter out the version we're deleting
                    versions.retain(|v| v.version_id != version_id);

                    // Sort by version_id descending (most recent first)
                    versions.sort_by(|a, b| b.version_id.cmp(&a.version_id));

                    // Find the most recent non-delete-marker version
                    if let Some(prev_version) = versions.iter().find(|v| !v.is_delete_marker) {
                        // Restore as current
                        let mut current_manifest = prev_version.clone();
                        current_manifest.key = ObjectKey::new(bucket, key);
                        self.metadata.put_object(&current_manifest)?;
                    } else {
                        // No previous non-delete-marker version exists, delete the current pointer
                        self.metadata.delete_object(bucket, key)?;
                    }
                }
            }

            return Ok(Some(manifest));
        }

        // For regular objects, delete the chunks
        let chunk_ids: Vec<ChunkId> = manifest
            .chunks
            .iter()
            .map(|id| ChunkId(id.to_hex()))
            .collect();

        self.chunks.delete_chunks(&chunk_ids).await?;

        // Delete the version-specific manifest
        self.metadata.delete_object_versioned(&version_key)?;

        // If this was the current version, update the current pointer
        if let Some(current) = self.metadata.get_object(bucket, key)? {
            if current.version_id == version_id {
                self.metadata.delete_object(bucket, key)?;
            }
        }

        Ok(Some(manifest))
    }

    /// Retrieve object metadata without fetching the data.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// # use anode_storage::{StorageEngine, StorageConfig, BucketConfig};
    /// # use std::path::PathBuf;
    /// # use std::collections::BTreeMap;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
    /// # let engine = StorageEngine::open(config).await?;
    /// # engine.create_bucket("data", BucketConfig::new("data")).await?;
    /// # engine.put_object("data", "file.bin", b"binary data", BTreeMap::new()).await?;
    /// // Get just the metadata, without downloading the data
    /// if let Some(manifest) = engine.head_object("data", "file.bin").await? {
    ///     println!("Object size: {} bytes", manifest.total_size);
    ///     println!("Number of chunks: {}", manifest.chunks.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn head_object(&self, bucket: &str, key: &str) -> Result<Option<ObjectManifest>> {
        debug!("Head object: {}/{}", bucket, key);
        self.metadata.get_object(bucket, key)
    }

    /// List objects in a bucket with optional prefix filtering.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Bucket name to list from
    /// * `prefix` - Only return objects with keys starting with this prefix (empty for all)
    /// * `limit` - Maximum number of objects to return
    /// * `_continuation` - Continuation token for pagination (not yet implemented)
    ///
    /// # Examples
    ///
    /// List all objects:
    ///
    /// ```rust,no_run
    /// # use anode_storage::{StorageEngine, StorageConfig, BucketConfig};
    /// # use std::path::PathBuf;
    /// # use std::collections::BTreeMap;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
    /// # let engine = StorageEngine::open(config).await?;
    /// # engine.create_bucket("files", BucketConfig::new("files")).await?;
    /// let result = engine.list_objects("files", "", 100, None).await?;
    /// for obj in result.objects {
    ///     println!("{}: {} bytes", obj.key.key, obj.total_size);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// List with prefix:
    ///
    /// ```rust,no_run
    /// # use anode_storage::{StorageEngine, StorageConfig, BucketConfig};
    /// # use std::path::PathBuf;
    /// # use std::collections::BTreeMap;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = StorageConfig::new(PathBuf::from("/tmp/storage"));
    /// # let engine = StorageEngine::open(config).await?;
    /// # engine.create_bucket("logs", BucketConfig::new("logs")).await?;
    /// # engine.put_object("logs", "2024/01/app.log", b"log1", BTreeMap::new()).await?;
    /// # engine.put_object("logs", "2024/02/app.log", b"log2", BTreeMap::new()).await?;
    /// # engine.put_object("logs", "2025/01/app.log", b"log3", BTreeMap::new()).await?;
    /// // List only logs from 2024
    /// let result = engine.list_objects("logs", "2024/", 100, None).await?;
    /// for obj in result.objects {
    ///     println!("2024 log: {}", obj.key.key);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_objects(
        &self,
        bucket: &str,
        prefix: &str,
        limit: usize,
        _continuation: Option<&str>,
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

    pub async fn list_object_versions(
        &self,
        bucket: &str,
        prefix: &str,
        max_keys: usize,
        key_marker: Option<&str>,
        version_id_marker: Option<&str>,
    ) -> Result<ListResult> {
        debug!(
            "Listing object versions: bucket={}, prefix={}, max_keys={}",
            bucket, prefix, max_keys
        );

        // Fetch one extra to determine if truncated
        let objects = self.metadata.list_all_object_versions(
            bucket,
            prefix,
            max_keys + 1,
            key_marker,
            version_id_marker,
        )?;

        let is_truncated = objects.len() > max_keys;
        let objects_to_return: Vec<ObjectManifest> = objects.into_iter().take(max_keys).collect();

        Ok(ListResult {
            objects: objects_to_return,
            continuation_token: None,
            is_truncated,
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

    /// Get bucket usage statistics including bytes used and object count.
    pub async fn get_bucket_usage(
        &self,
        bucket: &str,
    ) -> Result<Option<crate::metadata::BucketUsage>> {
        debug!("Getting bucket usage for: {}", bucket);
        self.metadata.get_bucket_usage(bucket)
    }

    /// Set storage quota for a bucket in bytes.
    /// Pass None to remove quota (unlimited storage).
    pub async fn set_bucket_quota(&self, bucket: &str, quota_bytes: Option<u64>) -> Result<()> {
        info!(
            "Setting quota for bucket: {} to {:?} bytes",
            bucket, quota_bytes
        );
        self.metadata.set_bucket_quota(bucket, quota_bytes)
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
                total_object_size += obj.total_size;
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

                // Convert protocol ChunkIds to storage ChunkIds for comparison
                let storage_chunk_ids: Vec<ChunkId> =
                    obj.chunks.iter().map(|id| ChunkId(id.to_hex())).collect();

                for chunk_id in &storage_chunk_ids {
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
                        obj.key.bucket, obj.key.key
                    );

                    match self.delete_object(&obj.key.bucket, &obj.key.key).await {
                        Ok(_) => {
                            result.objects_deleted += 1;
                        },
                        Err(e) => {
                            result.errors.push(format!(
                                "Failed to delete object {}/{}: {}",
                                obj.key.bucket, obj.key.key, e
                            ));
                        },
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

    /// Get a reference to the chunk manager.
    ///
    /// Provides access to the underlying chunk manager for advanced operations.
    pub fn chunks(&self) -> &ChunkManager {
        &self.chunks
    }
}

/// Result of a list objects operation.
///
/// Contains the list of matching objects and pagination information.
#[derive(Debug, Clone)]
pub struct ListResult {
    /// List of object manifests matching the query
    pub objects: Vec<ObjectManifest>,
    /// Token for fetching the next page of results (if truncated)
    pub continuation_token: Option<String>,
    /// Whether there are more results beyond this page
    pub is_truncated: bool,
}

/// Storage statistics.
///
/// Provides aggregate statistics about the storage engine including counts
/// of buckets, objects, and chunks, as well as total storage size.
#[derive(Debug, Clone)]
pub struct StorageStats {
    /// Number of buckets
    pub bucket_count: u64,
    /// Number of objects across all buckets
    pub object_count: u64,
    /// Number of unique chunks
    pub chunk_count: u64,
    /// Total size of all chunks in bytes
    pub total_size: u64,
    /// Total size of all objects in bytes (before chunking)
    pub object_size: u64,
}

/// Result of a repair operation.
///
/// Contains statistics about what was repaired or deleted during a repair operation.
#[derive(Debug, Clone)]
pub struct RepairResult {
    /// Number of chunks successfully repaired
    pub chunks_repaired: u64,
    /// Number of objects deleted due to unrecoverable corruption
    pub objects_deleted: u64,
    /// List of errors encountered during repair
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
        let metadata = BTreeMap::new();
        let manifest = engine
            .put_object("bucket1", "key1", data, metadata)
            .await
            .unwrap();

        assert_eq!(manifest.key.bucket, "bucket1");
        assert_eq!(manifest.key.key, "key1");
        assert_eq!(manifest.total_size, data.len() as u64);

        // Get object
        let result = engine.get_object("bucket1", "key1").await.unwrap();
        assert!(result.is_some());

        let (retrieved_manifest, retrieved_data) = result.unwrap();
        assert_eq!(retrieved_data, data);
        assert_eq!(retrieved_manifest.checksum, manifest.checksum);

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
                .put_object("bucket1", &key, &data, BTreeMap::new())
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
    async fn test_list_objects_with_prefix_filtering() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        // Create objects with different prefixes
        let test_data = b"test data";

        // Objects with "docs/" prefix
        engine
            .put_object("bucket1", "docs/readme.md", test_data, BTreeMap::new())
            .await
            .unwrap();
        engine
            .put_object("bucket1", "docs/guide.md", test_data, BTreeMap::new())
            .await
            .unwrap();
        engine
            .put_object(
                "bucket1",
                "docs/api/reference.md",
                test_data,
                BTreeMap::new(),
            )
            .await
            .unwrap();

        // Objects with "images/" prefix
        engine
            .put_object("bucket1", "images/logo.png", test_data, BTreeMap::new())
            .await
            .unwrap();
        engine
            .put_object("bucket1", "images/banner.jpg", test_data, BTreeMap::new())
            .await
            .unwrap();

        // Objects with "config/" prefix
        engine
            .put_object("bucket1", "config/app.toml", test_data, BTreeMap::new())
            .await
            .unwrap();

        // Root level object
        engine
            .put_object("bucket1", "root.txt", test_data, BTreeMap::new())
            .await
            .unwrap();

        // Test 1: List objects with "docs/" prefix - should return 3 objects
        let result = engine
            .list_objects("bucket1", "docs/", 100, None)
            .await
            .unwrap();
        assert_eq!(
            result.objects.len(),
            3,
            "Should find exactly 3 objects with 'docs/' prefix"
        );
        for obj in &result.objects {
            assert!(
                obj.key.key.starts_with("docs/"),
                "Object key '{}' should start with 'docs/'",
                obj.key.key
            );
        }

        // Test 2: List objects with "images/" prefix - should return 2 objects
        let result = engine
            .list_objects("bucket1", "images/", 100, None)
            .await
            .unwrap();
        assert_eq!(
            result.objects.len(),
            2,
            "Should find exactly 2 objects with 'images/' prefix"
        );
        for obj in &result.objects {
            assert!(
                obj.key.key.starts_with("images/"),
                "Object key '{}' should start with 'images/'",
                obj.key.key
            );
        }

        // Test 3: List objects with "config/" prefix - should return 1 object
        let result = engine
            .list_objects("bucket1", "config/", 100, None)
            .await
            .unwrap();
        assert_eq!(
            result.objects.len(),
            1,
            "Should find exactly 1 object with 'config/' prefix"
        );
        assert_eq!(result.objects[0].key.key, "config/app.toml");

        // Test 4: List objects with empty prefix - should return all 7 objects
        let result = engine.list_objects("bucket1", "", 100, None).await.unwrap();
        assert_eq!(
            result.objects.len(),
            7,
            "Should find all 7 objects with empty prefix"
        );

        // Test 5: List objects with non-existent prefix - should return 0 objects
        let result = engine
            .list_objects("bucket1", "nonexistent/", 100, None)
            .await
            .unwrap();
        assert_eq!(
            result.objects.len(),
            0,
            "Should find no objects with non-existent prefix"
        );

        // Test 6: List objects with "docs/api/" nested prefix - should return 1 object
        let result = engine
            .list_objects("bucket1", "docs/api/", 100, None)
            .await
            .unwrap();
        assert_eq!(
            result.objects.len(),
            1,
            "Should find exactly 1 object with 'docs/api/' prefix"
        );
        assert_eq!(result.objects[0].key.key, "docs/api/reference.md");

        // Test 7: Test prefix filtering with limit
        let result = engine
            .list_objects("bucket1", "docs/", 2, None)
            .await
            .unwrap();
        assert_eq!(result.objects.len(), 2, "Should respect limit parameter");
        for obj in &result.objects {
            assert!(
                obj.key.key.starts_with("docs/"),
                "Limited results should still match prefix"
            );
        }
    }

    #[tokio::test]
    async fn test_large_object() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        // Create large data (larger than chunk size)
        let large_data = vec![42u8; crate::chunks::CHUNK_SIZE * 2 + 1000];

        let manifest = engine
            .put_object("bucket1", "large-object", &large_data, BTreeMap::new())
            .await
            .unwrap();

        assert!(manifest.chunks.len() > 1);

        let result = engine.get_object("bucket1", "large-object").await.unwrap();
        assert!(result.is_some());

        let (_, retrieved_data) = result.unwrap();
        assert_eq!(retrieved_data, large_data);
    }

    #[tokio::test]
    async fn test_object_metadata() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        let mut metadata = BTreeMap::new();
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
            .put_object("bucket1", "key1", data1, BTreeMap::new())
            .await
            .unwrap();
        engine
            .put_object("bucket1", "key2", data2, BTreeMap::new())
            .await
            .unwrap();

        let stats = engine.get_stats().await.unwrap();

        assert_eq!(stats.bucket_count, 1);
        assert_eq!(stats.object_count, 2);
        assert_eq!(stats.chunk_count, 2);
        assert_eq!(stats.object_size, (data1.len() + data2.len()) as u64);
    }

    #[tokio::test]
    async fn test_stats_returns_accurate_counts() {
        let (engine, _temp) = create_test_engine().await;

        // Verify initial stats - no buckets, no objects
        let initial_stats = engine.get_stats().await.unwrap();
        assert_eq!(initial_stats.bucket_count, 0, "Should start with 0 buckets");
        assert_eq!(initial_stats.object_count, 0, "Should start with 0 objects");
        assert_eq!(initial_stats.chunk_count, 0, "Should start with 0 chunks");
        assert_eq!(
            initial_stats.object_size, 0,
            "Should start with 0 object size"
        );
        assert_eq!(
            initial_stats.total_size, 0,
            "Should start with 0 total size"
        );

        // Create a bucket
        let config = BucketConfig::new("stats-test-bucket");
        engine
            .create_bucket("stats-test-bucket", config)
            .await
            .unwrap();

        // Verify stats after bucket creation - 1 bucket, still no objects
        let after_bucket_stats = engine.get_stats().await.unwrap();
        assert_eq!(after_bucket_stats.bucket_count, 1, "Should have 1 bucket");
        assert_eq!(
            after_bucket_stats.object_count, 0,
            "Should still have 0 objects"
        );
        assert_eq!(
            after_bucket_stats.chunk_count, 0,
            "Should still have 0 chunks"
        );
        assert_eq!(
            after_bucket_stats.object_size, 0,
            "Should still have 0 object size"
        );

        // Add first object
        let data1 = b"First test object data";
        engine
            .put_object("stats-test-bucket", "object1", data1, BTreeMap::new())
            .await
            .unwrap();

        // Verify stats after first object
        let after_first_object_stats = engine.get_stats().await.unwrap();
        assert_eq!(
            after_first_object_stats.bucket_count, 1,
            "Should still have 1 bucket"
        );
        assert_eq!(
            after_first_object_stats.object_count, 1,
            "Should have 1 object"
        );
        assert_eq!(
            after_first_object_stats.chunk_count, 1,
            "Should have 1 chunk"
        );
        assert_eq!(
            after_first_object_stats.object_size,
            data1.len() as u64,
            "Object size should match first object"
        );

        // Add second object
        let data2 = b"Second test object with different data";
        engine
            .put_object("stats-test-bucket", "object2", data2, BTreeMap::new())
            .await
            .unwrap();

        // Verify stats after second object
        let after_second_object_stats = engine.get_stats().await.unwrap();
        assert_eq!(
            after_second_object_stats.bucket_count, 1,
            "Should still have 1 bucket"
        );
        assert_eq!(
            after_second_object_stats.object_count, 2,
            "Should have 2 objects"
        );
        assert_eq!(
            after_second_object_stats.chunk_count, 2,
            "Should have 2 chunks"
        );
        assert_eq!(
            after_second_object_stats.object_size,
            (data1.len() + data2.len()) as u64,
            "Object size should match sum of both objects"
        );

        // Add third object
        let data3 = b"Third object";
        engine
            .put_object("stats-test-bucket", "object3", data3, BTreeMap::new())
            .await
            .unwrap();

        // Verify stats after third object
        let after_third_object_stats = engine.get_stats().await.unwrap();
        assert_eq!(
            after_third_object_stats.bucket_count, 1,
            "Should still have 1 bucket"
        );
        assert_eq!(
            after_third_object_stats.object_count, 3,
            "Should have 3 objects"
        );
        assert_eq!(
            after_third_object_stats.chunk_count, 3,
            "Should have 3 chunks"
        );
        assert_eq!(
            after_third_object_stats.object_size,
            (data1.len() + data2.len() + data3.len()) as u64,
            "Object size should match sum of all three objects"
        );

        // Create a second bucket
        let config2 = BucketConfig::new("stats-test-bucket-2");
        engine
            .create_bucket("stats-test-bucket-2", config2)
            .await
            .unwrap();

        // Verify stats after second bucket - should have 2 buckets, same objects
        let after_second_bucket_stats = engine.get_stats().await.unwrap();
        assert_eq!(
            after_second_bucket_stats.bucket_count, 2,
            "Should have 2 buckets"
        );
        assert_eq!(
            after_second_bucket_stats.object_count, 3,
            "Should still have 3 objects"
        );

        // Add object to second bucket
        let data4 = b"Object in second bucket";
        engine
            .put_object("stats-test-bucket-2", "object4", data4, BTreeMap::new())
            .await
            .unwrap();

        // Verify final stats
        let final_stats = engine.get_stats().await.unwrap();
        assert_eq!(final_stats.bucket_count, 2, "Should have 2 buckets");
        assert_eq!(final_stats.object_count, 4, "Should have 4 objects total");
        assert_eq!(final_stats.chunk_count, 4, "Should have 4 chunks");
        assert_eq!(
            final_stats.object_size,
            (data1.len() + data2.len() + data3.len() + data4.len()) as u64,
            "Object size should match sum of all four objects"
        );

        // Delete an object and verify stats update
        engine
            .delete_object("stats-test-bucket", "object1")
            .await
            .unwrap();

        let after_delete_stats = engine.get_stats().await.unwrap();
        assert_eq!(
            after_delete_stats.bucket_count, 2,
            "Should still have 2 buckets"
        );
        assert_eq!(
            after_delete_stats.object_count, 3,
            "Should have 3 objects after deletion"
        );
        assert_eq!(
            after_delete_stats.object_size,
            (data2.len() + data3.len() + data4.len()) as u64,
            "Object size should reflect deleted object"
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
            .put_object("bucket1", "key1", data, BTreeMap::new())
            .await
            .unwrap();
        engine
            .put_object("bucket1", "key2", data, BTreeMap::new())
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
            .put_object("bucket1", "key1", data, BTreeMap::new())
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
            .put_object("bucket1", "key1", data, BTreeMap::new())
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
            .put_object("nonexistent", "key1", b"data", BTreeMap::new())
            .await;

        assert!(matches!(result, Err(StorageError::BucketNotFound(_))));
    }

    #[tokio::test]
    async fn test_delete_nonexistent_object() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        let deleted = engine
            .delete_object("bucket1", "nonexistent")
            .await
            .unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_delete_object_is_idempotent() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket
        let config = BucketConfig::new("bucket1");
        engine.create_bucket("bucket1", config).await.unwrap();

        // Create and store an object
        let data = b"Test data for idempotent delete";
        engine
            .put_object("bucket1", "test-key", data, BTreeMap::new())
            .await
            .unwrap();

        // First delete should succeed and return true
        let first_delete = engine.delete_object("bucket1", "test-key").await.unwrap();
        assert!(first_delete, "First delete should return true");

        // Verify object is gone
        let result = engine.get_object("bucket1", "test-key").await.unwrap();
        assert!(result.is_none(), "Object should be deleted");

        // Second delete should succeed and return false (idempotent)
        let second_delete = engine.delete_object("bucket1", "test-key").await.unwrap();
        assert!(
            !second_delete,
            "Second delete should return false but not error"
        );

        // Third delete should also succeed and return false (idempotent)
        let third_delete = engine.delete_object("bucket1", "test-key").await.unwrap();
        assert!(
            !third_delete,
            "Third delete should return false but not error"
        );
    }

    #[tokio::test]
    async fn test_get_object_returns_none_for_missing() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket
        let config = BucketConfig::new("test-bucket");
        engine.create_bucket("test-bucket", config).await.unwrap();

        // Try to get an object that was never created
        let result = engine
            .get_object("test-bucket", "nonexistent-key")
            .await
            .unwrap();
        assert!(result.is_none());

        // Also verify head_object returns None for missing objects
        let head_result = engine
            .head_object("test-bucket", "nonexistent-key")
            .await
            .unwrap();
        assert!(head_result.is_none());
    }

    #[tokio::test]
    async fn test_put_object_large_data_10mb() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket
        let config = BucketConfig::new("test-bucket");
        engine.create_bucket("test-bucket", config).await.unwrap();

        // Create 10MB of data with a deterministic pattern for verification
        // Using a repeating pattern that we can verify later
        let ten_mb = 10 * 1024 * 1024;
        let mut large_data = Vec::with_capacity(ten_mb);

        // Fill with a repeating 8-byte pattern: incrementing u64 values
        for i in 0..(ten_mb / 8) {
            large_data.extend_from_slice(&(i as u64).to_le_bytes());
        }
        // Fill any remaining bytes
        for i in 0..(ten_mb % 8) {
            large_data.push(i as u8);
        }

        assert_eq!(large_data.len(), ten_mb);

        // Put the large object
        let metadata = BTreeMap::new();
        let manifest = engine
            .put_object("test-bucket", "large-10mb-object", &large_data, metadata)
            .await
            .unwrap();

        // Verify manifest properties
        assert_eq!(manifest.key.bucket, "test-bucket");
        assert_eq!(manifest.key.key, "large-10mb-object");
        assert_eq!(manifest.total_size, ten_mb as u64);

        // With 4MB chunks, 10MB should be split into 3 chunks: 4MB + 4MB + 2MB
        assert_eq!(manifest.chunks.len(), 3);

        // Retrieve the object
        let result = engine
            .get_object("test-bucket", "large-10mb-object")
            .await
            .unwrap();
        assert!(result.is_some());

        let (retrieved_manifest, retrieved_data) = result.unwrap();

        // Verify retrieved data matches exactly
        assert_eq!(retrieved_data.len(), ten_mb);
        assert_eq!(retrieved_data, large_data);

        // Verify checksums match
        assert_eq!(retrieved_manifest.checksum, manifest.checksum);

        // Verify the data pattern is intact
        for i in 0..(ten_mb / 8) {
            let start = i * 8;
            let end = start + 8;
            let bytes = &retrieved_data[start..end];
            let value = u64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);
            assert_eq!(value, i as u64, "Data pattern mismatch at position {}", i);
        }
    }

    #[tokio::test]
    async fn test_put_object_with_metadata_preservation() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket
        let config = BucketConfig::new("metadata-bucket");
        engine
            .create_bucket("metadata-bucket", config)
            .await
            .unwrap();

        // Create custom metadata with various types of headers
        let mut metadata = BTreeMap::new();
        metadata.insert("content-type".to_string(), "application/json".to_string());
        metadata.insert("content-encoding".to_string(), "gzip".to_string());
        metadata.insert("cache-control".to_string(), "max-age=3600".to_string());
        metadata.insert("x-custom-header".to_string(), "custom-value".to_string());
        metadata.insert("x-author".to_string(), "test-user".to_string());
        metadata.insert("x-version".to_string(), "2.1.0".to_string());

        let data = b"{\"message\": \"Test JSON data with metadata\"}";

        // Put object with metadata
        let put_manifest = engine
            .put_object(
                "metadata-bucket",
                "test-object.json",
                data,
                metadata.clone(),
            )
            .await
            .unwrap();

        // Verify metadata is set on the manifest returned by put_object
        assert_eq!(put_manifest.metadata.len(), 6);
        assert_eq!(
            put_manifest.metadata.get("content-type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(
            put_manifest.metadata.get("content-encoding"),
            Some(&"gzip".to_string())
        );
        assert_eq!(
            put_manifest.metadata.get("cache-control"),
            Some(&"max-age=3600".to_string())
        );
        assert_eq!(
            put_manifest.metadata.get("x-custom-header"),
            Some(&"custom-value".to_string())
        );
        assert_eq!(
            put_manifest.metadata.get("x-author"),
            Some(&"test-user".to_string())
        );
        assert_eq!(
            put_manifest.metadata.get("x-version"),
            Some(&"2.1.0".to_string())
        );

        // Retrieve the object using get_object
        let get_result = engine
            .get_object("metadata-bucket", "test-object.json")
            .await
            .unwrap();
        assert!(get_result.is_some());

        let (get_manifest, retrieved_data) = get_result.unwrap();

        // Verify data is correct
        assert_eq!(retrieved_data, data);

        // Verify ALL metadata is preserved after retrieval
        assert_eq!(get_manifest.metadata.len(), 6);
        assert_eq!(
            get_manifest.metadata.get("content-type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(
            get_manifest.metadata.get("content-encoding"),
            Some(&"gzip".to_string())
        );
        assert_eq!(
            get_manifest.metadata.get("cache-control"),
            Some(&"max-age=3600".to_string())
        );
        assert_eq!(
            get_manifest.metadata.get("x-custom-header"),
            Some(&"custom-value".to_string())
        );
        assert_eq!(
            get_manifest.metadata.get("x-author"),
            Some(&"test-user".to_string())
        );
        assert_eq!(
            get_manifest.metadata.get("x-version"),
            Some(&"2.1.0".to_string())
        );

        // Verify metadata is also preserved with head_object
        let head_manifest = engine
            .head_object("metadata-bucket", "test-object.json")
            .await
            .unwrap();
        assert!(head_manifest.is_some());

        let head_manifest = head_manifest.unwrap();
        assert_eq!(head_manifest.metadata.len(), 6);
        assert_eq!(
            head_manifest.metadata.get("content-type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(
            head_manifest.metadata.get("content-encoding"),
            Some(&"gzip".to_string())
        );
        assert_eq!(
            head_manifest.metadata.get("cache-control"),
            Some(&"max-age=3600".to_string())
        );
        assert_eq!(
            head_manifest.metadata.get("x-custom-header"),
            Some(&"custom-value".to_string())
        );
        assert_eq!(
            head_manifest.metadata.get("x-author"),
            Some(&"test-user".to_string())
        );
        assert_eq!(
            head_manifest.metadata.get("x-version"),
            Some(&"2.1.0".to_string())
        );

        // Verify that the checksums match across all operations
        assert_eq!(get_manifest.checksum, put_manifest.checksum);
        assert_eq!(head_manifest.checksum, put_manifest.checksum);
    }

    #[tokio::test]
    async fn test_concurrent_puts_to_different_keys_succeed() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket
        let config = BucketConfig::new("concurrent-bucket");
        engine
            .create_bucket("concurrent-bucket", config)
            .await
            .unwrap();

        // Wrap engine in Arc to share across tasks
        let engine = Arc::new(engine);

        // Number of concurrent operations
        let num_tasks = 10;

        // Spawn concurrent tasks, each putting a different object
        let mut handles = Vec::new();

        for i in 0..num_tasks {
            let engine_clone = Arc::clone(&engine);
            let handle = tokio::spawn(async move {
                let key = format!("concurrent-key-{}", i);
                let data = format!("Concurrent data for object {}", i).into_bytes();
                let mut metadata = BTreeMap::new();
                metadata.insert("task-id".to_string(), i.to_string());

                engine_clone
                    .put_object("concurrent-bucket", &key, &data, metadata)
                    .await
                    .expect("Put operation should succeed")
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;

        // Verify all tasks completed successfully
        assert_eq!(results.len(), num_tasks);
        for result in results {
            assert!(result.is_ok(), "Task should complete without panicking");
        }

        // Verify all objects were stored correctly
        for i in 0..num_tasks {
            let key = format!("concurrent-key-{}", i);
            let expected_data = format!("Concurrent data for object {}", i).into_bytes();

            let result = engine
                .get_object("concurrent-bucket", &key)
                .await
                .expect("Get operation should succeed");

            assert!(result.is_some(), "Object {} should exist", key);

            let (manifest, retrieved_data) = result.unwrap();
            assert_eq!(
                retrieved_data, expected_data,
                "Data for {} should match",
                key
            );
            assert_eq!(manifest.key.key, key, "Key should match");
            assert_eq!(
                manifest.key.bucket, "concurrent-bucket",
                "Bucket should match"
            );
            assert_eq!(
                manifest.metadata.get("task-id"),
                Some(&i.to_string()),
                "Metadata should be preserved for {}",
                key
            );
        }

        // Verify total object count
        let list_result = engine
            .list_objects("concurrent-bucket", "", 100, None)
            .await
            .unwrap();
        assert_eq!(
            list_result.objects.len(),
            num_tasks,
            "Should have exactly {} objects",
            num_tasks
        );
    }

    #[tokio::test]
    async fn test_concurrent_puts_to_same_key_are_serialized() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket
        let config = BucketConfig::new("concurrent-bucket");
        engine
            .create_bucket("concurrent-bucket", config)
            .await
            .unwrap();

        // Wrap engine in Arc to share across tasks
        let engine = Arc::new(engine);

        // Number of concurrent operations to the SAME key
        let num_tasks = 20;

        // Spawn concurrent tasks, all putting to the SAME key with different data
        let mut handles = Vec::new();

        for i in 0..num_tasks {
            let engine_clone = Arc::clone(&engine);
            let handle = tokio::spawn(async move {
                let key = "same-key"; // All tasks write to the same key
                let data = format!("Data from task {}", i).into_bytes();
                let mut metadata = BTreeMap::new();
                metadata.insert("task-id".to_string(), i.to_string());

                engine_clone
                    .put_object("concurrent-bucket", key, &data, metadata)
                    .await
                    .expect("Put operation should succeed")
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let results = futures::future::join_all(handles).await;

        // Verify all tasks completed successfully
        assert_eq!(results.len(), num_tasks);
        for result in results {
            assert!(result.is_ok(), "Task should complete without panicking");
        }

        // Verify that the object exists (last write wins)
        let result = engine
            .get_object("concurrent-bucket", "same-key")
            .await
            .expect("Get operation should succeed");

        assert!(
            result.is_some(),
            "Object should exist after concurrent puts"
        );

        let (manifest, retrieved_data) = result.unwrap();

        // Verify the data is valid and matches one of the writes
        assert!(!retrieved_data.is_empty(), "Data should not be empty");

        // Check that the data matches the expected pattern from one of the tasks
        let data_str =
            String::from_utf8(retrieved_data.clone()).expect("Data should be valid UTF-8");
        assert!(
            data_str.starts_with("Data from task "),
            "Data should match the expected pattern: {}",
            data_str
        );

        // Extract the task ID from the data
        let task_id_from_data = data_str
            .strip_prefix("Data from task ")
            .and_then(|s| s.parse::<usize>().ok())
            .expect("Should be able to extract task ID from data");

        assert!(
            task_id_from_data < num_tasks,
            "Task ID {} should be in valid range 0-{}",
            task_id_from_data,
            num_tasks - 1
        );

        // Verify metadata matches the data (ensuring consistency)
        let metadata_task_id = manifest
            .metadata
            .get("task-id")
            .and_then(|s| s.parse::<usize>().ok())
            .expect("Metadata should contain valid task-id");

        assert_eq!(
            metadata_task_id, task_id_from_data,
            "Metadata task-id should match data task-id (no partial writes)"
        );

        // Verify no data corruption: checksum should be valid
        let mut hasher = Sha256::new();
        hasher.update(&retrieved_data);
        let computed_checksum = hasher.finalize();
        let computed_checksum_bytes: [u8; 32] = computed_checksum.into();

        assert_eq!(
            manifest.checksum, computed_checksum_bytes,
            "Checksum should match, indicating no data corruption"
        );

        // Verify size matches
        assert_eq!(
            manifest.total_size,
            retrieved_data.len() as u64,
            "Manifest size should match actual data size"
        );

        // Verify that there's only one object with this key
        let list_result = engine
            .list_objects("concurrent-bucket", "", 100, None)
            .await
            .unwrap();

        assert_eq!(
            list_result.objects.len(),
            1,
            "Should have exactly 1 object (not {} objects for concurrent writes to same key)",
            list_result.objects.len()
        );

        // Verify the single object is the one we expect
        assert_eq!(list_result.objects[0].key.key, "same-key");
        assert_eq!(list_result.objects[0].key.bucket, "concurrent-bucket");
    }

    #[tokio::test]
    async fn test_put_then_get_returns_identical_data_100_iterations() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket
        let config = BucketConfig::new("iteration-test-bucket");
        engine
            .create_bucket("iteration-test-bucket", config)
            .await
            .unwrap();

        // Use a simple PRNG for deterministic test data generation
        let mut seed: u64 = 42;

        // Helper function to generate pseudo-random number (simple LCG)
        let mut next_rand = || {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            seed
        };

        // Run 100 iterations with varying data sizes
        for i in 0..100 {
            // Generate varying size: range from 1 byte to ~5MB
            // Use different sizes to test small, medium, and large objects
            let size = match i % 10 {
                0 => 1,                       // Tiny: 1 byte
                1 => 100,                     // Small: 100 bytes
                2 => 1024,                    // 1KB
                3 => 10 * 1024,               // 10KB
                4 => 100 * 1024,              // 100KB
                5 => 512 * 1024,              // 512KB
                6 => 1024 * 1024,             // 1MB
                7 => 2 * 1024 * 1024,         // 2MB
                8 => 4 * 1024 * 1024,         // 4MB (exactly one chunk)
                _ => 5 * 1024 * 1024 + 12345, // ~5MB (multiple chunks)
            };

            // Generate pseudo-random data
            let mut data = Vec::with_capacity(size);
            for _ in 0..size {
                data.push((next_rand() & 0xFF) as u8);
            }

            let key = format!("iteration-test-key-{}", i);

            // Add some metadata
            let mut metadata = BTreeMap::new();
            metadata.insert("iteration".to_string(), i.to_string());
            metadata.insert("size".to_string(), size.to_string());

            // Put the object
            let put_manifest = engine
                .put_object("iteration-test-bucket", &key, &data, metadata.clone())
                .await
                .unwrap_or_else(|e| panic!("Failed to put object on iteration {}: {}", i, e));

            // Verify manifest properties
            assert_eq!(put_manifest.key.bucket, "iteration-test-bucket");
            assert_eq!(put_manifest.key.key, key);
            assert_eq!(put_manifest.total_size, size as u64);
            assert_eq!(put_manifest.metadata.get("iteration"), Some(&i.to_string()));

            // Get the object back immediately
            let get_result = engine
                .get_object("iteration-test-bucket", &key)
                .await
                .unwrap_or_else(|e| panic!("Failed to get object on iteration {}: {}", i, e));

            assert!(
                get_result.is_some(),
                "Object should exist after put on iteration {}",
                i
            );

            let (get_manifest, retrieved_data) = get_result.unwrap();

            // Verify data matches exactly
            assert_eq!(
                retrieved_data.len(),
                data.len(),
                "Data length mismatch on iteration {}: expected {}, got {}",
                i,
                data.len(),
                retrieved_data.len()
            );

            assert_eq!(
                retrieved_data, data,
                "Data content mismatch on iteration {}",
                i
            );

            // Verify manifest consistency
            assert_eq!(
                get_manifest.checksum, put_manifest.checksum,
                "Checksum mismatch on iteration {}",
                i
            );

            assert_eq!(
                get_manifest.total_size, put_manifest.total_size,
                "Total size mismatch on iteration {}",
                i
            );

            assert_eq!(
                get_manifest.metadata, put_manifest.metadata,
                "Metadata mismatch on iteration {}",
                i
            );

            // Verify checksum is correct by recomputing
            let mut hasher = Sha256::new();
            hasher.update(&retrieved_data);
            let computed_checksum = hasher.finalize();
            let computed_checksum_bytes: [u8; 32] = computed_checksum.into();

            assert_eq!(
                get_manifest.checksum, computed_checksum_bytes,
                "Computed checksum mismatch on iteration {}",
                i
            );
        }

        // Verify all 100 objects are stored
        let list_result = engine
            .list_objects("iteration-test-bucket", "", 200, None)
            .await
            .unwrap();

        assert_eq!(
            list_result.objects.len(),
            100,
            "Should have exactly 100 objects stored"
        );
    }

    #[tokio::test]
    #[ignore] // Marked as ignored since 100MB test may be too slow for CI
    async fn test_large_object_100mb_put_and_get_roundtrip() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket
        let config = BucketConfig::new("large-object-bucket");
        engine
            .create_bucket("large-object-bucket", config)
            .await
            .unwrap();

        // Create 100MB of deterministic data with a repeating pattern for verification
        let hundred_mb = 100 * 1024 * 1024;
        let mut large_data = Vec::with_capacity(hundred_mb);

        // Fill with a repeating 8-byte pattern: incrementing u64 values
        // This creates a deterministic pattern we can verify later
        for i in 0..(hundred_mb / 8) {
            large_data.extend_from_slice(&(i as u64).to_le_bytes());
        }
        // Fill any remaining bytes
        for i in 0..(hundred_mb % 8) {
            large_data.push(i as u8);
        }

        assert_eq!(large_data.len(), hundred_mb, "Data should be exactly 100MB");

        // Add metadata
        let mut metadata = BTreeMap::new();
        metadata.insert("test".to_string(), "100mb-roundtrip".to_string());
        metadata.insert("size".to_string(), "100MB".to_string());

        // Put the large object
        let put_manifest = engine
            .put_object(
                "large-object-bucket",
                "100mb-object",
                &large_data,
                metadata.clone(),
            )
            .await
            .expect("Should successfully put 100MB object");

        // Verify manifest properties
        assert_eq!(put_manifest.key.bucket, "large-object-bucket");
        assert_eq!(put_manifest.key.key, "100mb-object");
        assert_eq!(put_manifest.total_size, hundred_mb as u64);

        // With 4MB chunks, 100MB should be split into exactly 25 chunks
        // (25 chunks * 4MB = 100MB)
        assert_eq!(
            put_manifest.chunks.len(),
            25,
            "100MB object should be split into exactly 25 chunks (4MB each)"
        );

        // Verify metadata is preserved
        assert_eq!(
            put_manifest.metadata.get("test"),
            Some(&"100mb-roundtrip".to_string())
        );
        assert_eq!(
            put_manifest.metadata.get("size"),
            Some(&"100MB".to_string())
        );

        // Get the object back
        let get_result = engine
            .get_object("large-object-bucket", "100mb-object")
            .await
            .expect("Should successfully get 100MB object");

        assert!(get_result.is_some(), "Object should exist after put");

        let (get_manifest, retrieved_data) = get_result.unwrap();

        // Verify size matches
        assert_eq!(
            retrieved_data.len(),
            hundred_mb,
            "Retrieved data should be exactly 100MB"
        );

        // Verify all data matches exactly byte-for-byte
        assert_eq!(
            retrieved_data, large_data,
            "Retrieved data should match original data exactly"
        );

        // Verify checksums match
        assert_eq!(
            get_manifest.checksum, put_manifest.checksum,
            "Checksums should match"
        );

        // Verify the data pattern is intact by checking multiple points
        // Check first chunk (0-4MB range)
        for i in 0..(512 * 1024 / 8) {
            // Check first 512KB of pattern
            let start = i * 8;
            let end = start + 8;
            let bytes = &retrieved_data[start..end];
            let value = u64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            ]);
            assert_eq!(
                value, i as u64,
                "Data pattern mismatch at position {} in first chunk",
                i
            );
        }

        // Check middle chunk (around 50MB mark)
        let middle_offset = 50 * 1024 * 1024;
        let middle_pattern_start = middle_offset / 8;
        for i in 0..(512 * 1024 / 8) {
            // Check 512KB in the middle
            let pattern_idx = middle_pattern_start + i;
            let start = pattern_idx * 8;
            let end = start + 8;
            if end <= retrieved_data.len() {
                let bytes = &retrieved_data[start..end];
                let value = u64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]);
                assert_eq!(
                    value, pattern_idx as u64,
                    "Data pattern mismatch at position {} in middle chunk",
                    pattern_idx
                );
            }
        }

        // Check last chunk
        let last_chunk_offset = 96 * 1024 * 1024; // Last 4MB
        let last_pattern_start = last_chunk_offset / 8;
        for i in 0..(512 * 1024 / 8) {
            // Check 512KB in the last chunk
            let pattern_idx = last_pattern_start + i;
            let start = pattern_idx * 8;
            let end = start + 8;
            if end <= retrieved_data.len() {
                let bytes = &retrieved_data[start..end];
                let value = u64::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
                ]);
                assert_eq!(
                    value, pattern_idx as u64,
                    "Data pattern mismatch at position {} in last chunk",
                    pattern_idx
                );
            }
        }

        // Verify metadata is preserved
        assert_eq!(
            get_manifest.metadata.get("test"),
            Some(&"100mb-roundtrip".to_string())
        );
        assert_eq!(
            get_manifest.metadata.get("size"),
            Some(&"100MB".to_string())
        );

        // Verify total size
        assert_eq!(
            get_manifest.total_size, hundred_mb as u64,
            "Total size should be 100MB"
        );

        // Verify checksum is correct by recomputing
        let mut hasher = Sha256::new();
        hasher.update(&retrieved_data);
        let computed_checksum = hasher.finalize();
        let computed_checksum_bytes: [u8; 32] = computed_checksum.into();

        assert_eq!(
            get_manifest.checksum, computed_checksum_bytes,
            "Computed checksum should match manifest checksum"
        );
    }

    // Property-based tests
    use proptest::prelude::*;

    // Stateful proptest for CRUD operations
    #[derive(Debug, Clone)]
    enum CrudOp {
        Create { key: String, data: Vec<u8> },
        Read { key: String },
        Update { key: String, data: Vec<u8> },
        Delete { key: String },
    }

    fn arb_crud_op() -> impl Strategy<Value = CrudOp> {
        prop_oneof![
            // Create operation
            (
                prop::string::string_regex("[a-z]{1,10}").unwrap(),
                prop::collection::vec(any::<u8>(), 0..1000)
            )
                .prop_map(|(key, data)| CrudOp::Create { key, data }),
            // Read operation
            prop::string::string_regex("[a-z]{1,10}")
                .unwrap()
                .prop_map(|key| CrudOp::Read { key }),
            // Update operation
            (
                prop::string::string_regex("[a-z]{1,10}").unwrap(),
                prop::collection::vec(any::<u8>(), 0..1000)
            )
                .prop_map(|(key, data)| CrudOp::Update { key, data }),
            // Delete operation
            prop::string::string_regex("[a-z]{1,10}")
                .unwrap()
                .prop_map(|key| CrudOp::Delete { key }),
        ]
    }

    proptest! {
        #[test]
        fn proptest_stateful_crud_sequence_matches_model(
            ops in prop::collection::vec(arb_crud_op(), 1..50)
        ) {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                let temp_dir = TempDir::new().unwrap();
                let config = StorageConfig::new(temp_dir.path().to_path_buf());
                let engine = StorageEngine::open(config).await.unwrap();

                // Create bucket for testing
                let bucket_config = BucketConfig::new("crud-test-bucket");
                engine.create_bucket("crud-test-bucket", bucket_config).await.unwrap();

                // Model: BTreeMap tracking expected state
                let mut model: BTreeMap<String, Vec<u8>> = BTreeMap::new();

                // Apply each operation to both model and real storage
                for (op_idx, op) in ops.iter().enumerate() {
                    match op {
                        CrudOp::Create { key, data } => {
                            // Apply to model
                            model.insert(key.clone(), data.clone());

                            // Apply to real storage
                            let metadata = BTreeMap::new();
                            let result = engine
                                .put_object("crud-test-bucket", key, data, metadata)
                                .await;

                            prop_assert!(
                                result.is_ok(),
                                "Create operation {} failed for key '{}': {:?}",
                                op_idx,
                                key,
                                result.err()
                            );

                            let manifest = result.unwrap();
                            prop_assert_eq!(
                                manifest.total_size,
                                data.len() as u64,
                                "Create operation {}: size mismatch for key '{}'",
                                op_idx,
                                key
                            );
                        }

                        CrudOp::Read { key } => {
                            // Get expected from model
                            let expected = model.get(key);

                            // Get from real storage
                            let result = engine
                                .get_object("crud-test-bucket", key)
                                .await;

                            prop_assert!(
                                result.is_ok(),
                                "Read operation {} failed for key '{}': {:?}",
                                op_idx,
                                key,
                                result.err()
                            );

                            let actual = result.unwrap();

                            // Verify consistency with model
                            match (expected, actual) {
                                (Some(expected_data), Some((_, actual_data))) => {
                                    prop_assert_eq!(
                                        &actual_data,
                                        expected_data,
                                        "Read operation {}: data mismatch for key '{}'",
                                        op_idx,
                                        key
                                    );
                                }
                                (None, None) => {
                                    // Both model and storage agree: key doesn't exist
                                }
                                (Some(_), None) => {
                                    return Err(proptest::test_runner::TestCaseError::fail(
                                        format!(
                                            "Read operation {}: key '{}' exists in model but not in storage",
                                            op_idx,
                                            key
                                        )
                                    ));
                                }
                                (None, Some(_)) => {
                                    return Err(proptest::test_runner::TestCaseError::fail(
                                        format!(
                                            "Read operation {}: key '{}' exists in storage but not in model",
                                            op_idx,
                                            key
                                        )
                                    ));
                                }
                            }
                        }

                        CrudOp::Update { key, data } => {
                            // Only update if key exists in model
                            if model.contains_key(key) {
                                // Apply to model
                                model.insert(key.clone(), data.clone());

                                // Apply to real storage
                                let metadata = BTreeMap::new();
                                let result = engine
                                    .put_object("crud-test-bucket", key, data, metadata)
                                    .await;

                                prop_assert!(
                                    result.is_ok(),
                                    "Update operation {} failed for key '{}': {:?}",
                                    op_idx,
                                    key,
                                    result.err()
                                );

                                let manifest = result.unwrap();
                                prop_assert_eq!(
                                    manifest.total_size,
                                    data.len() as u64,
                                    "Update operation {}: size mismatch for key '{}'",
                                    op_idx,
                                    key
                                );
                            }
                            // If key doesn't exist, update is a no-op (could also be create)
                        }

                        CrudOp::Delete { key } => {
                            // Apply to model
                            let was_in_model = model.remove(key).is_some();

                            // Apply to real storage
                            let result = engine
                                .delete_object("crud-test-bucket", key)
                                .await;

                            prop_assert!(
                                result.is_ok(),
                                "Delete operation {} failed for key '{}': {:?}",
                                op_idx,
                                key,
                                result.err()
                            );

                            let was_in_storage = result.unwrap();

                            // Verify model and storage agree on existence
                            prop_assert_eq!(
                                was_in_model,
                                was_in_storage,
                                "Delete operation {}: existence mismatch for key '{}' (model={}, storage={})",
                                op_idx,
                                key,
                                was_in_model,
                                was_in_storage
                            );
                        }
                    }
                }

                // Final verification: check that all keys in model exist in storage with correct data
                for (key, expected_data) in &model {
                    let result = engine
                        .get_object("crud-test-bucket", key)
                        .await
                        .unwrap();

                    prop_assert!(
                        result.is_some(),
                        "Final check: key '{}' exists in model but not in storage",
                        key
                    );

                    let (_, actual_data) = result.unwrap();
                    prop_assert_eq!(
                        &actual_data,
                        expected_data,
                        "Final check: data mismatch for key '{}'",
                        key
                    );
                }

                // Final verification: check that storage doesn't have extra keys
                let list_result = engine
                    .list_objects("crud-test-bucket", "", 1000, None)
                    .await
                    .unwrap();

                prop_assert_eq!(
                    list_result.objects.len(),
                    model.len(),
                    "Final check: storage has {} objects but model has {}",
                    list_result.objects.len(),
                    model.len()
                );

                // Verify each listed object exists in model
                for obj in &list_result.objects {
                    prop_assert!(
                        model.contains_key(&obj.key.key),
                        "Final check: storage has key '{}' but model doesn't",
                        obj.key.key
                    );
                }

                Ok(())
            })?;
        }

        #[test]
        fn proptest_put_get_roundtrip_is_identity(data in prop::collection::vec(any::<u8>(), 0..1024*1024)) {
            // Run the async test in a blocking context
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                let temp_dir = TempDir::new().unwrap();
                let config = StorageConfig::new(temp_dir.path().to_path_buf());
                let engine = StorageEngine::open(config).await.unwrap();

                // Create bucket
                let bucket_config = BucketConfig::new("proptest-bucket");
                engine.create_bucket("proptest-bucket", bucket_config).await.unwrap();

                // Put object with arbitrary data
                let key = "proptest-key";
                let metadata = BTreeMap::new();
                let put_manifest = engine
                    .put_object("proptest-bucket", key, &data, metadata)
                    .await
                    .unwrap();

                // Get object back
                let get_result = engine
                    .get_object("proptest-bucket", key)
                    .await
                    .unwrap();

                prop_assert!(get_result.is_some(), "Object should exist after put");

                let (get_manifest, retrieved_data) = get_result.unwrap();

                // Property: put then get should return identical data
                prop_assert_eq!(&retrieved_data, &data, "Retrieved data should match original");

                // Property: size should match
                prop_assert_eq!(retrieved_data.len(), data.len(), "Size should match");
                prop_assert_eq!(get_manifest.total_size, data.len() as u64, "Manifest size should match");

                // Property: checksums should match
                prop_assert_eq!(get_manifest.checksum, put_manifest.checksum, "Checksums should match");

                // Property: checksum should be valid
                let mut hasher = Sha256::new();
                hasher.update(&retrieved_data);
                let computed_checksum = hasher.finalize();
                let computed_checksum_bytes: [u8; 32] = computed_checksum.into();
                prop_assert_eq!(get_manifest.checksum, computed_checksum_bytes, "Checksum should be valid");

                Ok(())
            })?;
        }

        #[test]
        fn proptest_metadata_roundtrip_preserves_all_keys_values(
            metadata in prop::collection::hash_map(
                prop::string::string_regex("[a-zA-Z0-9-_]{1,100}").unwrap(),
                prop::string::string_regex("[a-zA-Z0-9-_/.@:= ]{0,200}").unwrap(),
                0..20
            )
        ) {
            // Run the async test in a blocking context
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                let temp_dir = TempDir::new().unwrap();
                let config = StorageConfig::new(temp_dir.path().to_path_buf());
                let engine = StorageEngine::open(config).await.unwrap();

                // Create bucket
                let bucket_config = BucketConfig::new("metadata-proptest-bucket");
                engine.create_bucket("metadata-proptest-bucket", bucket_config).await.unwrap();

                // Convert HashMap to BTreeMap for put_object
                let metadata_btree: BTreeMap<String, String> = metadata.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                // Put object with arbitrary metadata
                let key = "metadata-proptest-key";
                let data = b"test data for metadata preservation";
                let put_manifest = engine
                    .put_object("metadata-proptest-bucket", key, data, metadata_btree.clone())
                    .await
                    .unwrap();

                // Property: put_object should preserve metadata in returned manifest
                prop_assert_eq!(
                    &put_manifest.metadata,
                    &metadata_btree,
                    "put_object should return manifest with same metadata"
                );

                // Get object back
                let get_result = engine
                    .get_object("metadata-proptest-bucket", key)
                    .await
                    .unwrap();

                prop_assert!(get_result.is_some(), "Object should exist after put");

                let (get_manifest, _retrieved_data) = get_result.unwrap();

                // Property: all metadata keys should be preserved
                prop_assert_eq!(
                    get_manifest.metadata.len(),
                    metadata_btree.len(),
                    "Same number of metadata entries should be preserved"
                );

                // Property: all metadata keys and values should match exactly
                for (key, value) in &metadata_btree {
                    prop_assert_eq!(
                        get_manifest.metadata.get(key),
                        Some(value),
                        "Metadata key '{}' should have value '{}'",
                        key,
                        value
                    );
                }

                // Property: no extra metadata keys should be added
                for (key, value) in &get_manifest.metadata {
                    prop_assert_eq!(
                        metadata_btree.get(key),
                        Some(value),
                        "Retrieved metadata should not contain extra key '{}'",
                        key
                    );
                }

                // Property: head_object should also preserve metadata
                let head_manifest = engine
                    .head_object("metadata-proptest-bucket", key)
                    .await
                    .unwrap();

                prop_assert!(head_manifest.is_some(), "head_object should return manifest");

                let head_manifest = head_manifest.unwrap();
                prop_assert_eq!(
                    &head_manifest.metadata,
                    &metadata_btree,
                    "head_object should preserve all metadata"
                );

                Ok(())
            })?;
        }

        #[test]
        fn proptest_etag_is_deterministic_for_same_content(data in prop::collection::vec(any::<u8>(), 0..1024*1024)) {
            // Run the async test in a blocking context
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                let temp_dir = TempDir::new().unwrap();
                let config = StorageConfig::new(temp_dir.path().to_path_buf());
                let engine = StorageEngine::open(config).await.unwrap();

                // Create bucket
                let bucket_config = BucketConfig::new("etag-proptest-bucket");
                engine.create_bucket("etag-proptest-bucket", bucket_config).await.unwrap();

                // Put the same content under two different keys
                let key1 = "etag-test-key-1";
                let key2 = "etag-test-key-2";
                let metadata = BTreeMap::new();

                let manifest1 = engine
                    .put_object("etag-proptest-bucket", key1, &data, metadata.clone())
                    .await
                    .unwrap();

                let manifest2 = engine
                    .put_object("etag-proptest-bucket", key2, &data, metadata.clone())
                    .await
                    .unwrap();

                // Property: identical content should produce identical checksums/ETags
                prop_assert_eq!(
                    manifest1.checksum,
                    manifest2.checksum,
                    "Identical content should produce identical checksums (ETags)"
                );

                // Verify both checksums are correct by computing from original data
                let mut hasher = Sha256::new();
                hasher.update(&data);
                let computed_checksum = hasher.finalize();
                let computed_checksum_bytes: [u8; 32] = computed_checksum.into();

                prop_assert_eq!(
                    manifest1.checksum,
                    computed_checksum_bytes,
                    "First object checksum should match computed checksum"
                );

                prop_assert_eq!(
                    manifest2.checksum,
                    computed_checksum_bytes,
                    "Second object checksum should match computed checksum"
                );

                Ok(())
            })?;
        }

        #[test]
        fn proptest_list_objects_returns_all_put_objects(
            keys in prop::collection::vec(
                prop::string::string_regex("[a-zA-Z0-9][a-zA-Z0-9/_.-]{0,99}").unwrap(),
                1..20
            )
        ) {
            // Run the async test in a blocking context
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                let temp_dir = TempDir::new().unwrap();
                let config = StorageConfig::new(temp_dir.path().to_path_buf());
                let engine = StorageEngine::open(config).await.unwrap();

                // Create bucket
                let bucket_config = BucketConfig::new("list-proptest-bucket");
                engine.create_bucket("list-proptest-bucket", bucket_config).await.unwrap();

                // Deduplicate keys to ensure uniqueness
                let mut unique_keys: Vec<String> = keys.into_iter().collect::<std::collections::HashSet<_>>().into_iter().collect();
                unique_keys.sort(); // Sort for deterministic ordering

                // Put each object
                for key in &unique_keys {
                    let data = format!("data for {}", key).into_bytes();
                    let metadata = BTreeMap::new();
                    engine
                        .put_object("list-proptest-bucket", key, &data, metadata)
                        .await
                        .unwrap();
                }

                // List all objects (no prefix, large limit)
                let list_result = engine
                    .list_objects("list-proptest-bucket", "", 1000, None)
                    .await
                    .unwrap();

                // Property: number of listed objects should match number of put objects
                prop_assert_eq!(
                    list_result.objects.len(),
                    unique_keys.len(),
                    "list_objects should return same count as put objects"
                );

                // Extract keys from list result
                let mut listed_keys: Vec<String> = list_result
                    .objects
                    .iter()
                    .map(|obj| obj.key.key.clone())
                    .collect();
                listed_keys.sort();

                // Property: every put object key should appear in list_objects result
                prop_assert_eq!(
                    &listed_keys,
                    &unique_keys,
                    "list_objects should return all put object keys"
                );

                // Property: verify each object in list result exists and has correct bucket
                for obj in &list_result.objects {
                    prop_assert_eq!(
                        &obj.key.bucket,
                        "list-proptest-bucket",
                        "All objects should have correct bucket name"
                    );

                    prop_assert!(
                        unique_keys.contains(&obj.key.key),
                        "Listed key '{}' should be in the original put keys",
                        obj.key.key
                    );
                }

                Ok(())
            })?;
        }

        #[test]
        fn proptest_delete_then_get_returns_none(data in prop::collection::vec(any::<u8>(), 0..1024*1024)) {
            // Run the async test in a blocking context
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                let temp_dir = TempDir::new().unwrap();
                let config = StorageConfig::new(temp_dir.path().to_path_buf());
                let engine = StorageEngine::open(config).await.unwrap();

                // Create bucket
                let bucket_config = BucketConfig::new("delete-proptest-bucket");
                engine.create_bucket("delete-proptest-bucket", bucket_config).await.unwrap();

                // Put object with arbitrary data
                let key = "delete-proptest-key";
                let metadata = BTreeMap::new();
                engine
                    .put_object("delete-proptest-bucket", key, &data, metadata)
                    .await
                    .unwrap();

                // Verify object exists before deletion
                let get_before = engine
                    .get_object("delete-proptest-bucket", key)
                    .await
                    .unwrap();
                prop_assert!(get_before.is_some(), "Object should exist before deletion");

                // Delete the object
                let deleted = engine
                    .delete_object("delete-proptest-bucket", key)
                    .await
                    .unwrap();
                prop_assert!(deleted, "delete_object should return true for existing object");

                // Property: get_object should return None after deletion
                let get_after = engine
                    .get_object("delete-proptest-bucket", key)
                    .await
                    .unwrap();
                prop_assert!(get_after.is_none(), "get_object should return None after deletion");

                Ok(())
            })?;
        }

        #[test]
        fn proptest_concurrent_puts_to_different_keys_maintain_consistency(
            num_keys in 2usize..20,
            data_sizes in prop::collection::vec(0usize..10000, 2..20)
        ) {
            // Run the async test in a blocking context
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async move {
                let temp_dir = TempDir::new().unwrap();
                let config = StorageConfig::new(temp_dir.path().to_path_buf());
                let engine = Arc::new(StorageEngine::open(config).await.unwrap());

                // Create bucket
                let bucket_config = BucketConfig::new("concurrent-proptest-bucket");
                engine.create_bucket("concurrent-proptest-bucket", bucket_config).await.unwrap();

                // Generate deterministic data for each key based on the sizes
                let num_operations = num_keys.min(data_sizes.len());
                let mut expected_data = BTreeMap::new();

                for i in 0..num_operations {
                    let key = format!("concurrent-key-{}", i);
                    let size = data_sizes[i];

                    // Generate deterministic data based on index
                    let mut data = Vec::with_capacity(size);
                    for j in 0..size {
                        data.push(((i + j) % 256) as u8);
                    }

                    expected_data.insert(key, data);
                }

                // Spawn concurrent put operations
                let mut handles = Vec::new();
                for (key, data) in &expected_data {
                    let engine_clone = Arc::clone(&engine);
                    let key_clone = key.clone();
                    let data_clone = data.clone();

                    let handle = tokio::spawn(async move {
                        let metadata = BTreeMap::new();
                        engine_clone
                            .put_object("concurrent-proptest-bucket", &key_clone, &data_clone, metadata)
                            .await
                    });
                    handles.push(handle);
                }

                // Wait for all put operations to complete
                let results = futures::future::join_all(handles).await;

                // Property: all concurrent put operations should succeed
                for (i, result) in results.iter().enumerate() {
                    prop_assert!(
                        result.is_ok(),
                        "Task {} should complete without panicking",
                        i
                    );
                    prop_assert!(
                        result.as_ref().unwrap().is_ok(),
                        "Put operation {} should succeed",
                        i
                    );
                }

                // Property: all put objects should be retrievable with correct data
                for (key, expected) in &expected_data {
                    let get_result = engine
                        .get_object("concurrent-proptest-bucket", key)
                        .await
                        .unwrap();

                    prop_assert!(
                        get_result.is_some(),
                        "Object with key '{}' should exist after concurrent puts",
                        key
                    );

                    let (manifest, retrieved_data) = get_result.unwrap();

                    // Property: retrieved data should match original data (no corruption)
                    prop_assert_eq!(
                        &retrieved_data,
                        expected,
                        "Data for key '{}' should not be corrupted",
                        key
                    );

                    // Property: size should be correct
                    prop_assert_eq!(
                        retrieved_data.len(),
                        expected.len(),
                        "Size for key '{}' should match",
                        key
                    );

                    // Property: checksum should be valid
                    let mut hasher = Sha256::new();
                    hasher.update(&retrieved_data);
                    let computed_checksum = hasher.finalize();
                    let computed_checksum_bytes: [u8; 32] = computed_checksum.into();

                    prop_assert_eq!(
                        manifest.checksum,
                        computed_checksum_bytes,
                        "Checksum for key '{}' should be valid",
                        key
                    );
                }

                // Property: list_objects should return all objects
                let list_result = engine
                    .list_objects("concurrent-proptest-bucket", "", 1000, None)
                    .await
                    .unwrap();

                prop_assert_eq!(
                    list_result.objects.len(),
                    expected_data.len(),
                    "All concurrent put objects should be listed"
                );

                Ok(())
            })?;
        }
    }

    #[tokio::test]
    async fn test_quota_enforcement_on_put() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket with 10KB quota
        let config = BucketConfig::new("quota-bucket").with_quota(10240);
        engine.create_bucket("quota-bucket", config).await.unwrap();

        // First put should succeed (5KB)
        let data1 = vec![1u8; 5120];
        engine
            .put_object("quota-bucket", "file1", &data1, BTreeMap::new())
            .await
            .unwrap();

        // Second put should succeed (3KB more - total 8KB)
        let data2 = vec![2u8; 3072];
        engine
            .put_object("quota-bucket", "file2", &data2, BTreeMap::new())
            .await
            .unwrap();

        // Third put should fail (5KB would exceed 10KB quota)
        let data3 = vec![3u8; 5120];
        let result = engine
            .put_object("quota-bucket", "file3", &data3, BTreeMap::new())
            .await;

        assert!(matches!(result, Err(StorageError::QuotaExceeded { .. })));

        // Verify usage
        let usage = engine
            .get_bucket_usage("quota-bucket")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(usage.bytes_used, 8192); // 5120 + 3072
        assert_eq!(usage.object_count, 2);
    }

    #[tokio::test]
    async fn test_quota_with_object_replacement() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket with 10KB quota
        let config = BucketConfig::new("quota-bucket").with_quota(10240);
        engine.create_bucket("quota-bucket", config).await.unwrap();

        // Put initial object (8KB)
        let data1 = vec![1u8; 8192];
        engine
            .put_object("quota-bucket", "file1", &data1, BTreeMap::new())
            .await
            .unwrap();

        // Replace with smaller object should work (4KB)
        let data2 = vec![2u8; 4096];
        engine
            .put_object("quota-bucket", "file1", &data2, BTreeMap::new())
            .await
            .unwrap();

        let usage = engine
            .get_bucket_usage("quota-bucket")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(usage.bytes_used, 4096);
        assert_eq!(usage.object_count, 1); // Still just 1 object

        // Replace with larger object within quota should work (9KB)
        let data3 = vec![3u8; 9216];
        engine
            .put_object("quota-bucket", "file1", &data3, BTreeMap::new())
            .await
            .unwrap();

        let usage = engine
            .get_bucket_usage("quota-bucket")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(usage.bytes_used, 9216);

        // Replace with object that would exceed quota should fail (11KB)
        let data4 = vec![4u8; 11264];
        let result = engine
            .put_object("quota-bucket", "file1", &data4, BTreeMap::new())
            .await;

        assert!(matches!(result, Err(StorageError::QuotaExceeded { .. })));

        // Original object should still be there
        let usage = engine
            .get_bucket_usage("quota-bucket")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(usage.bytes_used, 9216);
    }

    #[tokio::test]
    async fn test_quota_with_delete() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket with 5KB quota
        let config = BucketConfig::new("quota-bucket").with_quota(5120);
        engine.create_bucket("quota-bucket", config).await.unwrap();

        // Fill up quota (5KB)
        let data1 = vec![1u8; 5120];
        engine
            .put_object("quota-bucket", "file1", &data1, BTreeMap::new())
            .await
            .unwrap();

        // Try to add more - should fail
        let data2 = vec![2u8; 1024];
        let result = engine
            .put_object("quota-bucket", "file2", &data2, BTreeMap::new())
            .await;
        assert!(matches!(result, Err(StorageError::QuotaExceeded { .. })));

        // Delete the first file
        engine.delete_object("quota-bucket", "file1").await.unwrap();

        // Now we should be able to add (usage is back to 0)
        engine
            .put_object("quota-bucket", "file2", &data2, BTreeMap::new())
            .await
            .unwrap();

        let usage = engine
            .get_bucket_usage("quota-bucket")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(usage.bytes_used, 1024);
        assert_eq!(usage.object_count, 1);
    }

    #[tokio::test]
    async fn test_quota_unlimited_bucket() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket without quota
        let config = BucketConfig::new("unlimited-bucket");
        engine
            .create_bucket("unlimited-bucket", config)
            .await
            .unwrap();

        // Should be able to put large objects without limit
        for i in 0..10 {
            let data = vec![i as u8; 1024 * 1024]; // 1MB each
            engine
                .put_object(
                    "unlimited-bucket",
                    &format!("file{}", i),
                    &data,
                    BTreeMap::new(),
                )
                .await
                .unwrap();
        }

        let usage = engine
            .get_bucket_usage("unlimited-bucket")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(usage.bytes_used, 10 * 1024 * 1024); // 10MB
        assert_eq!(usage.object_count, 10);
    }

    #[tokio::test]
    async fn test_set_quota_after_creation() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket without quota
        let config = BucketConfig::new("test-bucket");
        engine.create_bucket("test-bucket", config).await.unwrap();

        // Add some data
        let data = vec![1u8; 5000];
        engine
            .put_object("test-bucket", "file1", &data, BTreeMap::new())
            .await
            .unwrap();

        // Set quota below current usage
        engine
            .set_bucket_quota("test-bucket", Some(3000))
            .await
            .unwrap();

        // Should not be able to add more
        let data2 = vec![2u8; 1000];
        let result = engine
            .put_object("test-bucket", "file2", &data2, BTreeMap::new())
            .await;
        assert!(matches!(result, Err(StorageError::QuotaExceeded { .. })));

        // Increase quota
        engine
            .set_bucket_quota("test-bucket", Some(10000))
            .await
            .unwrap();

        // Now should be able to add
        engine
            .put_object("test-bucket", "file2", &data2, BTreeMap::new())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_remove_quota() {
        let (engine, _temp) = create_test_engine().await;

        // Create bucket with quota
        let config = BucketConfig::new("test-bucket").with_quota(5000);
        engine.create_bucket("test-bucket", config).await.unwrap();

        // Fill up to quota
        let data1 = vec![1u8; 5000];
        engine
            .put_object("test-bucket", "file1", &data1, BTreeMap::new())
            .await
            .unwrap();

        // Cannot add more
        let data2 = vec![2u8; 1000];
        let result = engine
            .put_object("test-bucket", "file2", &data2, BTreeMap::new())
            .await;
        assert!(matches!(result, Err(StorageError::QuotaExceeded { .. })));

        // Remove quota
        engine.set_bucket_quota("test-bucket", None).await.unwrap();

        // Now should be able to add
        engine
            .put_object("test-bucket", "file2", &data2, BTreeMap::new())
            .await
            .unwrap();

        let usage = engine
            .get_bucket_usage("test-bucket")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(usage.bytes_used, 6000);
    }

    #[tokio::test]
    async fn test_usage_tracking_accuracy() {
        let (engine, _temp) = create_test_engine().await;

        let config = BucketConfig::new("test-bucket");
        engine.create_bucket("test-bucket", config).await.unwrap();

        // Track expected usage manually
        let mut expected_usage = 0u64;
        let mut expected_count = 0u64;

        // Add various sized objects
        let sizes = vec![100, 500, 1000, 250, 750];
        for (i, size) in sizes.iter().enumerate() {
            let data = vec![i as u8; *size];
            engine
                .put_object("test-bucket", &format!("file{}", i), &data, BTreeMap::new())
                .await
                .unwrap();
            expected_usage += *size as u64;
            expected_count += 1;
        }

        let usage = engine
            .get_bucket_usage("test-bucket")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(usage.bytes_used, expected_usage);
        assert_eq!(usage.object_count, expected_count);

        // Delete some objects
        for i in 0..3 {
            engine
                .delete_object("test-bucket", &format!("file{}", i))
                .await
                .unwrap();
            expected_usage -= sizes[i] as u64;
            expected_count -= 1;

            let usage = engine
                .get_bucket_usage("test-bucket")
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                usage.bytes_used, expected_usage,
                "Usage mismatch after deleting file{}",
                i
            );
            assert_eq!(
                usage.object_count, expected_count,
                "Count mismatch after deleting file{}",
                i
            );
        }

        // Replace remaining objects
        for i in 3..sizes.len() {
            let old_size = sizes[i];
            let new_size = old_size * 2;
            let data = vec![(i + 100) as u8; new_size];

            engine
                .put_object("test-bucket", &format!("file{}", i), &data, BTreeMap::new())
                .await
                .unwrap();

            expected_usage = expected_usage - old_size as u64 + new_size as u64;
            // Count stays the same for replacement

            let usage = engine
                .get_bucket_usage("test-bucket")
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                usage.bytes_used, expected_usage,
                "Usage mismatch after replacing file{}",
                i
            );
            assert_eq!(
                usage.object_count, expected_count,
                "Count mismatch after replacing file{}",
                i
            );
        }
    }
}

/// Generates a unique version ID for object versioning.
///
/// The version ID is constructed from:
/// - 8 bytes: millisecond timestamp (for time ordering)
/// - 12 bytes: random data (for uniqueness)
///
/// This 20-byte value is then encoded as base64url, producing a 27-character string.
/// Version IDs are lexicographically sortable by creation time.
fn generate_version_id() -> String {
    // Get current timestamp in milliseconds
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before UNIX epoch")
        .as_millis() as u64;

    // Create a 20-byte buffer: 8 bytes timestamp + 12 bytes random
    let mut bytes = [0u8; 20];

    // First 8 bytes: big-endian timestamp for lexicographic sorting
    bytes[0..8].copy_from_slice(&timestamp.to_be_bytes());

    // Last 12 bytes: random data for uniqueness
    let random_bytes = uuid::Uuid::new_v4().as_bytes()[0..12].to_vec();
    bytes[8..20].copy_from_slice(&random_bytes);

    // Encode as base64url (no padding) to get a 27-character string
    URL_SAFE_NO_PAD.encode(&bytes)
}
