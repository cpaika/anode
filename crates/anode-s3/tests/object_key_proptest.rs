// Property-based tests for object key validation

use anode_s3::S3Router;
use anode_storage::{StorageConfig, StorageEngine};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use proptest::prelude::*;
use std::sync::Arc;

fn valid_object_key_strategy() -> impl Strategy<Value = String> {
    // S3 object key rules:
    // - Up to 1024 bytes
    // - Can contain UTF-8 characters
    // - Safe characters commonly used: alphanumeric, !, -, _, ., *, ', (, ), /
    // - For this test, we'll generate keys using safe characters to ensure broad compatibility

    // Generate safe character
    let safe_char = prop_oneof![
        prop::char::range('a', 'z'),
        prop::char::range('A', 'Z'),
        prop::char::range('0', '9'),
        Just('/'),
        Just('-'),
        Just('_'),
        Just('.'),
        Just('!'),
        Just('*'),
        Just('\''),
        Just('('),
        Just(')'),
    ];

    // Generate object keys from 1 to 255 characters (reasonable subset of 1024 limit)
    // Using 255 as upper bound to keep tests fast while still testing variety
    prop::collection::vec(safe_char, 1..=255)
        .prop_map(|chars| chars.into_iter().collect::<String>())
        // Filter out keys that are just "." or ".." (these might cause path issues)
        .prop_filter("not dot or dotdot", |s| s != "." && s != "..")
}

proptest! {
    #[test]
    fn proptest_valid_object_keys_are_accepted(object_key in valid_object_key_strategy()) {
        use tower::Service;

        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
            let config = StorageConfig::new(temp_dir.path().to_path_buf());
            let storage = Arc::new(
                StorageEngine::open(config).await.expect("Failed to open storage engine"),
            );

            // Create a single router instance that will maintain state across requests
            let mut app = S3Router::new(storage.clone(), None).into_router();

            // Create a test bucket first
            let bucket_name = "test-bucket";
            {
                let request = Request::builder()
                    .uri(format!("/{}", bucket_name))
                    .method("PUT")
                    .body(Body::empty())
                    .unwrap();

                let response = Service::call(&mut app, request).await.unwrap();
                prop_assert_eq!(
                    response.status(),
                    StatusCode::OK,
                    "Failed to create test bucket"
                );
            }

            // Attempt to PUT an object with the generated valid key
            {
                let uri = format!("/{}/{}", bucket_name, urlencoding::encode(&object_key));
                let request = Request::builder()
                    .uri(&uri)
                    .method("PUT")
                    .body(Body::from("test data"))
                    .unwrap();

                let response = Service::call(&mut app, request).await.unwrap();

                // Valid object keys should be accepted (200 OK)
                prop_assert_eq!(
                    response.status(),
                    StatusCode::OK,
                    "Object key '{}' should be accepted", object_key
                );
            }

            // Verify we can GET the object back
            {
                let uri = format!("/{}/{}", bucket_name, urlencoding::encode(&object_key));
                let request = Request::builder()
                    .uri(&uri)
                    .method("GET")
                    .body(Body::empty())
                    .unwrap();

                let response = Service::call(&mut app, request).await.unwrap();

                // Should be able to retrieve the object
                prop_assert_eq!(
                    response.status(),
                    StatusCode::OK,
                    "Should be able to retrieve object with key '{}'", object_key
                );
            }

            Ok(())
        }).unwrap();
    }
}

#[tokio::test]
async fn test_various_object_key_patterns() {
    use tower::Service;

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let config = StorageConfig::new(temp_dir.path().to_path_buf());
    let storage = Arc::new(
        StorageEngine::open(config)
            .await
            .expect("Failed to open storage engine"),
    );

    let bucket_name = "test-bucket";

    // Create a single router instance that will maintain state across requests
    let mut app = S3Router::new(storage.clone(), None).into_router();

    // Create bucket
    let request = Request::builder()
        .uri(format!("/{}", bucket_name))
        .method("PUT")
        .body(Body::empty())
        .unwrap();
    let response = Service::call(&mut app, request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Test various key patterns that should be valid
    let test_keys = vec![
        "simple.txt",
        "path/to/object.txt",
        "deep/nested/path/to/file.dat",
        "file-with-dashes.txt",
        "file_with_underscores.txt",
        "file.with.dots.txt",
        "MixedCase.TXT",
        "file123.txt",
        "!important.txt",
        "data*.csv",
        "archive'file.zip",
        "report(2024).pdf",
        "a",                               // Single character
        "a/b/c/d/e/f/g/h/i/j/k/l/m/n/o/p", // Deep nesting
    ];

    for key in test_keys {
        let uri = format!("/{}/{}", bucket_name, urlencoding::encode(key));
        let request = Request::builder()
            .uri(&uri)
            .method("PUT")
            .body(Body::from("test data"))
            .unwrap();

        let response = Service::call(&mut app, request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "Object key '{}' should be accepted",
            key
        );
    }
}
