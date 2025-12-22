// Integration tests for Parquet file upload via S3 API

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

#[tokio::test]
async fn test_parquet_upload_success() {
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

    // Create a test parquet file
    let parquet_data = create_test_parquet_file();
    assert!(parquet_data.len() > 0, "Parquet file should not be empty");

    // Upload the parquet file
    let upload_request = Request::builder()
        .method(Method::PUT)
        .uri("/test-bucket/data.parquet")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(parquet_data.clone()))
        .unwrap();

    let response = Service::call(&mut router, upload_request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Parquet upload should succeed"
    );

    // Verify ETag header is present
    assert!(
        response.headers().contains_key("ETag"),
        "Response should contain ETag header"
    );

    // Verify the uploaded file by downloading it
    let download_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/data.parquet")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, download_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Download should succeed");

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        body.len(),
        parquet_data.len(),
        "Downloaded file should have same size"
    );
    assert_eq!(
        body.as_ref(),
        parquet_data.as_slice(),
        "Downloaded content should match uploaded content"
    );
}

#[tokio::test]
async fn test_parquet_upload_with_correct_content_type() {
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

    // Upload parquet file with application/parquet content type
    let parquet_data = create_test_parquet_file();
    let upload_request = Request::builder()
        .method(Method::PUT)
        .uri("/test-bucket/analytics.parquet")
        .header(header::CONTENT_TYPE, "application/parquet")
        .body(Body::from(parquet_data))
        .unwrap();

    let response = Service::call(&mut router, upload_request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Upload with application/parquet content-type should succeed"
    );
}

#[tokio::test]
async fn test_parquet_upload_large_file() {
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
    let upload_request = Request::builder()
        .method(Method::PUT)
        .uri("/test-bucket/large-data.parquet")
        .header(header::CONTENT_TYPE, "application/parquet")
        .body(Body::from(parquet_data.clone()))
        .unwrap();

    let response = Service::call(&mut router, upload_request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Large parquet upload should succeed"
    );

    // Verify retrieval
    let download_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/large-data.parquet")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, download_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body.len(), parquet_data.len());
}

#[tokio::test]
async fn test_parquet_upload_no_bucket() {
    let (_storage, router, _temp) = setup_test_env().await;

    // Try to upload parquet file to non-existent bucket
    let parquet_data = create_test_parquet_file();
    let upload_request = Request::builder()
        .method(Method::PUT)
        .uri("/nonexistent-bucket/data.parquet")
        .header(header::CONTENT_TYPE, "application/parquet")
        .body(Body::from(parquet_data))
        .unwrap();

    let response = router.oneshot(upload_request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "Upload to non-existent bucket should fail with 404"
    );
}

#[tokio::test]
async fn test_parquet_metadata_preservation() {
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

    // Upload parquet file with custom metadata
    let parquet_data = create_test_parquet_file();
    let upload_request = Request::builder()
        .method(Method::PUT)
        .uri("/test-bucket/metadata-test.parquet")
        .header(header::CONTENT_TYPE, "application/parquet")
        .header("x-amz-meta-dataset", "customer-analytics")
        .header("x-amz-meta-version", "v1.0")
        .body(Body::from(parquet_data))
        .unwrap();

    let response = Service::call(&mut router, upload_request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "Upload with metadata should succeed"
    );

    // Verify metadata is preserved by doing a HEAD request
    let head_request = Request::builder()
        .method(Method::HEAD)
        .uri("/test-bucket/metadata-test.parquet")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, head_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Check that content-type is preserved
    let content_type = response.headers().get(header::CONTENT_TYPE);
    assert!(content_type.is_some(), "Content-Type should be present");
}
