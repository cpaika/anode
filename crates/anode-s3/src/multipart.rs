// Multipart upload support for S3 API
// This module provides complete S3-compatible multipart upload functionality

use crate::error::{S3Error, S3Result};
use anode_protocol::{ChunkId, ObjectKey, ObjectManifest};
use anode_storage::StorageEngine;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Multipart upload manager
pub struct MultipartManager {
    uploads: RwLock<HashMap<String, MultipartUpload>>,
    storage: Arc<StorageEngine>,
    config: MultipartConfig,
}

#[derive(Debug, Clone)]
pub struct MultipartConfig {
    /// Minimum part size (5MB for S3 compatibility)
    pub min_part_size: usize,
    /// Maximum part size (5GB)
    pub max_part_size: usize,
    /// Maximum parts per upload (10000)
    pub max_parts: usize,
    /// Upload expiration time
    pub upload_expiry: Duration,
    /// Cleanup interval for expired uploads
    pub cleanup_interval: Duration,
}

impl Default for MultipartConfig {
    fn default() -> Self {
        Self {
            min_part_size: 5 * 1024 * 1024,           // 5MB
            max_part_size: 5 * 1024 * 1024 * 1024,    // 5GB
            max_parts: 10000,
            upload_expiry: Duration::from_secs(7 * 24 * 3600), // 7 days
            cleanup_interval: Duration::from_secs(3600),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MultipartUpload {
    pub upload_id: String,
    pub bucket: String,
    pub key: String,
    pub created_at: DateTime<Utc>,
    pub parts: HashMap<u32, UploadedPart>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct UploadedPart {
    pub part_number: u32,
    pub etag: String,
    pub size: u64,
    pub data: Bytes,
    pub uploaded_at: DateTime<Utc>,
}

impl MultipartManager {
    pub fn new(storage: Arc<StorageEngine>, config: MultipartConfig) -> Self {
        Self {
            uploads: RwLock::new(HashMap::new()),
            storage,
            config,
        }
    }

    /// Initiate a new multipart upload
    pub async fn create_upload(
        &self,
        bucket: &str,
        key: &str,
        metadata: HashMap<String, String>,
    ) -> S3Result<String> {
        info!(
            "Creating multipart upload: bucket={}, key={}",
            bucket, key
        );

        // Generate unique upload ID
        let upload_id = Uuid::new_v4().to_string();

        let upload = MultipartUpload {
            upload_id: upload_id.clone(),
            bucket: bucket.to_string(),
            key: key.to_string(),
            created_at: Utc::now(),
            parts: HashMap::new(),
            metadata,
        };

        self.uploads.write().await.insert(upload_id.clone(), upload);

        debug!("Created multipart upload: {}", upload_id);
        Ok(upload_id)
    }

    /// Upload a part
    pub async fn upload_part(
        &self,
        upload_id: &str,
        part_number: u32,
        data: Bytes,
    ) -> S3Result<UploadedPart> {
        debug!(
            "Uploading part: upload_id={}, part_number={}, size={}",
            upload_id,
            part_number,
            data.len()
        );

        // Validate part number
        if part_number < 1 || part_number as usize > self.config.max_parts {
            return Err(S3Error::InvalidArgument(format!(
                "Part number must be between 1 and {}",
                self.config.max_parts
            )));
        }

        // Validate part size
        if data.len() > self.config.max_part_size {
            return Err(S3Error::EntityTooLarge);
        }

        // Calculate ETag (MD5-like hash using SHA256 for consistency)
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hasher.finalize();
        let etag = hex::encode(hash);

        let part = UploadedPart {
            part_number,
            etag: etag.clone(),
            size: data.len() as u64,
            data: data.clone(),
            uploaded_at: Utc::now(),
        };

        // Store part info
        let mut uploads = self.uploads.write().await;
        let upload = uploads
            .get_mut(upload_id)
            .ok_or_else(|| S3Error::NoSuchUpload(upload_id.to_string()))?;

        upload.parts.insert(part_number, part.clone());

        info!(
            "Uploaded part {}/{} for upload {}",
            part_number,
            upload.parts.len(),
            upload_id
        );

        Ok(part)
    }

    /// Complete multipart upload - assemble all parts
    pub async fn complete_upload(
        &self,
        upload_id: &str,
        parts: Vec<CompletePart>,
    ) -> S3Result<ObjectManifest> {
        info!("Completing multipart upload: {}", upload_id);

        // Remove upload from active uploads
        let upload = {
            let mut uploads = self.uploads.write().await;
            uploads
                .remove(upload_id)
                .ok_or_else(|| S3Error::NoSuchUpload(upload_id.to_string()))?
        };

        // Validate all parts are present and in order
        if parts.is_empty() {
            return Err(S3Error::InvalidRequest(
                "Complete request must contain at least one part".to_string(),
            ));
        }

        // Verify parts are in ascending order
        for i in 1..parts.len() {
            if parts[i].part_number <= parts[i - 1].part_number {
                return Err(S3Error::InvalidPartOrder);
            }
        }

        // Verify all parts exist and ETags match
        let mut combined_data = Vec::new();
        for part_spec in &parts {
            let part = upload
                .parts
                .get(&part_spec.part_number)
                .ok_or_else(|| S3Error::InvalidPart)?;

            // Verify ETag matches (handle both quoted and unquoted)
            let expected_etag = part_spec.etag.trim_matches('"');
            let actual_etag = part.etag.trim_matches('"');

            if expected_etag != actual_etag {
                return Err(S3Error::InvalidPart);
            }

            // Validate part size (all parts except last must be >= min_part_size)
            if part_spec.part_number != parts.last().unwrap().part_number {
                if part.size < self.config.min_part_size as u64 {
                    return Err(S3Error::EntityTooSmall);
                }
            }

            combined_data.extend_from_slice(&part.data);
        }

        // Store the complete object
        let object_key = ObjectKey::new(&upload.bucket, &upload.key);
        let manifest = self
            .storage
            .put_object(
                &upload.bucket,
                &upload.key,
                &combined_data,
                upload.metadata,
            )
            .await
            .map_err(S3Error::from)?;

        info!(
            "Completed multipart upload {} - created object {}/{}",
            upload_id, upload.bucket, upload.key
        );

        Ok(manifest)
    }

    /// Abort multipart upload - cleanup all parts
    pub async fn abort_upload(&self, upload_id: &str) -> S3Result<()> {
        info!("Aborting multipart upload: {}", upload_id);

        let mut uploads = self.uploads.write().await;
        uploads
            .remove(upload_id)
            .ok_or_else(|| S3Error::NoSuchUpload(upload_id.to_string()))?;

        debug!("Aborted multipart upload: {}", upload_id);
        Ok(())
    }

    /// List parts for an upload
    pub async fn list_parts(
        &self,
        upload_id: &str,
        max_parts: Option<u32>,
        part_number_marker: Option<u32>,
    ) -> S3Result<ListPartsResult> {
        debug!("Listing parts for upload: {}", upload_id);

        let uploads = self.uploads.read().await;
        let upload = uploads
            .get(upload_id)
            .ok_or_else(|| S3Error::NoSuchUpload(upload_id.to_string()))?;

        let max_parts = max_parts.unwrap_or(1000).min(1000);
        let start_after = part_number_marker.unwrap_or(0);

        // Collect and sort parts
        let mut parts: Vec<UploadedPart> = upload
            .parts
            .values()
            .filter(|p| p.part_number > start_after)
            .cloned()
            .collect();

        parts.sort_by_key(|p| p.part_number);

        // Truncate if needed
        let is_truncated = parts.len() > max_parts as usize;
        let next_marker = if is_truncated {
            Some(parts[max_parts as usize - 1].part_number)
        } else {
            None
        };

        parts.truncate(max_parts as usize);

        Ok(ListPartsResult {
            upload_id: upload_id.to_string(),
            bucket: upload.bucket.clone(),
            key: upload.key.clone(),
            parts,
            is_truncated,
            next_part_number_marker: next_marker,
        })
    }

    /// List all active multipart uploads
    pub async fn list_uploads(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        max_uploads: Option<u32>,
        key_marker: Option<&str>,
        upload_id_marker: Option<&str>,
    ) -> S3Result<ListUploadsResult> {
        debug!("Listing multipart uploads for bucket: {}", bucket);

        let uploads = self.uploads.read().await;
        let max_uploads = max_uploads.unwrap_or(1000).min(1000);

        // Filter by bucket and prefix
        let mut matching_uploads: Vec<MultipartUploadInfo> = uploads
            .values()
            .filter(|u| u.bucket == bucket)
            .filter(|u| {
                if let Some(p) = prefix {
                    u.key.starts_with(p)
                } else {
                    true
                }
            })
            .filter(|u| {
                // Apply pagination markers
                if let Some(key_mark) = key_marker {
                    if u.key < key_mark {
                        return false;
                    }
                    if u.key == key_mark {
                        if let Some(upload_mark) = upload_id_marker {
                            return u.upload_id > upload_mark;
                        }
                    }
                }
                true
            })
            .map(|u| MultipartUploadInfo {
                key: u.key.clone(),
                upload_id: u.upload_id.clone(),
                initiated: u.created_at,
            })
            .collect();

        // Sort by key, then by upload_id
        matching_uploads.sort_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then_with(|| a.upload_id.cmp(&b.upload_id))
        });

        // Truncate if needed
        let is_truncated = matching_uploads.len() > max_uploads as usize;
        let (next_key_marker, next_upload_id_marker) = if is_truncated {
            let last = &matching_uploads[max_uploads as usize - 1];
            (Some(last.key.clone()), Some(last.upload_id.clone()))
        } else {
            (None, None)
        };

        matching_uploads.truncate(max_uploads as usize);

        Ok(ListUploadsResult {
            bucket: bucket.to_string(),
            uploads: matching_uploads,
            is_truncated,
            next_key_marker,
            next_upload_id_marker,
        })
    }

    /// Cleanup expired uploads
    pub async fn cleanup_expired(&self) -> S3Result<usize> {
        info!("Running cleanup of expired multipart uploads");

        let now = Utc::now();
        let mut uploads = self.uploads.write().await;

        let expired: Vec<String> = uploads
            .iter()
            .filter(|(_, u)| {
                let age = now.signed_duration_since(u.created_at);
                age > chrono::Duration::from_std(self.config.upload_expiry).unwrap()
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len();
        for upload_id in expired {
            uploads.remove(&upload_id);
            debug!("Cleaned up expired upload: {}", upload_id);
        }

        if count > 0 {
            info!("Cleaned up {} expired uploads", count);
        }

        Ok(count)
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(self: Arc<Self>) -> JoinHandle<()> {
        let interval = self.config.cleanup_interval;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);

            loop {
                interval.tick().await;

                match self.cleanup_expired().await {
                    Ok(count) => {
                        if count > 0 {
                            debug!("Cleanup task removed {} expired uploads", count);
                        }
                    }
                    Err(e) => {
                        warn!("Cleanup task error: {}", e);
                    }
                }
            }
        })
    }

    /// Get upload info (for testing/debugging)
    pub async fn get_upload(&self, upload_id: &str) -> Option<MultipartUpload> {
        self.uploads.read().await.get(upload_id).cloned()
    }

    /// Get number of active uploads
    pub async fn active_upload_count(&self) -> usize {
        self.uploads.read().await.len()
    }
}

#[derive(Debug, Clone)]
pub struct CompletePart {
    pub part_number: u32,
    pub etag: String,
}

#[derive(Debug, Clone)]
pub struct ListPartsResult {
    pub upload_id: String,
    pub bucket: String,
    pub key: String,
    pub parts: Vec<UploadedPart>,
    pub is_truncated: bool,
    pub next_part_number_marker: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct ListUploadsResult {
    pub bucket: String,
    pub uploads: Vec<MultipartUploadInfo>,
    pub is_truncated: bool,
    pub next_key_marker: Option<String>,
    pub next_upload_id_marker: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MultipartUploadInfo {
    pub key: String,
    pub upload_id: String,
    pub initiated: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anode_storage::{BucketConfig, StorageConfig};
    use tempfile::TempDir;

    async fn create_test_manager() -> (Arc<MultipartManager>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig::new(temp_dir.path().to_path_buf());
        let storage = Arc::new(StorageEngine::open(config).await.unwrap());

        // Create test bucket
        storage
            .create_bucket("test-bucket", BucketConfig::new("test-bucket"))
            .await
            .unwrap();

        let manager = Arc::new(MultipartManager::new(
            storage,
            MultipartConfig::default(),
        ));

        (manager, temp_dir)
    }

    #[tokio::test]
    async fn test_create_upload() {
        let (manager, _temp) = create_test_manager().await;

        let upload_id = manager
            .create_upload("test-bucket", "test-key", HashMap::new())
            .await
            .unwrap();

        assert!(!upload_id.is_empty());

        let upload = manager.get_upload(&upload_id).await;
        assert!(upload.is_some());

        let upload = upload.unwrap();
        assert_eq!(upload.bucket, "test-bucket");
        assert_eq!(upload.key, "test-key");
    }

    #[tokio::test]
    async fn test_upload_part() {
        let (manager, _temp) = create_test_manager().await;

        let upload_id = manager
            .create_upload("test-bucket", "test-key", HashMap::new())
            .await
            .unwrap();

        let data = Bytes::from_static(b"test data for part 1");
        let part = manager.upload_part(&upload_id, 1, data).await.unwrap();

        assert_eq!(part.part_number, 1);
        assert!(!part.etag.is_empty());
        assert_eq!(part.size, 20);
    }

    #[tokio::test]
    async fn test_complete_upload() {
        let (manager, _temp) = create_test_manager().await;

        let upload_id = manager
            .create_upload("test-bucket", "test-key", HashMap::new())
            .await
            .unwrap();

        // Upload multiple parts
        let data1 = Bytes::from(vec![1u8; 5 * 1024 * 1024]); // 5MB
        let data2 = Bytes::from(vec![2u8; 5 * 1024 * 1024]); // 5MB
        let data3 = Bytes::from(vec![3u8; 1024]); // 1KB (last part can be small)

        let part1 = manager.upload_part(&upload_id, 1, data1).await.unwrap();
        let part2 = manager.upload_part(&upload_id, 2, data2).await.unwrap();
        let part3 = manager.upload_part(&upload_id, 3, data3).await.unwrap();

        // Complete the upload
        let parts = vec![
            CompletePart {
                part_number: 1,
                etag: part1.etag.clone(),
            },
            CompletePart {
                part_number: 2,
                etag: part2.etag.clone(),
            },
            CompletePart {
                part_number: 3,
                etag: part3.etag.clone(),
            },
        ];

        let manifest = manager.complete_upload(&upload_id, parts).await.unwrap();

        assert_eq!(manifest.object_key.bucket, "test-bucket");
        assert_eq!(manifest.object_key.key, "test-key");
        assert_eq!(manifest.size, (5 * 1024 * 1024 * 2 + 1024) as u64);

        // Upload should be removed
        assert!(manager.get_upload(&upload_id).await.is_none());
    }

    #[tokio::test]
    async fn test_abort_upload() {
        let (manager, _temp) = create_test_manager().await;

        let upload_id = manager
            .create_upload("test-bucket", "test-key", HashMap::new())
            .await
            .unwrap();

        let data = Bytes::from_static(b"test data");
        manager.upload_part(&upload_id, 1, data).await.unwrap();

        // Abort the upload
        manager.abort_upload(&upload_id).await.unwrap();

        // Upload should be removed
        assert!(manager.get_upload(&upload_id).await.is_none());
    }

    #[tokio::test]
    async fn test_list_parts() {
        let (manager, _temp) = create_test_manager().await;

        let upload_id = manager
            .create_upload("test-bucket", "test-key", HashMap::new())
            .await
            .unwrap();

        // Upload multiple parts
        for i in 1..=5 {
            let data = Bytes::from(format!("part {}", i));
            manager.upload_part(&upload_id, i, data).await.unwrap();
        }

        // List all parts
        let result = manager.list_parts(&upload_id, None, None).await.unwrap();

        assert_eq!(result.parts.len(), 5);
        assert_eq!(result.bucket, "test-bucket");
        assert_eq!(result.key, "test-key");
        assert!(!result.is_truncated);

        // List with limit
        let result = manager
            .list_parts(&upload_id, Some(3), None)
            .await
            .unwrap();

        assert_eq!(result.parts.len(), 3);
        assert!(result.is_truncated);
        assert_eq!(result.next_part_number_marker, Some(3));
    }

    #[tokio::test]
    async fn test_list_uploads() {
        let (manager, _temp) = create_test_manager().await;

        // Create multiple uploads
        for i in 1..=3 {
            let key = format!("prefix/key-{}", i);
            manager
                .create_upload("test-bucket", &key, HashMap::new())
                .await
                .unwrap();
        }

        // List all uploads
        let result = manager
            .list_uploads("test-bucket", Some("prefix/"), None, None, None)
            .await
            .unwrap();

        assert_eq!(result.uploads.len(), 3);
        assert_eq!(result.bucket, "test-bucket");
        assert!(!result.is_truncated);

        // List with limit
        let result = manager
            .list_uploads("test-bucket", Some("prefix/"), Some(2), None, None)
            .await
            .unwrap();

        assert_eq!(result.uploads.len(), 2);
        assert!(result.is_truncated);
    }

    #[tokio::test]
    async fn test_part_size_validation() {
        let (manager, _temp) = create_test_manager().await;

        let upload_id = manager
            .create_upload("test-bucket", "test-key", HashMap::new())
            .await
            .unwrap();

        // Part too large
        let large_data = Bytes::from(vec![0u8; 6 * 1024 * 1024 * 1024]); // 6GB
        let result = manager.upload_part(&upload_id, 1, large_data).await;
        assert!(matches!(result, Err(S3Error::EntityTooLarge)));
    }

    #[tokio::test]
    async fn test_part_number_validation() {
        let (manager, _temp) = create_test_manager().await;

        let upload_id = manager
            .create_upload("test-bucket", "test-key", HashMap::new())
            .await
            .unwrap();

        let data = Bytes::from_static(b"test");

        // Part number 0 is invalid
        let result = manager.upload_part(&upload_id, 0, data.clone()).await;
        assert!(matches!(result, Err(S3Error::InvalidArgument(_))));

        // Part number too high
        let result = manager.upload_part(&upload_id, 10001, data).await;
        assert!(matches!(result, Err(S3Error::InvalidArgument(_))));
    }

    #[tokio::test]
    async fn test_invalid_part_order() {
        let (manager, _temp) = create_test_manager().await;

        let upload_id = manager
            .create_upload("test-bucket", "test-key", HashMap::new())
            .await
            .unwrap();

        let data = Bytes::from(vec![0u8; 5 * 1024 * 1024]);
        let part1 = manager.upload_part(&upload_id, 1, data.clone()).await.unwrap();
        let part2 = manager.upload_part(&upload_id, 2, data).await.unwrap();

        // Parts out of order
        let parts = vec![
            CompletePart {
                part_number: 2,
                etag: part2.etag,
            },
            CompletePart {
                part_number: 1,
                etag: part1.etag,
            },
        ];

        let result = manager.complete_upload(&upload_id, parts).await;
        assert!(matches!(result, Err(S3Error::InvalidPartOrder)));
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig::new(temp_dir.path().to_path_buf());
        let storage = Arc::new(StorageEngine::open(config).await.unwrap());

        storage
            .create_bucket("test-bucket", BucketConfig::new("test-bucket"))
            .await
            .unwrap();

        // Create manager with short expiry
        let mut multipart_config = MultipartConfig::default();
        multipart_config.upload_expiry = Duration::from_secs(1);

        let manager = Arc::new(MultipartManager::new(storage, multipart_config));

        // Create an upload
        manager
            .create_upload("test-bucket", "test-key", HashMap::new())
            .await
            .unwrap();

        assert_eq!(manager.active_upload_count().await, 1);

        // Wait for expiry
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Run cleanup
        let count = manager.cleanup_expired().await.unwrap();
        assert_eq!(count, 1);
        assert_eq!(manager.active_upload_count().await, 0);
    }

    #[tokio::test]
    async fn test_concurrent_part_uploads() {
        let (manager, _temp) = create_test_manager().await;

        let upload_id = manager
            .create_upload("test-bucket", "test-key", HashMap::new())
            .await
            .unwrap();

        // Upload parts concurrently
        let mut handles = vec![];
        for i in 1..=10 {
            let manager = manager.clone();
            let upload_id = upload_id.clone();
            let handle = tokio::spawn(async move {
                let data = Bytes::from(vec![i as u8; 5 * 1024 * 1024]);
                manager.upload_part(&upload_id, i, data).await
            });
            handles.push(handle);
        }

        // Wait for all uploads
        let mut parts = vec![];
        for handle in handles {
            let part = handle.await.unwrap().unwrap();
            parts.push(CompletePart {
                part_number: part.part_number,
                etag: part.etag,
            });
        }

        // Sort parts by number
        parts.sort_by_key(|p| p.part_number);

        // Complete the upload
        let manifest = manager.complete_upload(&upload_id, parts).await.unwrap();
        assert_eq!(manifest.size, (10 * 5 * 1024 * 1024) as u64);
    }
}
