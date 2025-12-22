// Integration tests for multipart upload operations

use anode_s3::S3Router;
use anode_storage::{StorageConfig, StorageEngine};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

/// Helper to create a test storage engine and router
async fn setup_test_env() -> (Arc<StorageEngine>, axum::Router, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let config = StorageConfig::new(temp_dir.path().to_path_buf());
    let storage = Arc::new(StorageEngine::open(config).await.unwrap());

    let router = S3Router::new(storage.clone(), None).into_router();

    // Create test bucket via S3 API (PUT /{bucket})
    let create_bucket_req = axum::http::Request::builder()
        .method(axum::http::Method::PUT)
        .uri("/test-bucket")
        .body(axum::body::Body::empty())
        .unwrap();

    let response = router.clone().oneshot(create_bucket_req).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);

    (storage, router, temp_dir)
}

#[tokio::test]
async fn test_initiate_multipart_upload() {
    let (_storage, router, _temp) = setup_test_env().await;

    let request = Request::builder()
        .method(Method::POST)
        .uri("/test-bucket/test-key?uploads")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let xml = String::from_utf8(body.to_vec()).unwrap();

    assert!(xml.contains("<InitiateMultipartUploadResult"));
    assert!(xml.contains("<Bucket>test-bucket</Bucket>"));
    assert!(xml.contains("<Key>test-key</Key>"));
    assert!(xml.contains("<UploadId>"));
}

#[tokio::test]
async fn test_upload_part() {
    let (_storage, router, _temp) = setup_test_env().await;

    // First, initiate multipart upload
    let request = Request::builder()
        .method(Method::POST)
        .uri("/test-bucket/test-key?uploads")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let xml = String::from_utf8(body.to_vec()).unwrap();

    // Extract upload ID from XML
    let upload_id = extract_xml_tag(&xml, "UploadId").unwrap();

    // Upload a part
    let part_data = vec![1u8; 5 * 1024 * 1024]; // 5MB
    let request = Request::builder()
        .method(Method::PUT)
        .uri(format!(
            "/test-bucket/test-key?partNumber=1&uploadId={}",
            upload_id
        ))
        .body(Body::from(part_data))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key("ETag"));
}

#[tokio::test]
async fn test_complete_multipart_upload() {
    let (_storage, router, _temp) = setup_test_env().await;

    // Initiate multipart upload
    let request = Request::builder()
        .method(Method::POST)
        .uri("/test-bucket/test-key?uploads")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let xml = String::from_utf8(body.to_vec()).unwrap();
    let upload_id = extract_xml_tag(&xml, "UploadId").unwrap();

    // Upload parts
    let mut etags = Vec::new();
    for i in 1..=3 {
        let part_data = vec![i as u8; 5 * 1024 * 1024]; // 5MB per part
        let request = Request::builder()
            .method(Method::PUT)
            .uri(format!(
                "/test-bucket/test-key?partNumber={}&uploadId={}",
                i, upload_id
            ))
            .body(Body::from(part_data))
            .unwrap();

        let response = router.clone().oneshot(request).await.unwrap();
        let etag = response
            .headers()
            .get("ETag")
            .unwrap()
            .to_str()
            .unwrap()
            .trim_matches('"')
            .to_string();
        etags.push((i, etag));
    }

    // Complete the multipart upload
    let complete_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUpload>
    <Part>
        <PartNumber>1</PartNumber>
        <ETag>"{}"</ETag>
    </Part>
    <Part>
        <PartNumber>2</PartNumber>
        <ETag>"{}"</ETag>
    </Part>
    <Part>
        <PartNumber>3</PartNumber>
        <ETag>"{}"</ETag>
    </Part>
</CompleteMultipartUpload>"#,
        etags[0].1, etags[1].1, etags[2].1
    );

    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/test-bucket/test-key?uploadId={}", upload_id))
        .body(Body::from(complete_xml))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let xml = String::from_utf8(body.to_vec()).unwrap();

    assert!(xml.contains("<CompleteMultipartUploadResult"));
    assert!(xml.contains("<Bucket>test-bucket</Bucket>"));
    assert!(xml.contains("<Key>test-key</Key>"));
    assert!(xml.contains("<ETag>"));
}

#[tokio::test]
async fn test_abort_multipart_upload() {
    let (_storage, router, _temp) = setup_test_env().await;

    // Initiate multipart upload
    let request = Request::builder()
        .method(Method::POST)
        .uri("/test-bucket/test-key?uploads")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let xml = String::from_utf8(body.to_vec()).unwrap();
    let upload_id = extract_xml_tag(&xml, "UploadId").unwrap();

    // Upload a part
    let part_data = vec![1u8; 5 * 1024 * 1024];
    let request = Request::builder()
        .method(Method::PUT)
        .uri(format!(
            "/test-bucket/test-key?partNumber=1&uploadId={}",
            upload_id
        ))
        .body(Body::from(part_data))
        .unwrap();

    let _response = router.clone().oneshot(request).await.unwrap();

    // Abort the upload
    let request = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/test-bucket/test-key?uploadId={}", upload_id))
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_list_parts() {
    let (_storage, router, _temp) = setup_test_env().await;

    // Initiate multipart upload
    let request = Request::builder()
        .method(Method::POST)
        .uri("/test-bucket/test-key?uploads")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let xml = String::from_utf8(body.to_vec()).unwrap();
    let upload_id = extract_xml_tag(&xml, "UploadId").unwrap();

    // Upload multiple parts
    for i in 1..=5 {
        let part_data = vec![i as u8; 5 * 1024 * 1024];
        let request = Request::builder()
            .method(Method::PUT)
            .uri(format!(
                "/test-bucket/test-key?partNumber={}&uploadId={}",
                i, upload_id
            ))
            .body(Body::from(part_data))
            .unwrap();

        let _response = router.clone().oneshot(request).await.unwrap();
    }

    // List parts
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/test-bucket/test-key?uploadId={}", upload_id))
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let xml = String::from_utf8(body.to_vec()).unwrap();

    assert!(xml.contains("<ListPartsResult"));
    assert!(xml.contains("<UploadId>"));
    assert!(xml.contains("<Part>"));
    assert!(xml.contains("<PartNumber>1</PartNumber>"));
}

#[tokio::test]
async fn test_list_multipart_uploads() {
    let (_storage, router, _temp) = setup_test_env().await;

    // Initiate multiple uploads
    for i in 1..=3 {
        let request = Request::builder()
            .method(Method::POST)
            .uri(format!("/test-bucket/test-key-{}?uploads", i))
            .body(Body::empty())
            .unwrap();

        let _response = router.clone().oneshot(request).await.unwrap();
    }

    // List multipart uploads
    let request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket?uploads")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let xml = String::from_utf8(body.to_vec()).unwrap();

    assert!(xml.contains("<ListMultipartUploadsResult"));
    assert!(xml.contains("<Upload>"));
    assert!(xml.contains("<Key>test-key-1</Key>"));
}

#[tokio::test]
async fn test_large_file_multipart_upload() {
    let (_storage, router, _temp) = setup_test_env().await;

    // Initiate upload
    let request = Request::builder()
        .method(Method::POST)
        .uri("/test-bucket/large-file?uploads")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let xml = String::from_utf8(body.to_vec()).unwrap();
    let upload_id = extract_xml_tag(&xml, "UploadId").unwrap();

    // Upload 10 parts of 5MB each (50MB total)
    let mut etags = Vec::new();
    for i in 1..=10 {
        let part_data = vec![i as u8; 5 * 1024 * 1024];
        let request = Request::builder()
            .method(Method::PUT)
            .uri(format!(
                "/test-bucket/large-file?partNumber={}&uploadId={}",
                i, upload_id
            ))
            .body(Body::from(part_data))
            .unwrap();

        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let etag = response
            .headers()
            .get("ETag")
            .unwrap()
            .to_str()
            .unwrap()
            .trim_matches('"')
            .to_string();
        etags.push((i, etag));
    }

    // Complete upload
    let mut complete_xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUpload>"#,
    );

    for (part_num, etag) in &etags {
        complete_xml.push_str(&format!(
            r#"
    <Part>
        <PartNumber>{}</PartNumber>
        <ETag>"{}"</ETag>
    </Part>"#,
            part_num, etag
        ));
    }

    complete_xml.push_str("\n</CompleteMultipartUpload>");

    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/test-bucket/large-file?uploadId={}", upload_id))
        .body(Body::from(complete_xml))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_invalid_part_number() {
    let (_storage, router, _temp) = setup_test_env().await;

    // Initiate upload
    let request = Request::builder()
        .method(Method::POST)
        .uri("/test-bucket/test-key?uploads")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let xml = String::from_utf8(body.to_vec()).unwrap();
    let upload_id = extract_xml_tag(&xml, "UploadId").unwrap();

    // Try to upload part with invalid part number (0)
    let part_data = vec![1u8; 1024];
    let request = Request::builder()
        .method(Method::PUT)
        .uri(format!(
            "/test-bucket/test-key?partNumber=0&uploadId={}",
            upload_id
        ))
        .body(Body::from(part_data))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_nonexistent_upload_id() {
    let (_storage, router, _temp) = setup_test_env().await;

    // Try to upload part with non-existent upload ID
    let part_data = vec![1u8; 1024];
    let request = Request::builder()
        .method(Method::PUT)
        .uri("/test-bucket/test-key?partNumber=1&uploadId=nonexistent")
        .body(Body::from(part_data))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_multipart_with_metadata() {
    let (_storage, router, _temp) = setup_test_env().await;

    // Initiate upload with metadata
    let request = Request::builder()
        .method(Method::POST)
        .uri("/test-bucket/test-key?uploads")
        .header("x-amz-meta-author", "test-user")
        .header("x-amz-meta-version", "1.0")
        .body(Body::empty())
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let xml = String::from_utf8(body.to_vec()).unwrap();
    let upload_id = extract_xml_tag(&xml, "UploadId").unwrap();

    // Upload a part
    let part_data = vec![1u8; 5 * 1024 * 1024];
    let request = Request::builder()
        .method(Method::PUT)
        .uri(format!(
            "/test-bucket/test-key?partNumber=1&uploadId={}",
            upload_id
        ))
        .body(Body::from(part_data))
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    let etag = response
        .headers()
        .get("ETag")
        .unwrap()
        .to_str()
        .unwrap()
        .trim_matches('"')
        .to_string();

    // Complete upload
    let complete_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<CompleteMultipartUpload>
    <Part>
        <PartNumber>1</PartNumber>
        <ETag>"{}"</ETag>
    </Part>
</CompleteMultipartUpload>"#,
        etag
    );

    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/test-bucket/test-key?uploadId={}", upload_id))
        .body(Body::from(complete_xml))
        .unwrap();

    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

/// Helper to extract a tag value from XML
fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    let start = xml.find(&start_tag)? + start_tag.len();
    let end = xml.find(&end_tag)?;

    Some(xml[start..end].to_string())
}
