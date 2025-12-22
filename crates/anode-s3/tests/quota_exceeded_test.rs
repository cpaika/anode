// Integration test for quota exceeded (403) error handling

use anode_s3::S3Router;
use anode_storage::{StorageConfig, StorageEngine};
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use std::sync::Arc;
use tower::Service;

#[tokio::test]
async fn test_quota_exceeded_returns_403() {
    // Create test storage engine
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let config = StorageConfig::new(temp_dir.path().to_path_buf());
    let storage = Arc::new(
        StorageEngine::open(config)
            .await
            .expect("Failed to open storage engine"),
    );

    let mut app = S3Router::new(storage.clone(), None).into_router();

    // Create a bucket
    let bucket_name = "quota-test-bucket";
    let request = Request::builder()
        .uri(&format!("/{}", bucket_name))
        .method("PUT")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut app, request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Set a very small quota (1000 bytes)
    storage
        .set_bucket_quota(bucket_name, Some(1000))
        .await
        .expect("Failed to set quota");

    // Try to upload a large object (2000 bytes) that exceeds quota
    let large_data = vec![0u8; 2000];
    let request = Request::builder()
        .uri(&format!("/{}/large-file.txt", bucket_name))
        .method("PUT")
        .header(header::CONTENT_LENGTH, large_data.len().to_string())
        .body(Body::from(large_data))
        .unwrap();

    let response = Service::call(&mut app, request).await.unwrap();

    // Verify we get 403 Forbidden
    assert_eq!(
        response.status(),
        StatusCode::FORBIDDEN,
        "Expected 403 FORBIDDEN when quota is exceeded"
    );

    // Verify Content-Type is XML
    let content_type = response.headers().get(header::CONTENT_TYPE);
    assert_eq!(
        content_type.map(|v| v.to_str().unwrap()),
        Some("application/xml"),
        "Expected XML content type"
    );

    // Read response body
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("Failed to read response body");
    let body_str = String::from_utf8(body_bytes.to_vec()).expect("Invalid UTF-8 in response");

    // Verify XML contains QuotaExceeded error code
    assert!(
        body_str.contains("<Code>QuotaExceeded</Code>"),
        "Response should contain QuotaExceeded error code. Body: {}",
        body_str
    );

    // Verify XML contains appropriate message
    assert!(
        body_str.contains("<Message>Quota exceeded</Message>"),
        "Response should contain quota exceeded message. Body: {}",
        body_str
    );

    // Verify XML structure
    assert!(
        body_str.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"),
        "Response should have XML declaration"
    );
    assert!(
        body_str.contains("<Error>") && body_str.contains("</Error>"),
        "Response should have Error element"
    );
}

#[tokio::test]
async fn test_quota_not_exceeded_succeeds() {
    // Create test storage engine
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let config = StorageConfig::new(temp_dir.path().to_path_buf());
    let storage = Arc::new(
        StorageEngine::open(config)
            .await
            .expect("Failed to open storage engine"),
    );

    let mut app = S3Router::new(storage.clone(), None).into_router();

    // Create a bucket
    let bucket_name = "quota-ok-bucket";
    let request = Request::builder()
        .uri(&format!("/{}", bucket_name))
        .method("PUT")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut app, request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Set a quota (10000 bytes)
    storage
        .set_bucket_quota(bucket_name, Some(10000))
        .await
        .expect("Failed to set quota");

    // Upload a small object (1000 bytes) that is within quota
    let small_data = vec![0u8; 1000];
    let request = Request::builder()
        .uri(&format!("/{}/small-file.txt", bucket_name))
        .method("PUT")
        .header(header::CONTENT_LENGTH, small_data.len().to_string())
        .body(Body::from(small_data))
        .unwrap();

    let response = Service::call(&mut app, request).await.unwrap();

    // Verify we get 200 OK
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Expected 200 OK when quota is not exceeded"
    );
}
