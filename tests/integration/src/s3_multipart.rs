//! Multipart upload tests
//!
//! Tests for S3 multipart upload operations including:
//! - Initiating multipart uploads
//! - Uploading parts
//! - Completing multipart uploads
//! - Aborting multipart uploads
//! - Listing parts
//! - Large object uploads

use crate::{
    setup_cluster, create_test_s3_client, random_bytes, unique_object_key,
    BucketFixture,
};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use bytes::Bytes;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_multipart_upload() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-multipart").await;

    let key = unique_object_key("multipart-object");

    // Initiate multipart upload
    let create_result = fixture.client
        .create_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    let upload_id = create_result.upload_id().unwrap();

    // Upload parts (3 parts of 5MB each)
    let part_size = 5 * 1024 * 1024; // 5MB
    let mut completed_parts = Vec::new();

    for part_number in 1..=3 {
        let part_data = random_bytes(part_size);

        let upload_result = fixture.client
            .upload_part()
            .bucket(&fixture.bucket)
            .key(&key)
            .upload_id(upload_id)
            .part_number(part_number)
            .body(ByteStream::from(Bytes::from(part_data)))
            .send()
            .await
            .unwrap();

        let completed_part = CompletedPart::builder()
            .part_number(part_number)
            .e_tag(upload_result.e_tag().unwrap())
            .build();

        completed_parts.push(completed_part);
    }

    // Complete multipart upload
    let completed_upload = CompletedMultipartUpload::builder()
        .set_parts(Some(completed_parts))
        .build();

    let complete_result = fixture.client
        .complete_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .multipart_upload(completed_upload)
        .send()
        .await;

    assert!(
        complete_result.is_ok(),
        "Failed to complete multipart upload: {:?}",
        complete_result.err()
    );

    // Verify object exists and has correct size
    let head_result = fixture.client
        .head_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    assert_eq!(
        head_result.content_length(),
        Some((part_size * 3) as i64),
        "Object size should be sum of all parts"
    );
}

#[tokio::test]
#[serial]
async fn test_multipart_abort() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-multipart-abort").await;

    let key = unique_object_key("multipart-object");

    // Initiate multipart upload
    let create_result = fixture.client
        .create_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    let upload_id = create_result.upload_id().unwrap();

    // Upload one part
    let part_data = random_bytes(5 * 1024 * 1024);
    fixture.client
        .upload_part()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .part_number(1)
        .body(ByteStream::from(Bytes::from(part_data)))
        .send()
        .await
        .unwrap();

    // Abort multipart upload
    let abort_result = fixture.client
        .abort_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .send()
        .await;

    assert!(
        abort_result.is_ok(),
        "Failed to abort multipart upload: {:?}",
        abort_result.err()
    );

    // Verify object doesn't exist
    let head_result = fixture.client
        .head_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await;

    assert!(
        head_result.is_err(),
        "Object should not exist after abort"
    );
}

#[tokio::test]
#[serial]
async fn test_multipart_list_parts() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-list-parts").await;

    let key = unique_object_key("multipart-object");

    // Initiate multipart upload
    let create_result = fixture.client
        .create_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    let upload_id = create_result.upload_id().unwrap();

    // Upload 3 parts
    let part_size = 5 * 1024 * 1024;
    for part_number in 1..=3 {
        let part_data = random_bytes(part_size);
        fixture.client
            .upload_part()
            .bucket(&fixture.bucket)
            .key(&key)
            .upload_id(upload_id)
            .part_number(part_number)
            .body(ByteStream::from(Bytes::from(part_data)))
            .send()
            .await
            .unwrap();
    }

    // List parts
    let list_result = fixture.client
        .list_parts()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .send()
        .await
        .unwrap();

    let parts = list_result.parts();
    assert_eq!(parts.len(), 3, "Should list all uploaded parts");

    for (i, part) in parts.iter().enumerate() {
        assert_eq!(
            part.part_number(),
            Some((i + 1) as i32),
            "Part numbers should be sequential"
        );
        assert_eq!(
            part.size(),
            Some(part_size as i64),
            "Part size should match"
        );
        assert!(part.e_tag().is_some(), "Part should have ETag");
    }

    // Cleanup - abort the upload
    fixture.client
        .abort_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .send()
        .await
        .unwrap();
}

#[tokio::test]
#[serial]
async fn test_large_object_multipart() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-large-object").await;

    let key = unique_object_key("large-object");

    // Initiate multipart upload
    let create_result = fixture.client
        .create_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    let upload_id = create_result.upload_id().unwrap();

    // Upload 100MB object in 10MB parts
    let part_size = 10 * 1024 * 1024; // 10MB
    let num_parts = 10;
    let mut completed_parts = Vec::new();

    for part_number in 1..=num_parts {
        let part_data = random_bytes(part_size);

        let upload_result = fixture.client
            .upload_part()
            .bucket(&fixture.bucket)
            .key(&key)
            .upload_id(upload_id)
            .part_number(part_number)
            .body(ByteStream::from(Bytes::from(part_data)))
            .send()
            .await
            .unwrap();

        let completed_part = CompletedPart::builder()
            .part_number(part_number)
            .e_tag(upload_result.e_tag().unwrap())
            .build();

        completed_parts.push(completed_part);
    }

    // Complete multipart upload
    let completed_upload = CompletedMultipartUpload::builder()
        .set_parts(Some(completed_parts))
        .build();

    let complete_result = fixture.client
        .complete_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .multipart_upload(completed_upload)
        .send()
        .await;

    assert!(
        complete_result.is_ok(),
        "Failed to complete large multipart upload: {:?}",
        complete_result.err()
    );

    // Verify object size
    let head_result = fixture.client
        .head_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    let expected_size = (part_size * num_parts) as i64;
    assert_eq!(
        head_result.content_length(),
        Some(expected_size),
        "Object size should be 100MB"
    );
}

#[tokio::test]
#[serial]
async fn test_multipart_with_metadata() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-multipart-metadata").await;

    let key = unique_object_key("multipart-object");

    // Initiate multipart upload with metadata
    let create_result = fixture.client
        .create_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .content_type("application/octet-stream")
        .metadata("test-key", "test-value")
        .send()
        .await
        .unwrap();

    let upload_id = create_result.upload_id().unwrap();

    // Upload one part
    let part_data = random_bytes(5 * 1024 * 1024);
    let upload_result = fixture.client
        .upload_part()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .part_number(1)
        .body(ByteStream::from(Bytes::from(part_data)))
        .send()
        .await
        .unwrap();

    // Complete upload
    let completed_part = CompletedPart::builder()
        .part_number(1)
        .e_tag(upload_result.e_tag().unwrap())
        .build();

    let completed_upload = CompletedMultipartUpload::builder()
        .parts(completed_part)
        .build();

    fixture.client
        .complete_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .multipart_upload(completed_upload)
        .send()
        .await
        .unwrap();

    // Verify metadata
    let head_result = fixture.client
        .head_object()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    assert_eq!(
        head_result.content_type(),
        Some("application/octet-stream")
    );
    assert_eq!(
        head_result.metadata().get("test-key").map(|s| s.as_str()),
        Some("test-value")
    );
}

#[tokio::test]
#[serial]
async fn test_multipart_list_uploads() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-list-uploads").await;

    // Initiate multiple uploads
    let mut upload_ids = Vec::new();
    for i in 1..=3 {
        let key = format!("multipart-object-{}", i);
        let create_result = fixture.client
            .create_multipart_upload()
            .bucket(&fixture.bucket)
            .key(&key)
            .send()
            .await
            .unwrap();

        upload_ids.push((key, create_result.upload_id().unwrap().to_string()));
    }

    // List multipart uploads
    let list_result = fixture.client
        .list_multipart_uploads()
        .bucket(&fixture.bucket)
        .send()
        .await
        .unwrap();

    let uploads = list_result.uploads();
    assert_eq!(
        uploads.len(),
        3,
        "Should list all in-progress uploads"
    );

    // Cleanup - abort all uploads
    for (key, upload_id) in upload_ids {
        fixture.client
            .abort_multipart_upload()
            .bucket(&fixture.bucket)
            .key(&key)
            .upload_id(&upload_id)
            .send()
            .await
            .unwrap();
    }

    // Verify uploads are gone
    let list_result = fixture.client
        .list_multipart_uploads()
        .bucket(&fixture.bucket)
        .send()
        .await
        .unwrap();

    assert_eq!(
        list_result.uploads().len(),
        0,
        "All uploads should be aborted"
    );
}

#[tokio::test]
#[serial]
async fn test_multipart_part_size_validation() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-part-size").await;

    let key = unique_object_key("multipart-object");

    // Initiate multipart upload
    let create_result = fixture.client
        .create_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .send()
        .await
        .unwrap();

    let upload_id = create_result.upload_id().unwrap();

    // Upload parts of different sizes
    // Part 1: 5MB
    let part1_data = random_bytes(5 * 1024 * 1024);
    let upload1 = fixture.client
        .upload_part()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .part_number(1)
        .body(ByteStream::from(Bytes::from(part1_data)))
        .send()
        .await
        .unwrap();

    // Part 2: 10MB
    let part2_data = random_bytes(10 * 1024 * 1024);
    let upload2 = fixture.client
        .upload_part()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .part_number(2)
        .body(ByteStream::from(Bytes::from(part2_data)))
        .send()
        .await
        .unwrap();

    // Part 3 (last part): 1MB (allowed to be smaller)
    let part3_data = random_bytes(1024 * 1024);
    let upload3 = fixture.client
        .upload_part()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .part_number(3)
        .body(ByteStream::from(Bytes::from(part3_data)))
        .send()
        .await
        .unwrap();

    // Complete upload
    let completed_upload = CompletedMultipartUpload::builder()
        .parts(
            CompletedPart::builder()
                .part_number(1)
                .e_tag(upload1.e_tag().unwrap())
                .build(),
        )
        .parts(
            CompletedPart::builder()
                .part_number(2)
                .e_tag(upload2.e_tag().unwrap())
                .build(),
        )
        .parts(
            CompletedPart::builder()
                .part_number(3)
                .e_tag(upload3.e_tag().unwrap())
                .build(),
        )
        .build();

    let complete_result = fixture.client
        .complete_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&key)
        .upload_id(upload_id)
        .multipart_upload(completed_upload)
        .send()
        .await;

    assert!(
        complete_result.is_ok(),
        "Should complete with variable part sizes"
    );
}

#[tokio::test]
#[serial]
async fn test_multipart_copy() {
    let cluster = setup_cluster(3).await.unwrap();
    let fixture = BucketFixture::new(&cluster, "test-multipart-copy").await;

    let source_key = unique_object_key("source");
    let dest_key = unique_object_key("dest");

    // Create a large source object (20MB)
    let source_data = random_bytes(20 * 1024 * 1024);
    fixture.client
        .put_object()
        .bucket(&fixture.bucket)
        .key(&source_key)
        .body(ByteStream::from(Bytes::from(source_data.clone())))
        .send()
        .await
        .unwrap();

    // Initiate multipart upload for copy
    let create_result = fixture.client
        .create_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&dest_key)
        .send()
        .await
        .unwrap();

    let upload_id = create_result.upload_id().unwrap();

    // Copy in parts (2 parts of 10MB each)
    let copy_source = format!("{}/{}", fixture.bucket, source_key);

    // Part 1: bytes 0-10485759 (10MB)
    let upload1 = fixture.client
        .upload_part_copy()
        .bucket(&fixture.bucket)
        .key(&dest_key)
        .upload_id(upload_id)
        .part_number(1)
        .copy_source(&copy_source)
        .copy_source_range("bytes=0-10485759")
        .send()
        .await
        .unwrap();

    // Part 2: bytes 10485760-20971519 (10MB)
    let upload2 = fixture.client
        .upload_part_copy()
        .bucket(&fixture.bucket)
        .key(&dest_key)
        .upload_id(upload_id)
        .part_number(2)
        .copy_source(&copy_source)
        .copy_source_range("bytes=10485760-20971519")
        .send()
        .await
        .unwrap();

    // Complete upload
    let completed_upload = CompletedMultipartUpload::builder()
        .parts(
            CompletedPart::builder()
                .part_number(1)
                .e_tag(upload1.copy_part_result().unwrap().e_tag().unwrap())
                .build(),
        )
        .parts(
            CompletedPart::builder()
                .part_number(2)
                .e_tag(upload2.copy_part_result().unwrap().e_tag().unwrap())
                .build(),
        )
        .build();

    fixture.client
        .complete_multipart_upload()
        .bucket(&fixture.bucket)
        .key(&dest_key)
        .upload_id(upload_id)
        .multipart_upload(completed_upload)
        .send()
        .await
        .unwrap();

    // Verify destination object
    let head_result = fixture.client
        .head_object()
        .bucket(&fixture.bucket)
        .key(&dest_key)
        .send()
        .await
        .unwrap();

    assert_eq!(
        head_result.content_length(),
        Some(20 * 1024 * 1024),
        "Copied object should have same size as source"
    );
}
