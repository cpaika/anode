// Property-based tests for bucket name validation

use anode_s3::S3Router;
use anode_storage::{StorageConfig, StorageEngine};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use proptest::prelude::*;
use std::sync::Arc;
use tower::ServiceExt;

fn valid_bucket_name_strategy() -> impl Strategy<Value = String> {
    // S3 bucket naming rules:
    // - 3-63 characters
    // - Lowercase letters, numbers, hyphens, and periods
    // - Cannot start or end with hyphen
    // - Must start and end with lowercase letter or number

    // Generate first character: lowercase letter or digit
    let first_char = prop_oneof![prop::char::range('a', 'z'), prop::char::range('0', '9'),];

    // Generate middle characters: lowercase letter, digit, hyphen, or period
    let middle_char = prop_oneof![
        prop::char::range('a', 'z'),
        prop::char::range('0', '9'),
        Just('-'),
        Just('.'),
    ];

    // Generate last character: lowercase letter or digit
    let last_char = prop_oneof![prop::char::range('a', 'z'), prop::char::range('0', '9'),];

    // For buckets with 3 characters: first + middle + last
    // For buckets with more: first + (1-61 middle chars) + last
    (first_char, prop::collection::vec(middle_char, 1..=61), last_char)
        .prop_map(|(first, middle, last)| {
            let mut name = String::new();
            name.push(first);
            for ch in middle {
                name.push(ch);
            }
            name.push(last);
            name
        })
        // Filter out names with consecutive periods (more strict S3 rule)
        .prop_filter("no consecutive periods", |s| !s.contains(".."))
}

proptest! {
    #[test]
    fn proptest_valid_bucket_names_are_accepted(bucket_name in valid_bucket_name_strategy()) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
            let config = StorageConfig::new(temp_dir.path().to_path_buf());
            let storage = Arc::new(
                StorageEngine::open(config).await.expect("Failed to open storage engine"),
            );
            let app = S3Router::new(storage, None).into_router();

            // Attempt to create bucket with the generated valid name
            let request = Request::builder()
                .uri(&format!("/{}", bucket_name))
                .method("PUT")
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();

            // Valid bucket names should be accepted (200 OK)
            prop_assert_eq!(
                response.status(),
                StatusCode::OK,
                "Bucket name '{}' should be accepted", bucket_name
            );

            Ok(())
        }).unwrap();
    }
}

#[tokio::test]
async fn test_invalid_bucket_names_are_rejected() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let config = StorageConfig::new(temp_dir.path().to_path_buf());
    let storage = Arc::new(
        StorageEngine::open(config)
            .await
            .expect("Failed to open storage engine"),
    );

    let long_name = "a".repeat(64);
    let invalid_names = vec![
        "ab",        // Too short (less than 3 chars)
        &long_name,  // Too long (more than 63 chars)
        "MyBucket",  // Contains uppercase
        "-bucket",   // Starts with hyphen
        "bucket-",   // Ends with hyphen
        "my_bucket", // Contains underscore
    ];

    for invalid_name in invalid_names {
        let app = S3Router::new(storage.clone(), None).into_router();
        let request = Request::builder()
            .uri(&format!("/{}", invalid_name))
            .method("PUT")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Invalid bucket names should be rejected (not 200 OK)
        assert_ne!(
            response.status(),
            StatusCode::OK,
            "Bucket name '{}' should be rejected",
            invalid_name
        );
    }
}
