// Integration tests for versioned DELETE operations and delete markers

use anode_s3::{S3Router, VersionId};
use anode_storage::{BucketConfig, StorageConfig, StorageEngine};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use std::sync::Arc;
use tower::Service;

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
async fn test_delete_on_versioned_bucket_creates_delete_marker() {
    let (mut app, storage) = create_test_router().await;

    // Create a versioned bucket
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    let object_key = "test-object.txt";

    // PUT an object
    let put_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Test data"))
        .unwrap();

    let put_response = Service::call(&mut app, put_request).await.unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);

    // DELETE the object (should create delete marker, not actually delete)
    let delete_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let delete_response = Service::call(&mut app, delete_request).await.unwrap();
    assert_eq!(
        delete_response.status(),
        StatusCode::NO_CONTENT,
        "DELETE should return 204 No Content"
    );

    // Verify delete marker version ID is returned
    let delete_marker_version_id = delete_response.headers().get("x-amz-version-id");
    assert!(
        delete_marker_version_id.is_some(),
        "DELETE should return x-amz-version-id header for delete marker"
    );

    // Verify delete marker flag is present
    let delete_marker_flag = delete_response.headers().get("x-amz-delete-marker");
    assert!(
        delete_marker_flag.is_some(),
        "DELETE should return x-amz-delete-marker header"
    );
    assert_eq!(
        delete_marker_flag.unwrap().to_str().unwrap(),
        "true",
        "x-amz-delete-marker should be 'true'"
    );
}

#[tokio::test]
async fn test_delete_marker_has_unique_version_id() {
    let (mut app, storage) = create_test_router().await;

    // Create a versioned bucket
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    let object_key = "test-object.txt";

    // PUT an object
    let put_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Test data"))
        .unwrap();

    let put_response = Service::call(&mut app, put_request).await.unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);
    let object_version_id = put_response
        .headers()
        .get("x-amz-version-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Wait to ensure different timestamp
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // DELETE the object
    let delete_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let delete_response = Service::call(&mut app, delete_request).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let delete_marker_version_id = delete_response
        .headers()
        .get("x-amz-version-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Verify delete marker has a different version ID than the object
    assert_ne!(
        object_version_id, delete_marker_version_id,
        "Delete marker should have a unique version ID"
    );

    // Verify the version ID is valid
    assert!(
        VersionId::is_valid(&delete_marker_version_id),
        "Delete marker version ID should be valid"
    );
}

#[tokio::test]
async fn test_get_on_object_with_delete_marker_returns_404() {
    let (mut app, storage) = create_test_router().await;

    // Create a versioned bucket
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    let object_key = "test-object.txt";

    // PUT an object
    let put_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Test data"))
        .unwrap();

    let put_response = Service::call(&mut app, put_request).await.unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);

    // DELETE the object (creates delete marker)
    let delete_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let delete_response = Service::call(&mut app, delete_request).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // GET the object (without version ID) should return 404
    let get_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let get_response = Service::call(&mut app, get_request).await.unwrap();
    assert_eq!(
        get_response.status(),
        StatusCode::NOT_FOUND,
        "GET on object with delete marker should return 404"
    );

    // Verify the response includes delete marker information
    let delete_marker_flag = get_response.headers().get("x-amz-delete-marker");
    assert!(
        delete_marker_flag.is_some(),
        "404 response should include x-amz-delete-marker header"
    );
    assert_eq!(
        delete_marker_flag.unwrap().to_str().unwrap(),
        "true",
        "x-amz-delete-marker should be 'true'"
    );
}

#[tokio::test]
async fn test_get_with_version_id_works_after_delete_marker() {
    let (mut app, storage) = create_test_router().await;

    // Create a versioned bucket
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    let object_key = "test-object.txt";

    // PUT an object
    let put_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Test data"))
        .unwrap();

    let put_response = Service::call(&mut app, put_request).await.unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);
    let object_version_id = put_response
        .headers()
        .get("x-amz-version-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // DELETE the object (creates delete marker)
    let delete_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let delete_response = Service::call(&mut app, delete_request).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // GET the object WITH the specific version ID should still work
    let get_request = Request::builder()
        .uri(&format!(
            "/{}/{}?versionId={}",
            bucket_name, object_key, object_version_id
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let get_response = Service::call(&mut app, get_request).await.unwrap();
    assert_eq!(
        get_response.status(),
        StatusCode::OK,
        "GET with specific version ID should work even after delete marker"
    );

    // Verify we got the correct data
    let body_bytes = axum::body::to_bytes(get_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert_eq!(body_str, "Test data");
}

#[tokio::test]
async fn test_delete_with_version_id_permanently_deletes() {
    let (mut app, storage) = create_test_router().await;

    // Create a versioned bucket
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    let object_key = "test-object.txt";

    // PUT first version
    let put_request1 = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Version 1"))
        .unwrap();

    let put_response1 = Service::call(&mut app, put_request1).await.unwrap();
    assert_eq!(put_response1.status(), StatusCode::OK);
    let version_id1 = put_response1
        .headers()
        .get("x-amz-version-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    // PUT second version
    let put_request2 = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Version 2"))
        .unwrap();

    let put_response2 = Service::call(&mut app, put_request2).await.unwrap();
    assert_eq!(put_response2.status(), StatusCode::OK);

    // DELETE with specific version ID (should permanently delete that version)
    let delete_request = Request::builder()
        .uri(&format!(
            "/{}/{}?versionId={}",
            bucket_name, object_key, version_id1
        ))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let delete_response = Service::call(&mut app, delete_request).await.unwrap();
    assert_eq!(
        delete_response.status(),
        StatusCode::NO_CONTENT,
        "DELETE with versionId should return 204"
    );

    // Verify the response includes the deleted version ID
    let deleted_version_id = delete_response.headers().get("x-amz-version-id");
    assert!(deleted_version_id.is_some());
    assert_eq!(
        deleted_version_id.unwrap().to_str().unwrap(),
        version_id1,
        "Response should include the deleted version ID"
    );

    // Verify NO delete marker flag (permanent deletion, not delete marker)
    let delete_marker_flag = delete_response.headers().get("x-amz-delete-marker");
    assert!(
        delete_marker_flag.is_none() || delete_marker_flag.unwrap().to_str().unwrap() == "false",
        "Permanent deletion should not set x-amz-delete-marker to true"
    );

    // Try to GET the deleted version - should return 404
    let get_request = Request::builder()
        .uri(&format!(
            "/{}/{}?versionId={}",
            bucket_name, object_key, version_id1
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let get_response = Service::call(&mut app, get_request).await.unwrap();
    assert_eq!(
        get_response.status(),
        StatusCode::NOT_FOUND,
        "GET on permanently deleted version should return 404"
    );
}

#[tokio::test]
async fn test_delete_on_non_versioned_bucket_actually_deletes() {
    let (mut app, storage) = create_test_router().await;

    // Create a NON-versioned bucket
    let bucket_name = "non-versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, false).await;

    let object_key = "test-object.txt";

    // PUT an object
    let put_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Test data"))
        .unwrap();

    let put_response = Service::call(&mut app, put_request).await.unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);

    // DELETE the object (should actually delete, not create delete marker)
    let delete_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let delete_response = Service::call(&mut app, delete_request).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // Verify NO delete marker flag
    let delete_marker_flag = delete_response.headers().get("x-amz-delete-marker");
    assert!(
        delete_marker_flag.is_none() || delete_marker_flag.unwrap().to_str().unwrap() == "false",
        "Non-versioned bucket deletion should not create delete marker"
    );

    // GET should return 404
    let get_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let get_response = Service::call(&mut app, get_request).await.unwrap();
    assert_eq!(
        get_response.status(),
        StatusCode::NOT_FOUND,
        "GET on deleted object in non-versioned bucket should return 404"
    );
}

#[tokio::test]
async fn test_multiple_delete_markers_create_new_versions() {
    let (mut app, storage) = create_test_router().await;

    // Create a versioned bucket
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    let object_key = "test-object.txt";

    // PUT an object
    let put_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Test data"))
        .unwrap();

    let put_response = Service::call(&mut app, put_request).await.unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);

    let mut delete_marker_version_ids = Vec::new();

    // Create multiple delete markers
    for _ in 0..3 {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let delete_request = Request::builder()
            .uri(&format!("/{}/{}", bucket_name, object_key))
            .method("DELETE")
            .body(Body::empty())
            .unwrap();

        let delete_response = Service::call(&mut app, delete_request).await.unwrap();
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let version_id = delete_response
            .headers()
            .get("x-amz-version-id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        delete_marker_version_ids.push(version_id);
    }

    // Verify all delete marker version IDs are unique
    for i in 0..delete_marker_version_ids.len() {
        for j in (i + 1)..delete_marker_version_ids.len() {
            assert_ne!(
                delete_marker_version_ids[i], delete_marker_version_ids[j],
                "Each delete marker should have a unique version ID"
            );
        }
    }
}

#[tokio::test]
async fn test_deleting_delete_marker_removes_it() {
    let (mut app, storage) = create_test_router().await;

    // Create a versioned bucket
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    let object_key = "test-object.txt";

    // PUT an object
    let put_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Test data"))
        .unwrap();

    let put_response = Service::call(&mut app, put_request).await.unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);

    // Create delete marker
    let delete_request1 = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let delete_response1 = Service::call(&mut app, delete_request1).await.unwrap();
    assert_eq!(delete_response1.status(), StatusCode::NO_CONTENT);
    let delete_marker_version_id = delete_response1
        .headers()
        .get("x-amz-version-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Object should appear deleted (404)
    let get_request1 = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let get_response1 = Service::call(&mut app, get_request1).await.unwrap();
    assert_eq!(get_response1.status(), StatusCode::NOT_FOUND);

    // DELETE the delete marker itself (using its version ID)
    let delete_request2 = Request::builder()
        .uri(&format!(
            "/{}/{}?versionId={}",
            bucket_name, object_key, delete_marker_version_id
        ))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let delete_response2 = Service::call(&mut app, delete_request2).await.unwrap();
    assert_eq!(
        delete_response2.status(),
        StatusCode::NO_CONTENT,
        "DELETE on delete marker should succeed"
    );

    // Verify the response indicates we deleted a delete marker
    let delete_marker_flag = delete_response2.headers().get("x-amz-delete-marker");
    assert!(
        delete_marker_flag.is_some(),
        "Response should indicate delete marker was deleted"
    );
    assert_eq!(
        delete_marker_flag.unwrap().to_str().unwrap(),
        "true",
        "x-amz-delete-marker should be true when deleting a delete marker"
    );

    // Now GET should work again (object is "undeleted")
    let get_request2 = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let get_response2 = Service::call(&mut app, get_request2).await.unwrap();
    assert_eq!(
        get_response2.status(),
        StatusCode::OK,
        "GET should work after delete marker is removed"
    );

    // Verify we got the original data
    let body_bytes = axum::body::to_bytes(get_response2.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert_eq!(body_str, "Test data");
}

#[tokio::test]
async fn test_head_on_object_with_delete_marker_returns_404() {
    let (mut app, storage) = create_test_router().await;

    // Create a versioned bucket
    let bucket_name = "versioned-bucket";
    create_bucket_with_versioning(&mut app, &storage, bucket_name, true).await;

    let object_key = "test-object.txt";

    // PUT an object
    let put_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("PUT")
        .body(Body::from("Test data"))
        .unwrap();

    let put_response = Service::call(&mut app, put_request).await.unwrap();
    assert_eq!(put_response.status(), StatusCode::OK);

    // DELETE the object (creates delete marker)
    let delete_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let delete_response = Service::call(&mut app, delete_request).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    // HEAD the object (without version ID) should return 404
    let head_request = Request::builder()
        .uri(&format!("/{}/{}", bucket_name, object_key))
        .method("HEAD")
        .body(Body::empty())
        .unwrap();

    let head_response = Service::call(&mut app, head_request).await.unwrap();
    assert_eq!(
        head_response.status(),
        StatusCode::NOT_FOUND,
        "HEAD on object with delete marker should return 404"
    );

    // Verify the response includes delete marker information
    let delete_marker_flag = head_response.headers().get("x-amz-delete-marker");
    assert!(
        delete_marker_flag.is_some(),
        "404 response should include x-amz-delete-marker header"
    );
    assert_eq!(
        delete_marker_flag.unwrap().to_str().unwrap(),
        "true",
        "x-amz-delete-marker should be 'true'"
    );
}
