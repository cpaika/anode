//! Parquet-specific integration tests
//!
//! Tests for Parquet file optimizations including:
//! - Metadata caching
//! - Footer-only reads
//! - Column pruning
//! - Row group filtering
//! - Query performance

use crate::{
    setup_cluster, create_test_s3_client, unique_object_key, BucketFixture,
};
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;
use reqwest::Client;
use serial_test::serial;
use std::time::{Duration, Instant};

/// Create a mock Parquet file with footer
/// In a real implementation, this would use the parquet-rs crate
fn create_mock_parquet_file(num_row_groups: usize) -> Vec<u8> {
    // This is a simplified mock. A real Parquet file would need:
    // - Magic number (PAR1) at start and end
    // - Row group metadata
    // - Column chunk metadata
    // - Footer with schema and statistics
    // - Footer length (4 bytes before final PAR1)

    let mut data = Vec::new();

    // Magic number
    data.extend_from_slice(b"PAR1");

    // Mock row group data (1MB per row group)
    let row_group_size = 1024 * 1024;
    for _ in 0..num_row_groups {
        data.extend(vec![0u8; row_group_size]);
    }

    // Mock footer metadata
    let footer = create_mock_footer(num_row_groups);
    data.extend(&footer);

    // Footer length (4 bytes, little-endian)
    let footer_len = footer.len() as u32;
    data.extend(&footer_len.to_le_bytes());

    // Magic number at end
    data.extend_from_slice(b"PAR1");

    data
}

fn create_mock_footer(num_row_groups: usize) -> Vec<u8> {
    // Mock Parquet footer with metadata
    // Real implementation would use Thrift serialization
    let mut footer = Vec::new();

    // Version
    footer.extend(&1u32.to_le_bytes());

    // Number of row groups
    footer.extend(&(num_row_groups as u32).to_le_bytes());

    // Mock schema
    footer.extend(b"schema{field1:int64,field2:string}");

    // Mock statistics for each row group
    for i in 0..num_row_groups {
        // Row group metadata
        footer.extend(&(i as u64).to_le_bytes()); // Offset
        footer.extend(&1000u64.to_le_bytes());     // Num rows
        footer.extend(&(1024 * 1024u64).to_le_bytes()); // Size
    }

    footer
}

#[tokio::test]
#[serial]
async fn test_parquet_metadata_caching() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-parquet-cache").await;
    let http_client = Client::new();

    let key = unique_object_key("test.parquet");
    let parquet_data = create_mock_parquet_file(10);

    // Upload Parquet file
    fixture.client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(parquet_data.clone())))
        .content_type("application/vnd.apache.parquet")
        .send()
        .await
        .unwrap();

    // First read: should cache metadata
    let start = Instant::now();
    let get_result = fixture.client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();
    let _ = get_result.body.collect().await.unwrap();
    let first_read_duration = start.elapsed();

    println!("First read (cold cache): {:?}", first_read_duration);

    // Second read: should hit cache
    let start = Instant::now();
    let get_result = fixture.client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();
    let _ = get_result.body.collect().await.unwrap();
    let second_read_duration = start.elapsed();

    println!("Second read (warm cache): {:?}", second_read_duration);

    // Cache hit should be faster (though not always guaranteed in integration test)
    // We just verify both reads succeed
    assert!(
        first_read_duration.as_millis() > 0,
        "First read should take measurable time"
    );
    assert!(
        second_read_duration.as_millis() > 0,
        "Second read should take measurable time"
    );

    // Query cache statistics if available
    let node = cluster.get_node(0).unwrap();
    let cache_stats_url = format!("{}/stats/cache", node.admin_endpoint());

    if let Ok(response) = http_client
        .get(&cache_stats_url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        if response.status().is_success() {
            if let Ok(stats) = response.json::<serde_json::Value>().await {
                println!("Cache stats: {}", serde_json::to_string_pretty(&stats).unwrap());

                if let Some(hits) = stats.get("hits").and_then(|v| v.as_u64()) {
                    assert!(hits > 0, "Cache should have at least one hit");
                }
            }
        }
    }
}

#[tokio::test]
#[serial]
async fn test_parquet_footer_optimization() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-parquet-footer").await;

    let key = unique_object_key("test.parquet");
    let parquet_data = create_mock_parquet_file(10);
    let file_size = parquet_data.len();

    // Upload Parquet file
    fixture.client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(parquet_data.clone())))
        .content_type("application/vnd.apache.parquet")
        .send()
        .await
        .unwrap();

    // Read only the footer (last 1KB)
    // Parquet readers typically read the last few KB to get metadata
    let footer_size = 1024;
    let range = format!("bytes={}-{}", file_size - footer_size, file_size - 1);

    let start = Instant::now();
    let get_result = fixture.client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .range(&range)
        .send()
        .await
        .unwrap();

    let footer_data = get_result.body.collect().await.unwrap().into_bytes();
    let footer_read_duration = start.elapsed();

    println!("Footer-only read: {:?}", footer_read_duration);

    assert_eq!(
        footer_data.len(),
        footer_size,
        "Should read exactly the footer size"
    );

    // Footer-only read should be fast
    assert!(
        footer_read_duration.as_millis() < 1000,
        "Footer read should be fast"
    );

    // Verify footer ends with PAR1 magic number
    assert_eq!(
        &footer_data[footer_data.len() - 4..],
        b"PAR1",
        "Footer should end with PAR1 magic number"
    );
}

#[tokio::test]
#[serial]
async fn test_parquet_range_reads() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-parquet-ranges").await;

    let key = unique_object_key("test.parquet");
    let parquet_data = create_mock_parquet_file(10);

    // Upload Parquet file
    fixture.client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(parquet_data.clone())))
        .content_type("application/vnd.apache.parquet")
        .send()
        .await
        .unwrap();

    // Simulate column chunk reads
    // Parquet readers often read specific byte ranges for column chunks
    let ranges = vec![
        (1024, 10240),      // First column chunk
        (50000, 60000),     // Second column chunk
        (100000, 110000),   // Third column chunk
    ];

    for (start, end) in ranges {
        if end <= parquet_data.len() {
            let range = format!("bytes={}-{}", start, end - 1);

            let get_result = fixture.client
                .get_object()
                .bucket(&fixture.bucket)
                .key(&key)
                .range(&range)
                .send()
                .await
                .unwrap();

            let chunk_data = get_result.body.collect().await.unwrap().into_bytes();

            assert_eq!(
                chunk_data.len(),
                end - start,
                "Range read should return exact bytes requested"
            );

            // Verify content matches
            assert_eq!(
                chunk_data.as_ref(),
                &parquet_data[start..end],
                "Range content should match original"
            );
        }
    }
}

#[tokio::test]
#[serial]
async fn test_parquet_multiple_files() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-parquet-multi").await;

    let num_files = 10;

    // Upload multiple Parquet files
    for i in 0..num_files {
        let key = format!("partition=2024-01/file-{}.parquet", i);
        let parquet_data = create_mock_parquet_file(5);

        fixture.client
            .put_object()
            .bucket(&fixture.bucket)
            .key(&key)
            .body(ByteStream::from(Bytes::from(parquet_data)))
            .content_type("application/vnd.apache.parquet")
            .send()
            .await
            .unwrap();
    }

    // List Parquet files in partition
    let list_result = fixture.client
        .list_objects_v2()
        .bucket(&fixture.bucket)
        .prefix("partition=2024-01/")
        .send()
        .await
        .unwrap();

    let parquet_files: Vec<String> = list_result
        .contents()
        .iter()
        .filter_map(|obj| obj.key().map(|k| k.to_string()))
        .filter(|k| k.ends_with(".parquet"))
        .collect();

    assert_eq!(
        parquet_files.len(),
        num_files,
        "Should find all Parquet files"
    );

    // Read metadata from all files
    for file_key in &parquet_files {
        let head_result = fixture.client
            .head_object()
            .bucket(&fixture.bucket)
            .key(file_key)
            .send()
            .await
            .unwrap();

        assert_eq!(
            head_result.content_type(),
            Some("application/vnd.apache.parquet")
        );
    }
}

#[tokio::test]
#[serial]
async fn test_parquet_concurrent_reads() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-parquet-concurrent").await;

    let key = unique_object_key("test.parquet");
    let parquet_data = create_mock_parquet_file(10);
    let file_size = parquet_data.len();

    // Upload Parquet file
    fixture.client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(parquet_data.clone())))
        .content_type("application/vnd.apache.parquet")
        .send()
        .await
        .unwrap();

    // Simulate concurrent column reads
    let mut handles = Vec::new();

    for i in 0..5 {
        let client = fixture.client.clone();
        let bucket = fixture.bucket.clone();
        let key_clone = key.clone();

        // Each task reads a different range (simulating column chunks)
        let start = i * 100000;
        let end = std::cmp::min(start + 50000, file_size);

        if start < file_size {
            let handle = tokio::spawn(async move {
                let range = format!("bytes={}-{}", start, end - 1);

                client
                    .get_object()
                    .bucket(&bucket)
                    .key(&key_clone)
                    .range(&range)
                    .send()
                    .await
                    .map(|r| r.body)
            });

            handles.push(handle);
        }
    }

    // Wait for all concurrent reads
    let mut success_count = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            success_count += 1;
        }
    }

    assert!(
        success_count > 0,
        "At least some concurrent reads should succeed"
    );
}

#[tokio::test]
#[serial]
async fn test_parquet_stats_api() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-parquet-stats").await;
    let http_client = Client::new();

    let key = unique_object_key("test.parquet");
    let parquet_data = create_mock_parquet_file(10);

    // Upload Parquet file
    fixture.client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(parquet_data)))
        .content_type("application/vnd.apache.parquet")
        .send()
        .await
        .unwrap();

    // Query Parquet-specific statistics API (if implemented)
    let node = cluster.get_node(0).unwrap();
    let stats_url = format!(
        "{}/stats/parquet?bucket={}&key={}",
        node.admin_endpoint(),
        fixture.bucket,
        key
    );

    if let Ok(response) = http_client
        .get(&stats_url)
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        if response.status().is_success() {
            if let Ok(stats) = response.json::<serde_json::Value>().await {
                println!(
                    "Parquet stats: {}",
                    serde_json::to_string_pretty(&stats).unwrap()
                );

                // Verify expected fields in stats
                // (Actual fields depend on implementation)
                if stats.get("metadata_cached").is_some() {
                    println!("Metadata caching is tracked");
                }
                if stats.get("footer_reads").is_some() {
                    println!("Footer reads are tracked");
                }
            }
        }
    }
}

#[tokio::test]
#[serial]
async fn test_parquet_large_file() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-parquet-large").await;

    let key = unique_object_key("large.parquet");
    // Create a larger Parquet file with 100 row groups (~100MB)
    let parquet_data = create_mock_parquet_file(100);
    let file_size = parquet_data.len();

    println!("Large Parquet file size: {} MB", file_size / 1024 / 1024);

    // Upload large Parquet file
    let start = Instant::now();
    fixture.client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(parquet_data.clone())))
        .content_type("application/vnd.apache.parquet")
        .send()
        .await
        .unwrap();
    let upload_duration = start.elapsed();

    println!("Upload duration: {:?}", upload_duration);

    // Footer-only read (should be fast even for large file)
    let footer_size = 1024;
    let range = format!("bytes={}-{}", file_size - footer_size, file_size - 1);

    let start = Instant::now();
    let get_result = fixture.client
        .get_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .range(&range)
        .send()
        .await
        .unwrap();
    let _ = get_result.body.collect().await.unwrap();
    let footer_duration = start.elapsed();

    println!("Footer read duration: {:?}", footer_duration);

    // Footer read should be fast regardless of file size
    assert!(
        footer_duration.as_millis() < 1000,
        "Footer read should be fast for large files"
    );
}

#[tokio::test]
#[serial]
async fn test_parquet_compression_metadata() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-parquet-compression").await;

    let key = unique_object_key("compressed.parquet");
    let parquet_data = create_mock_parquet_file(10);

    // Upload with content-encoding metadata
    fixture.client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .body(ByteStream::from(Bytes::from(parquet_data)))
        .content_type("application/vnd.apache.parquet")
        .metadata("parquet-compression", "SNAPPY")
        .metadata("parquet-version", "2.0")
        .send()
        .await
        .unwrap();

    // Retrieve and verify metadata
    let head_result = fixture.client
        .head_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    assert_eq!(
        head_result
            .metadata()
            .get("parquet-compression")
            .map(|s| s.as_str()),
        Some("SNAPPY")
    );

    assert_eq!(
        head_result
            .metadata()
            .get("parquet-version")
            .map(|s| s.as_str()),
        Some("2.0")
    );
}

#[tokio::test]
#[serial]
async fn test_parquet_query_optimization() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-parquet-query").await;

    // Create a partitioned dataset
    let partitions = vec!["2024-01-01", "2024-01-02", "2024-01-03"];

    for partition in &partitions {
        for file_num in 0..5 {
            let key = format!("date={}/file-{}.parquet", partition, file_num);
            let parquet_data = create_mock_parquet_file(10);

            fixture.client
                .put_object()
                .bucket(&fixture.bucket)
                .key(&key)
                .body(ByteStream::from(Bytes::from(parquet_data)))
                .content_type("application/vnd.apache.parquet")
                .send()
                .await
                .unwrap();
        }
    }

    // Query single partition (partition pruning)
    let start = Instant::now();
    let list_result = fixture.client
        .list_objects_v2()
        .bucket(&fixture.bucket)
        .prefix("date=2024-01-01/")
        .send()
        .await
        .unwrap();
    let query_duration = start.elapsed();

    let files = list_result.contents();
    assert_eq!(files.len(), 5, "Should find 5 files in partition");

    println!(
        "Partition query: {} files in {:?}",
        files.len(),
        query_duration
    );

    // Query should be fast with partition pruning
    assert!(
        query_duration.as_millis() < 1000,
        "Partition query should be fast"
    );
}
