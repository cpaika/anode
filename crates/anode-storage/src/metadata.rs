use crate::chunks::ChunkId;
use crate::error::{Result, StorageError};
use anode_protocol::{ObjectKey, ObjectManifest};
use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info, warn};

const OBJECTS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("objects");
const BUCKETS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("buckets");
const CHUNKS_TABLE: TableDefinition<&[u8], &[u8]> = TableDefinition::new("chunks");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketConfig {
    pub name: String,
    pub created_at: u64,
    pub versioning_enabled: bool,
    pub metadata: HashMap<String, String>,
}

impl BucketConfig {
    pub fn new(name: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            name: name.into(),
            created_at: now,
            versioning_enabled: false,
            metadata: HashMap::new(),
        }
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

pub struct MetadataStore {
    db: Database,
}

impl MetadataStore {
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
        }
        write_txn.commit()?;

        Ok(Self { db })
    }

    pub fn put_object(&self, manifest: &ObjectManifest) -> Result<()> {
        let object_key = format!("{}/{}", manifest.object_key.bucket, manifest.object_key.key);
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
            }
            None => Ok(None),
        }
    }

    pub fn delete_object(&self, bucket: &str, key: &str) -> Result<bool> {
        let object_key = format!("{}/{}", bucket, key);
        debug!("Deleting object metadata: {}", object_key);

        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut table = write_txn.open_table(OBJECTS_TABLE)?;
            table.remove(object_key.as_str())?.is_some()
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
            }
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
            table.remove(name)?.is_some()
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
                }
                None => {
                    // First reference to this chunk
                    ChunkMetadata::new(0)
                }
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
                        warn!("Attempting to decrement zero ref count for chunk: {}", chunk_id);
                        return Err(StorageError::RefCountError(
                            "Cannot decrement zero ref count".to_string(),
                        ));
                    }
                    meta.ref_count -= 1;
                    meta
                }
                None => {
                    return Err(StorageError::NotFound(format!("Chunk not found: {}", chunk_id)));
                }
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
            }
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
        let mut manifest = ObjectManifest::new(object_key.clone(), 1024, vec![]);

        store.put_object(&manifest).unwrap();

        let retrieved = store.get_object("bucket1", "key1").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().size, 1024);

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
            let manifest = ObjectManifest::new(object_key, i * 100, vec![]);
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
}
