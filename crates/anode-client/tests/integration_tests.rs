use anode_client::{AnodeClient, ClientConfig};
use anode_client::bucket::BucketExt;
use anode_client::object::ObjectExt;
use bytes::Bytes;
use std::time::Duration;

/// Helper function to create a test client
///
/// Set the environment variable ANODE_TEST_ENDPOINT to run tests against a live cluster.
/// If not set, tests will be skipped.
fn get_test_client() -> Option<AnodeClient> {
    let endpoint = std::env::var("ANODE_TEST_ENDPOINT").ok()?;

    let mut config = ClientConfig::new(endpoint)
        .with_timeout(Duration::from_secs(30))
        .with_retries(3);

    // Optional credentials
    if let Ok(access_key) = std::env::var("ANODE_TEST_ACCESS_KEY") {
        if let Ok(secret_key) = std::env::var("ANODE_TEST_SECRET_KEY") {
            config = config.with_credentials(access_key, secret_key);
        }
    }

    AnodeClient::new(config).ok()
}

fn test_bucket_name() -> String {
    format!("anode-test-{}", uuid::Uuid::new_v4())
}

#[tokio::test]
async fn test_bucket_lifecycle() {
    let Some(client) = get_test_client() else {
        eprintln!("Skipping test: ANODE_TEST_ENDPOINT not set");
        return;
    };

    let bucket_name = test_bucket_name();

    // Create bucket
    client.create_bucket(&bucket_name).await.unwrap();

    // Check bucket exists
    let exists = client.head_bucket(&bucket_name).await.unwrap();
    assert!(exists, "Bucket should exist after creation");

    // List buckets
    let buckets = client.list_buckets().await.unwrap();
    assert!(
        buckets.iter().any(|b| b.name == bucket_name),
        "Created bucket should appear in list"
    );

    // Delete bucket
    client.delete_bucket(&bucket_name).await.unwrap();

    // Verify bucket is gone
    let exists = client.head_bucket(&bucket_name).await.unwrap();
    assert!(!exists, "Bucket should not exist after deletion");
}

#[tokio::test]
async fn test_object_put_get_delete() {
    let Some(client) = get_test_client() else {
        eprintln!("Skipping test: ANODE_TEST_ENDPOINT not set");
        return;
    };

    let bucket_name = test_bucket_name();
    client.create_bucket(&bucket_name).await.unwrap();

    let test_data = Bytes::from("Hello, Anode!");
    let key = "test-object.txt";

    // Put object
    let put_result = client
        .put_object(&bucket_name, key, test_data.clone())
        .await
        .unwrap();
    assert!(!put_result.etag.is_empty(), "ETag should not be empty");

    // Get object
    let get_result = client.get_object(&bucket_name, key).await.unwrap();
    assert_eq!(get_result.body, test_data, "Retrieved data should match");
    assert_eq!(get_result.metadata.key, key);
    assert_eq!(get_result.metadata.size, test_data.len() as u64);

    // Head object
    let metadata = client.head_object(&bucket_name, key).await.unwrap();
    assert_eq!(metadata.key, key);
    assert_eq!(metadata.size, test_data.len() as u64);
    assert!(!metadata.etag.is_empty());

    // Delete object
    client.delete_object(&bucket_name, key).await.unwrap();

    // Verify object is gone
    let get_result = client.get_object(&bucket_name, key).await;
    assert!(get_result.is_err(), "Object should not exist after deletion");

    // Cleanup
    client.delete_bucket(&bucket_name).await.unwrap();
}

#[tokio::test]
async fn test_list_objects() {
    let Some(client) = get_test_client() else {
        eprintln!("Skipping test: ANODE_TEST_ENDPOINT not set");
        return;
    };

    let bucket_name = test_bucket_name();
    client.create_bucket(&bucket_name).await.unwrap();

    // Create multiple objects
    let objects = vec![
        ("dir1/file1.txt", "content1"),
        ("dir1/file2.txt", "content2"),
        ("dir2/file3.txt", "content3"),
        ("root.txt", "root content"),
    ];

    for (key, content) in &objects {
        client
            .put_object(&bucket_name, key, Bytes::from(*content))
            .await
            .unwrap();
    }

    // List all objects
    let list_result = client.list_objects(&bucket_name, None, None).await.unwrap();
    assert_eq!(list_result.objects.len(), 4, "Should have 4 objects");

    // List with prefix
    let list_result = client
        .list_objects(&bucket_name, Some("dir1/"), None)
        .await
        .unwrap();
    assert_eq!(
        list_result.objects.len(),
        2,
        "Should have 2 objects with dir1/ prefix"
    );

    // List with max keys
    let list_result = client
        .list_objects(&bucket_name, None, Some(2))
        .await
        .unwrap();
    assert_eq!(list_result.objects.len(), 2, "Should return max 2 objects");
    assert!(list_result.is_truncated, "Should be truncated");

    // Cleanup
    for (key, _) in &objects {
        client.delete_object(&bucket_name, key).await.unwrap();
    }
    client.delete_bucket(&bucket_name).await.unwrap();
}

#[tokio::test]
async fn test_copy_object() {
    let Some(client) = get_test_client() else {
        eprintln!("Skipping test: ANODE_TEST_ENDPOINT not set");
        return;
    };

    let bucket_name = test_bucket_name();
    client.create_bucket(&bucket_name).await.unwrap();

    let src_key = "source.txt";
    let dst_key = "destination.txt";
    let test_data = Bytes::from("Copy me!");

    // Put source object
    client
        .put_object(&bucket_name, src_key, test_data.clone())
        .await
        .unwrap();

    // Copy object
    let copy_result = client
        .copy_object(&bucket_name, src_key, &bucket_name, dst_key)
        .await
        .unwrap();
    assert!(!copy_result.etag.is_empty());

    // Verify copied object
    let get_result = client.get_object(&bucket_name, dst_key).await.unwrap();
    assert_eq!(get_result.body, test_data);

    // Cleanup
    client.delete_object(&bucket_name, src_key).await.unwrap();
    client.delete_object(&bucket_name, dst_key).await.unwrap();
    client.delete_bucket(&bucket_name).await.unwrap();
}

#[tokio::test]
async fn test_range_request() {
    let Some(client) = get_test_client() else {
        eprintln!("Skipping test: ANODE_TEST_ENDPOINT not set");
        return;
    };

    let bucket_name = test_bucket_name();
    client.create_bucket(&bucket_name).await.unwrap();

    let key = "range-test.txt";
    let test_data = Bytes::from("0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ");

    // Put object
    client
        .put_object(&bucket_name, key, test_data.clone())
        .await
        .unwrap();

    // Get range
    let range_data = client
        .get_object_range(&bucket_name, key, 10, 19)
        .await
        .unwrap();
    assert_eq!(range_data, Bytes::from("ABCDEFGHIJ"));

    // Cleanup
    client.delete_object(&bucket_name, key).await.unwrap();
    client.delete_bucket(&bucket_name).await.unwrap();
}

#[tokio::test]
async fn test_multipart_upload() {
    let Some(client) = get_test_client() else {
        eprintln!("Skipping test: ANODE_TEST_ENDPOINT not set");
        return;
    };

    let bucket_name = test_bucket_name();
    client.create_bucket(&bucket_name).await.unwrap();

    let key = "multipart-test.bin";

    // Create multipart upload
    let mut upload = client.multipart_upload(&bucket_name, key);

    // Initialize
    let upload_id = upload.init().await.unwrap();
    assert!(!upload_id.is_empty());

    // Upload parts
    let part1_data = Bytes::from(vec![0u8; 1024 * 1024]); // 1 MB
    let part2_data = Bytes::from(vec![1u8; 1024 * 1024]); // 1 MB

    let part1 = upload.upload_part(1, part1_data.clone()).await.unwrap();
    assert_eq!(part1.part_number, 1);
    assert!(!part1.etag.is_empty());

    let part2 = upload.upload_part(2, part2_data.clone()).await.unwrap();
    assert_eq!(part2.part_number, 2);
    assert!(!part2.etag.is_empty());

    // Complete upload
    let complete_result = upload.complete().await.unwrap();
    assert_eq!(complete_result.bucket, bucket_name);
    assert_eq!(complete_result.key, key);
    assert!(!complete_result.etag.is_empty());

    // Verify object exists
    let metadata = client.head_object(&bucket_name, key).await.unwrap();
    assert_eq!(metadata.size, 2 * 1024 * 1024);

    // Cleanup
    client.delete_object(&bucket_name, key).await.unwrap();
    client.delete_bucket(&bucket_name).await.unwrap();
}

#[tokio::test]
async fn test_multipart_upload_abort() {
    let Some(client) = get_test_client() else {
        eprintln!("Skipping test: ANODE_TEST_ENDPOINT not set");
        return;
    };

    let bucket_name = test_bucket_name();
    client.create_bucket(&bucket_name).await.unwrap();

    let key = "multipart-abort-test.bin";

    // Create and abort multipart upload
    let mut upload = client.multipart_upload(&bucket_name, key);
    upload.init().await.unwrap();

    let part_data = Bytes::from(vec![0u8; 1024]);
    upload.upload_part(1, part_data).await.unwrap();

    // Abort the upload
    upload.abort().await.unwrap();

    // Verify object doesn't exist
    let get_result = client.get_object(&bucket_name, key).await;
    assert!(get_result.is_err());

    // Cleanup
    client.delete_bucket(&bucket_name).await.unwrap();
}

#[tokio::test]
async fn test_fluent_api() {
    let Some(client) = get_test_client() else {
        eprintln!("Skipping test: ANODE_TEST_ENDPOINT not set");
        return;
    };

    let bucket_name = test_bucket_name();

    // Test fluent bucket API
    let bucket = client.bucket(&bucket_name);
    bucket.create().await.unwrap();

    let exists = bucket.exists().await.unwrap();
    assert!(exists);

    // Test fluent object API
    let object = client.object(&bucket_name, "fluent-test.txt");
    let test_data = Bytes::from("Fluent API test");

    object.put(test_data.clone()).await.unwrap();

    let get_result = object.get().await.unwrap();
    assert_eq!(get_result.body, test_data);

    let metadata = object.head().await.unwrap();
    assert_eq!(metadata.key, "fluent-test.txt");

    object.delete().await.unwrap();

    // Cleanup
    bucket.delete().await.unwrap();
}

#[tokio::test]
async fn test_object_copy_fluent() {
    let Some(client) = get_test_client() else {
        eprintln!("Skipping test: ANODE_TEST_ENDPOINT not set");
        return;
    };

    let bucket_name = test_bucket_name();
    client.create_bucket(&bucket_name).await.unwrap();

    let test_data = Bytes::from("Copy with fluent API");

    // Create source object
    let src_object = client.object(&bucket_name, "source.txt");
    src_object.put(test_data.clone()).await.unwrap();

    // Copy to destination
    src_object
        .copy_to(&bucket_name, "destination.txt")
        .await
        .unwrap();

    // Verify destination
    let dst_object = client.object(&bucket_name, "destination.txt");
    let get_result = dst_object.get().await.unwrap();
    assert_eq!(get_result.body, test_data);

    // Cleanup
    src_object.delete().await.unwrap();
    dst_object.delete().await.unwrap();
    client.delete_bucket(&bucket_name).await.unwrap();
}

#[tokio::test]
async fn test_concurrent_uploads() {
    let Some(client) = get_test_client() else {
        eprintln!("Skipping test: ANODE_TEST_ENDPOINT not set");
        return;
    };

    let bucket_name = test_bucket_name();
    client.create_bucket(&bucket_name).await.unwrap();

    // Upload multiple objects concurrently
    let mut handles = vec![];

    for i in 0..10 {
        let client_clone = client.clone();
        let bucket_clone = bucket_name.clone();
        let handle = tokio::spawn(async move {
            let key = format!("concurrent-{}.txt", i);
            let data = Bytes::from(format!("Content {}", i));
            client_clone
                .put_object(&bucket_clone, &key, data)
                .await
                .unwrap();
            key
        });
        handles.push(handle);
    }

    // Wait for all uploads
    let keys: Vec<String> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // Verify all objects exist
    let list_result = client.list_objects(&bucket_name, None, None).await.unwrap();
    assert_eq!(list_result.objects.len(), 10);

    // Cleanup
    for key in keys {
        client.delete_object(&bucket_name, &key).await.unwrap();
    }
    client.delete_bucket(&bucket_name).await.unwrap();
}

#[tokio::test]
async fn test_large_object() {
    let Some(client) = get_test_client() else {
        eprintln!("Skipping test: ANODE_TEST_ENDPOINT not set");
        return;
    };

    let bucket_name = test_bucket_name();
    client.create_bucket(&bucket_name).await.unwrap();

    let key = "large-object.bin";
    // 10 MB object
    let large_data = Bytes::from(vec![42u8; 10 * 1024 * 1024]);

    // Put large object
    client
        .put_object(&bucket_name, key, large_data.clone())
        .await
        .unwrap();

    // Get large object
    let get_result = client.get_object(&bucket_name, key).await.unwrap();
    assert_eq!(get_result.body.len(), large_data.len());
    assert_eq!(get_result.body, large_data);

    // Cleanup
    client.delete_object(&bucket_name, key).await.unwrap();
    client.delete_bucket(&bucket_name).await.unwrap();
}
