/// S3 object copy operations
///
/// This module provides server-side copy functionality for S3 objects,
/// allowing objects to be copied within the same bucket or across buckets
/// without transferring data through the client.

use crate::error::{S3Error, S3Result};
use anode_protocol::ObjectKey;
use anode_storage::StorageEngine;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;

/// Handler for object copy operations
pub struct CopyHandler {
    storage: Arc<StorageEngine>,
}

impl CopyHandler {
    /// Creates a new CopyHandler
    pub fn new(storage: Arc<StorageEngine>) -> Self {
        Self { storage }
    }

    /// Copy object within same bucket or across buckets
    ///
    /// This performs a server-side copy, reading the source object and
    /// writing it to the destination without transferring through the client.
    pub async fn copy_object(
        &self,
        source_bucket: &str,
        source_key: &str,
        dest_bucket: &str,
        dest_key: &str,
        options: CopyOptions,
    ) -> S3Result<CopyResult> {
        // Check conditional copy requirements
        if let Some(etag) = &options.copy_source_if_match {
            let src_object_key = ObjectKey::new(source_bucket, source_key);
            let data = self
                .storage
                .get_object(&src_object_key)
                .await
                .map_err(|e| {
                    if matches!(e, anode_storage::StorageError::NotFound(_)) {
                        S3Error::NoSuchKey(source_key.to_string())
                    } else {
                        S3Error::StorageError(e)
                    }
                })?;

            // Get source metadata to check ETag
            // For now, we'll compute it from the data
            let computed_etag = compute_etag(&data);
            let expected_etag = etag.trim_matches('"');

            if computed_etag != expected_etag {
                return Err(S3Error::PreconditionFailed);
            }
        }

        if let Some(etag) = &options.copy_source_if_none_match {
            let src_object_key = ObjectKey::new(source_bucket, source_key);
            let data = self
                .storage
                .get_object(&src_object_key)
                .await
                .map_err(|e| {
                    if matches!(e, anode_storage::StorageError::NotFound(_)) {
                        S3Error::NoSuchKey(source_key.to_string())
                    } else {
                        S3Error::StorageError(e)
                    }
                })?;

            let computed_etag = compute_etag(&data);
            let expected_etag = etag.trim_matches('"');

            if computed_etag == expected_etag {
                return Err(S3Error::PreconditionFailed);
            }
        }

        // Perform the server-side copy
        let source = ObjectKey::new(source_bucket, source_key);
        self.server_side_copy(&source, dest_bucket, dest_key, options)
            .await
    }

    /// Server-side copy implementation (no data transfer through client)
    ///
    /// This retrieves the source object and stores it at the destination,
    /// optionally preserving or replacing metadata based on the directive.
    pub async fn server_side_copy(
        &self,
        source: &ObjectKey,
        dest_bucket: &str,
        dest_key: &str,
        options: CopyOptions,
    ) -> S3Result<CopyResult> {
        // Get source object
        let data = self.storage.get_object(source).await.map_err(|e| {
            if matches!(e, anode_storage::StorageError::NotFound(_)) {
                S3Error::NoSuchKey(source.key.clone())
            } else {
                S3Error::StorageError(e)
            }
        })?;

        // Determine metadata to use
        let metadata = match options.metadata_directive {
            MetadataDirective::Copy => {
                // In a real implementation, we would retrieve the source object's metadata
                // For now, we'll use an empty HashMap
                None
            }
            MetadataDirective::Replace => options.metadata,
        };

        // Store the copy at destination
        let dest_object_key = ObjectKey::new(dest_bucket, dest_key);
        let manifest = self
            .storage
            .put_object(dest_object_key, data, metadata)
            .await?;

        // Convert timestamp from i64 milliseconds to DateTime<Utc>
        let last_modified = DateTime::from_timestamp(
            manifest.modified_at / 1000,
            ((manifest.modified_at % 1000) * 1_000_000) as u32,
        )
        .unwrap_or_else(|| Utc::now());

        Ok(CopyResult {
            etag: hex::encode(manifest.checksum),
            last_modified,
        })
    }
}

/// Options for copy operations
#[derive(Debug, Default, Clone)]
pub struct CopyOptions {
    /// COPY (preserve metadata) or REPLACE (use new metadata)
    pub metadata_directive: MetadataDirective,
    /// New metadata if REPLACE
    pub metadata: Option<HashMap<String, String>>,
    /// Conditional copy - only if source matches etag
    pub copy_source_if_match: Option<String>,
    /// Conditional copy - only if source doesn't match
    pub copy_source_if_none_match: Option<String>,
    /// Conditional copy - only if modified since
    pub copy_source_if_modified_since: Option<DateTime<Utc>>,
    /// Conditional copy - only if unmodified since
    pub copy_source_if_unmodified_since: Option<DateTime<Utc>>,
}

/// Metadata directive for copy operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataDirective {
    /// Copy metadata from source object
    Copy,
    /// Replace with new metadata
    Replace,
}

impl Default for MetadataDirective {
    fn default() -> Self {
        Self::Copy
    }
}

/// Result of a copy operation
#[derive(Debug, Clone)]
pub struct CopyResult {
    /// ETag of the copied object
    pub etag: String,
    /// Last modified timestamp
    pub last_modified: DateTime<Utc>,
}

/// Computes an ETag from object data
fn compute_etag(data: &Bytes) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    hex::encode(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_directive_default() {
        let directive = MetadataDirective::default();
        assert_eq!(directive, MetadataDirective::Copy);
    }

    #[test]
    fn test_copy_options_default() {
        let options = CopyOptions::default();
        assert_eq!(options.metadata_directive, MetadataDirective::Copy);
        assert!(options.metadata.is_none());
        assert!(options.copy_source_if_match.is_none());
    }

    #[test]
    fn test_compute_etag() {
        let data = Bytes::from("test data");
        let etag = compute_etag(&data);
        assert_eq!(etag.len(), 64); // SHA-256 hex is 64 chars
    }

    #[test]
    fn test_compute_etag_consistency() {
        let data = Bytes::from("test data");
        let etag1 = compute_etag(&data);
        let etag2 = compute_etag(&data);
        assert_eq!(etag1, etag2);
    }
}
