// Integration tests for versioned PUT operations

use anode_s3::{S3Router, VersionId};
use anode_storage::{BucketConfig, StorageConfig, StorageEngine};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::sync::Arc;
use tower::Service;
use tower::ServiceExt;

/// Helper to create a test router with unique temp directory
async fn create_test_router() -> (axum::Router, Arc<StorageEngine>) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let config = StorageConfig::new(temp_dir.path().to_path_buf());
    let storage = Arc::new(
        StorageEngine::open(config)
            .await
            .expect("Failed to open storage engine"),
    );
    // Keep temp_dir alive by leaking it (tests are short-lived anyway)
    std::mem::forget(temp_dir);
    let router = S3Router::new(storage.clone(), None).into_router();
    (router, storage)
}

/// Helper to create a bucket through the S3 API and optionally enable versioning
async fn create_bucket_with_versioning(
    app: &mut axum::Router,
    storage: &Arc<StorageEngine>,
    bucket_name: &str,
    versioning_enabled: bool,
) {
    // Create the bucket via HTTP API
    let create_bucket_req = Request::builder()
        .uri(&format!("/{}", bucket_name))
        .method("PUT")
        .body(Body::empty())
        .unwrap();

    let create_response = Service::call(app, create_bucket_req).await.unwrap();
    assert_eq!(
        create_response.status(),
        StatusCode::OK,
        "Failed to create bucket"
    );

    // If versioning should be enabled, update the bucket config in storage
    if versioning_enabled {
        // Create a new config with versioning enabled
        let mut new_config = BucketConfig::new(bucket_name);
        new_config.versioning_enabled = true;

        // Delete and recreate (simple approach for tests)
        storage.delete_bucket(bucket_name).await.ok();
        storage
            .create_bucket(bucket_name, new_config)
            .await
            .expect("Failed to recreate bucket with versioning");
    }
}

#[tokio::test]
async fn test_put_object_on_versioned_bucket_creates_version() {
    let (mut app, storage) = create_test_router().await;

    // Create a bucket with versioning enabled
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    // PUT an object to the versioned bucket
    let object_key = "test-object.txt";
    let object_data = "Hello, versioned world!";

    let request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from(object_data))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Verify response status
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "PUT object should return 200 OK"
    );

    // Verify version ID header is present
    let version_id_header = response.headers().get("x-amz-version-id");
    assert!(
        version_id_header.is_some(),
        "x-amz-version-id header should be present for versioned bucket"
    );

    let version_id = version_id_header.unwrap().to_str().unwrap();

    // Version ID should not be "null" for versioned buckets
    assert_ne!(
        version_id, "null",
        "Version ID should not be 'null' for versioned bucket"
    );

    // Verify the version ID is valid
    assert!(
        VersionId::is_valid(version_id),
        "Version ID should be valid: {}",
        version_id
    );
}

#[tokio::test]
async fn test_put_object_on_non_versioned_bucket_returns_null() {
    let (mut app, storage) = create_test_router().await;

    // Create a bucket WITHOUT versioning enabled
    let bucket_name = "non-versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, false).await;

    // PUT an object to the non-versioned bucket
    let object_key = "test-object.txt";
    let object_data = "Hello, world!";

    let request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from(object_data))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Verify response status
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "PUT object should return 200 OK"
    );

    // Verify version ID header is present but set to "null"
    let version_id_header = response.headers().get("x-amz-version-id");
    assert!(
        version_id_header.is_some(),
        "x-amz-version-id header should be present even for non-versioned bucket"
    );

    let version_id = version_id_header.unwrap().to_str().unwrap();
    assert_eq!(
        version_id, "null",
        "Version ID should be 'null' for non-versioned bucket"
    );
}

#[tokio::test]
async fn test_multiple_puts_create_unique_version_ids() {
    let (mut app, storage) = create_test_router().await;

    // Create a bucket with versioning enabled
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    let object_key = "test-object.txt";
    let mut version_ids = Vec::new();

    // PUT the same object multiple times
    for i in 0..5 {
        let object_data = format!("Version {} of the object", i);

        let request = Request::builder()
            .uri(&format!("/{}/{}", bucket_name, object_key))
            .method("PUT")
            .body(Body::from(object_data))
            .unwrap();

        let response = Service::call(&mut app, request).await.unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "PUT object {} should return 200 OK",
            i
        );

        // Extract and store version ID
        let version_id_header = response.headers().get("x-amz-version-id");
        assert!(
            version_id_header.is_some(),
            "x-amz-version-id header should be present for PUT {}",
            i
        );

        let version_id = version_id_header.unwrap().to_str().unwrap().to_string();
        assert_ne!(
            version_id, "null",
            "Version ID should not be 'null' for PUT {}",
            i
        );

        version_ids.push(version_id);

        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Verify all version IDs are unique
    for i in 0..version_ids.len() {
        for j in (i + 1)..version_ids.len() {
            assert_ne!(
                version_ids[i], version_ids[j],
                "Version IDs at indices {} and {} should be unique",
                i, j
            );
        }
    }

    // Verify all version IDs are valid
    for (i, version_id) in version_ids.iter().enumerate() {
        assert!(
            VersionId::is_valid(version_id),
            "Version ID at index {} should be valid: {}",
            i,
            version_id
        );
    }
}

#[tokio::test]
async fn test_version_ids_are_lexicographically_ordered() {
    let (mut app, storage) = create_test_router().await;

    // Create a bucket with versioning enabled
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    let object_key = "test-object.txt";
    let mut version_ids = Vec::new();

    // PUT the same object multiple times with delays to ensure different timestamps
    for i in 0..5 {
        let object_data = format!("Version {} of the object", i);

        let request = Request::builder()
            .uri(&format!("/{}/{}", bucket_name, object_key))
            .method("PUT")
            .body(Body::from(object_data))
            .unwrap();

        let response = Service::call(&mut app, request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let version_id_header = response.headers().get("x-amz-version-id");
        let version_id = version_id_header.unwrap().to_str().unwrap().to_string();
        version_ids.push(version_id);

        // Sleep to ensure different timestamps (version IDs are time-ordered)
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Verify version IDs are in lexicographic order (because they're timestamp-based)
    for i in 0..(version_ids.len() - 1) {
        assert!(
            version_ids[i] <= version_ids[i + 1],
            "Version ID at index {} should be <= version ID at index {}. Got: {} vs {}",
            i,
            i + 1,
            version_ids[i],
            version_ids[i + 1]
        );
    }
}

#[tokio::test]
async fn test_put_object_etag_present_with_version() {
    let (mut app, storage) = create_test_router().await;

    // Create a bucket with versioning enabled
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    // PUT an object
    let object_key = "test-object.txt";
    let object_data = "Hello, versioned world!";

    let request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from(object_data))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Verify both ETag and version ID are present
    assert_eq!(response.status(), StatusCode::OK);

    let etag_header = response.headers().get("ETag");
    assert!(etag_header.is_some(), "ETag header should be present");

    let version_id_header = response.headers().get("x-amz-version-id");
    assert!(
        version_id_header.is_some(),
        "x-amz-version-id header should be present"
    );

    // Verify both are non-empty
    let etag = etag_header.unwrap().to_str().unwrap();
    assert!(!etag.is_empty(), "ETag should not be empty");

    let version_id = version_id_header.unwrap().to_str().unwrap();
    assert!(!version_id.is_empty(), "Version ID should not be empty");
    assert_ne!(version_id, "null", "Version ID should not be 'null'");
}

#[tokio::test]
async fn test_overwrite_object_creates_new_version() {
    let (mut app, storage) = create_test_router().await;

    // Create a bucket with versioning enabled
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    let object_key = "test-object.txt";

    // First PUT
    let request1 = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Version 1"))
        .unwrap();

    let response1 = Service::call(&mut app, request1).await.unwrap();
    assert_eq!(response1.status(), StatusCode::OK);
    let version_id1 = response1
        .headers()
        .get("x-amz-version-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Wait to ensure different timestamp
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // Second PUT (overwrite)
    let request2 = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Version 2"))
        .unwrap();

    let response2 = Service::call(&mut app, request2).await.unwrap();
    assert_eq!(response2.status(), StatusCode::OK);
    let version_id2 = response2
        .headers()
        .get("x-amz-version-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Verify version IDs are different
    assert_ne!(
        version_id1, version_id2,
        "Overwriting an object should create a new version"
    );

    // Verify second version is "greater" (newer timestamp)
    assert!(
        version_id2 > version_id1,
        "Newer version ID should be lexicographically greater"
    );
}
