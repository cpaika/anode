use anode_client::{AnodeClient, ClientConfig};
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client configuration
    let config = ClientConfig::new("http://localhost:8080")
        .with_credentials("minioadmin", "minioadmin");

    let client = AnodeClient::new(config)?;

    println!("Anode Client SDK Example\n");

    // Create a bucket
    let bucket = "example-bucket";
    println!("Creating bucket: {}", bucket);
    match client.create_bucket(bucket).await {
        Ok(_) => println!("  ✓ Bucket created successfully"),
        Err(e) => println!("  ✗ Failed to create bucket: {}", e),
    }

    // Upload an object
    let key = "hello.txt";
    let data = Bytes::from("Hello, Anode!");
    println!("\nUploading object: {}/{}", bucket, key);
    match client.put_object(bucket, key, data).await {
        Ok(result) => println!("  ✓ Object uploaded, ETag: {}", result.etag),
        Err(e) => println!("  ✗ Failed to upload object: {}", e),
    }

    // List objects
    println!("\nListing objects in bucket:");
    match client.list_objects(bucket, None, None).await {
        Ok(result) => {
            for obj in result.objects {
                println!("  - {} ({} bytes)", obj.key, obj.size);
            }
        }
        Err(e) => println!("  ✗ Failed to list objects: {}", e),
    }

    // Download the object
    println!("\nDownloading object: {}/{}", bucket, key);
    match client.get_object(bucket, key).await {
        Ok(result) => {
            let content = String::from_utf8_lossy(&result.body);
            println!("  ✓ Downloaded: {}", content);
        }
        Err(e) => println!("  ✗ Failed to download object: {}", e),
    }

    // Get object metadata
    println!("\nGetting object metadata:");
    match client.head_object(bucket, key).await {
        Ok(metadata) => {
            println!("  Key: {}", metadata.key);
            println!("  Size: {} bytes", metadata.size);
            println!("  ETag: {}", metadata.etag);
            println!("  Last Modified: {}", metadata.last_modified);
        }
        Err(e) => println!("  ✗ Failed to get metadata: {}", e),
    }

    // Delete the object
    println!("\nDeleting object: {}/{}", bucket, key);
    match client.delete_object(bucket, key).await {
        Ok(_) => println!("  ✓ Object deleted successfully"),
        Err(e) => println!("  ✗ Failed to delete object: {}", e),
    }

    // Delete the bucket
    println!("\nDeleting bucket: {}", bucket);
    match client.delete_bucket(bucket).await {
        Ok(_) => println!("  ✓ Bucket deleted successfully"),
        Err(e) => println!("  ✗ Failed to delete bucket: {}", e),
    }

    Ok(())
}
