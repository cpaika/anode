use crate::blobs::BlobStore;
use crate::chunks::ChunkId;
use crate::error::{Result, StorageError};
use crate::integrity::{CorruptedChunk, IntegrityReport, MissingChunk, RefCountError};
use crate::metadata::MetadataStore;
use anode_protocol::NodeId;
use sha2::Digest;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Placeholder cluster client for fetching chunks from other nodes
/// In production, this would be the actual cluster client that can communicate with other nodes
pub trait ClusterClient: Send + Sync {
    /// Fetch a chunk from a specific node
    fn fetch_chunk_from_node(&self, chunk_id: &ChunkId, node_id: NodeId)
        -> Result<Option<Vec<u8>>>;

    /// Find which nodes have a copy of a chunk
    fn find_chunk_replicas(&self, chunk_id: &ChunkId) -> Result<Vec<NodeId>>;
}

/// Mock cluster client for testing
pub struct MockClusterClient {
    // In tests, we can populate this with test data
    available_chunks: std::collections::BTreeMap<ChunkId, Vec<u8>>,
}

impl MockClusterClient {
    pub fn new() -> Self {
        Self {
            available_chunks: std::collections::BTreeMap::new(),
        }
    }

    pub fn add_chunk(&mut self, chunk_id: ChunkId, data: Vec<u8>) {
        self.available_chunks.insert(chunk_id, data);
    }
}

impl Default for MockClusterClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ClusterClient for MockClusterClient {
    fn fetch_chunk_from_node(
        &self,
        chunk_id: &ChunkId,
        _node_id: NodeId,
    ) -> Result<Option<Vec<u8>>> {
        Ok(self.available_chunks.get(chunk_id).cloned())
    }

    fn find_chunk_replicas(&self, chunk_id: &ChunkId) -> Result<Vec<NodeId>> {
        if self.available_chunks.contains_key(chunk_id) {
            Ok(vec![NodeId(1), NodeId(2)]) // Mock replicas
        } else {
            Ok(vec![])
        }
    }
}

/// Repair operations for corrupted or missing data
pub struct RepairManager<C: ClusterClient> {
    metadata: Arc<MetadataStore>,
    blobs: Arc<BlobStore>,
    cluster: Arc<C>,
}

#[derive(Debug, Clone)]
pub struct RepairResult {
    pub chunks_repaired: usize,
    pub chunks_failed: usize,
    pub bytes_transferred: u64,
    pub repair_time: Duration,
    pub errors: Vec<RepairError>,
}

impl RepairResult {
    pub fn new() -> Self {
        Self {
            chunks_repaired: 0,
            chunks_failed: 0,
            bytes_transferred: 0,
            repair_time: Duration::default(),
            errors: Vec::new(),
        }
    }

    pub fn success_rate(&self) -> f64 {
        let total = self.chunks_repaired + self.chunks_failed;
        if total == 0 {
            1.0
        } else {
            self.chunks_repaired as f64 / total as f64
        }
    }
}

impl Default for RepairResult {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum RepairError {
    NoReplicaAvailable(ChunkId),
    TransferFailed { chunk_id: ChunkId, error: String },
    VerificationFailed(ChunkId),
    MetadataUpdateFailed { chunk_id: ChunkId, error: String },
}

impl<C: ClusterClient> RepairManager<C> {
    pub fn new(metadata: Arc<MetadataStore>, blobs: Arc<BlobStore>, cluster: Arc<C>) -> Self {
        Self {
            metadata,
            blobs,
            cluster,
        }
    }

    /// Repair all issues found in integrity report
    pub async fn repair(&self, report: &IntegrityReport) -> Result<RepairResult> {
        info!("Starting repair for {} issues", report.total_issues());
        let start = Instant::now();

        let mut result = RepairResult::new();

        // Repair corrupted chunks
        for corrupted in &report.corrupted_chunks {
            debug!("Repairing corrupted chunk: {}", corrupted.chunk_id);

            match self.repair_corrupted_chunk(corrupted).await {
                Ok(bytes) => {
                    result.chunks_repaired += 1;
                    result.bytes_transferred += bytes;
                    info!("Successfully repaired chunk: {}", corrupted.chunk_id);
                },
                Err(e) => {
                    result.chunks_failed += 1;
                    warn!("Failed to repair chunk {}: {}", corrupted.chunk_id, e);
                    result.errors.push(RepairError::TransferFailed {
                        chunk_id: corrupted.chunk_id.clone(),
                        error: e.to_string(),
                    });
                },
            }
        }

        // Rebuild missing chunks
        for missing in &report.missing_chunks {
            debug!("Rebuilding missing chunk: {}", missing.chunk_id);

            match self.rebuild_chunk(missing).await {
                Ok(bytes) => {
                    result.chunks_repaired += 1;
                    result.bytes_transferred += bytes;
                    info!("Successfully rebuilt chunk: {}", missing.chunk_id);
                },
                Err(e) => {
                    result.chunks_failed += 1;
                    warn!("Failed to rebuild chunk {}: {}", missing.chunk_id, e);
                    result.errors.push(RepairError::TransferFailed {
                        chunk_id: missing.chunk_id.clone(),
                        error: e.to_string(),
                    });
                },
            }
        }

        // Fix reference count errors
        if !report.reference_count_errors.is_empty() {
            match self
                .fix_reference_counts(&report.reference_count_errors)
                .await
            {
                Ok(_) => {
                    info!(
                        "Fixed {} reference count errors",
                        report.reference_count_errors.len()
                    );
                },
                Err(e) => {
                    warn!("Failed to fix some reference counts: {}", e);
                },
            }
        }

        // Cleanup orphaned chunks
        if !report.orphaned_chunks.is_empty() {
            match self.cleanup_orphans(&report.orphaned_chunks).await {
                Ok(cleaned) => {
                    info!("Cleaned up {} orphaned chunks", cleaned);
                },
                Err(e) => {
                    warn!("Failed to cleanup some orphaned chunks: {}", e);
                },
            }
        }

        result.repair_time = start.elapsed();

        info!(
            "Repair complete: {}/{} chunks repaired, {} bytes transferred in {:?}",
            result.chunks_repaired,
            result.chunks_repaired + result.chunks_failed,
            result.bytes_transferred,
            result.repair_time
        );

        Ok(result)
    }

    /// Repair a single corrupted chunk by fetching from replica
    async fn repair_corrupted_chunk(&self, corrupted: &CorruptedChunk) -> Result<u64> {
        debug!(
            "Attempting to repair corrupted chunk: {}",
            corrupted.chunk_id
        );

        // Find available replicas
        let replicas = self.cluster.find_chunk_replicas(&corrupted.chunk_id)?;

        if replicas.is_empty() {
            return Err(StorageError::NotFound(format!(
                "No replicas found for chunk {}",
                corrupted.chunk_id
            )));
        }

        // Try each replica until we get a valid chunk
        for node_id in replicas {
            debug!(
                "Fetching chunk {} from node {}",
                corrupted.chunk_id, node_id
            );

            match self
                .cluster
                .fetch_chunk_from_node(&corrupted.chunk_id, node_id)
            {
                Ok(Some(data)) => {
                    // Verify the fetched chunk
                    let computed_hash = hex::encode(sha2::Sha256::digest(&data));

                    if computed_hash == corrupted.chunk_id.0 {
                        // Valid chunk, write it
                        let size = data.len() as u64;
                        self.blobs.write_chunk(&corrupted.chunk_id, &data)?;
                        info!(
                            "Successfully repaired chunk {} from node {}",
                            corrupted.chunk_id, node_id
                        );
                        return Ok(size);
                    } else {
                        warn!("Fetched chunk from node {} has invalid hash", node_id);
                    }
                },
                Ok(None) => {
                    debug!(
                        "Node {} does not have chunk {}",
                        node_id, corrupted.chunk_id
                    );
                },
                Err(e) => {
                    warn!("Error fetching chunk from node {}: {}", node_id, e);
                },
            }
        }

        Err(StorageError::NotFound(format!(
            "Could not find valid replica for chunk {}",
            corrupted.chunk_id
        )))
    }

    /// Rebuild missing chunk from replicas
    pub async fn rebuild_chunk(&self, missing: &MissingChunk) -> Result<u64> {
        debug!("Attempting to rebuild missing chunk: {}", missing.chunk_id);

        // Use available replicas if provided, otherwise query cluster
        let replicas = if !missing.available_replicas.is_empty() {
            missing.available_replicas.clone()
        } else {
            self.cluster.find_chunk_replicas(&missing.chunk_id)?
        };

        if replicas.is_empty() {
            return Err(StorageError::NotFound(format!(
                "No replicas found for missing chunk {}",
                missing.chunk_id
            )));
        }

        // Try each replica
        for node_id in replicas {
            debug!(
                "Fetching missing chunk {} from node {}",
                missing.chunk_id, node_id
            );

            match self
                .cluster
                .fetch_chunk_from_node(&missing.chunk_id, node_id)
            {
                Ok(Some(data)) => {
                    // Verify the chunk
                    let computed_hash = hex::encode(sha2::Sha256::digest(&data));

                    if computed_hash == missing.chunk_id.0 {
                        // Valid chunk, write it
                        let size = data.len() as u64;
                        self.blobs.write_chunk(&missing.chunk_id, &data)?;
                        info!(
                            "Successfully rebuilt chunk {} from node {}",
                            missing.chunk_id, node_id
                        );
                        return Ok(size);
                    } else {
                        warn!("Fetched chunk from node {} has invalid hash", node_id);
                    }
                },
                Ok(None) => {
                    debug!("Node {} does not have chunk {}", node_id, missing.chunk_id);
                },
                Err(e) => {
                    warn!("Error fetching chunk from node {}: {}", node_id, e);
                },
            }
        }

        Err(StorageError::NotFound(format!(
            "Could not find valid replica for missing chunk {}",
            missing.chunk_id
        )))
    }

    /// Fix reference count errors
    pub async fn fix_reference_counts(&self, errors: &[RefCountError]) -> Result<()> {
        debug!("Fixing {} reference count errors", errors.len());

        for error in errors {
            debug!(
                "Fixing ref count for chunk {}: stored={}, actual={}",
                error.chunk_id, error.stored_count, error.actual_count
            );

            // We need to adjust the stored count to match actual count
            // This is done by carefully incrementing or decrementing

            if error.actual_count > error.stored_count {
                // Need to increment
                let diff = error.actual_count - error.stored_count;
                for _ in 0..diff {
                    self.metadata.increment_chunk_ref(&error.chunk_id)?;
                }
            } else if error.stored_count > error.actual_count {
                // Need to decrement
                let diff = error.stored_count - error.actual_count;

                // Special handling: if actual count is 0, we should just remove metadata
                if error.actual_count == 0 {
                    for _ in 0..diff {
                        // Decrement will remove when reaching 0
                        match self.metadata.decrement_chunk_ref(&error.chunk_id) {
                            Ok(_) => {},
                            Err(e) => {
                                warn!("Error decrementing ref count for {}: {}", error.chunk_id, e);
                                break;
                            },
                        }
                    }
                } else {
                    for _ in 0..diff {
                        match self.metadata.decrement_chunk_ref(&error.chunk_id) {
                            Ok(_) => {},
                            Err(e) => {
                                warn!("Error decrementing ref count for {}: {}", error.chunk_id, e);
                                break;
                            },
                        }
                    }
                }
            }

            info!(
                "Fixed ref count for chunk {}: {} -> {}",
                error.chunk_id, error.stored_count, error.actual_count
            );
        }

        Ok(())
    }

    /// Remove orphaned chunks
    pub async fn cleanup_orphans(&self, orphans: &[ChunkId]) -> Result<usize> {
        debug!("Cleaning up {} orphaned chunks", orphans.len());

        let mut cleaned = 0;

        for chunk_id in orphans {
            debug!("Deleting orphaned chunk: {}", chunk_id);

            match self.blobs.delete_chunk(chunk_id) {
                Ok(true) => {
                    cleaned += 1;
                    info!("Deleted orphaned chunk: {}", chunk_id);
                },
                Ok(false) => {
                    debug!("Orphaned chunk {} already deleted", chunk_id);
                },
                Err(e) => {
                    warn!("Failed to delete orphaned chunk {}: {}", chunk_id, e);
                },
            }
        }

        info!("Cleaned up {} orphaned chunks", cleaned);
        Ok(cleaned)
    }

    /// Repair a single chunk by ID
    pub async fn repair_chunk(&self, chunk_id: &ChunkId) -> Result<()> {
        debug!("Repairing single chunk: {}", chunk_id);

        // Check if chunk exists
        let exists = self.blobs.chunk_exists(chunk_id)?;

        if !exists {
            // Missing chunk - rebuild it
            let missing = MissingChunk {
                chunk_id: chunk_id.clone(),
                affected_objects: Vec::new(),
                available_replicas: Vec::new(),
            };

            self.rebuild_chunk(&missing).await?;
        } else {
            // Verify chunk integrity
            let data = self.blobs.read_chunk(chunk_id)?;
            let computed_hash = hex::encode(sha2::Sha256::digest(&data));

            if computed_hash != chunk_id.0 {
                // Corrupted chunk - repair it
                let mut expected_hash = [0u8; 32];
                if let Ok(bytes) = hex::decode(&chunk_id.0) {
                    if bytes.len() == 32 {
                        expected_hash.copy_from_slice(&bytes);
                    }
                }

                let mut actual_hash = [0u8; 32];
                if let Ok(bytes) = hex::decode(&computed_hash) {
                    if bytes.len() == 32 {
                        actual_hash.copy_from_slice(&bytes);
                    }
                }

                let corrupted = CorruptedChunk {
                    chunk_id: chunk_id.clone(),
                    expected_hash,
                    actual_hash,
                    affected_objects: Vec::new(),
                };

                self.repair_corrupted_chunk(&corrupted).await?;
            } else {
                debug!("Chunk {} is valid, no repair needed", chunk_id);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs::BlobStore;
    use crate::chunks::compute_chunk_id;
    use crate::integrity::IntegrityChecker;
    use crate::metadata::{BucketConfig, MetadataStore};
    use anode_protocol::{
        ChunkId as ProtocolChunkId, ObjectKey, ObjectManifest as ProtocolManifest,
    };
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    fn setup_test_env() -> (
        RepairManager<MockClusterClient>,
        Arc<MetadataStore>,
        Arc<BlobStore>,
        TempDir,
    ) {
        let temp_dir = TempDir::new().unwrap();
        let blob_dir = temp_dir.path().join("blobs");
        let metadata_path = temp_dir.path().join("metadata.db");

        let blobs = Arc::new(BlobStore::new(blob_dir).unwrap());
        let metadata = Arc::new(MetadataStore::open(&metadata_path).unwrap());
        let cluster = Arc::new(MockClusterClient::new());

        let repair_manager = RepairManager::new(metadata.clone(), blobs.clone(), cluster);
        (repair_manager, metadata, blobs, temp_dir)
    }

    #[tokio::test]
    async fn test_cleanup_orphans() {
        let (repair_manager, _metadata, blobs, _temp) = setup_test_env();

        // Create orphaned chunks
        let data1 = b"Orphaned chunk 1";
        let data2 = b"Orphaned chunk 2";

        let chunk_id1 = compute_chunk_id(data1);
        let chunk_id2 = compute_chunk_id(data2);

        blobs.write_chunk(&chunk_id1, data1).unwrap();
        blobs.write_chunk(&chunk_id2, data2).unwrap();

        // Cleanup
        let orphans = vec![chunk_id1.clone(), chunk_id2.clone()];
        let cleaned = repair_manager.cleanup_orphans(&orphans).await.unwrap();

        assert_eq!(cleaned, 2);
        assert!(!blobs.chunk_exists(&chunk_id1).unwrap());
        assert!(!blobs.chunk_exists(&chunk_id2).unwrap());
    }

    #[tokio::test]
    async fn test_fix_reference_counts_increment() {
        let (repair_manager, metadata, _blobs, _temp) = setup_test_env();

        let data = b"Test chunk";
        let chunk_id = compute_chunk_id(data);

        // Set ref count to 1
        metadata.increment_chunk_ref(&chunk_id).unwrap();

        // Fix to 3
        let error = RefCountError {
            chunk_id: chunk_id.clone(),
            stored_count: 1,
            actual_count: 3,
        };

        repair_manager.fix_reference_counts(&[error]).await.unwrap();

        let ref_count = metadata.get_chunk_ref_count(&chunk_id).unwrap();
        assert_eq!(ref_count, 3);
    }

    #[tokio::test]
    async fn test_fix_reference_counts_decrement() {
        let (repair_manager, metadata, _blobs, _temp) = setup_test_env();

        let data = b"Test chunk";
        let chunk_id = compute_chunk_id(data);

        // Set ref count to 5
        for _ in 0..5 {
            metadata.increment_chunk_ref(&chunk_id).unwrap();
        }

        // Fix to 2
        let error = RefCountError {
            chunk_id: chunk_id.clone(),
            stored_count: 5,
            actual_count: 2,
        };

        repair_manager.fix_reference_counts(&[error]).await.unwrap();

        let ref_count = metadata.get_chunk_ref_count(&chunk_id).unwrap();
        assert_eq!(ref_count, 2);
    }

    #[tokio::test]
    async fn test_rebuild_missing_chunk() {
        let (mut repair_manager, metadata, blobs, _temp) = setup_test_env();

        let data = b"Missing chunk data";
        let chunk_id = compute_chunk_id(data);

        // Add chunk to mock cluster
        let cluster = Arc::get_mut(&mut repair_manager.cluster).unwrap();
        cluster.add_chunk(chunk_id.clone(), data.to_vec());

        // Create missing chunk entry
        let missing = MissingChunk {
            chunk_id: chunk_id.clone(),
            affected_objects: Vec::new(),
            available_replicas: vec![NodeId(1)],
        };

        // Rebuild
        let bytes = repair_manager.rebuild_chunk(&missing).await.unwrap();

        assert_eq!(bytes, data.len() as u64);
        assert!(blobs.chunk_exists(&chunk_id).unwrap());

        // Verify data
        let stored_data = blobs.read_chunk(&chunk_id).unwrap();
        assert_eq!(stored_data, data);
    }

    #[tokio::test]
    async fn test_rebuild_chunk_no_replica() {
        let (repair_manager, _metadata, _blobs, _temp) = setup_test_env();

        let data = b"No replica";
        let chunk_id = compute_chunk_id(data);

        let missing = MissingChunk {
            chunk_id: chunk_id.clone(),
            affected_objects: Vec::new(),
            available_replicas: Vec::new(),
        };

        // Should fail - no replicas available
        let result = repair_manager.rebuild_chunk(&missing).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_repair_full_report() {
        let (mut repair_manager, metadata, blobs, _temp) = setup_test_env();

        // Setup: Create bucket
        let bucket_config = BucketConfig::new("test-bucket");
        metadata.create_bucket(&bucket_config).unwrap();

        // Create a corrupted chunk
        let original_data = b"Original data";
        let chunk_id = compute_chunk_id(original_data);

        // Write corrupted version
        blobs.write_chunk(&chunk_id, b"Corrupted!").unwrap();
        metadata.increment_chunk_ref(&chunk_id).unwrap();

        // Add correct version to cluster
        let cluster = Arc::get_mut(&mut repair_manager.cluster).unwrap();
        cluster.add_chunk(chunk_id.clone(), original_data.to_vec());

        // Create object referencing this chunk
        let object_key = ObjectKey::new("test-bucket", "test-key");
        let protocol_chunk_id = ProtocolChunkId::from_hex(&chunk_id.0).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(original_data);
        let checksum_bytes = hasher.finalize();
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(&checksum_bytes);

        let manifest = ProtocolManifest::new(
            object_key.clone(),
            original_data.len() as u64,
            checksum,
            vec![protocol_chunk_id],
        );

        metadata.put_object(&manifest).unwrap();

        // Run integrity check
        let checker = IntegrityChecker::new(metadata.clone(), blobs.clone());
        let report = checker.full_scan().await.unwrap();

        assert!(!report.is_healthy());
        assert_eq!(report.corrupted_chunks.len(), 1);

        // Repair
        let result = repair_manager.repair(&report).await.unwrap();

        assert_eq!(result.chunks_repaired, 1);
        assert_eq!(result.chunks_failed, 0);

        // Verify repair worked
        let stored_data = blobs.read_chunk(&chunk_id).unwrap();
        assert_eq!(stored_data, original_data);
    }

    #[tokio::test]
    async fn test_repair_result_success_rate() {
        let mut result = RepairResult::new();

        // Empty result should have 100% success
        assert_eq!(result.success_rate(), 1.0);

        result.chunks_repaired = 8;
        result.chunks_failed = 2;

        assert_eq!(result.success_rate(), 0.8);
    }
}
