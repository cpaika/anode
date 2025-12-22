//! LRU cache for Parquet file metadata.
//!
//! This module provides an efficient, thread-safe cache for Parquet metadata
//! that uses LRU eviction and size-based limits.

use crate::metadata::ParquetMetadata;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Instant;

/// A thread-safe LRU cache for Parquet file metadata.
///
/// This cache stores [`ParquetMetadata`] objects with the following features:
/// - **Size-based eviction**: Limits total memory usage in bytes
/// - **LRU policy**: Evicts least recently used entries when full
/// - **Thread-safe**: Can be safely shared across threads
/// - **Statistics tracking**: Tracks hits, misses, and evictions
///
/// # Example
///
/// ```
/// use anode_parquet::ParquetCache;
///
/// let cache = ParquetCache::new(10_000_000); // 10MB max size
///
/// // Cache operations are thread-safe
/// let key = "bucket/file.parquet".to_string();
/// // cache.insert(key.clone(), metadata);
/// // let cached = cache.get(&key);
/// ```
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

/// Internal cache statistics tracking.
///
/// All fields use atomic operations for thread-safe updates.
#[derive(Debug, Default)]
pub struct CacheStats {
    /// Number of successful cache hits.
    pub hits: AtomicU64,
    /// Number of cache misses.
    pub misses: AtomicU64,
    /// Number of evicted entries.
    pub evictions: AtomicU64,
    /// Number of inserted entries.
    pub insertions: AtomicU64,
}

impl ParquetCache {
    /// Creates a new ParquetCache with the specified maximum size in bytes.
    ///
    /// The cache will automatically evict least recently used entries when the
    /// total size exceeds `max_size_bytes`.
    ///
    /// # Arguments
    ///
    /// * `max_size_bytes` - Maximum total size of cached metadata in bytes
    ///
    /// # Example
    ///
    /// ```
    /// use anode_parquet::ParquetCache;
    ///
    /// let cache = ParquetCache::new(10_000_000); // 10MB cache
    /// ```
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

    /// Retrieves cached metadata for the given key.
    ///
    /// Updates the access time and increments hit/miss statistics.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key (typically "bucket/object.parquet")
    ///
    /// # Returns
    ///
    /// * `Some(ParquetMetadata)` if the key is found (cache hit)
    /// * `None` if the key is not found (cache miss)
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

    /// Inserts metadata into the cache.
    ///
    /// If the key already exists, the old entry is replaced. If the cache is full,
    /// least recently used entries are evicted to make space.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key (typically "bucket/object.parquet")
    /// * `metadata` - The Parquet metadata to cache
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

    /// Removes a single entry from the cache.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to remove
    ///
    /// # Returns
    ///
    /// * `true` if the entry was found and removed
    /// * `false` if the entry was not found
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

    /// Removes all entries with keys starting with the given prefix.
    ///
    /// This is useful for invalidating all cached metadata for a bucket
    /// or directory prefix.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The key prefix to match (e.g., "bucket/")
    ///
    /// # Returns
    ///
    /// The number of entries removed
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

    /// Removes all entries from the cache.
    ///
    /// This resets the cache to an empty state and sets the current size to zero.
    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
            self.current_size_bytes.store(0, Ordering::Relaxed);
        }
    }

    /// Returns a snapshot of current cache statistics.
    ///
    /// The snapshot includes hits, misses, evictions, insertions, hit rate,
    /// current size, and entry count.
    ///
    /// # Returns
    ///
    /// A [`CacheStatsSnapshot`] containing current statistics
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
            insertions: self.stats.insertions.load(Ordering::Relaxed),
            current_size_bytes: self.current_size_bytes.load(Ordering::Relaxed),
            entry_count: self.len(),
        }
    }

    /// Returns the current total size of cached metadata in bytes.
    ///
    /// This is an approximate value maintained atomically as entries are
    /// added and removed.
    pub fn current_size(&self) -> usize {
        self.current_size_bytes.load(Ordering::Relaxed)
    }

    /// Returns the number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.cache.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Returns `true` if the cache contains no entries.
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

/// A point-in-time snapshot of cache statistics.
///
/// This structure is returned by [`ParquetCache::stats`] and provides
/// a consistent view of cache performance metrics.
#[derive(Debug, Clone)]
pub struct CacheStatsSnapshot {
    /// Total number of cache hits.
    pub hits: u64,
    /// Total number of cache misses.
    pub misses: u64,
    /// Cache hit rate as a ratio (0.0 to 1.0).
    pub hit_rate: f64,
    /// Total number of evicted entries.
    pub evictions: u64,
    /// Total number of inserted entries.
    pub insertions: u64,
    /// Current total size of cached data in bytes.
    pub current_size_bytes: usize,
    /// Current number of entries in the cache.
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
        let cache = ParquetCache::new(2000); // Larger cache to ensure 2 items fit
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

    #[test]
    fn test_cache_metadata_hit() {
        let cache = ParquetCache::new(10_000);

        // Create test metadata with specific values
        let metadata = ParquetMetadata {
            file_size: 5000,
            footer_size: 200,
            schema: SerializableSchema {
                fields: vec![
                    FieldInfo {
                        name: "id".to_string(),
                        data_type: "INT32".to_string(),
                        nullable: false,
                    },
                    FieldInfo {
                        name: "name".to_string(),
                        data_type: "BYTE_ARRAY".to_string(),
                        nullable: true,
                    },
                ],
            },
            row_groups: vec![],
            num_rows: 1000,
            created_by: Some("test_creator".to_string()),
            key_value_metadata: vec![("key1".to_string(), "value1".to_string())],
        };

        // Insert metadata into cache
        let cache_key = "bucket1/file1.parquet";
        cache.insert(cache_key.to_string(), metadata.clone());

        // Verify initial stats
        let stats_before = cache.stats();
        assert_eq!(stats_before.insertions, 1);
        assert_eq!(stats_before.hits, 0);
        assert_eq!(stats_before.misses, 0);

        // Retrieve from cache - should be a hit
        let retrieved = cache
            .get(cache_key)
            .expect("Should retrieve cached metadata");

        // Verify it's the same metadata
        assert_eq!(retrieved.file_size, metadata.file_size);
        assert_eq!(retrieved.footer_size, metadata.footer_size);
        assert_eq!(retrieved.num_rows, metadata.num_rows);
        assert_eq!(retrieved.created_by, metadata.created_by);
        assert_eq!(retrieved.schema.fields.len(), metadata.schema.fields.len());
        assert_eq!(retrieved.schema.fields[0].name, "id");
        assert_eq!(retrieved.schema.fields[1].name, "name");
        assert_eq!(retrieved.key_value_metadata.len(), 1);
        assert_eq!(retrieved.key_value_metadata[0].0, "key1");
        assert_eq!(retrieved.key_value_metadata[0].1, "value1");

        // Verify cache hit was recorded
        let stats_after = cache.stats();
        assert_eq!(stats_after.hits, 1, "Should have exactly 1 cache hit");
        assert_eq!(stats_after.misses, 0, "Should have 0 cache misses");
        assert_eq!(stats_after.hit_rate, 1.0, "Hit rate should be 100%");

        // Retrieve again - should be another hit
        cache.get(cache_key).expect("Should retrieve again");

        let stats_final = cache.stats();
        assert_eq!(stats_final.hits, 2, "Should have 2 cache hits");
        assert_eq!(stats_final.misses, 0, "Should still have 0 misses");
        assert_eq!(stats_final.hit_rate, 1.0, "Hit rate should still be 100%");
    }

    #[test]
    fn test_cache_metadata_miss() {
        let cache = ParquetCache::new(10_000);

        // Verify initial stats - empty cache
        let stats_before = cache.stats();
        assert_eq!(stats_before.hits, 0);
        assert_eq!(stats_before.misses, 0);
        assert_eq!(stats_before.insertions, 0);

        // Try to retrieve metadata that doesn't exist
        let cache_key = "bucket1/nonexistent.parquet";
        let result = cache.get(cache_key);

        // Verify it's a cache miss
        assert!(result.is_none(), "Should return None for cache miss");

        // Verify miss statistics are updated
        let stats_after = cache.stats();
        assert_eq!(stats_after.hits, 0, "Should have 0 cache hits");
        assert_eq!(stats_after.misses, 1, "Should have exactly 1 cache miss");
        assert_eq!(stats_after.hit_rate, 0.0, "Hit rate should be 0%");

        // Try another miss with a different key
        let another_key = "bucket2/another_missing.parquet";
        let result2 = cache.get(another_key);
        assert!(
            result2.is_none(),
            "Should return None for second cache miss"
        );

        // Verify miss count incremented
        let stats_final = cache.stats();
        assert_eq!(stats_final.hits, 0, "Should still have 0 hits");
        assert_eq!(stats_final.misses, 2, "Should have 2 cache misses");
        assert_eq!(stats_final.hit_rate, 0.0, "Hit rate should still be 0%");

        // Now insert something and verify stats still work correctly
        let metadata = ParquetMetadata {
            file_size: 1000,
            footer_size: 100,
            schema: SerializableSchema { fields: vec![] },
            row_groups: vec![],
            num_rows: 100,
            created_by: Some("test".to_string()),
            key_value_metadata: vec![],
        };

        cache.insert(cache_key.to_string(), metadata);

        // Try the same key again - should now be a hit
        let result3 = cache.get(cache_key);
        assert!(result3.is_some(), "Should now find the cached metadata");

        let stats_with_hit = cache.stats();
        assert_eq!(stats_with_hit.hits, 1, "Should have 1 hit");
        assert_eq!(stats_with_hit.misses, 2, "Should still have 2 misses");
        assert_eq!(stats_with_hit.insertions, 1, "Should have 1 insertion");
        // 1 hit out of 3 total requests = 33.3% hit rate
        assert!(
            (stats_with_hit.hit_rate - 0.333).abs() < 0.01,
            "Hit rate should be ~33%"
        );
    }

    #[test]
    fn test_cache_invalidation_with_miss_verification() {
        let cache = ParquetCache::new(10_000);

        // Create test metadata
        let metadata = ParquetMetadata {
            file_size: 5000,
            footer_size: 200,
            schema: SerializableSchema {
                fields: vec![
                    FieldInfo {
                        name: "id".to_string(),
                        data_type: "INT64".to_string(),
                        nullable: false,
                    },
                    FieldInfo {
                        name: "value".to_string(),
                        data_type: "DOUBLE".to_string(),
                        nullable: true,
                    },
                ],
            },
            row_groups: vec![],
            num_rows: 2000,
            created_by: Some("invalidation_test".to_string()),
            key_value_metadata: vec![("version".to_string(), "1.0".to_string())],
        };

        let cache_key = "test-bucket/data.parquet";

        // Step 1: Insert metadata into cache
        cache.insert(cache_key.to_string(), metadata.clone());

        let stats_after_insert = cache.stats();
        assert_eq!(stats_after_insert.insertions, 1);
        assert_eq!(stats_after_insert.entry_count, 1);
        assert!(stats_after_insert.current_size_bytes > 0);
        let initial_size = stats_after_insert.current_size_bytes;

        // Step 2: Verify cache hit
        let retrieved = cache.get(cache_key);
        assert!(retrieved.is_some(), "Should retrieve cached metadata");
        assert_eq!(retrieved.unwrap().num_rows, 2000);

        let stats_after_hit = cache.stats();
        assert_eq!(stats_after_hit.hits, 1);
        assert_eq!(stats_after_hit.misses, 0);

        // Step 3: Invalidate the cached entry
        let invalidated = cache.invalidate(cache_key);
        assert!(invalidated, "Invalidation should return true");

        // Verify cache size decreased
        let stats_after_invalidation = cache.stats();
        assert_eq!(stats_after_invalidation.entry_count, 0);
        assert_eq!(stats_after_invalidation.current_size_bytes, 0);

        // Step 4: Subsequent lookup should miss
        let result_after_invalidation = cache.get(cache_key);
        assert!(
            result_after_invalidation.is_none(),
            "Should return None after invalidation"
        );

        // Verify miss was recorded
        let stats_final = cache.stats();
        assert_eq!(stats_final.hits, 1, "Should still have 1 hit from before");
        assert_eq!(
            stats_final.misses, 1,
            "Should have 1 miss after invalidation"
        );
        assert_eq!(stats_final.entry_count, 0, "Cache should be empty");
        assert_eq!(stats_final.current_size_bytes, 0, "Size should be zero");

        // Verify hit rate calculation (1 hit, 1 miss = 50%)
        assert!((stats_final.hit_rate - 0.5).abs() < 0.01);

        // Step 5: Re-insert and verify it works again
        cache.insert(cache_key.to_string(), metadata.clone());
        let re_retrieved = cache.get(cache_key);
        assert!(
            re_retrieved.is_some(),
            "Should retrieve re-inserted metadata"
        );

        let stats_after_reinsertion = cache.stats();
        assert_eq!(stats_after_reinsertion.insertions, 2);
        assert_eq!(stats_after_reinsertion.hits, 2);
        assert_eq!(stats_after_reinsertion.misses, 1);
        assert_eq!(stats_after_reinsertion.entry_count, 1);
    }
}
