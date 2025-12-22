use anode_client::bucket::BucketExt;
use anode_client::object::ObjectExt;
use anode_client::{AnodeClient, ClientConfig};
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client configuration
    let config =
        ClientConfig::new("http://localhost:8080").with_credentials("minioadmin", "minioadmin");

    let client = AnodeClient::new(config)?;

    println!("Fluent API Example\n");

    // Using fluent bucket API
    println!("Creating bucket using fluent API...");
    let bucket = client.bucket("fluent-example");
    bucket.create().await?;
    println!("  ✓ Bucket created\n");

    // Check if bucket exists
    let exists = bucket.exists().await?;
    println!("Bucket exists: {}\n", exists);

    // Using fluent object API
    println!("Working with objects using fluent API:");

    // Put object
    let object = client.object("fluent-example", "document.txt");
    let data = Bytes::from("This is a document created with the fluent API!");

    println!("  Uploading object...");
    let result = object.put(data.clone()).await?;
    println!("    ✓ ETag: {}\n", result.etag);

    // Get object metadata
    println!("  Getting metadata...");
    let metadata = object.head().await?;
    println!("    Key: {}", metadata.key);
    println!("    Size: {} bytes", metadata.size);
    println!("    ETag: {}\n", metadata.etag);

    // Download object
    println!("  Downloading object...");
    let download = object.get().await?;
    let content = String::from_utf8_lossy(&download.body);
    println!("    Content: {}\n", content);

    // Copy object
    println!("  Copying object...");
    object
        .copy_to("fluent-example", "document-copy.txt")
        .await?;
    println!("    ✓ Object copied\n");

    // List objects
    println!("Listing all objects:");
    let list = bucket.list_objects().await.send().await?;
    for obj in list.objects {
        println!("  - {} ({} bytes)", obj.key, obj.size);
    }
    println!();

    // Delete objects
    println!("Cleaning up...");
    object.delete().await?;
    let copy_object = client.object("fluent-example", "document-copy.txt");
    copy_object.delete().await?;
    bucket.delete().await?;
    println!("  ✓ Cleanup complete");

    Ok(())
}
