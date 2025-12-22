use anode_client::{AnodeClient, ClientConfig};
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client configuration
    let config =
        ClientConfig::new("http://localhost:8080").with_credentials("minioadmin", "minioadmin");

    let client = AnodeClient::new(config)?;

    println!("Multipart Upload Example\n");

    // Create a bucket
    let bucket = "multipart-example";
    println!("Creating bucket: {}", bucket);
    client.create_bucket(bucket).await?;
    println!("  ✓ Bucket created\n");

    // Prepare data for multipart upload
    // Each part must be at least 5 MB (except the last part)
    let part1_data = Bytes::from(vec![1u8; 5 * 1024 * 1024]); // 5 MB
    let part2_data = Bytes::from(vec![2u8; 5 * 1024 * 1024]); // 5 MB
    let part3_data = Bytes::from(vec![3u8; 1024 * 1024]); // 1 MB (last part)

    // Create multipart upload
    let key = "large-file.bin";
    let mut upload = client.multipart_upload(bucket, key);

    // Initialize the upload
    println!("Initializing multipart upload...");
    let upload_id = upload.init().await?;
    println!("  ✓ Upload ID: {}\n", upload_id);

    // Upload parts
    println!("Uploading parts:");
    println!("  Part 1 ({} bytes)...", part1_data.len());
    let part1 = upload.upload_part(1, part1_data).await?;
    println!("    ✓ ETag: {}", part1.etag);

    println!("  Part 2 ({} bytes)...", part2_data.len());
    let part2 = upload.upload_part(2, part2_data).await?;
    println!("    ✓ ETag: {}", part2.etag);

    println!("  Part 3 ({} bytes)...", part3_data.len());
    let part3 = upload.upload_part(3, part3_data).await?;
    println!("    ✓ ETag: {}", part3.etag);

    // Complete the upload
    println!("\nCompleting multipart upload...");
    let result = upload.complete().await?;
    println!("  ✓ Upload complete!");
    println!("    Location: {}", result.location);
    println!("    ETag: {}", result.etag);

    // Verify the uploaded file
    println!("\nVerifying uploaded file...");
    let metadata = client.head_object(bucket, key).await?;
    println!("  Size: {} bytes", metadata.size);
    println!("  ETag: {}", metadata.etag);

    // Cleanup
    println!("\nCleaning up...");
    client.delete_object(bucket, key).await?;
    client.delete_bucket(bucket).await?;
    println!("  ✓ Cleanup complete");

    Ok(())
}
