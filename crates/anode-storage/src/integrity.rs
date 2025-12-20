use crate::blobs::BlobStore;
use crate::chunks::ChunkId;
use crate::error::{Result, StorageError};
use crate::metadata::MetadataStore;
use anode_protocol::{ChunkId as ProtocolChunkId, ObjectKey};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Result of integrity verification
#[derive(Debug, Clone)]
pub struct IntegrityReport {
    pub total_objects: usize,
    pub total_chunks: usize,
    pub corrupted_chunks: Vec<CorruptedChunk>,
    pub missing_chunks: Vec<MissingChunk>,
    pub orphaned_chunks: Vec<ChunkId>,
    pub reference_count_errors: Vec<RefCountError>,
    pub verification_time: Duration,
}

impl IntegrityReport {
    /// Returns true if no issues were found
    pub fn is_healthy(&self) -> bool {
        self.corrupted_chunks.is_empty()
            && self.missing_chunks.is_empty()
            && self.orphaned_chunks.is_empty()
            && self.reference_count_errors.is_empty()
    }

    /// Returns total number of issues found
    pub fn total_issues(&self) -> usize {
        self.corrupted_chunks.len()
            + self.missing_chunks.len()
            + self.orphaned_chunks.len()
            + self.reference_count_errors.len()
    }
}

#[derive(Debug, Clone)]
pub struct CorruptedChunk {
    pub chunk_id: ChunkId,
    pub expected_hash: [u8; 32],
    pub actual_hash: [u8; 32],
    pub affected_objects: Vec<ObjectKey>,
}

#[derive(Debug, Clone)]
pub struct MissingChunk {
    pub chunk_id: ChunkId,
    pub affected_objects: Vec<ObjectKey>,
    pub available_replicas: Vec<anode_protocol::NodeId>,
}

#[derive(Debug, Clone)]
pub struct RefCountError {
    pub chunk_id: ChunkId,
    pub stored_count: u32,
    pub actual_count: u32,
}

pub struct IntegrityChecker {
    metadata: Arc<MetadataStore>,
    blobs: Arc<BlobStore>,
}

impl IntegrityChecker {
    pub fn new(metadata: Arc<MetadataStore>, blobs: Arc<BlobStore>) -> Self {
        Self { metadata, blobs }
    }

    /// Full integrity scan of all data
    pub async fn full_scan(&self) -> Result<IntegrityReport> {
        info!("Starting full integrity scan");
        let start = Instant::now();

        let mut report = IntegrityReport {
            total_objects: 0,
            total_chunks: 0,
            corrupted_chunks: Vec::new(),
            missing_chunks: Vec::new(),
            orphaned_chunks: Vec::new(),
            reference_count_errors: Vec::new(),
            verification_time: Duration::default(),
        };

        // Build a map of which objects reference which chunks
        let chunk_to_objects = self.build_chunk_reference_map()?;
        report.total_objects = chunk_to_objects
            .values()
            .flat_map(|v| v.iter())
            .collect::<HashSet<_>>()
            .len();

        // Get all chunks from metadata
        let metadata_chunks = self.metadata.list_all_chunks()?;
        report.total_chunks = metadata_chunks.len();

        debug!(
            "Scanning {} chunks referenced by {} objects",
            report.total_chunks, report.total_objects
        );

        // Verify each chunk
        for chunk_id in &metadata_chunks {
            // Check if chunk exists in blob store
            let exists = self.blobs.chunk_exists(chunk_id)?;

            if !exists {
                let affected_objects = chunk_to_objects
                    .get(chunk_id)
                    .cloned()
                    .unwrap_or_default();

                report.missing_chunks.push(MissingChunk {
                    chunk_id: chunk_id.clone(),
                    affected_objects,
                    available_replicas: Vec::new(), // Would be populated by cluster query
                });
                continue;
            }

            // Verify chunk integrity (hash matches)
            match self.verify_chunk(chunk_id) {
                Ok(true) => {
                    // Chunk is valid
                }
                Ok(false) => {
                    // Chunk is corrupted
                    let data = self.blobs.read_chunk(chunk_id)?;
                    let mut hasher = Sha256::new();
                    hasher.update(&data);
                    let actual_hash_bytes = hasher.finalize();
                    let mut actual_hash = [0u8; 32];
                    actual_hash.copy_from_slice(&actual_hash_bytes);

                    let expected_hash = hex::decode(&chunk_id.0)
                        .ok()
                        .and_then(|v| {
                            if v.len() == 32 {
                                let mut arr = [0u8; 32];
                                arr.copy_from_slice(&v);
                                Some(arr)
                            } else {
                                None
                            }
                        })
                        .unwrap_or([0u8; 32]);

                    let affected_objects = chunk_to_objects
                        .get(chunk_id)
                        .cloned()
                        .unwrap_or_default();

                    report.corrupted_chunks.push(CorruptedChunk {
                        chunk_id: chunk_id.clone(),
                        expected_hash,
                        actual_hash,
                        affected_objects,
                    });
                }
                Err(e) => {
                    warn!("Error verifying chunk {}: {}", chunk_id, e);
                    // Treat as corrupted
                    let affected_objects = chunk_to_objects
                        .get(chunk_id)
                        .cloned()
                        .unwrap_or_default();

                    report.corrupted_chunks.push(CorruptedChunk {
                        chunk_id: chunk_id.clone(),
                        expected_hash: [0u8; 32],
                        actual_hash: [0u8; 32],
                        affected_objects,
                    });
                }
            }
        }

        // Find orphaned chunks
        report.orphaned_chunks = self.find_orphaned_chunks().await?;

        // Verify reference counts
        report.reference_count_errors = self.verify_reference_counts().await?;

        report.verification_time = start.elapsed();

        info!(
            "Integrity scan complete: {} issues found in {:?}",
            report.total_issues(),
            report.verification_time
        );

        Ok(report)
    }

    /// Verify a single object
    pub async fn verify_object(&self, key: &ObjectKey) -> Result<bool> {
        debug!("Verifying object: {}", key);

        // Get object manifest
        let manifest = self
            .metadata
            .get_object(&key.bucket, &key.key)?
            .ok_or_else(|| {
                StorageError::ObjectNotFound {
                    bucket: key.bucket.clone(),
                    key: key.key.clone(),
                }
            })?;

        // Convert protocol ChunkIds to storage ChunkIds
        let chunk_ids: Vec<ChunkId> = manifest
            .chunks
            .iter()
            .map(|c| ChunkId(c.to_hex()))
            .collect();

        // Verify all chunks exist and are valid
        for chunk_id in &chunk_ids {
            if !self.blobs.chunk_exists(chunk_id)? {
                warn!("Missing chunk {} for object {}", chunk_id, key);
                return Ok(false);
            }

            if !self.verify_chunk(chunk_id)? {
                warn!("Corrupted chunk {} for object {}", chunk_id, key);
                return Ok(false);
            }
        }

        debug!("Object {} verified successfully", key);
        Ok(true)
    }

    /// Verify a single chunk
    pub fn verify_chunk(&self, chunk_id: &ChunkId) -> Result<bool> {
        debug!("Verifying chunk: {}", chunk_id);

        // Read chunk data
        let data = self.blobs.read_chunk(chunk_id)?;

        // Compute hash
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let computed_hash = hex::encode(hasher.finalize());

        // Compare with chunk ID
        if computed_hash != chunk_id.0 {
            warn!(
                "Chunk verification failed for {}: expected {}, got {}",
                chunk_id, chunk_id.0, computed_hash
            );
            return Ok(false);
        }

        Ok(true)
    }

    /// Scan for orphaned chunks (not referenced by any object)
    pub async fn find_orphaned_chunks(&self) -> Result<Vec<ChunkId>> {
        debug!("Scanning for orphaned chunks");

        // Get all chunks in blob store
        let blob_chunks = self.blobs.list_chunks()?;

        // Get all chunks referenced in metadata
        let metadata_chunks: HashSet<ChunkId> =
            self.metadata.list_all_chunks()?.into_iter().collect();

        // Find chunks in blob store not in metadata
        let mut orphaned = Vec::new();
        for chunk_id in blob_chunks {
            if !metadata_chunks.contains(&chunk_id) {
                // Double-check reference count
                let ref_count = self.metadata.get_chunk_ref_count(&chunk_id)?;
                if ref_count == 0 {
                    orphaned.push(chunk_id);
                }
            }
        }

        info!("Found {} orphaned chunks", orphaned.len());
        Ok(orphaned)
    }

    /// Verify reference counts
    pub async fn verify_reference_counts(&self) -> Result<Vec<RefCountError>> {
        debug!("Verifying reference counts");

        let mut errors = Vec::new();

        // Build actual reference counts by scanning all objects
        let chunk_refs = self.build_chunk_reference_map()?;
        let actual_counts: HashMap<ChunkId, u32> = chunk_refs
            .iter()
            .map(|(chunk_id, objects)| (chunk_id.clone(), objects.len() as u32))
            .collect();

        // Compare with stored reference counts
        let metadata_chunks = self.metadata.list_all_chunks()?;

        for chunk_id in metadata_chunks {
            let stored_count = self.metadata.get_chunk_ref_count(&chunk_id)?;
            let actual_count = *actual_counts.get(&chunk_id).unwrap_or(&0);

            if stored_count != actual_count {
                warn!(
                    "Reference count mismatch for chunk {}: stored={}, actual={}",
                    chunk_id, stored_count, actual_count
                );

                errors.push(RefCountError {
                    chunk_id,
                    stored_count,
                    actual_count,
                });
            }
        }

        info!("Found {} reference count errors", errors.len());
        Ok(errors)
    }

    /// Build a map of chunk_id -> Vec<ObjectKey> showing which objects reference each chunk
    fn build_chunk_reference_map(&self) -> Result<HashMap<ChunkId, Vec<ObjectKey>>> {
        let mut chunk_to_objects: HashMap<ChunkId, Vec<ObjectKey>> = HashMap::new();

        // Get all buckets
        let buckets = self.metadata.list_buckets()?;

        for bucket in buckets {
            // List all objects in bucket
            let objects = self.metadata.list_objects(&bucket.name, "", usize::MAX)?;

            for manifest in objects {
                let object_key = manifest.object_key.clone();

                // Convert protocol ChunkIds to storage ChunkIds
                for chunk_id in &manifest.chunks {
                    let storage_chunk_id = ChunkId(chunk_id.to_hex());
                    chunk_to_objects
                        .entry(storage_chunk_id)
                        .or_insert_with(Vec::new)
                        .push(object_key.clone());
                }
            }
        }

        Ok(chunk_to_objects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs::BlobStore;
    use crate::chunks::compute_chunk_id;
    use crate::metadata::{BucketConfig, MetadataStore};
    use anode_protocol::ObjectManifest as ProtocolManifest;
    use tempfile::TempDir;

    fn setup_test_env() -> (IntegrityChecker, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let blob_dir = temp_dir.path().join("blobs");
        let metadata_path = temp_dir.path().join("metadata.db");

        let blobs = Arc::new(BlobStore::new(blob_dir).unwrap());
        let metadata = Arc::new(MetadataStore::open(&metadata_path).unwrap());

        let checker = IntegrityChecker::new(metadata, blobs);
        (checker, temp_dir)
    }

    #[tokio::test]
    async fn test_verify_chunk_valid() {
        let (checker, _temp) = setup_test_env();

        let data = b"Hello, world!";
        let chunk_id = compute_chunk_id(data);

        checker.blobs.write_chunk(&chunk_id, data).unwrap();

        let result = checker.verify_chunk(&chunk_id).unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_verify_chunk_corrupted() {
        let (checker, _temp) = setup_test_env();

        let original_data = b"Original data";
        let chunk_id = compute_chunk_id(original_data);

        // Write different data with same chunk ID
        let corrupted_data = b"Corrupted!!!";
        checker.blobs.write_chunk(&chunk_id, corrupted_data).unwrap();

        let result = checker.verify_chunk(&chunk_id).unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_verify_object() {
        let (checker, _temp) = setup_test_env();

        // Create bucket
        let bucket_config = BucketConfig::new("test-bucket");
        checker.metadata.create_bucket(&bucket_config).unwrap();

        // Store chunk
        let data = b"Test data for object";
        let chunk_id = compute_chunk_id(data);
        checker.blobs.write_chunk(&chunk_id, data).unwrap();
        checker.metadata.increment_chunk_ref(&chunk_id).unwrap();

        // Create object manifest
        let object_key = ObjectKey::new("test-bucket", "test-key");
        let protocol_chunk_id = ProtocolChunkId::from_hex(&chunk_id.0).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(data);
        let checksum_bytes = hasher.finalize();
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&checksum_bytes);

        let manifest = ProtocolManifest::new(
            object_key.clone(),
            data.len() as u64,
            checksum,
            vec![protocol_chunk_id],
        );

        checker.metadata.put_object(&manifest).unwrap();

        // Verify object
        let result = checker.verify_object(&object_key).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_find_orphaned_chunks() {
        let (checker, _temp) = setup_test_env();

        // Write chunk without metadata reference
        let data = b"Orphaned chunk";
        let chunk_id = compute_chunk_id(data);
        checker.blobs.write_chunk(&chunk_id, data).unwrap();

        let orphaned = checker.find_orphaned_chunks().await.unwrap();
        assert_eq!(orphaned.len(), 1);
        assert_eq!(orphaned[0], chunk_id);
    }

    #[tokio::test]
    async fn test_verify_reference_counts() {
        let (checker, _temp) = setup_test_env();

        // Create bucket
        let bucket_config = BucketConfig::new("test-bucket");
        checker.metadata.create_bucket(&bucket_config).unwrap();

        // Store chunk and set incorrect ref count
        let data = b"Test chunk";
        let chunk_id = compute_chunk_id(data);
        checker.blobs.write_chunk(&chunk_id, data).unwrap();

        // Set ref count to 5 in metadata
        for _ in 0..5 {
            checker.metadata.increment_chunk_ref(&chunk_id).unwrap();
        }

        // But only reference it in 2 objects
        for i in 0..2 {
            let object_key = ObjectKey::new("test-bucket", format!("key-{}", i));
            let protocol_chunk_id = ProtocolChunkId::from_hex(&chunk_id.0).unwrap();

            let mut hasher = Sha256::new();
            hasher.update(data);
            let checksum_bytes = hasher.finalize();
            let mut checksum = [0u8; 32];
            checksum.copy_from_slice(&checksum_bytes);

            let manifest = ProtocolManifest::new(
                object_key,
                data.len() as u64,
                checksum,
                vec![protocol_chunk_id],
            );

            checker.metadata.put_object(&manifest).unwrap();
        }

        let errors = checker.verify_reference_counts().await.unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].stored_count, 5);
        assert_eq!(errors[0].actual_count, 2);
    }

    #[tokio::test]
    async fn test_full_scan_healthy() {
        let (checker, _temp) = setup_test_env();

        // Create bucket
        let bucket_config = BucketConfig::new("test-bucket");
        checker.metadata.create_bucket(&bucket_config).unwrap();

        // Store valid chunk
        let data = b"Valid chunk data";
        let chunk_id = compute_chunk_id(data);
        checker.blobs.write_chunk(&chunk_id, data).unwrap();
        checker.metadata.increment_chunk_ref(&chunk_id).unwrap();

        // Create object
        let object_key = ObjectKey::new("test-bucket", "test-key");
        let protocol_chunk_id = ProtocolChunkId::from_hex(&chunk_id.0).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(data);
        let checksum_bytes = hasher.finalize();
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&checksum_bytes);

        let manifest = ProtocolManifest::new(
            object_key,
            data.len() as u64,
            checksum,
            vec![protocol_chunk_id],
        );

        checker.metadata.put_object(&manifest).unwrap();

        // Run full scan
        let report = checker.full_scan().await.unwrap();

        assert!(report.is_healthy());
        assert_eq!(report.total_issues(), 0);
        assert_eq!(report.total_objects, 1);
        assert_eq!(report.total_chunks, 1);
    }

    #[tokio::test]
    async fn test_full_scan_with_corruption() {
        let (checker, _temp) = setup_test_env();

        // Create bucket
        let bucket_config = BucketConfig::new("test-bucket");
        checker.metadata.create_bucket(&bucket_config).unwrap();

        // Store corrupted chunk
        let original_data = b"Original data";
        let chunk_id = compute_chunk_id(original_data);

        // Write different data
        checker
            .blobs
            .write_chunk(&chunk_id, b"Corrupted!")
            .unwrap();
        checker.metadata.increment_chunk_ref(&chunk_id).unwrap();

        // Create object
        let object_key = ObjectKey::new("test-bucket", "test-key");
        let protocol_chunk_id = ProtocolChunkId::from_hex(&chunk_id.0).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(original_data);
        let checksum_bytes = hasher.finalize();
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&checksum_bytes);

        let manifest = ProtocolManifest::new(
            object_key,
            original_data.len() as u64,
            checksum,
            vec![protocol_chunk_id],
        );

        checker.metadata.put_object(&manifest).unwrap();

        // Run full scan
        let report = checker.full_scan().await.unwrap();

        assert!(!report.is_healthy());
        assert_eq!(report.corrupted_chunks.len(), 1);
        assert_eq!(report.total_chunks, 1);
    }
}
