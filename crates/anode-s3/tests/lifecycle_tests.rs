//! Integration tests for S3 bucket lifecycle configuration API.

use anode_s3::S3Router;
use anode_storage::{StorageConfig, StorageEngine};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::sync::Arc;
use tower::ServiceExt;

/// Helper to create a test router with unique temp directory
async fn create_test_router() -> axum::Router {
    // Use a unique temp directory for each test to avoid lock conflicts
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let config = StorageConfig::new(temp_dir.path().to_path_buf());
    let storage = Arc::new(
        StorageEngine::open(config)
            .await
            .expect("Failed to open storage engine"),
    );
    // Keep temp_dir alive by leaking it (tests are short-lived anyway)
    std::mem::forget(temp_dir);
    S3Router::new(storage, None).into_router()
}

#[tokio::test]
async fn test_put_get_lifecycle_configuration() {
    let app = create_test_router().await;

    // Create a bucket
    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Put lifecycle configuration
    let lifecycle_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Rule>
        <ID>rule-1</ID>
        <Status>Enabled</Status>
        <Filter>
            <Prefix>logs/</Prefix>
        </Filter>
        <Expiration>
            <Days>30</Days>
        </Expiration>
    </Rule>
</LifecycleConfiguration>"#;

    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket?lifecycle")
        .body(Body::from(lifecycle_xml))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Get lifecycle configuration
    let request = Request::builder()
        .method("GET")
        .uri("/test-bucket?lifecycle")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(body.contains("<ID>rule-1</ID>"));
    assert!(body.contains("<Status>Enabled</Status>"));
    assert!(body.contains("<Prefix>logs/</Prefix>"));
    assert!(body.contains("<Days>30</Days>"));
}

#[tokio::test]
async fn test_get_lifecycle_not_found() {
    let app = create_test_router().await;

    // Create a bucket
    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Try to get lifecycle configuration (should not exist)
    let request = Request::builder()
        .method("GET")
        .uri("/test-bucket?lifecycle")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(body.contains("<Code>NoSuchLifecycleConfiguration</Code>"));
}

#[tokio::test]
async fn test_delete_lifecycle_configuration() {
    let app = create_test_router().await;

    // Create a bucket
    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Put lifecycle configuration
    let lifecycle_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Rule>
        <ID>rule-1</ID>
        <Status>Enabled</Status>
        <Expiration>
            <Days>30</Days>
        </Expiration>
    </Rule>
</LifecycleConfiguration>"#;

    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket?lifecycle")
        .body(Body::from(lifecycle_xml))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Delete lifecycle configuration
    let request = Request::builder()
        .method("DELETE")
        .uri("/test-bucket?lifecycle")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify it's deleted
    let request = Request::builder()
        .method("GET")
        .uri("/test-bucket?lifecycle")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_lifecycle_multiple_rules() {
    let app = create_test_router().await;

    // Create a bucket
    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Put lifecycle configuration with multiple rules
    let lifecycle_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Rule>
        <ID>rule-1</ID>
        <Status>Enabled</Status>
        <Filter>
            <Prefix>logs/</Prefix>
        </Filter>
        <Expiration>
            <Days>30</Days>
        </Expiration>
    </Rule>
    <Rule>
        <ID>rule-2</ID>
        <Status>Enabled</Status>
        <Filter>
            <Prefix>tmp/</Prefix>
        </Filter>
        <Expiration>
            <Days>7</Days>
        </Expiration>
    </Rule>
    <Rule>
        <ID>rule-3</ID>
        <Status>Disabled</Status>
        <NoncurrentVersionExpiration>
            <NoncurrentDays>90</NoncurrentDays>
        </NoncurrentVersionExpiration>
    </Rule>
</LifecycleConfiguration>"#;

    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket?lifecycle")
        .body(Body::from(lifecycle_xml))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Get lifecycle configuration
    let request = Request::builder()
        .method("GET")
        .uri("/test-bucket?lifecycle")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(body.contains("<ID>rule-1</ID>"));
    assert!(body.contains("<ID>rule-2</ID>"));
    assert!(body.contains("<ID>rule-3</ID>"));
    assert!(body.contains("<Prefix>logs/</Prefix>"));
    assert!(body.contains("<Prefix>tmp/</Prefix>"));
    assert!(body.contains("<NoncurrentDays>90</NoncurrentDays>"));
}

#[tokio::test]
async fn test_lifecycle_abort_incomplete_multipart() {
    let app = create_test_router().await;

    // Create a bucket
    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Put lifecycle configuration with abort incomplete multipart upload
    let lifecycle_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Rule>
        <ID>abort-multipart</ID>
        <Status>Enabled</Status>
        <AbortIncompleteMultipartUpload>
            <DaysAfterInitiation>7</DaysAfterInitiation>
        </AbortIncompleteMultipartUpload>
    </Rule>
</LifecycleConfiguration>"#;

    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket?lifecycle")
        .body(Body::from(lifecycle_xml))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Get lifecycle configuration
    let request = Request::builder()
        .method("GET")
        .uri("/test-bucket?lifecycle")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(body.contains("<DaysAfterInitiation>7</DaysAfterInitiation>"));
}

#[tokio::test]
async fn test_lifecycle_invalid_configuration() {
    let app = create_test_router().await;

    // Create a bucket
    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Try to put invalid lifecycle configuration (no rules)
    let lifecycle_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
</LifecycleConfiguration>"#;

    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket?lifecycle")
        .body(Body::from(lifecycle_xml))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(body.contains("<Code>InvalidRequest</Code>"));
}

#[tokio::test]
async fn test_lifecycle_invalid_days_zero() {
    let app = create_test_router().await;

    // Create a bucket
    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Try to put lifecycle configuration with 0 days
    let lifecycle_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Rule>
        <ID>invalid-rule</ID>
        <Status>Enabled</Status>
        <Expiration>
            <Days>0</Days>
        </Expiration>
    </Rule>
</LifecycleConfiguration>"#;

    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket?lifecycle")
        .body(Body::from(lifecycle_xml))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_lifecycle_no_action() {
    let app = create_test_router().await;

    // Create a bucket
    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Try to put lifecycle configuration with no action
    let lifecycle_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<LifecycleConfiguration xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
    <Rule>
        <ID>no-action</ID>
        <Status>Enabled</Status>
        <Filter>
            <Prefix>test/</Prefix>
        </Filter>
    </Rule>
</LifecycleConfiguration>"#;

    let request = Request::builder()
        .method("PUT")
        .uri("/test-bucket?lifecycle")
        .body(Body::from(lifecycle_xml))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_lifecycle_bucket_not_found() {
    let app = create_test_router().await;

    // Try to get lifecycle configuration for non-existent bucket
    let request = Request::builder()
        .method("GET")
        .uri("/nonexistent-bucket?lifecycle")
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();

    assert!(body.contains("<Code>NoSuchBucket</Code>"));
}
