use crate::client::{AnodeClient, ListObjectsOutput};
use crate::error::Result;

/// Fluent API for bucket operations.
///
/// `BucketOperations` provides a fluent interface for working with a specific bucket.
/// It allows you to perform bucket-level operations like create, delete, check existence,
/// and list objects without repeatedly specifying the bucket name.
///
/// # Example
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
/// use anode_client::bucket::BucketExt;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ClientConfig::new("http://localhost:8080");
/// let client = AnodeClient::new(config)?;
///
/// let bucket = client.bucket("my-bucket");
/// bucket.create().await?;
///
/// if bucket.exists().await? {
///     println!("Bucket exists!");
/// }
/// # Ok(())
/// # }
/// ```
pub struct BucketOperations<'a> {
    client: &'a AnodeClient,
    bucket: String,
}

impl<'a> BucketOperations<'a> {
    /// Creates a new `BucketOperations` instance.
    ///
    /// This is typically not called directly. Use [`BucketExt::bucket`] instead.
    pub fn new(client: &'a AnodeClient, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
        }
    }

    /// Creates this bucket.
    ///
    /// # Errors
    ///
    /// Returns an error if the bucket already exists or authentication fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::bucket::BucketExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let bucket = client.bucket("my-bucket");
    /// bucket.create().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create(&self) -> Result<()> {
        self.client.create_bucket(&self.bucket).await
    }

    /// Deletes this bucket.
    ///
    /// The bucket must be empty before it can be deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the bucket is not empty or does not exist.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::bucket::BucketExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let bucket = client.bucket("my-bucket");
    /// bucket.delete().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete(&self) -> Result<()> {
        self.client.delete_bucket(&self.bucket).await
    }

    /// Checks if this bucket exists.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::bucket::BucketExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let bucket = client.bucket("my-bucket");
    /// if bucket.exists().await? {
    ///     println!("Bucket exists!");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn exists(&self) -> Result<bool> {
        self.client.head_bucket(&self.bucket).await
    }

    /// Returns a builder for listing objects in this bucket.
    ///
    /// Use the builder to configure prefix filtering and maximum keys.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::bucket::BucketExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let bucket = client.bucket("my-bucket");
    /// let result = bucket.list_objects().await
    ///     .prefix("photos/")
    ///     .max_keys(100)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_objects(&self) -> ListObjectsBuilder<'a> {
        ListObjectsBuilder::new(self.client, &self.bucket)
    }

    /// Returns the bucket name.
    ///
    /// # Example
    ///
    /// ```
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::bucket::BucketExt;
    /// # let client = AnodeClient::new(ClientConfig::default()).unwrap();
    /// let bucket = client.bucket("my-bucket");
    /// assert_eq!(bucket.name(), "my-bucket");
    /// ```
    pub fn name(&self) -> &str {
        &self.bucket
    }
}

/// Builder for listing objects with optional filtering parameters.
///
/// This builder allows you to configure prefix filtering and maximum keys
/// before executing the list objects request.
///
/// # Example
///
/// ```no_run
/// # use anode_client::{ClientConfig, AnodeClient};
/// # use anode_client::bucket::BucketExt;
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let client = AnodeClient::new(ClientConfig::default())?;
/// let bucket = client.bucket("my-bucket");
/// let result = bucket.list_objects().await
///     .prefix("photos/2024/")
///     .max_keys(1000)
///     .send()
///     .await?;
///
/// for obj in result.objects {
///     println!("{}: {} bytes", obj.key, obj.size);
/// }
/// # Ok(())
/// # }
/// ```
pub struct ListObjectsBuilder<'a> {
    client: &'a AnodeClient,
    bucket: String,
    prefix: Option<String>,
    max_keys: Option<i32>,
}

impl<'a> ListObjectsBuilder<'a> {
    /// Creates a new `ListObjectsBuilder`.
    ///
    /// This is typically not called directly. Use [`BucketOperations::list_objects`] instead.
    pub fn new(client: &'a AnodeClient, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
            prefix: None,
            max_keys: None,
        }
    }

    /// Sets the prefix filter for object keys.
    ///
    /// Only objects with keys starting with this prefix will be returned.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::bucket::BucketExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let bucket = client.bucket("my-bucket");
    /// let result = bucket.list_objects().await
    ///     .prefix("photos/")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Sets the maximum number of keys to return.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::bucket::BucketExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let bucket = client.bucket("my-bucket");
    /// let result = bucket.list_objects().await
    ///     .max_keys(100)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn max_keys(mut self, max_keys: i32) -> Self {
        self.max_keys = Some(max_keys);
        self
    }

    /// Executes the list objects request.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::bucket::BucketExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let bucket = client.bucket("my-bucket");
    /// let result = bucket.list_objects().await.send().await?;
    /// println!("Found {} objects", result.objects.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send(self) -> Result<ListObjectsOutput> {
        self.client
            .list_objects(&self.bucket, self.prefix.as_deref(), self.max_keys)
            .await
    }
}

/// Extension trait for [`AnodeClient`] to provide fluent bucket API.
///
/// This trait adds the [`bucket`](BucketExt::bucket) method to [`AnodeClient`],
/// enabling a more expressive fluent API for bucket operations.
///
/// # Example
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
/// use anode_client::bucket::BucketExt;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ClientConfig::new("http://localhost:8080");
/// let client = AnodeClient::new(config)?;
///
/// // Use the fluent API
/// let bucket = client.bucket("my-bucket");
/// bucket.create().await?;
/// # Ok(())
/// # }
/// ```
pub trait BucketExt {
    /// Creates a [`BucketOperations`] instance for the specified bucket.
    ///
    /// # Arguments
    ///
    /// * `name` - The bucket name
    ///
    /// # Example
    ///
    /// ```
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use anode_client::bucket::BucketExt;
    /// # let client = AnodeClient::new(ClientConfig::default()).unwrap();
    /// let bucket = client.bucket("my-bucket");
    /// assert_eq!(bucket.name(), "my-bucket");
    /// ```
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
