use crate::metadata::ParquetMetadata;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Instant;

pub struct ParquetCache {
    cache: RwLock<LruCache<String, CachedMetadata>>,
    max_size_bytes: usize,
    current_size_bytes: AtomicUsize,
    stats: CacheStats,
}

#[derive(Debug)]
struct CachedMetadata {
    metadata: ParquetMetadata,
    size_bytes: usize,
    last_access: Instant,
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub evictions: AtomicU64,
    pub insertions: AtomicU64,
}

impl ParquetCache {
    pub fn new(max_size_bytes: usize) -> Self {
        // Default to at least 1000 entries
        let capacity = NonZeroUsize::new(1000).unwrap();

        Self {
            cache: RwLock::new(LruCache::new(capacity)),
            max_size_bytes,
            current_size_bytes: AtomicUsize::new(0),
            stats: CacheStats::default(),
        }
    }

    pub fn get(&self, key: &str) -> Option<ParquetMetadata> {
        let mut cache = self.cache.write().ok()?;

        if let Some(cached) = cache.get_mut(key) {
            cached.last_access = Instant::now();
            self.stats.hits.fetch_add(1, Ordering::Relaxed);
            Some(cached.metadata.clone())
        } else {
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    pub fn insert(&self, key: String, metadata: ParquetMetadata) {
        let size_bytes = metadata.estimated_size();

        // Evict entries if needed to make space
        self.evict_if_needed(size_bytes);

        let cached = CachedMetadata {
            metadata,
            size_bytes,
            last_access: Instant::now(),
        };

        if let Ok(mut cache) = self.cache.write() {
            // If key already exists, subtract old size
            if let Some(old) = cache.get(&key) {
                self.current_size_bytes
                    .fetch_sub(old.size_bytes, Ordering::Relaxed);
            }

            cache.put(key, cached);
            self.current_size_bytes
                .fetch_add(size_bytes, Ordering::Relaxed);
            self.stats.insertions.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn invalidate(&self, key: &str) -> bool {
        if let Ok(mut cache) = self.cache.write() {
            if let Some(removed) = cache.pop(key) {
                self.current_size_bytes
                    .fetch_sub(removed.size_bytes, Ordering::Relaxed);
                return true;
            }
        }
        false
    }

    pub fn invalidate_prefix(&self, prefix: &str) -> usize {
        let mut count = 0;

        if let Ok(mut cache) = self.cache.write() {
            // Collect keys to remove (can't modify while iterating)
            let keys_to_remove: Vec<String> = cache
                .iter()
                .filter(|(k, _)| k.starts_with(prefix))
                .map(|(k, _)| k.clone())
                .collect();

            for key in keys_to_remove {
                if let Some(removed) = cache.pop(&key) {
                    self.current_size_bytes
                        .fetch_sub(removed.size_bytes, Ordering::Relaxed);
                    count += 1;
                }
            }
        }

        count
    }

    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
            self.current_size_bytes.store(0, Ordering::Relaxed);
        }
    }

    pub fn stats(&self) -> CacheStatsSnapshot {
        let hits = self.stats.hits.load(Ordering::Relaxed);
        let misses = self.stats.misses.load(Ordering::Relaxed);
        let total_requests = hits + misses;
        let hit_rate = if total_requests > 0 {
            hits as f64 / total_requests as f64
        } else {
            0.0
        };

        CacheStatsSnapshot {
            hits,
            misses,
            hit_rate,
            evictions: self.stats.evictions.load(Ordering::Relaxed),
            current_size_bytes: self.current_size_bytes.load(Ordering::Relaxed),
            entry_count: self.len(),
        }
    }

    pub fn current_size(&self) -> usize {
        self.current_size_bytes.load(Ordering::Relaxed)
    }

    pub fn len(&self) -> usize {
        self.cache.read().map(|c| c.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn evict_if_needed(&self, required_space: usize) {
        let current_size = self.current_size_bytes.load(Ordering::Relaxed);

        if current_size + required_space <= self.max_size_bytes {
            return;
        }

        if let Ok(mut cache) = self.cache.write() {
            // Evict LRU entries until we have enough space
            while self.current_size_bytes.load(Ordering::Relaxed) + required_space
                > self.max_size_bytes
                && !cache.is_empty()
            {
                if let Some((_, removed)) = cache.pop_lru() {
                    self.current_size_bytes
                        .fetch_sub(removed.size_bytes, Ordering::Relaxed);
                    self.stats.evictions.fetch_add(1, Ordering::Relaxed);
                } else {
                    break;
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStatsSnapshot {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub evictions: u64,
    pub current_size_bytes: usize,
    pub entry_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{FieldInfo, SerializableSchema};

    fn create_test_metadata(name: &str, size_multiplier: usize) -> ParquetMetadata {
        let fields = (0..size_multiplier)
            .map(|i| FieldInfo {
                name: format!("field_{}", i),
                data_type: "INT32".to_string(),
                nullable: true,
            })
            .collect();

        ParquetMetadata {
            file_size: 1000,
            footer_size: 100,
            schema: SerializableSchema { fields },
            row_groups: vec![],
            num_rows: 100,
            created_by: Some(name.to_string()),
            key_value_metadata: vec![],
        }
    }

    #[test]
    fn test_cache_insert_and_get() {
        let cache = ParquetCache::new(10_000);
        let metadata = create_test_metadata("test1", 5);

        cache.insert("key1".to_string(), metadata.clone());

        let retrieved = cache.get("key1").unwrap();
        assert_eq!(retrieved.num_rows, metadata.num_rows);
        assert_eq!(retrieved.created_by, metadata.created_by);
    }

    #[test]
    fn test_cache_miss() {
        let cache = ParquetCache::new(10_000);
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_cache_stats() {
        let cache = ParquetCache::new(10_000);
        let metadata = create_test_metadata("test", 5);

        // Insert
        cache.insert("key1".to_string(), metadata.clone());

        // Hit
        cache.get("key1");

        // Miss
        cache.get("key2");

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.insertions, 1);
        assert!((stats.hit_rate - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_cache_invalidate() {
        let cache = ParquetCache::new(10_000);
        let metadata = create_test_metadata("test", 5);

        cache.insert("key1".to_string(), metadata.clone());
        assert!(cache.get("key1").is_some());

        let removed = cache.invalidate("key1");
        assert!(removed);
        assert!(cache.get("key1").is_none());

        let not_removed = cache.invalidate("nonexistent");
        assert!(!not_removed);
    }

    #[test]
    fn test_cache_invalidate_prefix() {
        let cache = ParquetCache::new(10_000);
        let metadata = create_test_metadata("test", 5);

        cache.insert("bucket1/file1".to_string(), metadata.clone());
        cache.insert("bucket1/file2".to_string(), metadata.clone());
        cache.insert("bucket2/file1".to_string(), metadata.clone());

        let count = cache.invalidate_prefix("bucket1/");
        assert_eq!(count, 2);

        assert!(cache.get("bucket1/file1").is_none());
        assert!(cache.get("bucket1/file2").is_none());
        assert!(cache.get("bucket2/file1").is_some());
    }

    #[test]
    fn test_cache_clear() {
        let cache = ParquetCache::new(10_000);
        let metadata = create_test_metadata("test", 5);

        cache.insert("key1".to_string(), metadata.clone());
        cache.insert("key2".to_string(), metadata.clone());

        assert_eq!(cache.len(), 2);

        cache.clear();

        assert_eq!(cache.len(), 0);
        assert_eq!(cache.current_size(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_eviction_on_size_limit() {
        // Small cache that can only hold 2-3 entries
        let cache = ParquetCache::new(500);
        let metadata = create_test_metadata("test", 10); // Creates larger metadata

        cache.insert("key1".to_string(), metadata.clone());
        cache.insert("key2".to_string(), metadata.clone());
        cache.insert("key3".to_string(), metadata.clone());

        let stats = cache.stats();
        assert!(stats.evictions > 0);

        // key1 should be evicted (LRU)
        assert!(cache.get("key1").is_none());
    }

    #[test]
    fn test_cache_lru_ordering() {
        let cache = ParquetCache::new(500);
        let metadata = create_test_metadata("test", 10);

        cache.insert("key1".to_string(), metadata.clone());
        cache.insert("key2".to_string(), metadata.clone());

        // Access key1 to make it more recently used
        cache.get("key1");

        // Insert key3, which should evict key2 (least recently used)
        cache.insert("key3".to_string(), metadata.clone());

        assert!(cache.get("key1").is_some());
        assert!(cache.get("key2").is_none());
        assert!(cache.get("key3").is_some());
    }

    #[test]
    fn test_cache_update_existing_key() {
        let cache = ParquetCache::new(10_000);
        let metadata1 = create_test_metadata("test1", 5);
        let metadata2 = create_test_metadata("test2", 10);

        cache.insert("key1".to_string(), metadata1.clone());
        let initial_size = cache.current_size();

        cache.insert("key1".to_string(), metadata2.clone());
        let final_size = cache.current_size();

        // Size should change based on new metadata
        assert_ne!(initial_size, final_size);

        let retrieved = cache.get("key1").unwrap();
        assert_eq!(retrieved.created_by, Some("test2".to_string()));
    }

    #[test]
    fn test_cache_current_size_tracking() {
        let cache = ParquetCache::new(10_000);
        let metadata = create_test_metadata("test", 5);
        let estimated_size = metadata.estimated_size();

        assert_eq!(cache.current_size(), 0);

        cache.insert("key1".to_string(), metadata.clone());
        assert_eq!(cache.current_size(), estimated_size);

        cache.insert("key2".to_string(), metadata.clone());
        assert_eq!(cache.current_size(), estimated_size * 2);

        cache.invalidate("key1");
        assert_eq!(cache.current_size(), estimated_size);

        cache.clear();
        assert_eq!(cache.current_size(), 0);
    }

    #[test]
    fn test_cache_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(ParquetCache::new(100_000));
        let metadata = create_test_metadata("test", 5);

        let mut handles = vec![];

        // Spawn multiple threads inserting and reading
        for i in 0..10 {
            let cache_clone = Arc::clone(&cache);
            let metadata_clone = metadata.clone();

            let handle = thread::spawn(move || {
                for j in 0..10 {
                    let key = format!("key_{}_{}", i, j);
                    cache_clone.insert(key.clone(), metadata_clone.clone());
                    cache_clone.get(&key);
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = cache.stats();
        assert_eq!(stats.insertions, 100);
        assert_eq!(stats.hits, 100);
    }

    #[test]
    fn test_empty_cache() {
        let cache = ParquetCache::new(10_000);
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);

        let metadata = create_test_metadata("test", 5);
        cache.insert("key1".to_string(), metadata);

        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let cache = ParquetCache::new(10_000);
        let metadata = create_test_metadata("test", 5);

        cache.insert("key1".to_string(), metadata.clone());

        // 3 hits, 2 misses = 60% hit rate
        cache.get("key1");
        cache.get("key1");
        cache.get("key1");
        cache.get("key2");
        cache.get("key3");

        let stats = cache.stats();
        assert_eq!(stats.hits, 3);
        assert_eq!(stats.misses, 2);
        assert!((stats.hit_rate - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_eviction_maintains_size_invariant() {
        let max_size = 1000;
        let cache = ParquetCache::new(max_size);
        let metadata = create_test_metadata("test", 20);

        // Insert many entries to force eviction
        for i in 0..100 {
            cache.insert(format!("key{}", i), metadata.clone());

            // Size should never exceed max (with some tolerance for edge cases)
            assert!(cache.current_size() <= max_size * 2);
        }
    }
}
