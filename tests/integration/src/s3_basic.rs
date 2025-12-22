//! Basic S3 API operation tests
//!
//! Tests for fundamental S3 operations including:
//! - Bucket creation, deletion, and listing
//! - Object put, get, and delete operations
//! - Object metadata and conditional operations
//! - Range requests
//! - Object copying

use crate::{
    create_test_s3_client, random_bytes, setup_cluster, unique_bucket_name, unique_object_key,
    BucketFixture,
};
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_create_and_delete_bucket() {
    let cluster = setup_cluster(3).await.unwrap();
    let client = create_test_s3_client(&cluster).await;

    let bucket = unique_bucket_name("test-bucket");

    // Create bucket
    let create_result = client.create_bucket().bucket(&bucket).send().await;
    assert!(
        create_result.is_ok(),
        "Failed to create bucket: {:?}",
        create_result.err()
    );

    // Verify bucket exists with head_bucket
    let head_result = client.head_bucket().bucket(&bucket).send().await;
    assert!(
        head_result.is_ok(),
        "Bucket should exist: {:?}",
        head_result.err()
    );

    // Verify bucket appears in list
    let list_result = client.list_buckets().send().await.unwrap();
    let bucket_names: Vec<String> = list_result
        .buckets()
        .iter()
        .filter_map(|b| b.name().map(|n| n.to_string()))
        .collect();
    assert!(
        bucket_names.contains(&bucket),
        "Bucket should appear in list: {:?}",
        bucket_names
    );

    // Delete bucket
    let delete_result = client.delete_bucket().bucket(&bucket).send().await;
    assert!(
        delete_result.is_ok(),
        "Failed to delete bucket: {:?}",
        delete_result.err()
    );

    // Verify bucket is gone
    let head_result = client.head_bucket().bucket(&bucket).send().await;
    assert!(
        head_result.is_err(),
        "Bucket should not exist after deletion"
    );
}

#[tokio::test]
#[serial]
async fn test_put_get_object() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-put-get").await;

    let key = unique_object_key("test-object");
    let data = random_bytes(1024);

    // Put object
    let put_result = fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await;
    assert!(
        put_result.is_ok(),
        "Failed to put object: {:?}",
        put_result.err()
    );

    // Get object
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(
        data, retrieved_data,
        "Retrieved data should match original data"
    );
}

#[tokio::test]
#[serial]
async fn test_list_objects() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-list").await;

    // Create multiple objects
    let keys = vec!["obj-1", "obj-2", "obj-3", "prefix/obj-4"];
    for key in &keys {
        fixture
            .client
            .put_object()
            .bucket(&fixture.bucket)
            .key(*key)
            .body(ByteStream::from(Bytes::from_static(b"test data")))
            .send()
            .await
            .unwrap();
    }

    // List all objects
    let list_result = fixture
        .client
        .list_objects_v2()
        .bucket(&fixture.bucket)
        .send()
        .await
        .unwrap();

    let listed_keys: Vec<String> = list_result
        .contents()
        .iter()
        .filter_map(|obj| obj.key().map(|k| k.to_string()))
        .collect();

    assert_eq!(listed_keys.len(), 4, "Should list all objects");
    for key in &keys {
        assert!(
            listed_keys.contains(&key.to_string()),
            "Should list object: {}",
            key
        );
    }

    // List with prefix
    let list_result = fixture
        .client
        .list_objects_v2()
        .bucket(&fixture.bucket)
        .prefix("prefix/")
        .send()
        .await
        .unwrap();

    let listed_keys: Vec<String> = list_result
        .contents()
        .iter()
        .filter_map(|obj| obj.key().map(|k| k.to_string()))
        .collect();

    assert_eq!(listed_keys.len(), 1, "Should list only prefixed objects");
    assert_eq!(listed_keys[0], "prefix/obj-4");
}

#[tokio::test]
#[serial]
async fn test_delete_objects_batch() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-delete-batch").await;

    // Create multiple objects
    let keys = vec!["obj-1", "obj-2", "obj-3"];
    for key in &keys {
        fixture
            .client
            .put_object()
            .bucket(&fixture.bucket)
            .key(*key)
            .body(ByteStream::from(Bytes::from_static(b"test data")))
            .send()
            .await
            .unwrap();
    }

    // Delete objects one by one
    for key in &keys {
        let delete_result = fixture
            .client
            .delete_object()
            .bucket(&fixture.bucket)
            .key(*key)
            .send()
            .await;
        assert!(delete_result.is_ok(), "Failed to delete object: {}", key);
    }

    // Verify all objects are deleted
    let list_result = fixture
        .client
        .list_objects_v2()
        .bucket(&fixture.bucket)
        .send()
        .await
        .unwrap();

    assert_eq!(
        list_result.contents().len(),
        0,
        "All objects should be deleted"
    );
}

#[tokio::test]
#[serial]
async fn test_copy_object() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-copy").await;

    let source_key = unique_object_key("source");
    let dest_key = unique_object_key("dest");
    let data = random_bytes(1024);

    // Put source object
    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&source_key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();

    // Copy object
    let copy_source = format!("{}/{}", fixture.bucket, source_key);
    let copy_result = fixture
        .client
        .copy_object()
        .bucket(&fixture.bucket)
        .key(&dest_key)
        .copy_source(&copy_source)
        .send()
        .await;
    assert!(
        copy_result.is_ok(),
        "Failed to copy object: {:?}",
        copy_result.err()
    );

    // Verify destination object
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&dest_key)
        .send()
        .await
        .unwrap();

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(data, retrieved_data, "Copied object should match source");
}

#[tokio::test]
#[serial]
async fn test_object_metadata() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-metadata").await;

    let key = unique_object_key("test-object");
    let data = random_bytes(1024);

    // Put object with metadata
    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .content_type("application/octet-stream")
        .metadata("custom-key", "custom-value")
        .send()
        .await
        .unwrap();

    // Get object metadata with head_object
    let head_result = fixture
        .client
        .head_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    assert_eq!(head_result.content_length(), Some(1024));
    assert_eq!(head_result.content_type(), Some("application/octet-stream"));
    assert!(head_result
        .metadata()
        .map(|m| m.contains_key("custom-key"))
        .unwrap_or(false));
    assert_eq!(
        head_result
            .metadata()
            .and_then(|m| m.get("custom-key"))
            .map(|s| s.as_str()),
        Some("custom-value")
    );
}

#[tokio::test]
#[serial]
async fn test_range_get() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-range").await;

    let key = unique_object_key("test-object");
    let data = random_bytes(1024);

    // Put object
    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();

    // Get range of bytes (bytes 100-199)
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .range("bytes=100-199")
        .send()
        .await
        .unwrap();

    // Verify content range header before consuming body
    let content_range = get_result.content_range().map(|s| s.to_string());

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(retrieved_data.len(), 100, "Should retrieve 100 bytes");
    assert_eq!(retrieved_data, &data[100..200], "Range data should match");

    // Verify content range header
    assert_eq!(content_range.as_deref(), Some("bytes 100-199/1024"));
}

#[tokio::test]
#[serial]
async fn test_conditional_get() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-conditional").await;

    let key = unique_object_key("test-object");
    let data = random_bytes(1024);

    // Put object
    let put_result = fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();

    let etag = put_result.e_tag().unwrap().to_string();

    // Get with matching ETag (should succeed)
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .if_match(&etag)
        .send()
        .await;
    assert!(get_result.is_ok(), "Get with matching ETag should succeed");

    // Get with non-matching ETag (should fail with precondition failed)
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .if_match("\"wrong-etag\"")
        .send()
        .await;
    assert!(get_result.is_err(), "Get with wrong ETag should fail");

    // Get with if-none-match (should return 304 Not Modified)
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .if_none_match(&etag)
        .send()
        .await;
    assert!(
        get_result.is_err(),
        "Get with matching if-none-match should return 304"
    );
}

#[tokio::test]
#[serial]
async fn test_head_object() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-head").await;

    let key = unique_object_key("test-object");
    let data = random_bytes(2048);

    // Put object
    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(data.clone())))
        .send()
        .await
        .unwrap();

    // Head object
    let head_result = fixture
        .client
        .head_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    assert_eq!(head_result.content_length(), Some(2048));
    assert!(head_result.e_tag().is_some());
    assert!(head_result.last_modified().is_some());
}

#[tokio::test]
#[serial]
async fn test_object_overwrite() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-overwrite").await;

    let key = unique_object_key("test-object");

    // Put object v1
    let data_v1 = random_bytes(512);
    let put_result_v1 = fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(data_v1.clone())))
        .send()
        .await
        .unwrap();
    let etag_v1 = put_result_v1.e_tag().unwrap().to_string();

    // Put object v2 (overwrite)
    let data_v2 = random_bytes(1024);
    let put_result_v2 = fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(data_v2.clone())))
        .send()
        .await
        .unwrap();
    let etag_v2 = put_result_v2.e_tag().unwrap().to_string();

    // ETags should be different
    assert_ne!(etag_v1, etag_v2, "ETags should differ after overwrite");

    // Get object should return v2
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(retrieved_data, data_v2, "Retrieved data should be v2");
}

#[tokio::test]
#[serial]
async fn test_delete_nonexistent_object() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-delete-missing").await;

    let key = unique_object_key("nonexistent");

    // Delete should succeed even if object doesn't exist (S3 behavior)
    let delete_result = fixture
        .client
        .delete_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await;
    assert!(
        delete_result.is_ok(),
        "Delete of nonexistent object should succeed"
    );
}

#[tokio::test]
#[serial]
async fn test_empty_object() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-empty").await;

    let key = unique_object_key("empty-object");

    // Put empty object
    fixture
        .client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from_static(b"")))
        .send()
        .await
        .unwrap();

    // Get empty object
    let get_result = fixture
        .client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    let retrieved_data = get_result
        .body
        .collect()
        .await
        .unwrap()
        .into_bytes()
        .to_vec();

    assert_eq!(
        retrieved_data.len(),
        0,
        "Empty object should have zero bytes"
    );
}
