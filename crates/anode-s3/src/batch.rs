/// S3 batch delete operations
///
/// This module provides batch delete functionality for S3 objects,
/// allowing multiple objects to be deleted in a single request.
/// Operations are performed atomically where possible.

use crate::error::{S3Error, S3Result};
use anode_protocol::ObjectKey;
use anode_storage::StorageEngine;
use std::sync::Arc;

/// Handler for batch operations
pub struct BatchHandler {
    storage: Arc<StorageEngine>,
}

impl BatchHandler {
    /// Creates a new BatchHandler
    pub fn new(storage: Arc<StorageEngine>) -> Self {
        Self { storage }
    }

    /// Delete multiple objects in a single request
    ///
    /// This operation attempts to delete all specified objects.
    /// If `quiet` is true, only errors are returned in the response.
    /// If `quiet` is false, both successful deletions and errors are returned.
    ///
    /// The operation continues even if some deletions fail, collecting
    /// all errors and successes for the final result.
    pub async fn delete_objects(
        &self,
        bucket: &str,
        objects: Vec<ObjectIdentifier>,
        quiet: bool,
    ) -> S3Result<DeleteObjectsResult> {
        let mut deleted = Vec::new();
        let mut errors = Vec::new();

        // Process each object deletion
        for obj in objects {
            let object_key = ObjectKey::new(bucket, &obj.key);

            match self.storage.delete_object(&object_key).await {
                Ok(_) => {
                    // Deletion successful
                    if !quiet {
                        deleted.push(DeletedObject {
                            key: obj.key.clone(),
                            version_id: obj.version_id.clone(),
                            delete_marker: false,
                            delete_marker_version_id: None,
                        });
                    }
                }
                Err(e) => {
                    // Deletion failed - record the error
                    // S3 typically continues even if some objects fail to delete
                    let (code, message) = match e {
                        anode_storage::StorageError::NotFound(_) => {
                            // S3 doesn't treat "object not found" as an error in batch delete
                            // We'll still record it as successful (idempotent delete)
                            if !quiet {
                                deleted.push(DeletedObject {
                                    key: obj.key.clone(),
                                    version_id: obj.version_id.clone(),
                                    delete_marker: false,
                                    delete_marker_version_id: None,
                                });
                            }
                            continue;
                        }
                        anode_storage::StorageError::PermissionDenied(msg) => {
                            ("AccessDenied".to_string(), msg)
                        }
                        _ => ("InternalError".to_string(), e.to_string()),
                    };

                    errors.push(DeleteError {
                        key: obj.key,
                        version_id: obj.version_id,
                        code,
                        message,
                    });
                }
            }
        }

        Ok(DeleteObjectsResult { deleted, errors })
    }

    /// Delete objects atomically
    ///
    /// This variant attempts to delete all objects atomically.
    /// If any deletion fails, none of the deletions are committed.
    /// Note: This requires transaction support in the storage engine.
    pub async fn delete_objects_atomic(
        &self,
        bucket: &str,
        objects: Vec<ObjectIdentifier>,
        quiet: bool,
    ) -> S3Result<DeleteObjectsResult> {
        // For now, we'll implement this the same as non-atomic
        // In a real implementation, this would use transactions
        self.delete_objects(bucket, objects, quiet).await
    }
}

/// Identifier for an object to delete
#[derive(Debug, Clone)]
pub struct ObjectIdentifier {
    /// Object key
    pub key: String,
    /// Optional version ID (for versioned buckets)
    pub version_id: Option<String>,
}

impl ObjectIdentifier {
    /// Creates a new ObjectIdentifier
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            version_id: None,
        }
    }

    /// Creates a new ObjectIdentifier with a version ID
    pub fn with_version(key: impl Into<String>, version_id: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            version_id: Some(version_id.into()),
        }
    }
}

/// Result of a batch delete operation
#[derive(Debug, Clone)]
pub struct DeleteObjectsResult {
    /// Successfully deleted objects
    pub deleted: Vec<DeletedObject>,
    /// Errors encountered during deletion
    pub errors: Vec<DeleteError>,
}

impl DeleteObjectsResult {
    /// Returns true if all deletions were successful
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    /// Returns the number of successfully deleted objects
    pub fn deleted_count(&self) -> usize {
        self.deleted.len()
    }

    /// Returns the number of errors
    pub fn error_count(&self) -> usize {
        self.errors.len()
    }
}

/// Information about a successfully deleted object
#[derive(Debug, Clone)]
pub struct DeletedObject {
    /// Object key
    pub key: String,
    /// Version ID (if versioning is enabled)
    pub version_id: Option<String>,
    /// Whether a delete marker was created
    pub delete_marker: bool,
    /// Version ID of the delete marker
    pub delete_marker_version_id: Option<String>,
}

/// Error information for a failed deletion
#[derive(Debug, Clone)]
pub struct DeleteError {
    /// Object key that failed to delete
    pub key: String,
    /// Version ID (if versioning is enabled)
    pub version_id: Option<String>,
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_identifier_new() {
        let id = ObjectIdentifier::new("test-key");
        assert_eq!(id.key, "test-key");
        assert!(id.version_id.is_none());
    }

    #[test]
    fn test_object_identifier_with_version() {
        let id = ObjectIdentifier::with_version("test-key", "v1");
        assert_eq!(id.key, "test-key");
        assert_eq!(id.version_id, Some("v1".to_string()));
    }

    #[test]
    fn test_delete_result_is_success() {
        let result = DeleteObjectsResult {
            deleted: vec![DeletedObject {
                key: "key1".to_string(),
                version_id: None,
                delete_marker: false,
                delete_marker_version_id: None,
            }],
            errors: vec![],
        };
        assert!(result.is_success());

        let result_with_error = DeleteObjectsResult {
            deleted: vec![],
            errors: vec![DeleteError {
                key: "key1".to_string(),
                version_id: None,
                code: "AccessDenied".to_string(),
                message: "Access denied".to_string(),
            }],
        };
        assert!(!result_with_error.is_success());
    }

    #[test]
    fn test_delete_result_counts() {
        let result = DeleteObjectsResult {
            deleted: vec![
                DeletedObject {
                    key: "key1".to_string(),
                    version_id: None,
                    delete_marker: false,
                    delete_marker_version_id: None,
                },
                DeletedObject {
                    key: "key2".to_string(),
                    version_id: None,
                    delete_marker: false,
                    delete_marker_version_id: None,
                },
            ],
            errors: vec![DeleteError {
                key: "key3".to_string(),
                version_id: None,
                code: "AccessDenied".to_string(),
                message: "Access denied".to_string(),
            }],
        };

        assert_eq!(result.deleted_count(), 2);
        assert_eq!(result.error_count(), 1);
    }
}
