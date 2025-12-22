//! Blob storage for chunk data.
//!
//! This module provides low-level storage of chunk data on the filesystem.
//! Chunks are stored in a sharded directory structure based on their hash prefix
//! to avoid filesystem performance issues with too many files in a single directory.

use crate::chunks::ChunkId;
use crate::error::{Result, StorageError};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Blob store for chunk data storage.
///
/// The `BlobStore` manages physical storage of chunk data on disk.
/// It uses a sharded directory structure (based on chunk ID prefix) to
/// distribute files across directories for better filesystem performance.
///
/// # Directory Structure
///
/// Chunks are stored in a two-level directory structure:
/// ```text
/// data_dir/
///   ab/
///     cd/
///       abcd1234...  (chunk file)
/// ```
///
/// The first 2 characters of the chunk ID form the first level directory,
/// and the next 2 characters form the second level.
pub struct BlobStore {
    data_dir: PathBuf,
}

impl BlobStore {
    /// Create a new blob store.
    ///
    /// Creates the data directory if it doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `data_dir` - Base directory for storing blob data
    ///
    /// # Returns
    ///
    /// A new `BlobStore` instance.
    ///
    /// # Errors
    ///
    /// Returns error if the directory cannot be created.
    pub fn new(data_dir: PathBuf) -> Result<Self> {
        info!("Initializing blob store at {:?}", data_dir);
        fs::create_dir_all(&data_dir)?;
        Ok(Self { data_dir })
    }

    pub fn write_chunk(&self, chunk_id: &ChunkId, data: &[u8]) -> Result<()> {
        debug!("Writing chunk: {} ({} bytes)", chunk_id, data.len());

        let chunk_path = self.get_chunk_path(chunk_id);
        let shard_dir = self.ensure_shard_dir(chunk_id)?;

        // Write to a temporary file first
        let temp_path = shard_dir.join(format!("{}.tmp", chunk_id.0));
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(data)?;
        file.sync_all()?;

        // Rename to final location (atomic operation)
        fs::rename(temp_path, chunk_path)?;

        Ok(())
    }

    pub fn read_chunk(&self, chunk_id: &ChunkId) -> Result<Vec<u8>> {
        debug!("Reading chunk: {}", chunk_id);

        let chunk_path = self.get_chunk_path(chunk_id);

        if !chunk_path.exists() {
            return Err(StorageError::NotFound(format!(
                "Chunk not found: {}",
                chunk_id
            )));
        }

        let mut file = fs::File::open(&chunk_path)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        Ok(data)
    }

    pub fn delete_chunk(&self, chunk_id: &ChunkId) -> Result<bool> {
        debug!("Deleting chunk: {}", chunk_id);

        let chunk_path = self.get_chunk_path(chunk_id);

        if !chunk_path.exists() {
            return Ok(false);
        }

        fs::remove_file(chunk_path)?;
        Ok(true)
    }

    pub fn chunk_exists(&self, chunk_id: &ChunkId) -> Result<bool> {
        let chunk_path = self.get_chunk_path(chunk_id);
        Ok(chunk_path.exists())
    }

    pub fn verify_chunk(&self, chunk_id: &ChunkId) -> Result<bool> {
        debug!("Verifying chunk: {}", chunk_id);

        let data = self.read_chunk(chunk_id)?;

        // Compute the hash of the data
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let computed_hash = hex::encode(hasher.finalize());

        // Compare with the chunk ID
        if computed_hash != chunk_id.0 {
            warn!(
                "Chunk verification failed for {}: expected {}, got {}",
                chunk_id, chunk_id.0, computed_hash
            );
            return Ok(false);
        }

        Ok(true)
    }

    pub fn list_chunks(&self) -> Result<Vec<ChunkId>> {
        debug!("Listing all chunks in blob store");

        let mut chunks = Vec::new();

        // Walk through the shard directories
        for entry in fs::read_dir(&self.data_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let level1_dir = entry.path();

            for entry2 in fs::read_dir(level1_dir)? {
                let entry2 = entry2?;
                if !entry2.file_type()?.is_dir() {
                    continue;
                }

                let level2_dir = entry2.path();

                for entry3 in fs::read_dir(level2_dir)? {
                    let entry3 = entry3?;
                    if entry3.file_type()?.is_file() {
                        if let Some(filename) = entry3.file_name().to_str() {
                            // Skip temporary files
                            if !filename.ends_with(".tmp") {
                                chunks.push(ChunkId(filename.to_string()));
                            }
                        }
                    }
                }
            }
        }

        Ok(chunks)
    }

    pub fn get_chunk_path(&self, chunk_id: &ChunkId) -> PathBuf {
        // Use first 2 and next 2 characters for sharding
        // e.g., abcd1234... -> /data/blobs/ab/cd/abcd1234...
        let id = &chunk_id.0;

        if id.len() >= 4 {
            let level1 = &id[0..2];
            let level2 = &id[2..4];
            self.data_dir.join(level1).join(level2).join(&chunk_id.0)
        } else {
            // Fallback for short IDs
            self.data_dir.join("misc").join(&chunk_id.0)
        }
    }

    fn ensure_shard_dir(&self, chunk_id: &ChunkId) -> Result<PathBuf> {
        let chunk_path = self.get_chunk_path(chunk_id);
        let shard_dir = chunk_path
            .parent()
            .ok_or_else(|| StorageError::Internal("Invalid chunk path".to_string()))?;

        fs::create_dir_all(shard_dir)?;
        Ok(shard_dir.to_path_buf())
    }

    pub fn get_stats(&self) -> Result<BlobStoreStats> {
        let chunks = self.list_chunks()?;
        let mut total_size = 0u64;
        let mut chunk_count = 0u64;

        for chunk_id in &chunks {
            if let Ok(data) = self.read_chunk(chunk_id) {
                total_size += data.len() as u64;
                chunk_count += 1;
            }
        }

        Ok(BlobStoreStats {
            chunk_count,
            total_size,
        })
    }
}

/// Statistics about the blob store.
///
/// Provides counts of stored chunks and total storage size.
#[derive(Debug, Clone)]
pub struct BlobStoreStats {
    /// Number of chunks stored
    pub chunk_count: u64,
    /// Total size of all chunks in bytes
    pub total_size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_store() -> (BlobStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = BlobStore::new(temp_dir.path().to_path_buf()).unwrap();
        (store, temp_dir)
    }

    fn compute_chunk_id(data: &[u8]) -> ChunkId {
        let mut hasher = Sha256::new();
        hasher.update(data);
        ChunkId(hex::encode(hasher.finalize()))
    }

    #[test]
    fn test_write_and_read_chunk() {
        let (store, _temp) = create_test_store();

        let data = b"Hello, world!";
        let chunk_id = compute_chunk_id(data);

        store.write_chunk(&chunk_id, data).unwrap();

        let read_data = store.read_chunk(&chunk_id).unwrap();
        assert_eq!(data, read_data.as_slice());
    }

    #[test]
    fn test_chunk_exists() {
        let (store, _temp) = create_test_store();

        let data = b"Test data";
        let chunk_id = compute_chunk_id(data);

        assert!(!store.chunk_exists(&chunk_id).unwrap());

        store.write_chunk(&chunk_id, data).unwrap();

        assert!(store.chunk_exists(&chunk_id).unwrap());
    }

    #[test]
    fn test_delete_chunk() {
        let (store, _temp) = create_test_store();

        let data = b"To be deleted";
        let chunk_id = compute_chunk_id(data);

        store.write_chunk(&chunk_id, data).unwrap();
        assert!(store.chunk_exists(&chunk_id).unwrap());

        let deleted = store.delete_chunk(&chunk_id).unwrap();
        assert!(deleted);
        assert!(!store.chunk_exists(&chunk_id).unwrap());

        let deleted_again = store.delete_chunk(&chunk_id).unwrap();
        assert!(!deleted_again);
    }

    #[test]
    fn test_verify_chunk() {
        let (store, _temp) = create_test_store();

        let data = b"Verify this data";
        let chunk_id = compute_chunk_id(data);

        store.write_chunk(&chunk_id, data).unwrap();

        let verified = store.verify_chunk(&chunk_id).unwrap();
        assert!(verified);
    }

    #[test]
    fn test_verify_corrupted_chunk() {
        let (store, _temp) = create_test_store();

        let data = b"Original data";
        let chunk_id = compute_chunk_id(data);

        // Write different data with the same ID
        let corrupted_data = b"Corrupted data";
        store.write_chunk(&chunk_id, corrupted_data).unwrap();

        let verified = store.verify_chunk(&chunk_id).unwrap();
        assert!(!verified);
    }

    #[test]
    fn test_list_chunks() {
        let (store, _temp) = create_test_store();

        let data1 = b"Chunk 1";
        let data2 = b"Chunk 2";
        let data3 = b"Chunk 3";

        let id1 = compute_chunk_id(data1);
        let id2 = compute_chunk_id(data2);
        let id3 = compute_chunk_id(data3);

        store.write_chunk(&id1, data1).unwrap();
        store.write_chunk(&id2, data2).unwrap();
        store.write_chunk(&id3, data3).unwrap();

        let chunks = store.list_chunks().unwrap();
        assert_eq!(chunks.len(), 3);

        assert!(chunks.contains(&id1));
        assert!(chunks.contains(&id2));
        assert!(chunks.contains(&id3));
    }

    #[test]
    fn test_sharded_directory_structure() {
        let (store, _temp) = create_test_store();

        let data = b"Test sharding";
        let chunk_id = compute_chunk_id(data);

        store.write_chunk(&chunk_id, data).unwrap();

        let chunk_path = store.get_chunk_path(&chunk_id);
        assert!(chunk_path.exists());

        // Verify sharding structure
        let id = &chunk_id.0;
        if id.len() >= 4 {
            let path_str = chunk_path.to_str().unwrap();
            assert!(path_str.contains(&id[0..2]));
            assert!(path_str.contains(&id[2..4]));
        }
    }

    #[test]
    fn test_read_nonexistent_chunk() {
        let (store, _temp) = create_test_store();

        let chunk_id = ChunkId("nonexistent123".to_string());

        let result = store.read_chunk(&chunk_id);
        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[test]
    fn test_get_stats() {
        let (store, _temp) = create_test_store();

        let data1 = b"First chunk data";
        let data2 = b"Second chunk data with more content";

        let id1 = compute_chunk_id(data1);
        let id2 = compute_chunk_id(data2);

        store.write_chunk(&id1, data1).unwrap();
        store.write_chunk(&id2, data2).unwrap();

        let stats = store.get_stats().unwrap();
        assert_eq!(stats.chunk_count, 2);
        assert_eq!(stats.total_size, (data1.len() + data2.len()) as u64);
    }

    #[test]
    fn test_new_creates_data_directory() {
        let temp = TempDir::new().unwrap();
        let data_dir = temp.path().join("blob_store_data");

        // Verify directory doesn't exist yet
        assert!(!data_dir.exists());

        // Create BlobStore
        let _store = BlobStore::new(data_dir.clone()).unwrap();

        // Verify directory was created
        assert!(data_dir.exists());
        assert!(data_dir.is_dir());
    }

    #[test]
    fn test_write_chunk_stores_at_correct_path() {
        let (store, _temp) = create_test_store();

        let data = b"Test data for path verification";
        let chunk_id = compute_chunk_id(data);

        // Write the chunk
        store.write_chunk(&chunk_id, data).unwrap();

        // Get the expected path
        let expected_path = store.get_chunk_path(&chunk_id);

        // Verify the file exists at the expected path
        assert!(
            expected_path.exists(),
            "Chunk file should exist at path: {:?}",
            expected_path
        );
        assert!(
            expected_path.is_file(),
            "Expected path should be a file: {:?}",
            expected_path
        );

        // Verify the hierarchical directory structure
        let id = &chunk_id.0;
        if id.len() >= 4 {
            let level1 = &id[0..2];
            let level2 = &id[2..4];

            // Expected path should be: data_dir/level1/level2/chunk_id
            let expected_parent = store.data_dir.join(level1).join(level2);
            assert_eq!(
                expected_path.parent().unwrap(),
                expected_parent,
                "Chunk should be stored in correct hierarchical directory"
            );

            // Verify directory structure exists
            assert!(
                store.data_dir.join(level1).exists(),
                "Level 1 directory should exist"
            );
            assert!(expected_parent.exists(), "Level 2 directory should exist");
        }

        // Verify the file contains the correct data
        let file_data = fs::read(&expected_path).unwrap();
        assert_eq!(
            data,
            file_data.as_slice(),
            "File contents should match written data"
        );
    }

    #[test]
    fn test_delete_chunk_removes_file() {
        let (store, _temp) = create_test_store();

        let data = b"Data to be deleted from disk";
        let chunk_id = compute_chunk_id(data);

        // Write the chunk
        store.write_chunk(&chunk_id, data).unwrap();

        // Get the file path and verify it exists
        let chunk_path = store.get_chunk_path(&chunk_id);
        assert!(
            chunk_path.exists(),
            "Chunk file should exist before deletion at path: {:?}",
            chunk_path
        );
        assert!(
            chunk_path.is_file(),
            "Chunk path should be a file before deletion"
        );

        // Delete the chunk
        let deleted = store.delete_chunk(&chunk_id).unwrap();
        assert!(
            deleted,
            "delete_chunk should return true when deleting existing chunk"
        );

        // Verify the file no longer exists on disk
        assert!(
            !chunk_path.exists(),
            "Chunk file should not exist after deletion at path: {:?}",
            chunk_path
        );

        // Verify attempting to delete again returns false
        let deleted_again = store.delete_chunk(&chunk_id).unwrap();
        assert!(
            !deleted_again,
            "delete_chunk should return false when chunk doesn't exist"
        );
    }
}
