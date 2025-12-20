/// Integration tests for S3 copy and batch delete operations
use anode_protocol::ObjectKey;
use anode_s3::{
    batch::{BatchHandler, ObjectIdentifier},
    copy::{CopyHandler, CopyOptions, MetadataDirective},
};
use anode_storage::StorageEngine;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a test storage engine
async fn create_test_storage() -> (Arc<StorageEngine>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let storage = StorageEngine::new(temp_dir.path()).await.unwrap();
    (Arc::new(storage), temp_dir)
}

#[tokio::test]
async fn test_copy_within_bucket() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Create test object
    let source_key = ObjectKey::new("test-bucket", "source.txt");
    let data = Bytes::from("test data for copy");
    storage
        .put_object(source_key.clone(), data.clone(), None)
        .await
        .unwrap();

    // Copy the object
    let handler = CopyHandler::new(storage.clone());
    let result = handler
        .copy_object(
            "test-bucket",
            "source.txt",
            "test-bucket",
            "dest.txt",
            CopyOptions::default(),
        )
        .await
        .unwrap();

    // Verify the copy
    assert!(!result.etag.is_empty());
    assert!(result.last_modified.timestamp() > 0);

    // Verify destination object exists
    let dest_key = ObjectKey::new("test-bucket", "dest.txt");
    let copied_data = storage.get_object(&dest_key).await.unwrap();
    assert_eq!(copied_data, data);
}

#[tokio::test]
async fn test_copy_across_buckets() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Create test object in source bucket
    let source_key = ObjectKey::new("source-bucket", "file.txt");
    let data = Bytes::from("cross-bucket copy test");
    storage
        .put_object(source_key.clone(), data.clone(), None)
        .await
        .unwrap();

    // Copy to destination bucket
    let handler = CopyHandler::new(storage.clone());
    let result = handler
        .copy_object(
            "source-bucket",
            "file.txt",
            "dest-bucket",
            "file.txt",
            CopyOptions::default(),
        )
        .await
        .unwrap();

    assert!(!result.etag.is_empty());

    // Verify destination object exists
    let dest_key = ObjectKey::new("dest-bucket", "file.txt");
    let copied_data = storage.get_object(&dest_key).await.unwrap();
    assert_eq!(copied_data, data);
}

#[tokio::test]
async fn test_copy_with_metadata_replace() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Create test object
    let source_key = ObjectKey::new("test-bucket", "source.txt");
    let data = Bytes::from("test data");
    let mut original_metadata = HashMap::new();
    original_metadata.insert("original".to_string(), "value".to_string());

    storage
        .put_object(source_key.clone(), data.clone(), Some(original_metadata))
        .await
        .unwrap();

    // Copy with new metadata
    let mut new_metadata = HashMap::new();
    new_metadata.insert("new-key".to_string(), "new-value".to_string());

    let mut options = CopyOptions::default();
    options.metadata_directive = MetadataDirective::Replace;
    options.metadata = Some(new_metadata.clone());

    let handler = CopyHandler::new(storage.clone());
    let result = handler
        .copy_object(
            "test-bucket",
            "source.txt",
            "test-bucket",
            "dest.txt",
            options,
        )
        .await
        .unwrap();

    assert!(!result.etag.is_empty());
}

#[tokio::test]
async fn test_copy_with_conditional_match() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Create test object
    let source_key = ObjectKey::new("test-bucket", "source.txt");
    let data = Bytes::from("test data");
    let manifest = storage
        .put_object(source_key.clone(), data.clone(), None)
        .await
        .unwrap();

    // Copy with matching ETag (should succeed)
    let etag = format!("\"{}\"", hex::encode(manifest.checksum));
    let mut options = CopyOptions::default();
    options.copy_source_if_match = Some(etag);

    let handler = CopyHandler::new(storage.clone());
    let result = handler
        .copy_object(
            "test-bucket",
            "source.txt",
            "test-bucket",
            "dest.txt",
            options,
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_copy_with_conditional_no_match() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Create test object
    let source_key = ObjectKey::new("test-bucket", "source.txt");
    let data = Bytes::from("test data");
    storage
        .put_object(source_key.clone(), data.clone(), None)
        .await
        .unwrap();

    // Copy with non-matching ETag (should succeed)
    let mut options = CopyOptions::default();
    options.copy_source_if_none_match = Some("\"different-etag\"".to_string());

    let handler = CopyHandler::new(storage.clone());
    let result = handler
        .copy_object(
            "test-bucket",
            "source.txt",
            "test-bucket",
            "dest.txt",
            options,
        )
        .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_batch_delete_single_object() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Create test object
    let key = ObjectKey::new("test-bucket", "file1.txt");
    storage
        .put_object(key.clone(), Bytes::from("data1"), None)
        .await
        .unwrap();

    // Delete using batch handler
    let handler = BatchHandler::new(storage.clone());
    let objects = vec![ObjectIdentifier::new("file1.txt")];
    let result = handler
        .delete_objects("test-bucket", objects, false)
        .await
        .unwrap();

    assert_eq!(result.deleted.len(), 1);
    assert_eq!(result.errors.len(), 0);
    assert_eq!(result.deleted[0].key, "file1.txt");

    // Verify object is deleted
    let get_result = storage.get_object(&key).await;
    assert!(get_result.is_err());
}

#[tokio::test]
async fn test_batch_delete_multiple_objects() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Create test objects
    for i in 1..=5 {
        let key = ObjectKey::new("test-bucket", &format!("file{}.txt", i));
        storage
            .put_object(key, Bytes::from(format!("data{}", i)), None)
            .await
            .unwrap();
    }

    // Delete all using batch handler
    let handler = BatchHandler::new(storage.clone());
    let objects = (1..=5)
        .map(|i| ObjectIdentifier::new(format!("file{}.txt", i)))
        .collect();

    let result = handler
        .delete_objects("test-bucket", objects, false)
        .await
        .unwrap();

    assert_eq!(result.deleted.len(), 5);
    assert_eq!(result.errors.len(), 0);
    assert!(result.is_success());

    // Verify all objects are deleted
    for i in 1..=5 {
        let key = ObjectKey::new("test-bucket", &format!("file{}.txt", i));
        let get_result = storage.get_object(&key).await;
        assert!(get_result.is_err());
    }
}

#[tokio::test]
async fn test_batch_delete_quiet_mode() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Create test objects
    for i in 1..=3 {
        let key = ObjectKey::new("test-bucket", &format!("file{}.txt", i));
        storage
            .put_object(key, Bytes::from(format!("data{}", i)), None)
            .await
            .unwrap();
    }

    // Delete in quiet mode
    let handler = BatchHandler::new(storage.clone());
    let objects = (1..=3)
        .map(|i| ObjectIdentifier::new(format!("file{}.txt", i)))
        .collect();

    let result = handler
        .delete_objects("test-bucket", objects, true)
        .await
        .unwrap();

    // In quiet mode, deleted list should be empty (only errors are returned)
    assert_eq!(result.deleted.len(), 0);
    assert_eq!(result.errors.len(), 0);
}

#[tokio::test]
async fn test_batch_delete_nonexistent_objects() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Try to delete objects that don't exist
    let handler = BatchHandler::new(storage.clone());
    let objects = vec![
        ObjectIdentifier::new("nonexistent1.txt"),
        ObjectIdentifier::new("nonexistent2.txt"),
    ];

    let result = handler
        .delete_objects("test-bucket", objects, false)
        .await
        .unwrap();

    // S3 treats non-existent objects as successfully deleted (idempotent)
    assert_eq!(result.deleted.len(), 2);
    assert_eq!(result.errors.len(), 0);
}

#[tokio::test]
async fn test_batch_delete_partial_failures() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Create some objects
    let key1 = ObjectKey::new("test-bucket", "exists.txt");
    storage
        .put_object(key1, Bytes::from("data"), None)
        .await
        .unwrap();

    // Mix of existing and non-existing objects
    let handler = BatchHandler::new(storage.clone());
    let objects = vec![
        ObjectIdentifier::new("exists.txt"),
        ObjectIdentifier::new("nonexistent.txt"),
    ];

    let result = handler
        .delete_objects("test-bucket", objects, false)
        .await
        .unwrap();

    // Both should be marked as deleted (non-existent is treated as success)
    assert_eq!(result.deleted.len(), 2);
    assert_eq!(result.errors.len(), 0);
}

#[tokio::test]
async fn test_batch_delete_with_version_id() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Create test object
    let key = ObjectKey::new("test-bucket", "versioned.txt");
    storage
        .put_object(key.clone(), Bytes::from("data"), None)
        .await
        .unwrap();

    // Delete with version ID
    let handler = BatchHandler::new(storage.clone());
    let objects = vec![ObjectIdentifier::with_version("versioned.txt", "v1")];

    let result = handler
        .delete_objects("test-bucket", objects, false)
        .await
        .unwrap();

    // Should succeed (version ID is stored but not validated in basic implementation)
    assert_eq!(result.deleted.len(), 1);
    assert_eq!(result.deleted[0].version_id, Some("v1".to_string()));
}

#[tokio::test]
async fn test_delete_result_counts() {
    let (storage, _temp_dir) = create_test_storage().await;

    // Create test objects
    for i in 1..=10 {
        let key = ObjectKey::new("test-bucket", &format!("file{}.txt", i));
        storage
            .put_object(key, Bytes::from(format!("data{}", i)), None)
            .await
            .unwrap();
    }

    let handler = BatchHandler::new(storage.clone());
    let objects = (1..=10)
        .map(|i| ObjectIdentifier::new(format!("file{}.txt", i)))
        .collect();

    let result = handler
        .delete_objects("test-bucket", objects, false)
        .await
        .unwrap();

    assert_eq!(result.deleted_count(), 10);
    assert_eq!(result.error_count(), 0);
    assert!(result.is_success());
}
