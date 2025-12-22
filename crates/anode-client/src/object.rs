use crate::client::{
    AnodeClient, CopyObjectOutput, GetObjectOutput, ObjectMetadata, PutObjectOutput,
};
use crate::error::Result;
use crate::multipart::MultipartUpload;
use bytes::Bytes;
use futures::Stream;
use std::time::Duration;

/// Fluent API for object operations.
///
/// `ObjectOperations` provides a fluent interface for working with a specific object.
/// It allows you to perform object-level operations like upload, download, delete,
/// and copy without repeatedly specifying the bucket and key.
///
/// # Example
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
/// use anode_client::object::ObjectExt;
/// use bytes::Bytes;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ClientConfig::new("http://localhost:8080");
/// let client = AnodeClient::new(config)?;
///
/// let object = client.object("my-bucket", "my-file.txt");
/// object.put(Bytes::from("Hello, World!")).await?;
///
/// let data = object.get().await?;
/// println!("Downloaded {} bytes", data.body.len());
///
/// object.delete().await?;
/// # Ok(())
/// # }
/// ```
pub struct ObjectOperations<'a> {
    client: &'a AnodeClient,
    bucket: String,
    key: String,
}

impl<'a> ObjectOperations<'a> {
    /// Creates a new `ObjectOperations` instance.
    ///
    /// This is typically not called directly. Use [`ObjectExt::object`] instead.
    pub fn new(client: &'a AnodeClient, bucket: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
            key: key.into(),
        }
    }

    /// Uploads this object with the given data.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # use bytes::Bytes;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let object = client.object("my-bucket", "file.txt");
    /// object.put(Bytes::from("Hello!")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn put(&self, data: impl Into<Bytes>) -> Result<PutObjectOutput> {
        self.client.put_object(&self.bucket, &self.key, data).await
    }

    /// Downloads this object.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let object = client.object("my-bucket", "file.txt");
    /// let result = object.get().await?;
    /// println!("Downloaded {} bytes", result.body.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get(&self) -> Result<GetObjectOutput> {
        self.client.get_object(&self.bucket, &self.key).await
    }

    /// Gets metadata for this object without downloading its body.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let object = client.object("my-bucket", "file.txt");
    /// let metadata = object.head().await?;
    /// println!("Object size: {} bytes", metadata.size);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn head(&self) -> Result<ObjectMetadata> {
        self.client.head_object(&self.bucket, &self.key).await
    }

    /// Deletes this object.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let object = client.object("my-bucket", "file.txt");
    /// object.delete().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete(&self) -> Result<()> {
        self.client.delete_object(&self.bucket, &self.key).await
    }

    /// Copies this object to a new location.
    ///
    /// This is a server-side copy, so the object data doesn't need to be
    /// transferred through the client.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let object = client.object("my-bucket", "file.txt");
    /// object.copy_to("my-bucket", "backup.txt").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn copy_to(
        &self,
        dst_bucket: impl Into<String>,
        dst_key: impl Into<String>,
    ) -> Result<CopyObjectOutput> {
        self.client
            .copy_object(&self.bucket, &self.key, &dst_bucket.into(), &dst_key.into())
            .await
    }

    /// Gets a specific byte range from this object.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let object = client.object("my-bucket", "file.bin");
    /// let bytes = object.get_range(0, 1023).await?;
    /// println!("Downloaded first 1024 bytes");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_range(&self, start: u64, end: u64) -> Result<Bytes> {
        self.client
            .get_object_range(&self.bucket, &self.key, start, end)
            .await
    }

    /// Downloads this object as a stream.
    ///
    /// Useful for downloading large files without loading them entirely into memory.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # use futures::StreamExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let object = client.object("my-bucket", "large-file.bin");
    /// let mut stream = object.get_stream().await?;
    /// while let Some(chunk) = stream.next().await {
    ///     let bytes = chunk?;
    ///     println!("Received {} bytes", bytes.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_stream(&self) -> Result<impl Stream<Item = Result<Bytes>>> {
        self.client.get_object_stream(&self.bucket, &self.key).await
    }

    /// Uploads this object from a stream.
    ///
    /// Useful for uploading large files without loading them entirely into memory.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # use bytes::Bytes;
    /// # use futures::stream;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let object = client.object("my-bucket", "file.bin");
    /// let data = vec![Bytes::from("chunk1"), Bytes::from("chunk2")];
    /// let stream = stream::iter(data.into_iter().map(Ok));
    /// object.put_stream(stream, 12).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn put_stream<S>(&self, stream: S, content_length: u64) -> Result<PutObjectOutput>
    where
        S: Stream<Item = std::result::Result<Bytes, std::io::Error>> + Send + 'static,
    {
        self.client
            .put_object_stream(&self.bucket, &self.key, stream, content_length)
            .await
    }

    /// Creates a multipart upload for this object.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # use bytes::Bytes;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let object = client.object("my-bucket", "large-file.bin");
    /// let mut upload = object.multipart_upload();
    /// upload.init().await?;
    /// upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
    /// upload.complete().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn multipart_upload(&self) -> MultipartUpload<'a> {
        self.client.multipart_upload(&self.bucket, &self.key)
    }

    /// Generates a presigned GET URL for this object.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # use std::time::Duration;
    /// # let config = ClientConfig::new("http://localhost:8080").with_credentials("key", "secret");
    /// # let client = AnodeClient::new(config).unwrap();
    /// let object = client.object("my-bucket", "file.txt");
    /// let url = object.presign_get(Duration::from_secs(3600))?;
    /// println!("Download URL (valid for 1 hour): {}", url);
    /// # Ok::<(), anode_client::ClientError>(())
    /// ```
    pub fn presign_get(&self, expires_in: Duration) -> Result<String> {
        self.client.presign_get(&self.bucket, &self.key, expires_in)
    }

    /// Generates a presigned PUT URL for this object.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # use std::time::Duration;
    /// # let config = ClientConfig::new("http://localhost:8080").with_credentials("key", "secret");
    /// # let client = AnodeClient::new(config).unwrap();
    /// let object = client.object("my-bucket", "file.txt");
    /// let url = object.presign_put(Duration::from_secs(3600))?;
    /// println!("Upload URL (valid for 1 hour): {}", url);
    /// # Ok::<(), anode_client::ClientError>(())
    /// ```
    pub fn presign_put(&self, expires_in: Duration) -> Result<String> {
        self.client.presign_put(&self.bucket, &self.key, expires_in)
    }

    /// Returns the bucket name.
    ///
    /// # Example
    ///
    /// ```
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # let client = AnodeClient::new(ClientConfig::default()).unwrap();
    /// let object = client.object("my-bucket", "my-key");
    /// assert_eq!(object.bucket(), "my-bucket");
    /// ```
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// Returns the object key.
    ///
    /// # Example
    ///
    /// ```
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # let client = AnodeClient::new(ClientConfig::default()).unwrap();
    /// let object = client.object("my-bucket", "my-key");
    /// assert_eq!(object.key(), "my-key");
    /// ```
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// Extension trait for [`AnodeClient`] to provide fluent object API.
///
/// This trait adds the [`object`](ObjectExt::object) method to [`AnodeClient`],
/// enabling a more expressive fluent API for object operations.
///
/// # Example
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
/// use anode_client::object::ObjectExt;
/// use bytes::Bytes;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ClientConfig::new("http://localhost:8080");
/// let client = AnodeClient::new(config)?;
///
/// // Use the fluent API
/// let object = client.object("my-bucket", "my-file.txt");
/// object.put(Bytes::from("Hello!")).await?;
/// # Ok(())
/// # }
/// ```
pub trait ObjectExt {
    /// Creates an [`ObjectOperations`] instance for the specified object.
    ///
    /// # Arguments
    ///
    /// * `bucket` - The bucket name
    /// * `key` - The object key
    ///
    /// # Example
    ///
    /// ```
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::object::ObjectExt;
    /// # let client = AnodeClient::new(ClientConfig::default()).unwrap();
    /// let object = client.object("my-bucket", "my-key");
    /// assert_eq!(object.bucket(), "my-bucket");
    /// assert_eq!(object.key(), "my-key");
    /// ```
    fn object(&self, bucket: impl Into<String>, key: impl Into<String>) -> ObjectOperations<'_>;
}

impl ObjectExt for AnodeClient {
    fn object(&self, bucket: impl Into<String>, key: impl Into<String>) -> ObjectOperations<'_> {
        ObjectOperations::new(self, bucket, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_operations_getters() {
        use crate::config::ClientConfig;

        let config = ClientConfig::default();
        let client = AnodeClient::new(config).unwrap();
        let object = ObjectOperations::new(&client, "test-bucket", "test-key");

        assert_eq!(object.bucket(), "test-bucket");
        assert_eq!(object.key(), "test-key");
    }
}
