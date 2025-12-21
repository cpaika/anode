// Example S3-compatible server using anode-s3
//
// This example demonstrates how to set up a simple S3-compatible server
// using the anode-s3 crate.
//
// To run this example:
//   cargo run --example simple_server
//
// Then you can use the AWS CLI to interact with it:
//   aws --endpoint-url http://localhost:8080 s3 mb s3://my-bucket
//   aws --endpoint-url http://localhost:8080 s3 cp file.txt s3://my-bucket/
//   aws --endpoint-url http://localhost:8080 s3 ls s3://my-bucket

use anode_s3::{AuthConfig, S3Router};
use anode_storage::{StorageEngine, StorageConfig};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create storage engine
    let data_dir = PathBuf::from("./data");
    let config = StorageConfig::new(data_dir);
    let storage = Arc::new(StorageEngine::open(config).await.unwrap());

    // Configure authentication (optional)
    let auth = if std::env::var("S3_DISABLE_AUTH").is_ok() {
        None
    } else {
        let mut access_keys = HashMap::new();
        access_keys.insert(
            "AKIAIOSFODNN7EXAMPLE".to_string(),
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
        );
        Some(AuthConfig::enabled(access_keys))
    };

    // Create S3 router
    let app = S3Router::new(storage, auth).into_router();

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("S3-compatible server listening on {}", addr);
    println!("\nExample commands:");
    println!("  aws --endpoint-url http://localhost:8080 s3 mb s3://my-bucket");
    println!("  aws --endpoint-url http://localhost:8080 s3 cp file.txt s3://my-bucket/");
    println!("  aws --endpoint-url http://localhost:8080 s3 ls s3://my-bucket");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
