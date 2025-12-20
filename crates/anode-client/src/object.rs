use crate::client::{
    AnodeClient, CopyObjectOutput, GetObjectOutput, ObjectMetadata, PutObjectOutput,
};
use crate::error::Result;
use crate::multipart::MultipartUpload;
use bytes::Bytes;
use futures::Stream;
use std::time::Duration;

/// Fluent API for object operations
pub struct ObjectOperations<'a> {
    client: &'a AnodeClient,
    bucket: String,
    key: String,
}

impl<'a> ObjectOperations<'a> {
    pub fn new(client: &'a AnodeClient, bucket: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
            key: key.into(),
        }
    }

    /// Upload this object with the given data
    pub async fn put(&self, data: impl Into<Bytes>) -> Result<PutObjectOutput> {
        self.client.put_object(&self.bucket, &self.key, data).await
    }

    /// Download this object
    pub async fn get(&self) -> Result<GetObjectOutput> {
        self.client.get_object(&self.bucket, &self.key).await
    }

    /// Get metadata for this object
    pub async fn head(&self) -> Result<ObjectMetadata> {
        self.client.head_object(&self.bucket, &self.key).await
    }

    /// Delete this object
    pub async fn delete(&self) -> Result<()> {
        self.client.delete_object(&self.bucket, &self.key).await
    }

    /// Copy this object to a new location
    pub async fn copy_to(
        &self,
        dst_bucket: impl Into<String>,
        dst_key: impl Into<String>,
    ) -> Result<CopyObjectOutput> {
        self.client
            .copy_object(
                &self.bucket,
                &self.key,
                &dst_bucket.into(),
                &dst_key.into(),
            )
            .await
    }

    /// Get a range of bytes from this object
    pub async fn get_range(&self, start: u64, end: u64) -> Result<Bytes> {
        self.client
            .get_object_range(&self.bucket, &self.key, start, end)
            .await
    }

    /// Get this object as a stream
    pub async fn get_stream(&self) -> Result<impl Stream<Item = Result<Bytes>>> {
        self.client.get_object_stream(&self.bucket, &self.key).await
    }

    /// Upload this object from a stream
    pub async fn put_stream<S>(
        &self,
        stream: S,
        content_length: u64,
    ) -> Result<PutObjectOutput>
    where
        S: Stream<Item = std::result::Result<Bytes, std::io::Error>> + Send + 'static,
    {
        self.client
            .put_object_stream(&self.bucket, &self.key, stream, content_length)
            .await
    }

    /// Create a multipart upload for this object
    pub fn multipart_upload(&self) -> MultipartUpload<'a> {
        self.client.multipart_upload(&self.bucket, &self.key)
    }

    /// Generate a presigned GET URL for this object
    pub fn presign_get(&self, expires_in: Duration) -> Result<String> {
        self.client.presign_get(&self.bucket, &self.key, expires_in)
    }

    /// Generate a presigned PUT URL for this object
    pub fn presign_put(&self, expires_in: Duration) -> Result<String> {
        self.client.presign_put(&self.bucket, &self.key, expires_in)
    }

    /// Get the bucket name
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// Get the object key
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// Extension trait for AnodeClient to provide fluent object API
pub trait ObjectExt {
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
