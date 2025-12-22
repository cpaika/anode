//! Workload generation for simulations

use crate::{BucketName, ObjectKey};
use async_trait::async_trait;
use rand::distributions::Alphanumeric;
use rand::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Operation types that can be performed
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Operation {
    /// Put an object
    Put {
        bucket: BucketName,
        key: ObjectKey,
        value: Vec<u8>,
    },
    /// Get an object
    Get { bucket: BucketName, key: ObjectKey },
    /// Delete an object
    Delete { bucket: BucketName, key: ObjectKey },
    /// List objects
    List {
        bucket: BucketName,
        prefix: Option<String>,
    },
    /// Create bucket
    CreateBucket { bucket: BucketName },
    /// Delete bucket
    DeleteBucket { bucket: BucketName },
}

/// Trait for workload generators
#[async_trait]
pub trait Workload: Send + Sync {
    /// Generate the next operation
    async fn generate_operation(&self) -> Operation;

    /// Reset the workload generator
    fn reset(&mut self);

    /// Get workload statistics
    fn stats(&self) -> WorkloadStats;
}

/// Statistics about a workload
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkloadStats {
    pub total_operations: u64,
    pub puts: u64,
    pub gets: u64,
    pub deletes: u64,
    pub lists: u64,
    pub create_buckets: u64,
    pub delete_buckets: u64,
}

/// Random workload generator
pub struct RandomWorkload {
    rng: Arc<std::sync::Mutex<StdRng>>,
    bucket_count: usize,
    key_space: usize,
    value_size_range: (usize, usize),
    read_ratio: f64,
    write_ratio: f64,
    _delete_ratio: f64,
    stats: WorkloadStats,
}

impl RandomWorkload {
    /// Create a new random workload
    pub fn new(
        bucket_count: usize,
        key_space: usize,
        value_size_range: (usize, usize),
        read_ratio: f64,
        write_ratio: f64,
        delete_ratio: f64,
    ) -> Self {
        let rng = Arc::new(std::sync::Mutex::new(StdRng::from_entropy()));
        Self {
            rng,
            bucket_count,
            key_space,
            value_size_range,
            read_ratio,
            write_ratio,
            _delete_ratio: delete_ratio,
            stats: WorkloadStats::default(),
        }
    }

    fn random_bucket(&self) -> BucketName {
        let bucket_id = self.rng.lock().unwrap().gen_range(0..self.bucket_count);
        format!("bucket-{}", bucket_id)
    }

    fn random_key(&self) -> ObjectKey {
        let key_id = self.rng.lock().unwrap().gen_range(0..self.key_space);
        format!("key-{:08}", key_id)
    }

    fn random_value(&self) -> Vec<u8> {
        let size = self
            .rng
            .lock()
            .unwrap()
            .gen_range(self.value_size_range.0..=self.value_size_range.1);
        let mut rng = self.rng.lock().unwrap();
        (0..size).map(|_| rng.gen()).collect()
    }
}

#[async_trait]
impl Workload for RandomWorkload {
    async fn generate_operation(&self) -> Operation {
        let choice: f64 = self.rng.lock().unwrap().gen();

        if choice < self.read_ratio {
            Operation::Get {
                bucket: self.random_bucket(),
                key: self.random_key(),
            }
        } else if choice < self.read_ratio + self.write_ratio {
            Operation::Put {
                bucket: self.random_bucket(),
                key: self.random_key(),
                value: self.random_value(),
            }
        } else {
            Operation::Delete {
                bucket: self.random_bucket(),
                key: self.random_key(),
            }
        }
    }

    fn reset(&mut self) {
        self.stats = WorkloadStats::default();
    }

    fn stats(&self) -> WorkloadStats {
        self.stats.clone()
    }
}

/// Sequential workload generator
pub struct SequentialWorkload {
    bucket: BucketName,
    current_key: Arc<std::sync::atomic::AtomicU64>,
    value_size: usize,
    stats: WorkloadStats,
}

impl SequentialWorkload {
    /// Create a new sequential workload
    pub fn new(bucket: BucketName, value_size: usize) -> Self {
        Self {
            bucket,
            current_key: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            value_size,
            stats: WorkloadStats::default(),
        }
    }

    fn next_key(&self) -> ObjectKey {
        let key_id = self
            .current_key
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("key-{:08}", key_id)
    }

    fn random_value(&self) -> Vec<u8> {
        let mut rng = thread_rng();
        (0..self.value_size).map(|_| rng.gen()).collect()
    }
}

#[async_trait]
impl Workload for SequentialWorkload {
    async fn generate_operation(&self) -> Operation {
        Operation::Put {
            bucket: self.bucket.clone(),
            key: self.next_key(),
            value: self.random_value(),
        }
    }

    fn reset(&mut self) {
        self.current_key
            .store(0, std::sync::atomic::Ordering::SeqCst);
        self.stats = WorkloadStats::default();
    }

    fn stats(&self) -> WorkloadStats {
        self.stats.clone()
    }
}

/// Hotspot workload generator (80/20 rule)
pub struct HotspotWorkload {
    rng: Arc<std::sync::Mutex<StdRng>>,
    bucket: BucketName,
    hot_keys: Vec<ObjectKey>,
    cold_key_space: usize,
    value_size_range: (usize, usize),
    hot_ratio: f64,
    stats: WorkloadStats,
}

impl HotspotWorkload {
    /// Create a new hotspot workload
    pub fn new(
        bucket: BucketName,
        hot_key_count: usize,
        cold_key_space: usize,
        value_size_range: (usize, usize),
        hot_ratio: f64,
    ) -> Self {
        let hot_keys: Vec<ObjectKey> = (0..hot_key_count)
            .map(|i| format!("hot-key-{:08}", i))
            .collect();

        Self {
            rng: Arc::new(std::sync::Mutex::new(StdRng::from_entropy())),
            bucket,
            hot_keys,
            cold_key_space,
            value_size_range,
            hot_ratio,
            stats: WorkloadStats::default(),
        }
    }

    fn select_key(&self) -> ObjectKey {
        let choice: f64 = self.rng.lock().unwrap().gen();

        if choice < self.hot_ratio {
            // Access hot key
            let index = self.rng.lock().unwrap().gen_range(0..self.hot_keys.len());
            self.hot_keys[index].clone()
        } else {
            // Access cold key
            let key_id = self.rng.lock().unwrap().gen_range(0..self.cold_key_space);
            format!("cold-key-{:08}", key_id)
        }
    }

    fn random_value(&self) -> Vec<u8> {
        let size = self
            .rng
            .lock()
            .unwrap()
            .gen_range(self.value_size_range.0..=self.value_size_range.1);
        let mut rng = self.rng.lock().unwrap();
        (0..size).map(|_| rng.gen()).collect()
    }
}

#[async_trait]
impl Workload for HotspotWorkload {
    async fn generate_operation(&self) -> Operation {
        let choice: f64 = self.rng.lock().unwrap().gen();

        if choice < 0.7 {
            Operation::Get {
                bucket: self.bucket.clone(),
                key: self.select_key(),
            }
        } else {
            Operation::Put {
                bucket: self.bucket.clone(),
                key: self.select_key(),
                value: self.random_value(),
            }
        }
    }

    fn reset(&mut self) {
        self.stats = WorkloadStats::default();
    }

    fn stats(&self) -> WorkloadStats {
        self.stats.clone()
    }
}

/// Mixed workload combining multiple workload generators
pub struct MixedWorkload {
    workloads: Vec<(Box<dyn Workload>, f64)>,
    rng: Arc<std::sync::Mutex<StdRng>>,
    stats: WorkloadStats,
}

impl MixedWorkload {
    /// Create a new mixed workload
    pub fn new() -> Self {
        Self {
            workloads: Vec::new(),
            rng: Arc::new(std::sync::Mutex::new(StdRng::from_entropy())),
            stats: WorkloadStats::default(),
        }
    }

    /// Add a workload with a given weight
    pub fn add_workload(mut self, workload: Box<dyn Workload>, weight: f64) -> Self {
        self.workloads.push((workload, weight));
        self
    }
}

impl Default for MixedWorkload {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Workload for MixedWorkload {
    async fn generate_operation(&self) -> Operation {
        if self.workloads.is_empty() {
            return Operation::Get {
                bucket: "default".to_string(),
                key: "key".to_string(),
            };
        }

        let total_weight: f64 = self.workloads.iter().map(|(_, w)| w).sum();
        let mut choice = self.rng.lock().unwrap().gen::<f64>() * total_weight;

        for (workload, weight) in &self.workloads {
            if choice < *weight {
                return workload.generate_operation().await;
            }
            choice -= weight;
        }

        // Fallback to first workload
        self.workloads[0].0.generate_operation().await
    }

    fn reset(&mut self) {
        for (workload, _) in &mut self.workloads {
            workload.reset();
        }
        self.stats = WorkloadStats::default();
    }

    fn stats(&self) -> WorkloadStats {
        self.stats.clone()
    }
}

/// Generate a random string of given length
pub fn random_string(len: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

/// Generate random bytes of given length
pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut rng = thread_rng();
    (0..len).map(|_| rng.gen()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_random_workload() {
        let workload = RandomWorkload::new(5, 100, (1024, 4096), 0.7, 0.2, 0.1);
        let op = workload.generate_operation().await;

        match op {
            Operation::Get { bucket, key } => {
                assert!(bucket.starts_with("bucket-"));
                assert!(key.starts_with("key-"));
            },
            Operation::Put { bucket, key, value } => {
                assert!(bucket.starts_with("bucket-"));
                assert!(key.starts_with("key-"));
                assert!(value.len() >= 1024 && value.len() <= 4096);
            },
            Operation::Delete { bucket, key } => {
                assert!(bucket.starts_with("bucket-"));
                assert!(key.starts_with("key-"));
            },
            _ => panic!("Unexpected operation type"),
        }
    }

    #[tokio::test]
    async fn test_sequential_workload() {
        let workload = SequentialWorkload::new("test-bucket".to_string(), 1024);

        let op1 = workload.generate_operation().await;
        let op2 = workload.generate_operation().await;

        if let (Operation::Put { key: key1, .. }, Operation::Put { key: key2, .. }) = (op1, op2) {
            assert_ne!(key1, key2);
        } else {
            panic!("Expected Put operations");
        }
    }

    #[tokio::test]
    async fn test_hotspot_workload() {
        let workload = HotspotWorkload::new("test-bucket".to_string(), 10, 1000, (1024, 4096), 0.8);

        let op = workload.generate_operation().await;

        match op {
            Operation::Get { key, .. } | Operation::Put { key, .. } => {
                assert!(key.starts_with("hot-key-") || key.starts_with("cold-key-"));
            },
            _ => panic!("Unexpected operation type"),
        }
    }

    #[test]
    fn test_random_string() {
        let s = random_string(10);
        assert_eq!(s.len(), 10);
    }

    #[test]
    fn test_random_bytes() {
        let b = random_bytes(100);
        assert_eq!(b.len(), 100);
    }
}
