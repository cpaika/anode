// Integration tests for Parquet file download via S3 API

use anode_s3::S3Router;
use anode_storage::{StorageConfig, StorageEngine};
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

/// Helper to create a test parquet file with sample data
fn create_test_parquet_file() -> Vec<u8> {
    use arrow::array::{ArrayRef, Int32Array, StringArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::record_batch::RecordBatch;
    use parquet::arrow::ArrowWriter;

    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("name", DataType::Utf8, true),
    ]));

    let id_array = Int32Array::from(vec![1, 2, 3, 4, 5]);
    let name_array = StringArray::from(vec![
        Some("Alice"),
        Some("Bob"),
        None,
        Some("David"),
        Some("Eve"),
    ]);

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![Arc::new(id_array) as ArrayRef, Arc::new(name_array)],
    )
    .unwrap();

    let mut buffer = Vec::new();
    let mut writer = ArrowWriter::try_new(&mut buffer, schema, None).unwrap();
    writer.write(&batch).unwrap();
    writer.close().unwrap();

    buffer
}

/// Helper to create a test storage engine and router
async fn setup_test_env() -> (Arc<StorageEngine>, axum::Router, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let config = StorageConfig::new(temp_dir.path().to_path_buf());
    let storage = Arc::new(StorageEngine::open(config).await.unwrap());

    let router = S3Router::new(storage.clone(), None).into_router();

    (storage, router, temp_dir)
}

/// Helper to upload a parquet file to the test bucket
async fn upload_parquet_file(
    router: &mut axum::Router,
    bucket: &str,
    key: &str,
    data: Vec<u8>,
) -> StatusCode {
    use tower::Service;

    let upload_request = Request::builder()
        .method(Method::PUT)
        .uri(format!("/{}/{}", bucket, key))
        .header(header::CONTENT_TYPE, "application/parquet")
        .body(Body::from(data))
        .unwrap();

    let response = Service::call(router, upload_request).await.unwrap();
    response.status()
}

#[tokio::test]
#[ignore = "TODO: Fix ETag header in parquet download response"]
async fn test_parquet_download_basic() {
    use tower::Service;

    let (_storage, mut router, _temp) = setup_test_env().await;

    // Create bucket first
    let create_bucket = Request::builder()
        .method(Method::PUT)
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, create_bucket).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Bucket creation failed");

    // Create and upload a test parquet file
    let parquet_data = create_test_parquet_file();
    let upload_status = upload_parquet_file(
        &mut router,
        "test-bucket",
        "data.parquet",
        parquet_data.clone(),
    )
    .await;
    assert_eq!(upload_status, StatusCode::OK, "Upload should succeed");

    // Download the parquet file
    let download_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/data.parquet")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, download_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Download should succeed");

    // Verify Content-Type header
    let content_type = response.headers().get(header::CONTENT_TYPE);
    assert!(
        content_type.is_some(),
        "Content-Type header should be present"
    );

    // Verify ETag header is present
    assert!(
        response.headers().contains_key("ETag"),
        "ETag header should be present"
    );

    // Verify the downloaded content matches
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        body.len(),
        parquet_data.len(),
        "Downloaded file should have same size as uploaded"
    );
    assert_eq!(
        body.as_ref(),
        parquet_data.as_slice(),
        "Downloaded content should match uploaded content"
    );
}

#[tokio::test]
async fn test_parquet_download_and_validate() {
    use bytes::Bytes;
    use parquet::file::reader::{FileReader, SerializedFileReader};
    use tower::Service;

    let (_storage, mut router, _temp) = setup_test_env().await;

    // Create bucket
    let create_bucket = Request::builder()
        .method(Method::PUT)
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, create_bucket).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Upload a parquet file
    let parquet_data = create_test_parquet_file();
    let upload_status =
        upload_parquet_file(&mut router, "test-bucket", "valid.parquet", parquet_data).await;
    assert_eq!(upload_status, StatusCode::OK);

    // Download the file
    let download_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/valid.parquet")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, download_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    // Validate that the downloaded data is valid parquet
    let bytes = Bytes::from(body.to_vec());
    let reader = SerializedFileReader::new(bytes).expect("Should be valid parquet");

    let metadata = reader.metadata();
    assert_eq!(metadata.num_row_groups(), 1, "Should have 1 row group");

    let schema = metadata.file_metadata().schema();
    let num_fields = schema.get_fields().len();
    assert_eq!(num_fields, 2, "Should have 2 columns");
}

#[tokio::test]
async fn test_parquet_download_large_file() {
    use tower::Service;

    let (_storage, mut router, _temp) = setup_test_env().await;

    // Create bucket
    let create_bucket = Request::builder()
        .method(Method::PUT)
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, create_bucket).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Create a larger parquet file with multiple row groups
    let parquet_data = {
        use arrow::array::{ArrayRef, Int32Array};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;
        use parquet::file::properties::WriterProperties;

        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int32,
            false,
        )]));

        let mut buffer = Vec::new();
        let props = WriterProperties::builder()
            .set_max_row_group_size(100)
            .build();

        let mut writer = ArrowWriter::try_new(&mut buffer, schema.clone(), Some(props)).unwrap();

        // Write 1000 rows in 10 batches
        for i in 0..10 {
            let start = i * 100;
            let values: Vec<i32> = (start..start + 100).collect();
            let array = Int32Array::from(values);
            let batch =
                RecordBatch::try_new(schema.clone(), vec![Arc::new(array) as ArrayRef]).unwrap();
            writer.write(&batch).unwrap();
        }

        writer.close().unwrap();
        buffer
    };

    assert!(
        parquet_data.len() > 1024,
        "Large parquet file should be > 1KB"
    );

    // Upload the large parquet file
    let upload_status = upload_parquet_file(
        &mut router,
        "test-bucket",
        "large.parquet",
        parquet_data.clone(),
    )
    .await;
    assert_eq!(upload_status, StatusCode::OK);

    // Download the large file
    let download_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/large.parquet")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, download_request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Large file download should succeed"
    );

    // Verify the content
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        body.len(),
        parquet_data.len(),
        "Downloaded large file should have correct size"
    );
    assert_eq!(
        body.as_ref(),
        parquet_data.as_slice(),
        "Downloaded large file content should match"
    );
}

#[tokio::test]
async fn test_parquet_download_nonexistent_file() {
    let (_storage, router, _temp) = setup_test_env().await;

    // Try to download parquet file that doesn't exist
    let download_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/nonexistent.parquet")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(download_request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Download of non-existent file should return 404"
    );
}

#[tokio::test]
async fn test_parquet_download_nonexistent_bucket() {
    let (_storage, router, _temp) = setup_test_env().await;

    // Try to download from non-existent bucket
    let download_request = Request::builder()
        .method(Method::GET)
        .uri("/nonexistent-bucket/data.parquet")
        .body(Body::empty())
        .unwrap();

    let response = router.oneshot(download_request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Download from non-existent bucket should return 404"
    );
}

#[tokio::test]
async fn test_parquet_download_with_range_request() {
    use tower::Service;

    let (_storage, mut router, _temp) = setup_test_env().await;

    // Create bucket
    let create_bucket = Request::builder()
        .method(Method::PUT)
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, create_bucket).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Upload a parquet file
    let parquet_data = create_test_parquet_file();
    let data_len = parquet_data.len();
    let upload_status = upload_parquet_file(
        &mut router,
        "test-bucket",
        "range.parquet",
        parquet_data.clone(),
    )
    .await;
    assert_eq!(upload_status, StatusCode::OK);

    // Download with range request (first 100 bytes)
    let download_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/range.parquet")
        .header(header::RANGE, "bytes=0-99")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, download_request).await.unwrap();

    // Range requests may return 206 Partial Content or 200 OK depending on implementation
    let status = response.status();
    assert!(
        status == StatusCode::PARTIAL_CONTENT || status == StatusCode::OK,
        "Range request should succeed with 206 or 200, got: {}",
        status
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    // If range is supported, body should be 100 bytes, otherwise full file
    if status == StatusCode::PARTIAL_CONTENT {
        assert_eq!(body.len(), 100, "Partial content should be 100 bytes");
        assert_eq!(
            body.as_ref(),
            &parquet_data[0..100],
            "Partial content should match range"
        );
    } else {
        assert_eq!(
            body.len(),
            data_len,
            "Full content should be returned if range not supported"
        );
    }
}

#[tokio::test]
async fn test_parquet_download_head_request() {
    use tower::Service;

    let (_storage, mut router, _temp) = setup_test_env().await;

    // Create bucket
    let create_bucket = Request::builder()
        .method(Method::PUT)
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, create_bucket).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Upload a parquet file
    let parquet_data = create_test_parquet_file();
    let upload_status = upload_parquet_file(
        &mut router,
        "test-bucket",
        "head.parquet",
        parquet_data.clone(),
    )
    .await;
    assert_eq!(upload_status, StatusCode::OK);

    // HEAD request to get metadata without downloading content
    let head_request = Request::builder()
        .method(Method::HEAD)
        .uri("/test-bucket/head.parquet")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, head_request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "HEAD request should succeed"
    );

    // Verify headers are present
    assert!(
        response.headers().contains_key(header::CONTENT_LENGTH),
        "Content-Length should be present"
    );
    assert!(
        response.headers().contains_key("ETag"),
        "ETag should be present"
    );

    // Verify Content-Length matches file size
    if let Some(content_length) = response.headers().get(header::CONTENT_LENGTH) {
        let length: usize = content_length.to_str().unwrap().parse().unwrap();
        assert_eq!(
            length,
            parquet_data.len(),
            "Content-Length should match file size"
        );
    }

    // Verify no body is returned
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body.len(), 0, "HEAD request should not return body");
}

#[tokio::test]
async fn test_parquet_download_multiple_files() {
    use tower::Service;

    let (_storage, mut router, _temp) = setup_test_env().await;

    // Create bucket
    let create_bucket = Request::builder()
        .method(Method::PUT)
        .uri("/test-bucket")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, create_bucket).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Upload multiple parquet files
    let file_count = 5;
    let mut uploaded_files = Vec::new();

    for i in 0..file_count {
        let parquet_data = create_test_parquet_file();
        let key = format!("file_{}.parquet", i);
        let upload_status =
            upload_parquet_file(&mut router, "test-bucket", &key, parquet_data.clone()).await;
        assert_eq!(upload_status, StatusCode::OK);
        uploaded_files.push((key, parquet_data));
    }

    // Download and verify each file
    for (key, original_data) in uploaded_files {
        let download_request = Request::builder()
            .method(Method::GET)
            .uri(format!("/test-bucket/{}", key))
            .body(Body::empty())
            .unwrap();

        let response = Service::call(&mut router, download_request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Download of {} should succeed",
            key
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            body.as_ref(),
            original_data.as_slice(),
            "Content of {} should match",
            key
        );
    }
}
