use crate::blobs::BlobStore;
use crate::error::{Result, StorageError};
use crate::metadata::MetadataStore;
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;
use tracing::{debug, info, warn};

pub const CHUNK_SIZE: usize = 4 * 1024 * 1024; // 4MB
pub const MAX_CHUNK_SIZE: usize = 16 * 1024 * 1024; // 16MB hard limit

/// Storage-layer chunk identifier (hex-encoded SHA-256 hash)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ChunkId(pub String);

impl fmt::Display for ChunkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chunk {
    pub id: ChunkId,
    pub data: Vec<u8>,
    pub size: usize,
}

impl Chunk {
    pub fn new(data: Vec<u8>) -> Self {
        let size = data.len();
        let id = compute_chunk_id(&data);
        Self { id, data, size }
    }

    pub fn from_slice(data: &[u8]) -> Self {
        Self::new(data.to_vec())
    }
}

pub fn compute_chunk_id(data: &[u8]) -> ChunkId {
    let mut hasher = Sha256::new();
    hasher.update(data);
    ChunkId(hex::encode(hasher.finalize()))
}

pub struct ChunkManager {
    blob_store: Arc<BlobStore>,
    metadata: Arc<MetadataStore>,
}

impl ChunkManager {
    pub fn new(blob_store: Arc<BlobStore>, metadata: Arc<MetadataStore>) -> Self {
        Self {
            blob_store,
            metadata,
        }
    }

    pub fn split_into_chunks(data: &[u8]) -> Vec<Chunk> {
        if data.is_empty() {
            return vec![];
        }

        let mut chunks = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            let end = (offset + CHUNK_SIZE).min(data.len());
            let chunk_data = data[offset..end].to_vec();
            chunks.push(Chunk::new(chunk_data));
            offset = end;
        }

        debug!("Split {} bytes into {} chunks", data.len(), chunks.len());
        chunks
    }

    pub async fn store_chunks(&self, chunks: Vec<Chunk>) -> Result<Vec<ChunkId>> {
        let mut chunk_ids = Vec::new();

        for chunk in chunks {
            if chunk.size > MAX_CHUNK_SIZE {
                return Err(StorageError::ChunkTooLarge {
                    size: chunk.size,
                    max: MAX_CHUNK_SIZE,
                });
            }

            let chunk_id = chunk.id.clone();

            // Check if chunk already exists (deduplication)
            let exists = self.blob_store.chunk_exists(&chunk_id)?;

            if exists {
                debug!("Chunk {} already exists, incrementing ref count", chunk_id);
                // Increment reference count
                self.metadata.increment_chunk_ref(&chunk_id)?;
            } else {
                debug!("Storing new chunk: {} ({} bytes)", chunk_id, chunk.size);
                // Write the chunk data
                self.blob_store.write_chunk(&chunk_id, &chunk.data)?;

                // Initialize reference count
                self.metadata.increment_chunk_ref(&chunk_id)?;
                self.metadata
                    .set_chunk_size(&chunk_id, chunk.size as u64)?;
            }

            chunk_ids.push(chunk_id);
        }

        Ok(chunk_ids)
    }

    pub async fn retrieve_chunks(&self, chunk_ids: &[ChunkId]) -> Result<Vec<u8>> {
        if chunk_ids.is_empty() {
            return Ok(Vec::new());
        }

        debug!("Retrieving {} chunks", chunk_ids.len());

        let mut data = Vec::new();

        for chunk_id in chunk_ids {
            let chunk_data = self.blob_store.read_chunk(chunk_id)?;
            data.extend_from_slice(&chunk_data);
        }

        debug!("Retrieved total of {} bytes", data.len());
        Ok(data)
    }

    pub async fn delete_chunks(&self, chunk_ids: &[ChunkId]) -> Result<()> {
        for chunk_id in chunk_ids {
            // Decrement reference count
            let ref_count = self.metadata.decrement_chunk_ref(chunk_id)?;

            // If reference count reaches zero, delete the chunk
            if ref_count == 0 {
                info!("Deleting unreferenced chunk: {}", chunk_id);
                self.blob_store.delete_chunk(chunk_id)?;
            } else {
                debug!("Chunk {} still has {} references", chunk_id, ref_count);
            }
        }

        Ok(())
    }

    pub async fn garbage_collect(&self) -> Result<GcStats> {
        info!("Starting garbage collection");

        let mut stats = GcStats {
            chunks_scanned: 0,
            chunks_deleted: 0,
            bytes_freed: 0,
        };

        // Get all chunks from blob store
        let blob_chunks = self.blob_store.list_chunks()?;
        stats.chunks_scanned = blob_chunks.len() as u64;

        for chunk_id in blob_chunks {
            let ref_count = self.metadata.get_chunk_ref_count(&chunk_id)?;

            if ref_count == 0 {
                // This chunk has no references, delete it
                warn!("Found orphaned chunk: {}", chunk_id);

                // Get size before deletion
                if let Ok(data) = self.blob_store.read_chunk(&chunk_id) {
                    stats.bytes_freed += data.len() as u64;
                }

                self.blob_store.delete_chunk(&chunk_id)?;
                stats.chunks_deleted += 1;
            }
        }

        info!(
            "Garbage collection complete: scanned {}, deleted {}, freed {} bytes",
            stats.chunks_scanned, stats.chunks_deleted, stats.bytes_freed
        );

        Ok(stats)
    }

    pub async fn verify_integrity(&self) -> Result<IntegrityReport> {
        info!("Starting integrity verification");

        let mut report = IntegrityReport {
            total_chunks: 0,
            verified_chunks: 0,
            corrupted_chunks: Vec::new(),
            missing_chunks: Vec::new(),
        };

        // Get all chunks from metadata
        let metadata_chunks = self.metadata.list_all_chunks()?;
        report.total_chunks = metadata_chunks.len() as u64;

        for chunk_id in metadata_chunks {
            // Check if chunk exists in blob store
            let exists = self.blob_store.chunk_exists(&chunk_id)?;

            if !exists {
                report.missing_chunks.push(chunk_id);
                continue;
            }

            // Verify chunk integrity
            match self.blob_store.verify_chunk(&chunk_id) {
                Ok(true) => {
                    report.verified_chunks += 1;
                }
                Ok(false) => {
                    report.corrupted_chunks.push(chunk_id);
                }
                Err(e) => {
                    warn!("Error verifying chunk {}: {}", chunk_id, e);
                    report.corrupted_chunks.push(chunk_id);
                }
            }
        }

        info!(
            "Integrity check complete: {}/{} verified, {} corrupted, {} missing",
            report.verified_chunks,
            report.total_chunks,
            report.corrupted_chunks.len(),
            report.missing_chunks.len()
        );

        Ok(report)
    }
}

#[derive(Debug, Clone)]
pub struct GcStats {
    pub chunks_scanned: u64,
    pub chunks_deleted: u64,
    pub bytes_freed: u64,
}

#[derive(Debug, Clone)]
pub struct IntegrityReport {
    pub total_chunks: u64,
    pub verified_chunks: u64,
    pub corrupted_chunks: Vec<ChunkId>,
    pub missing_chunks: Vec<ChunkId>,
}

impl IntegrityReport {
    pub fn is_healthy(&self) -> bool {
        self.corrupted_chunks.is_empty() && self.missing_chunks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blobs::BlobStore;
    use crate::metadata::MetadataStore;
    use tempfile::TempDir;

    fn create_test_manager() -> (ChunkManager, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let blob_dir = temp_dir.path().join("blobs");
        let metadata_path = temp_dir.path().join("metadata.db");

        let blob_store = Arc::new(BlobStore::new(blob_dir).unwrap());
        let metadata = Arc::new(MetadataStore::open(&metadata_path).unwrap());

        let manager = ChunkManager::new(blob_store, metadata);
        (manager, temp_dir)
    }

    #[test]
    fn test_compute_chunk_id() {
        let data = b"Hello, world!";
        let id1 = compute_chunk_id(data);
        let id2 = compute_chunk_id(data);

        // Same data should produce same ID
        assert_eq!(id1, id2);

        let different_data = b"Different data";
        let id3 = compute_chunk_id(different_data);

        // Different data should produce different ID
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_chunk_creation() {
        let data = b"Test data";
        let chunk = Chunk::from_slice(data);

        assert_eq!(chunk.size, data.len());
        assert_eq!(chunk.data, data);
    }

    #[test]
    fn test_split_into_chunks_zero_bytes() {
        // Test with 0 byte (empty) input
        let empty_data = &[];
        let chunks = ChunkManager::split_into_chunks(empty_data);

        // Should return empty vector for empty input
        assert_eq!(chunks.len(), 0);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_split_into_chunks_one_byte() {
        // Test with 1 byte input
        let one_byte_data = &[42u8];
        let chunks = ChunkManager::split_into_chunks(one_byte_data);

        // Should return exactly one chunk
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].size, 1);
        assert_eq!(chunks[0].data, vec![42u8]);

        // Verify chunk ID is computed correctly
        let expected_id = compute_chunk_id(one_byte_data);
        assert_eq!(chunks[0].id, expected_id);
    }

    #[test]
    fn test_split_into_chunks() {
        // Test empty data
        let chunks = ChunkManager::split_into_chunks(&[]);
        assert_eq!(chunks.len(), 0);

        // Test small data (less than chunk size)
        let small_data = vec![0u8; 1024];
        let chunks = ChunkManager::split_into_chunks(&small_data);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].size, 1024);

        // Test data larger than chunk size
        let large_data = vec![0u8; CHUNK_SIZE + 1000];
        let chunks = ChunkManager::split_into_chunks(&large_data);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].size, CHUNK_SIZE);
        assert_eq!(chunks[1].size, 1000);

        // Test exact multiple of chunk size
        let exact_data = vec![0u8; CHUNK_SIZE * 3];
        let chunks = ChunkManager::split_into_chunks(&exact_data);
        assert_eq!(chunks.len(), 3);
        for chunk in chunks {
            assert_eq!(chunk.size, CHUNK_SIZE);
        }
    }

    #[tokio::test]
    async fn test_store_and_retrieve_chunks() {
        let (manager, _temp) = create_test_manager();

        let data = b"Hello, this is test data for chunking!";
        let chunks = ChunkManager::split_into_chunks(data);

        let chunk_ids = manager.store_chunks(chunks).await.unwrap();
        assert_eq!(chunk_ids.len(), 1);

        let retrieved = manager.retrieve_chunks(&chunk_ids).await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_chunk_deduplication() {
        let (manager, _temp) = create_test_manager();

        let data = b"Duplicate data";
        let chunk = Chunk::from_slice(data);
        let chunk_id = chunk.id.clone();

        // Store the same chunk twice
        manager
            .store_chunks(vec![chunk.clone()])
            .await
            .unwrap();
        manager.store_chunks(vec![chunk]).await.unwrap();

        // Reference count should be 2
        let ref_count = manager.metadata.get_chunk_ref_count(&chunk_id).unwrap();
        assert_eq!(ref_count, 2);
    }

    #[tokio::test]
    async fn test_delete_chunks() {
        let (manager, _temp) = create_test_manager();

        let data = b"Data to be deleted";
        let chunk = Chunk::from_slice(data);
        let chunk_id = chunk.id.clone();

        manager.store_chunks(vec![chunk]).await.unwrap();

        // Delete the chunk
        manager.delete_chunks(&[chunk_id.clone()]).await.unwrap();

        // Chunk should be deleted
        let exists = manager.blob_store.chunk_exists(&chunk_id).unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_delete_chunks_with_multiple_refs() {
        let (manager, _temp) = create_test_manager();

        let data = b"Shared chunk data";
        let chunk = Chunk::from_slice(data);
        let chunk_id = chunk.id.clone();

        // Store the chunk twice (simulate two objects using same chunk)
        manager
            .store_chunks(vec![chunk.clone()])
            .await
            .unwrap();
        manager.store_chunks(vec![chunk]).await.unwrap();

        // Delete once
        manager.delete_chunks(&[chunk_id.clone()]).await.unwrap();

        // Chunk should still exist (ref count = 1)
        let exists = manager.blob_store.chunk_exists(&chunk_id).unwrap();
        assert!(exists);

        // Delete again
        manager.delete_chunks(&[chunk_id.clone()]).await.unwrap();

        // Now chunk should be deleted (ref count = 0)
        let exists = manager.blob_store.chunk_exists(&chunk_id).unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_garbage_collection() {
        let (manager, _temp) = create_test_manager();

        let data = b"Orphaned chunk";
        let chunk = Chunk::from_slice(data);
        let chunk_id = chunk.id.clone();

        // Write chunk directly without metadata
        manager
            .blob_store
            .write_chunk(&chunk_id, &chunk.data)
            .unwrap();

        // Run garbage collection
        let stats = manager.garbage_collect().await.unwrap();

        assert_eq!(stats.chunks_scanned, 1);
        assert_eq!(stats.chunks_deleted, 1);
        assert_eq!(stats.bytes_freed, data.len() as u64);

        // Chunk should be deleted
        let exists = manager.blob_store.chunk_exists(&chunk_id).unwrap();
        assert!(!exists);
    }

    #[tokio::test]
    async fn test_verify_integrity() {
        let (manager, _temp) = create_test_manager();

        let data = b"Valid chunk data";
        let chunks = ChunkManager::split_into_chunks(data);
        manager.store_chunks(chunks).await.unwrap();

        let report = manager.verify_integrity().await.unwrap();

        assert_eq!(report.total_chunks, 1);
        assert_eq!(report.verified_chunks, 1);
        assert!(report.corrupted_chunks.is_empty());
        assert!(report.missing_chunks.is_empty());
        assert!(report.is_healthy());
    }

    #[tokio::test]
    async fn test_verify_integrity_with_corruption() {
        let (manager, _temp) = create_test_manager();

        let data = b"Original data";
        let chunk = Chunk::from_slice(data);
        let chunk_id = chunk.id.clone();

        // Store chunk
        manager
            .store_chunks(vec![chunk.clone()])
            .await
            .unwrap();

        // Corrupt the chunk by overwriting with different data
        let corrupted_data = b"Corrupted!!!";
        manager
            .blob_store
            .write_chunk(&chunk_id, corrupted_data)
            .unwrap();

        let report = manager.verify_integrity().await.unwrap();

        assert_eq!(report.total_chunks, 1);
        assert_eq!(report.verified_chunks, 0);
        assert_eq!(report.corrupted_chunks.len(), 1);
        assert!(!report.is_healthy());
    }

    #[tokio::test]
    async fn test_large_data_chunking() {
        let (manager, _temp) = create_test_manager();

        // Create data larger than chunk size
        let large_data = vec![42u8; CHUNK_SIZE * 2 + 1000];
        let chunks = ChunkManager::split_into_chunks(&large_data);

        assert_eq!(chunks.len(), 3);

        let chunk_ids = manager.store_chunks(chunks).await.unwrap();
        assert_eq!(chunk_ids.len(), 3);

        let retrieved = manager.retrieve_chunks(&chunk_ids).await.unwrap();
        assert_eq!(retrieved, large_data);
    }

    #[test]
    fn test_split_into_chunks_exactly_4mb() {
        // Test with exactly 4MB (the chunk size boundary)
        let exact_4mb_data = vec![0u8; CHUNK_SIZE];
        let chunks = ChunkManager::split_into_chunks(&exact_4mb_data);

        // Should create exactly 1 chunk since 4MB is exactly the chunk size
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].size, CHUNK_SIZE);
        assert_eq!(chunks[0].data.len(), CHUNK_SIZE);
    }

    #[test]
    fn test_split_into_chunks_4mb_plus_one_byte() {
        // Test with 4MB+1 byte (just over the chunk size boundary)
        let data = vec![0u8; CHUNK_SIZE + 1];
        let chunks = ChunkManager::split_into_chunks(&data);

        // Should create exactly 2 chunks since it's 1 byte over the 4MB boundary
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].size, CHUNK_SIZE);
        assert_eq!(chunks[1].size, 1);

        // Verify total size matches input
        let total_size: usize = chunks.iter().map(|c| c.size).sum();
        assert_eq!(total_size, CHUNK_SIZE + 1);
    }
}
