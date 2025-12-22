//! Chunk management and deduplication.
//!
//! This module provides chunking, deduplication, and reference counting for stored data.
//! Data is split into fixed-size chunks (4MB by default), each identified by its SHA-256 hash.
//! Identical chunks are deduplicated automatically through content-addressable storage.

use crate::blobs::BlobStore;
use crate::error::{Result, StorageError};
use crate::metadata::MetadataStore;
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Default chunk size: 4MB.
///
/// Data larger than this size will be split into multiple chunks.
pub const CHUNK_SIZE: usize = 4 * 1024 * 1024; // 4MB

/// Maximum allowed chunk size: 16MB.
///
/// Chunks larger than this will be rejected to prevent excessive memory usage.
pub const MAX_CHUNK_SIZE: usize = 16 * 1024 * 1024; // 16MB hard limit

/// Storage-layer chunk identifier.
///
/// A chunk ID is the hex-encoded SHA-256 hash of the chunk data,
/// providing content-addressable storage and automatic deduplication.
///
/// # Example
///
/// ```rust
/// use anode_storage::chunks::compute_chunk_id;
///
/// let data = b"Hello, world!";
/// let chunk_id = compute_chunk_id(data);
/// println!("Chunk ID: {}", chunk_id);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChunkId(pub String);

impl fmt::Display for ChunkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A chunk of data with its content-addressable ID.
///
/// Represents a piece of data along with its computed chunk ID (SHA-256 hash).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chunk {
    /// The chunk's content-addressable ID (SHA-256 hash)
    pub id: ChunkId,
    /// The chunk data
    pub data: Vec<u8>,
    /// Size of the chunk in bytes
    pub size: usize,
}

impl Chunk {
    /// Create a new chunk from data.
    ///
    /// Computes the SHA-256 hash of the data to generate the chunk ID.
    ///
    /// # Arguments
    ///
    /// * `data` - The chunk data
    ///
    /// # Returns
    ///
    /// A new `Chunk` with computed ID.
    pub fn new(data: Vec<u8>) -> Self {
        let size = data.len();
        let id = compute_chunk_id(&data);
        Self { id, data, size }
    }

    /// Create a new chunk from a data slice.
    ///
    /// Convenience method that clones the slice into a vector.
    ///
    /// # Arguments
    ///
    /// * `data` - The chunk data as a slice
    pub fn from_slice(data: &[u8]) -> Self {
        Self::new(data.to_vec())
    }
}

/// Compute the chunk ID for given data.
///
/// Computes the SHA-256 hash and returns it as a hex-encoded string.
///
/// # Arguments
///
/// * `data` - The data to hash
///
/// # Returns
///
/// The chunk ID as a hex-encoded SHA-256 hash.
pub fn compute_chunk_id(data: &[u8]) -> ChunkId {
    let mut hasher = Sha256::new();
    hasher.update(data);
    ChunkId(hex::encode(hasher.finalize()))
}

/// Chunk manager handling chunking, storage, and deduplication.
///
/// The `ChunkManager` coordinates between the blob store (for chunk data)
/// and metadata store (for reference counting) to provide:
///
/// - Automatic chunking of large data
/// - Content-addressable storage with SHA-256 hashing
/// - Automatic deduplication of identical chunks
/// - Reference counting for garbage collection
/// - Integrity verification
pub struct ChunkManager {
    blob_store: Arc<BlobStore>,
    metadata: Arc<MetadataStore>,
}

impl ChunkManager {
    /// Create a new chunk manager.
    ///
    /// # Arguments
    ///
    /// * `blob_store` - The blob store for chunk data
    /// * `metadata` - The metadata store for reference counts
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
                self.metadata.set_chunk_size(&chunk_id, chunk.size as u64)?;
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
                },
                Ok(false) => {
                    report.corrupted_chunks.push(chunk_id);
                },
                Err(e) => {
                    warn!("Error verifying chunk {}: {}", chunk_id, e);
                    report.corrupted_chunks.push(chunk_id);
                },
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

/// Statistics from a garbage collection run.
///
/// Tracks how many chunks were scanned, deleted, and how much space was freed.
#[derive(Debug, Clone)]
pub struct GcStats {
    /// Number of chunks scanned during garbage collection
    pub chunks_scanned: u64,
    /// Number of orphaned chunks deleted
    pub chunks_deleted: u64,
    /// Total bytes freed by deleting orphaned chunks
    pub bytes_freed: u64,
}

/// Report from an integrity verification scan.
///
/// Contains statistics about chunk verification including any corrupted or missing chunks found.
#[derive(Debug, Clone)]
pub struct IntegrityReport {
    /// Total number of chunks checked
    pub total_chunks: u64,
    /// Number of chunks that passed verification
    pub verified_chunks: u64,
    /// List of corrupted chunks (hash mismatch)
    pub corrupted_chunks: Vec<ChunkId>,
    /// List of missing chunks (referenced but not found)
    pub missing_chunks: Vec<ChunkId>,
}

impl IntegrityReport {
    /// Check if the storage is healthy.
    ///
    /// Returns `true` if no corrupted or missing chunks were found.
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
    fn test_chunk_id_consistent_sha256_hash() {
        // Test that ChunkId produces consistent SHA-256 hashes for the same content
        let content = b"test content for hashing";

        // Create ChunkId multiple times from the same content
        let id1 = compute_chunk_id(content);
        let id2 = compute_chunk_id(content);
        let id3 = compute_chunk_id(content);

        // All IDs should be identical
        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
        assert_eq!(id1, id3);

        // Verify the hash matches expected SHA-256 output
        // Expected hash computed externally: echo -n "test content for hashing" | shasum -a 256
        let expected_hash = "e25dd806d495b413931f4eea50b677a7a5c02d00460924661283f211a37f7e7f";
        assert_eq!(id1.0, expected_hash);

        // Test with different content to ensure different hashes
        let different_content = b"different content";
        let id4 = compute_chunk_id(different_content);
        assert_ne!(id1, id4);

        // Test with empty content
        let empty_content = b"";
        let id5 = compute_chunk_id(empty_content);
        let id6 = compute_chunk_id(empty_content);
        assert_eq!(id5, id6);
        // SHA-256 of empty string
        let expected_empty_hash =
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(id5.0, expected_empty_hash);
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
        manager.store_chunks(vec![chunk.clone()]).await.unwrap();
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
        manager.store_chunks(vec![chunk.clone()]).await.unwrap();
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
        manager.store_chunks(vec![chunk.clone()]).await.unwrap();

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

    #[test]
    fn test_split_into_chunks_100mb() {
        // Test with 100MB input (large data)
        let size_100mb = 100 * 1024 * 1024; // 100MB
        let data = vec![0u8; size_100mb];
        let chunks = ChunkManager::split_into_chunks(&data);

        // Should create exactly 25 chunks (100MB / 4MB = 25)
        assert_eq!(chunks.len(), 25);

        // All chunks should be exactly CHUNK_SIZE (4MB)
        for chunk in &chunks {
            assert_eq!(chunk.size, CHUNK_SIZE);
            assert_eq!(chunk.data.len(), CHUNK_SIZE);
        }

        // Verify total size matches input
        let total_size: usize = chunks.iter().map(|c| c.size).sum();
        assert_eq!(total_size, size_100mb);
    }

    #[test]
    fn test_identical_content_produces_identical_chunk_id() {
        // Test that identical content produces identical ChunkIds (deduplication)
        let content = b"This is test content for deduplication testing";

        // Create multiple chunks with identical content
        let chunk1 = Chunk::from_slice(content);
        let chunk2 = Chunk::from_slice(content);
        let chunk3 = Chunk::new(content.to_vec());

        // All ChunkIds should be identical
        assert_eq!(chunk1.id, chunk2.id);
        assert_eq!(chunk2.id, chunk3.id);
        assert_eq!(chunk1.id, chunk3.id);

        // Verify using compute_chunk_id directly
        let id1 = compute_chunk_id(content);
        let id2 = compute_chunk_id(content);
        assert_eq!(id1, id2);
        assert_eq!(id1, chunk1.id);

        // Create chunks with slightly different content
        let different_content = b"This is test content for deduplication testinG"; // Different last char
        let chunk4 = Chunk::from_slice(different_content);

        // Different content should produce different ChunkId
        assert_ne!(chunk1.id, chunk4.id);

        // Test with larger identical content (realistic scenario)
        let large_content = vec![42u8; 1024 * 1024]; // 1MB of identical data
        let chunk5 = Chunk::new(large_content.clone());
        let chunk6 = Chunk::new(large_content.clone());
        assert_eq!(chunk5.id, chunk6.id);

        // Test with empty content (edge case)
        let empty1 = Chunk::from_slice(b"");
        let empty2 = Chunk::from_slice(b"");
        assert_eq!(empty1.id, empty2.id);
    }

    #[tokio::test]
    async fn test_retrieve_chunks_roundtrip_small_data() {
        // Test roundtrip: split -> store -> retrieve -> verify for small data
        let (manager, _temp) = create_test_manager();

        // Create small test data (a few KB)
        let original_data =
            b"This is a small test payload for chunk roundtrip testing. ".repeat(100);
        let original_vec = original_data.to_vec();

        // Step 1: Split data into chunks
        let chunks = ChunkManager::split_into_chunks(&original_vec);
        assert!(!chunks.is_empty(), "Should create at least one chunk");

        // Step 2: Store the chunks
        let chunk_ids = manager.store_chunks(chunks).await.unwrap();
        assert_eq!(chunk_ids.len(), 1, "Small data should fit in one chunk");

        // Step 3: Retrieve the chunks
        let retrieved_data = manager.retrieve_chunks(&chunk_ids).await.unwrap();

        // Step 4: Verify the retrieved data matches the original
        assert_eq!(
            retrieved_data.len(),
            original_vec.len(),
            "Retrieved data size should match original"
        );
        assert_eq!(
            retrieved_data, original_vec,
            "Retrieved data should exactly match original data"
        );
    }

    #[tokio::test]
    async fn test_retrieve_chunks_roundtrip_large_data() {
        // Test roundtrip: split -> store -> retrieve -> verify for large multi-chunk data
        let (manager, _temp) = create_test_manager();

        // Create large test data (15MB to ensure multiple chunks)
        // 15MB = 15 * 1024 * 1024 bytes
        // With 4MB chunks, this should create 4 chunks (4MB + 4MB + 4MB + 3MB)
        let size_15mb = 15 * 1024 * 1024;

        // Use a pattern that allows us to verify data integrity
        // Fill with incrementing bytes modulo 256 to create a deterministic pattern
        let mut original_data = Vec::with_capacity(size_15mb);
        for i in 0..size_15mb {
            original_data.push((i % 256) as u8);
        }

        // Step 1: Split data into chunks
        let chunks = ChunkManager::split_into_chunks(&original_data);
        assert_eq!(
            chunks.len(),
            4,
            "15MB should split into 4 chunks (4MB each for first 3, 3MB for last)"
        );

        // Verify chunk sizes
        assert_eq!(chunks[0].size, CHUNK_SIZE, "First chunk should be 4MB");
        assert_eq!(chunks[1].size, CHUNK_SIZE, "Second chunk should be 4MB");
        assert_eq!(chunks[2].size, CHUNK_SIZE, "Third chunk should be 4MB");
        assert_eq!(
            chunks[3].size,
            3 * 1024 * 1024,
            "Fourth chunk should be 3MB"
        );

        // Step 2: Store the chunks
        let chunk_ids = manager.store_chunks(chunks).await.unwrap();
        assert_eq!(chunk_ids.len(), 4, "Should have 4 chunk IDs");

        // Step 3: Retrieve the chunks
        let retrieved_data = manager.retrieve_chunks(&chunk_ids).await.unwrap();

        // Step 4: Verify the retrieved data matches the original
        assert_eq!(
            retrieved_data.len(),
            original_data.len(),
            "Retrieved data size should match original"
        );
        assert_eq!(
            retrieved_data, original_data,
            "Retrieved data should exactly match original data"
        );

        // Step 5: Additional verification - spot check data integrity at chunk boundaries
        // Verify data at the boundary between chunk 0 and chunk 1 (at 4MB mark)
        let boundary_1 = CHUNK_SIZE;
        assert_eq!(
            retrieved_data[boundary_1 - 1],
            ((boundary_1 - 1) % 256) as u8,
            "Data should be correct at first chunk boundary"
        );
        assert_eq!(
            retrieved_data[boundary_1],
            (boundary_1 % 256) as u8,
            "Data should be correct after first chunk boundary"
        );

        // Verify data at the boundary between chunk 1 and chunk 2 (at 8MB mark)
        let boundary_2 = CHUNK_SIZE * 2;
        assert_eq!(
            retrieved_data[boundary_2 - 1],
            ((boundary_2 - 1) % 256) as u8,
            "Data should be correct at second chunk boundary"
        );
        assert_eq!(
            retrieved_data[boundary_2],
            (boundary_2 % 256) as u8,
            "Data should be correct after second chunk boundary"
        );
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn test_chunk_split_retrieve_roundtrip_is_identity(data in prop::collection::vec(any::<u8>(), 0..20*1024*1024)) {
                // Create a test runtime for async operations
                let runtime = tokio::runtime::Runtime::new().unwrap();

                let retrieved_data = runtime.block_on(async {
                    let (manager, _temp) = create_test_manager();

                    // Step 1: Split data into chunks
                    let chunks = ChunkManager::split_into_chunks(&data);

                    // Step 2: Store the chunks
                    let chunk_ids = manager.store_chunks(chunks).await.unwrap();

                    // Step 3: Retrieve the chunks
                    manager.retrieve_chunks(&chunk_ids).await.unwrap()
                });

                // Step 4: Verify the retrieved data matches the original (identity property)
                assert_eq!(retrieved_data, data);
            }
        }
    }
}
