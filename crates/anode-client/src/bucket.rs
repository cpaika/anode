use crate::client::{AnodeClient, ListObjectsOutput};
use crate::error::Result;

/// Fluent API for bucket operations
pub struct BucketOperations<'a> {
    client: &'a AnodeClient,
    bucket: String,
}

impl<'a> BucketOperations<'a> {
    pub fn new(client: &'a AnodeClient, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
        }
    }

    /// Create this bucket
    pub async fn create(&self) -> Result<()> {
        self.client.create_bucket(&self.bucket).await
    }

    /// Delete this bucket
    pub async fn delete(&self) -> Result<()> {
        self.client.delete_bucket(&self.bucket).await
    }

    /// Check if this bucket exists
    pub async fn exists(&self) -> Result<bool> {
        self.client.head_bucket(&self.bucket).await
    }

    /// List objects in this bucket
    pub async fn list_objects(&self) -> ListObjectsBuilder<'a> {
        ListObjectsBuilder::new(self.client, &self.bucket)
    }

    /// Get the bucket name
    pub fn name(&self) -> &str {
        &self.bucket
    }
}

/// Fluent API for listing objects with optional parameters
pub struct ListObjectsBuilder<'a> {
    client: &'a AnodeClient,
    bucket: String,
    prefix: Option<String>,
    max_keys: Option<i32>,
}

impl<'a> ListObjectsBuilder<'a> {
    pub fn new(client: &'a AnodeClient, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
            prefix: None,
            max_keys: None,
        }
    }

    /// Set the prefix filter for object keys
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Set the maximum number of keys to return
    pub fn max_keys(mut self, max_keys: i32) -> Self {
        self.max_keys = Some(max_keys);
        self
    }

    /// Execute the list objects request
    pub async fn send(self) -> Result<ListObjectsOutput> {
        self.client
            .list_objects(&self.bucket, self.prefix.as_deref(), self.max_keys)
            .await
    }
}

/// Extension trait for AnodeClient to provide fluent bucket API
pub trait BucketExt {
    fn bucket(&self, name: impl Into<String>) -> BucketOperations<'_>;
}

impl BucketExt for AnodeClient {
    fn bucket(&self, name: impl Into<String>) -> BucketOperations<'_> {
        BucketOperations::new(self, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket_operations_name() {
        use crate::config::ClientConfig;

        let config = ClientConfig::default();
        let client = AnodeClient::new(config).unwrap();
        let bucket = BucketOperations::new(&client, "test-bucket");

        assert_eq!(bucket.name(), "test-bucket");
    }
}
