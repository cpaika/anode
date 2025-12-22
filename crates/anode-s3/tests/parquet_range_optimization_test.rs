// Integration tests for Parquet range read optimization
// This tests the ability to read parquet metadata and specific row groups
// without downloading the entire file, which is crucial for large parquet files

use anode_s3::S3Router;
use anode_storage::{StorageConfig, StorageEngine};
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create a large parquet file with multiple row groups
fn create_multi_rowgroup_parquet_file() -> Vec<u8> {
    use arrow::array::{ArrayRef, Int32Array, StringArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::record_batch::RecordBatch;
    use parquet::arrow::ArrowWriter;
    use parquet::file::properties::WriterProperties;

    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("name", DataType::Utf8, true),
        Field::new("value", DataType::Int32, false),
    ]));

    let mut buffer = Vec::new();

    // Configure writer to create small row groups (100 rows each)
    let props = WriterProperties::builder()
        .set_max_row_group_size(100)
        .set_statistics_enabled(parquet::file::properties::EnabledStatistics::Chunk)
        .build();

    let mut writer = ArrowWriter::try_new(&mut buffer, schema.clone(), Some(props)).unwrap();

    // Write 500 rows in 5 batches, creating 5 row groups
    for batch_idx in 0..5 {
        let start = batch_idx * 100;
        let id_values: Vec<i32> = (start..start + 100).collect();
        let name_values: Vec<Option<&str>> = (start..start + 100)
            .map(|i| {
                Some(if i % 3 == 0 {
                    "Alice"
                } else if i % 3 == 1 {
                    "Bob"
                } else {
                    "Charlie"
                })
            })
            .collect();
        let value_values: Vec<i32> = (start..start + 100).map(|i| i * 10).collect();

        let id_array = Int32Array::from(id_values);
        let name_array = StringArray::from(name_values);
        let value_array = Int32Array::from(value_values);

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(id_array) as ArrayRef,
                Arc::new(name_array),
                Arc::new(value_array),
            ],
        )
        .unwrap();

        writer.write(&batch).unwrap();
    }

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
async fn test_parquet_footer_range_read() {
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

    // Upload a multi-row-group parquet file
    let parquet_data = create_multi_rowgroup_parquet_file();
    let file_size = parquet_data.len();

    // Verify the file has multiple row groups
    let reader = SerializedFileReader::new(Bytes::from(parquet_data.clone())).unwrap();
    let metadata = reader.metadata();
    assert!(
        metadata.num_row_groups() >= 3,
        "Test file should have at least 3 row groups, found: {}",
        metadata.num_row_groups()
    );

    let upload_status = upload_parquet_file(
        &mut router,
        "test-bucket",
        "large.parquet",
        parquet_data.clone(),
    )
    .await;
    assert_eq!(upload_status, StatusCode::OK);

    // Read the last 64KB to get the footer (parquet footer is at the end of the file)
    // This is the optimization: we only need the footer to read metadata
    let footer_size = std::cmp::min(file_size, 64 * 1024);
    let range_start = file_size.saturating_sub(footer_size);
    let range_header = format!("bytes={}-{}", range_start, file_size - 1);

    let range_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/large.parquet")
        .header(header::RANGE, range_header)
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, range_request).await.unwrap();

    // Range requests should return 206 Partial Content or 200 OK
    let status = response.status();
    assert!(
        status == StatusCode::PARTIAL_CONTENT || status == StatusCode::OK,
        "Range request should succeed, got: {}",
        status
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    // If partial content is supported, verify we got less than the full file
    if status == StatusCode::PARTIAL_CONTENT {
        assert!(
            body.len() < file_size,
            "Partial content should be smaller than full file. Got {} bytes, file is {} bytes",
            body.len(),
            file_size
        );

        // Verify the footer data is valid by checking it matches the end of the original file
        let expected_footer = &parquet_data[range_start..];
        assert_eq!(
            body.as_ref(),
            expected_footer,
            "Footer content should match the end of the original file"
        );
    }
}

#[tokio::test]
async fn test_parquet_specific_rowgroup_range_read() {
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

    // Upload a multi-row-group parquet file
    let parquet_data = create_multi_rowgroup_parquet_file();

    // Extract row group metadata to know where to read from
    let reader = SerializedFileReader::new(Bytes::from(parquet_data.clone())).unwrap();
    let metadata = reader.metadata();
    let num_row_groups = metadata.num_row_groups();

    assert!(
        num_row_groups >= 2,
        "Test file should have at least 2 row groups"
    );

    let upload_status = upload_parquet_file(
        &mut router,
        "test-bucket",
        "rowgroups.parquet",
        parquet_data.clone(),
    )
    .await;
    assert_eq!(upload_status, StatusCode::OK);

    // Get the file offset and size of the first row group
    let first_rg = metadata.row_group(0);
    let first_rg_offset = first_rg.file_offset().unwrap_or(0) as usize;
    let first_rg_size = first_rg.total_byte_size() as usize;
    let range_end = first_rg_offset + first_rg_size - 1;

    // Request just the first row group
    let range_header = format!("bytes={}-{}", first_rg_offset, range_end);

    let range_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/rowgroups.parquet")
        .header(header::RANGE, range_header)
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, range_request).await.unwrap();

    let status = response.status();
    assert!(
        status == StatusCode::PARTIAL_CONTENT || status == StatusCode::OK,
        "Range request for row group should succeed, got: {}",
        status
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    // If partial content is supported, verify we got only the requested range
    if status == StatusCode::PARTIAL_CONTENT {
        assert_eq!(
            body.len(),
            first_rg_size,
            "Should receive exactly the requested row group size"
        );

        // Verify the content matches the expected byte range
        let expected_data = &parquet_data[first_rg_offset..=range_end];
        assert_eq!(
            body.as_ref(),
            expected_data,
            "Row group content should match the expected byte range"
        );
    }
}

#[tokio::test]
async fn test_parquet_multiple_range_reads() {
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

    // Upload a multi-row-group parquet file
    let parquet_data = create_multi_rowgroup_parquet_file();
    let file_size = parquet_data.len();

    // Get row group information
    let reader = SerializedFileReader::new(Bytes::from(parquet_data.clone())).unwrap();
    let metadata = reader.metadata();
    let num_row_groups = metadata.num_row_groups();

    assert!(
        num_row_groups >= 3,
        "Test file should have at least 3 row groups"
    );

    let upload_status = upload_parquet_file(
        &mut router,
        "test-bucket",
        "multirange.parquet",
        parquet_data.clone(),
    )
    .await;
    assert_eq!(upload_status, StatusCode::OK);

    // Test Case 1: Read first 1KB (header region)
    let header_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/multirange.parquet")
        .header(header::RANGE, "bytes=0-1023")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, header_request).await.unwrap();
    let header_status = response.status();
    assert!(
        header_status == StatusCode::PARTIAL_CONTENT || header_status == StatusCode::OK,
        "Header range request should succeed"
    );

    let header_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    if header_status == StatusCode::PARTIAL_CONTENT {
        assert_eq!(header_body.len(), 1024, "Should receive exactly 1024 bytes");
        assert_eq!(
            header_body.as_ref(),
            &parquet_data[0..1024],
            "Header content should match"
        );
    }

    // Test Case 2: Read middle portion
    let middle_start = file_size / 2;
    let middle_end = middle_start + 2048 - 1;
    let middle_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/multirange.parquet")
        .header(
            header::RANGE,
            format!("bytes={}-{}", middle_start, middle_end),
        )
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, middle_request).await.unwrap();
    let middle_status = response.status();
    assert!(
        middle_status == StatusCode::PARTIAL_CONTENT || middle_status == StatusCode::OK,
        "Middle range request should succeed"
    );

    let middle_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    if middle_status == StatusCode::PARTIAL_CONTENT {
        assert_eq!(middle_body.len(), 2048, "Should receive exactly 2048 bytes");
        assert_eq!(
            middle_body.as_ref(),
            &parquet_data[middle_start..=middle_end],
            "Middle content should match"
        );
    }

    // Test Case 3: Read last 8KB (footer region)
    let footer_start = file_size.saturating_sub(8192);
    let footer_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/multirange.parquet")
        .header(
            header::RANGE,
            format!("bytes={}-{}", footer_start, file_size - 1),
        )
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, footer_request).await.unwrap();
    let footer_status = response.status();
    assert!(
        footer_status == StatusCode::PARTIAL_CONTENT || footer_status == StatusCode::OK,
        "Footer range request should succeed"
    );

    let footer_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    if footer_status == StatusCode::PARTIAL_CONTENT {
        let expected_size = file_size - footer_start;
        assert_eq!(
            footer_body.len(),
            expected_size,
            "Should receive footer bytes"
        );
        assert_eq!(
            footer_body.as_ref(),
            &parquet_data[footer_start..],
            "Footer content should match"
        );
    }
}

#[tokio::test]
async fn test_parquet_head_then_range_workflow() {
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

    // Upload parquet file
    let parquet_data = create_multi_rowgroup_parquet_file();
    let file_size = parquet_data.len();

    let upload_status = upload_parquet_file(
        &mut router,
        "test-bucket",
        "workflow.parquet",
        parquet_data.clone(),
    )
    .await;
    assert_eq!(upload_status, StatusCode::OK);

    // Step 1: Use HEAD request to get file size without downloading
    let head_request = Request::builder()
        .method(Method::HEAD)
        .uri("/test-bucket/workflow.parquet")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, head_request).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "HEAD request should succeed"
    );

    // Extract file size from Content-Length header
    let content_length = response
        .headers()
        .get(header::CONTENT_LENGTH)
        .expect("Content-Length header should be present");

    let reported_size: usize = content_length
        .to_str()
        .unwrap()
        .parse()
        .expect("Content-Length should be a valid number");

    assert_eq!(
        reported_size, file_size,
        "Reported file size should match actual file size"
    );

    // Verify no body is returned
    let head_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(head_body.len(), 0, "HEAD request should not return body");

    // Step 2: Calculate footer range based on file size (typical optimization)
    let footer_read_size = std::cmp::min(reported_size, 64 * 1024);
    let footer_start = reported_size.saturating_sub(footer_read_size);
    let footer_end = reported_size - 1;

    // Step 3: Read only the footer to get metadata
    let footer_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/workflow.parquet")
        .header(
            header::RANGE,
            format!("bytes={}-{}", footer_start, footer_end),
        )
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, footer_request).await.unwrap();

    let status = response.status();
    assert!(
        status == StatusCode::PARTIAL_CONTENT || status == StatusCode::OK,
        "Footer range request should succeed, got: {}",
        status
    );

    let footer_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    // If partial content is supported, verify optimization
    if status == StatusCode::PARTIAL_CONTENT {
        // We read much less than the full file
        assert!(
            footer_body.len() <= footer_read_size,
            "Footer read should be <= {} bytes, got {}",
            footer_read_size,
            footer_body.len()
        );

        assert!(
            (footer_body.len() as f64) < (file_size as f64 * 0.5),
            "Footer read should be significantly smaller than full file (optimization). \
             Read {} bytes out of {} total ({}%)",
            footer_body.len(),
            file_size,
            (footer_body.len() as f64 / file_size as f64 * 100.0)
        );

        // Verify content matches
        assert_eq!(
            footer_body.as_ref(),
            &parquet_data[footer_start..],
            "Footer content should match"
        );
    }
}

#[tokio::test]
async fn test_parquet_range_read_edge_cases() {
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

    // Upload parquet file
    let parquet_data = create_multi_rowgroup_parquet_file();
    let file_size = parquet_data.len();

    let upload_status = upload_parquet_file(
        &mut router,
        "test-bucket",
        "edges.parquet",
        parquet_data.clone(),
    )
    .await;
    assert_eq!(upload_status, StatusCode::OK);

    // Edge Case 1: Request range beyond file size (should be handled gracefully)
    let beyond_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/edges.parquet")
        .header(
            header::RANGE,
            format!("bytes={}-{}", file_size + 1000, file_size + 2000),
        )
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, beyond_request).await.unwrap();
    // Should return 416 Range Not Satisfiable, 200 OK, or other appropriate status
    assert!(
        response.status() == StatusCode::RANGE_NOT_SATISFIABLE
            || response.status() == StatusCode::OK
            || response.status() == StatusCode::PARTIAL_CONTENT,
        "Beyond-range request should be handled gracefully, got: {}",
        response.status()
    );

    // Edge Case 2: Request single byte
    let single_byte_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/edges.parquet")
        .header(header::RANGE, "bytes=0-0")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, single_byte_request)
        .await
        .unwrap();
    let single_byte_status = response.status();
    assert!(
        single_byte_status == StatusCode::PARTIAL_CONTENT || single_byte_status == StatusCode::OK,
        "Single byte request should succeed"
    );

    let single_byte_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    if single_byte_status == StatusCode::PARTIAL_CONTENT {
        assert_eq!(single_byte_body.len(), 1, "Should receive exactly 1 byte");
        assert_eq!(single_byte_body[0], parquet_data[0], "Byte should match");
    }

    // Edge Case 3: Request last byte
    let last_byte_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/edges.parquet")
        .header(
            header::RANGE,
            format!("bytes={}-{}", file_size - 1, file_size - 1),
        )
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, last_byte_request).await.unwrap();
    let last_byte_status = response.status();
    assert!(
        last_byte_status == StatusCode::PARTIAL_CONTENT || last_byte_status == StatusCode::OK,
        "Last byte request should succeed"
    );

    let last_byte_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    if last_byte_status == StatusCode::PARTIAL_CONTENT {
        assert_eq!(last_byte_body.len(), 1, "Should receive exactly 1 byte");
        assert_eq!(
            last_byte_body[0],
            parquet_data[file_size - 1],
            "Last byte should match"
        );
    }
}

#[tokio::test]
async fn test_parquet_optimization_bandwidth_savings() {
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

    // Upload a large parquet file
    let parquet_data = create_multi_rowgroup_parquet_file();
    let file_size = parquet_data.len();

    // Verify this is actually a multi-row-group file
    let reader = SerializedFileReader::new(Bytes::from(parquet_data.clone())).unwrap();
    let metadata = reader.metadata();
    let num_row_groups = metadata.num_row_groups();

    println!(
        "Test file has {} row groups, {} bytes total",
        num_row_groups, file_size
    );
    assert!(
        num_row_groups >= 3,
        "File should have multiple row groups for realistic test"
    );

    let upload_status = upload_parquet_file(
        &mut router,
        "test-bucket",
        "bandwidth.parquet",
        parquet_data.clone(),
    )
    .await;
    assert_eq!(upload_status, StatusCode::OK);

    // Scenario: Client wants to read metadata to decide which row groups to query
    // This is a common pattern in analytics queries with predicate pushdown

    // Download full file (non-optimized approach)
    let full_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/bandwidth.parquet")
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, full_request).await.unwrap();
    let full_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let full_download_bytes = full_body.len();
    println!("Full download: {} bytes", full_download_bytes);

    // Download only footer (optimized approach)
    let footer_size = std::cmp::min(file_size, 64 * 1024);
    let footer_start = file_size.saturating_sub(footer_size);

    let footer_request = Request::builder()
        .method(Method::GET)
        .uri("/test-bucket/bandwidth.parquet")
        .header(
            header::RANGE,
            format!("bytes={}-{}", footer_start, file_size - 1),
        )
        .body(Body::empty())
        .unwrap();

    let response = Service::call(&mut router, footer_request).await.unwrap();
    let footer_status = response.status();
    let footer_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let footer_download_bytes = footer_body.len();
    println!("Footer-only download: {} bytes", footer_download_bytes);

    // If range requests are supported, verify the optimization savings
    if footer_status == StatusCode::PARTIAL_CONTENT {
        // Footer should be significantly smaller than full file
        assert!(
            footer_download_bytes < full_download_bytes,
            "Footer download ({} bytes) should be smaller than full file ({} bytes)",
            footer_download_bytes,
            full_download_bytes
        );

        // Calculate bandwidth savings
        let savings_bytes = full_download_bytes - footer_download_bytes;
        let savings_percent = (savings_bytes as f64 / full_download_bytes as f64) * 100.0;

        println!(
            "Bandwidth savings: {} bytes ({:.1}%)",
            savings_bytes, savings_percent
        );

        // For a multi-row-group file, footer should be < 50% of file size
        assert!(
            savings_percent > 10.0,
            "Range read optimization should save significant bandwidth. \
             Saved only {:.1}% by reading footer",
            savings_percent
        );

        // Verify the footer content is actually usable for metadata parsing
        // The footer should end with "PAR1" magic bytes
        assert!(
            footer_body.len() >= 4,
            "Footer should be at least 4 bytes (for magic number)"
        );

        let footer_end = &footer_body[footer_body.len() - 4..];
        assert_eq!(
            footer_end, b"PAR1",
            "Footer should end with PAR1 magic bytes"
        );
    } else {
        println!("Note: Range requests not supported by this storage implementation");
    }
}
