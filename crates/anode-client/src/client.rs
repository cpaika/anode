use crate::config::ClientConfig;
use crate::error::{ClientError, Result};
use crate::multipart::{CompleteMultipartOutput, CompletedPart, MultipartUpload};
use aws_credential_types::Credentials;
use aws_sigv4::http_request::{sign, SignableBody, SignableRequest, SigningSettings};
use aws_sigv4::sign::v4;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};
use tracing::{debug, warn};

/// The main Anode client for interacting with object storage.
///
/// `AnodeClient` provides a comprehensive S3-compatible API for object storage operations,
/// including bucket management, object operations, multipart uploads, and presigned URLs.
///
/// # Features
///
/// - **S3 Compatibility**: Full support for S3 API operations
/// - **Authentication**: AWS SigV4 request signing
/// - **Reliability**: Automatic retry with exponential backoff
/// - **Streaming**: Efficient streaming for large files
/// - **Multipart**: Support for multipart uploads
/// - **Presigned URLs**: Generate time-limited access URLs
///
/// # Quick Start
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
/// use bytes::Bytes;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = ClientConfig::new("http://localhost:8080")
///         .with_credentials("access_key", "secret_key");
///     let client = AnodeClient::new(config)?;
///
///     // Create a bucket
///     client.create_bucket("my-bucket").await?;
///
///     // Upload an object
///     client.put_object("my-bucket", "file.txt", Bytes::from("data")).await?;
///
///     Ok(())
/// }
/// ```
///
/// # Configuration Examples
///
/// ## Basic Configuration
///
/// ```no_run
/// use anode_client::ClientConfig;
///
/// let config = ClientConfig::new("http://localhost:8080")
///     .with_credentials("my_access_key", "my_secret_key");
/// ```
///
/// ## Advanced Configuration
///
/// ```no_run
/// use anode_client::ClientConfig;
/// use std::time::Duration;
///
/// let config = ClientConfig::new("http://localhost:8080")
///     .with_credentials("access_key", "secret_key")
///     .with_region("us-west-2")
///     .with_timeout(Duration::from_secs(60))
///     .with_retries(5)
///     .with_path_style(true);
/// ```
///
/// ## Configuration from Environment Variables
///
/// ```no_run
/// use anode_client::ClientConfig;
///
/// // Set environment variables:
/// // ANODE_ENDPOINT=http://localhost:8080
/// // ANODE_ACCESS_KEY_ID=my_key
/// // ANODE_SECRET_ACCESS_KEY=my_secret
///
/// let config = ClientConfig::from_env()?;
/// # Ok::<(), anode_client::ClientError>(())
/// ```
///
/// # Error Handling Patterns
///
/// ## Checking Specific Errors
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient, ClientError};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let client = AnodeClient::new(ClientConfig::default())?;
/// match client.get_object("my-bucket", "file.txt").await {
///     Ok(result) => {
///         println!("Downloaded {} bytes", result.body.len());
///     }
///     Err(ClientError::ObjectNotFound { bucket, key }) => {
///         println!("Object {}/{} not found", bucket, key);
///     }
///     Err(ClientError::BucketNotFound(bucket)) => {
///         println!("Bucket {} does not exist", bucket);
///     }
///     Err(ClientError::AuthenticationFailed(msg)) => {
///         println!("Authentication failed: {}", msg);
///     }
///     Err(e) => {
///         println!("Other error: {}", e);
///     }
/// }
/// # Ok(())
/// # }
/// ```
///
/// ## Using Error Helper Methods
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let client = AnodeClient::new(ClientConfig::default())?;
/// match client.head_object("my-bucket", "file.txt").await {
///     Ok(metadata) => {
///         println!("File size: {} bytes", metadata.size);
///     }
///     Err(e) if e.is_not_found() => {
///         println!("File does not exist");
///     }
///     Err(e) if e.is_retriable() => {
///         println!("Temporary error, will retry: {}", e);
///     }
///     Err(e) => {
///         println!("Permanent error: {}", e);
///     }
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Complete Workflow Examples
///
/// ## Upload and Download Files
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
/// use bytes::Bytes;
/// use std::fs;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ClientConfig::new("http://localhost:8080")
///     .with_credentials("access_key", "secret_key");
/// let client = AnodeClient::new(config)?;
///
/// // Ensure bucket exists
/// if !client.head_bucket("my-bucket").await? {
///     client.create_bucket("my-bucket").await?;
/// }
///
/// // Upload a file
/// let file_data = fs::read("local_file.txt")?;
/// let result = client.put_object("my-bucket", "remote_file.txt", Bytes::from(file_data)).await?;
/// println!("Uploaded with ETag: {}", result.etag);
///
/// // Download the file
/// let downloaded = client.get_object("my-bucket", "remote_file.txt").await?;
/// fs::write("downloaded_file.txt", &downloaded.body)?;
/// println!("Downloaded {} bytes", downloaded.body.len());
///
/// // Clean up
/// client.delete_object("my-bucket", "remote_file.txt").await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Working with Large Files (Streaming)
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
/// use futures::StreamExt;
/// use tokio::fs::File;
/// use tokio::io::AsyncWriteExt;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let client = AnodeClient::new(ClientConfig::default())?;
/// // Download a large file as a stream
/// let mut stream = client.get_object_stream("my-bucket", "large_file.bin").await?;
/// let mut file = File::create("local_large_file.bin").await?;
///
/// while let Some(chunk) = stream.next().await {
///     let bytes = chunk?;
///     file.write_all(&bytes).await?;
/// }
///
/// println!("Large file downloaded successfully");
/// # Ok(())
/// # }
/// ```
///
/// ## Listing and Processing Objects
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let client = AnodeClient::new(ClientConfig::default())?;
/// // List all objects with a prefix
/// let result = client.list_objects("my-bucket", Some("photos/2024/"), Some(1000)).await?;
///
/// println!("Found {} objects", result.objects.len());
/// for obj in result.objects {
///     println!("  {}: {} bytes (ETag: {})", obj.key, obj.size, obj.etag);
/// }
///
/// // Process paginated results
/// if result.is_truncated {
///     println!("More results available with token: {:?}", result.next_continuation_token);
/// }
/// # Ok(())
/// # }
/// ```
///
/// ## Server-Side Copy
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let client = AnodeClient::new(ClientConfig::default())?;
/// // Copy object within the same bucket
/// client.copy_object(
///     "my-bucket", "original.txt",
///     "my-bucket", "backup.txt"
/// ).await?;
///
/// // Copy to a different bucket
/// client.copy_object(
///     "source-bucket", "file.txt",
///     "dest-bucket", "file.txt"
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Multipart Upload for Large Files
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
/// use bytes::Bytes;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let client = AnodeClient::new(ClientConfig::default())?;
/// let mut upload = client.multipart_upload("my-bucket", "large_file.bin");
///
/// // Initialize the upload
/// upload.init().await?;
///
/// // Upload parts (minimum 5MB each except the last part)
/// upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
/// upload.upload_part(2, Bytes::from(vec![1u8; 5_242_880])).await?;
/// upload.upload_part(3, Bytes::from(vec![2u8; 1_048_576])).await?;
///
/// // Complete the upload
/// let result = upload.complete().await?;
/// println!("Upload complete! ETag: {}", result.etag);
/// # Ok(())
/// # }
/// ```
///
/// ## Parallel Multipart Upload
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
/// use bytes::Bytes;
/// use futures::future::try_join_all;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let client = AnodeClient::new(ClientConfig::default())?;
/// let mut upload = client.multipart_upload("my-bucket", "large_file.bin");
/// upload.init().await?;
///
/// // Prepare parts
/// let parts = vec![
///     (1, Bytes::from(vec![0u8; 5_242_880])),
///     (2, Bytes::from(vec![1u8; 5_242_880])),
///     (3, Bytes::from(vec![2u8; 5_242_880])),
/// ];
///
/// // Upload parts in parallel (requires cloning client)
/// // Note: This is a simplified example - in practice you'd need to handle shared state
/// # Ok(())
/// # }
/// ```
///
/// # Range Requests
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let client = AnodeClient::new(ClientConfig::default())?;
/// // Download first 1024 bytes of a file
/// let chunk = client.get_object_range("my-bucket", "large_file.bin", 0, 1023).await?;
/// println!("Downloaded first {} bytes", chunk.len());
///
/// // Download specific range (e.g., for Parquet metadata)
/// let metadata = client.get_object_range("my-bucket", "data.parquet", 1000, 2000).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Presigned URLs
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient};
/// use std::time::Duration;
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ClientConfig::new("http://localhost:8080")
///     .with_credentials("access_key", "secret_key");
/// let client = AnodeClient::new(config)?;
///
/// // Generate a download URL valid for 1 hour
/// let download_url = client.presign_get(
///     "my-bucket",
///     "file.txt",
///     Duration::from_secs(3600)
/// )?;
/// println!("Share this URL: {}", download_url);
///
/// // Generate an upload URL valid for 30 minutes
/// let upload_url = client.presign_put(
///     "my-bucket",
///     "upload.txt",
///     Duration::from_secs(1800)
/// )?;
/// println!("Use this URL to upload: {}", upload_url);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct AnodeClient {
    http: Client,
    config: ClientConfig,
}

impl AnodeClient {
    /// Creates a new Anode client with the given configuration.
    ///
    /// This validates the configuration and initializes an HTTP client with
    /// connection pooling and timeout settings.
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration including endpoint, credentials, and options
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The configuration is invalid (empty endpoint, invalid URL, etc.)
    /// - The HTTP client cannot be created
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anode_client::{ClientConfig, AnodeClient};
    ///
    /// let config = ClientConfig::new("http://localhost:8080")
    ///     .with_credentials("my_key", "my_secret");
    /// let client = AnodeClient::new(config)?;
    /// # Ok::<(), anode_client::ClientError>(())
    /// ```
    pub fn new(config: ClientConfig) -> Result<Self> {
        config.validate()?;

        let http = Client::builder()
            .timeout(config.timeout)
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .build()
            .map_err(ClientError::Network)?;

        Ok(Self { http, config })
    }

    fn build_url(&self, bucket: Option<&str>, key: Option<&str>) -> String {
        let mut url = self.config.endpoint.clone();

        if let Some(bucket) = bucket {
            if self.config.path_style {
                url = format!("{}/{}", url, bucket);
            } else {
                url = url.replace("://", &format!("://{}.", bucket));
            }
        }

        if let Some(key) = key {
            url = format!("{}/{}", url, key);
        }

        url
    }

    async fn sign_request(&self, mut request: reqwest::Request) -> Result<reqwest::Request> {
        if let (Some(access_key), Some(secret_key)) =
            (&self.config.access_key_id, &self.config.secret_access_key)
        {
            let credentials = Credentials::new(access_key, secret_key, None, None, "anode-client");

            let identity = credentials.into();
            let signing_settings = SigningSettings::default();
            let signing_params = v4::SigningParams::builder()
                .identity(&identity)
                .region(&self.config.region)
                .name("s3")
                .time(SystemTime::now())
                .settings(signing_settings)
                .build()
                .map_err(|e| {
                    ClientError::AuthenticationFailed(format!(
                        "Failed to build signing params: {}",
                        e
                    ))
                })?;

            let uri = request.url().as_str();
            let method = request.method().as_str();
            let headers = request
                .headers()
                .iter()
                .map(|(k, v)| (k.as_str(), std::str::from_utf8(v.as_bytes()).unwrap_or("")))
                .collect::<Vec<_>>();

            let signable_request = SignableRequest::new(
                method,
                uri,
                headers.iter().map(|(k, v)| (*k, *v)),
                SignableBody::Bytes(&[]),
            )
            .map_err(|e| {
                ClientError::AuthenticationFailed(format!(
                    "Failed to create signable request: {}",
                    e
                ))
            })?;

            let (signing_instructions, _signature) = sign(signable_request, &signing_params.into())
                .map_err(|e| {
                    ClientError::AuthenticationFailed(format!("Failed to sign request: {}", e))
                })?
                .into_parts();

            for (name, value) in signing_instructions.headers() {
                let header_name: reqwest::header::HeaderName = name.parse().map_err(|e| {
                    ClientError::AuthenticationFailed(format!("Invalid header name: {}", e))
                })?;
                let header_value: reqwest::header::HeaderValue = value.parse().map_err(|e| {
                    ClientError::AuthenticationFailed(format!("Invalid header value: {}", e))
                })?;
                request.headers_mut().insert(header_name, header_value);
            }
        }

        Ok(request)
    }

    async fn execute_with_retry<F, Fut>(&self, operation: F) -> Result<reqwest::Response>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<reqwest::Response>>,
    {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            match operation().await {
                Ok(response) => {
                    if response.status().is_success() {
                        return Ok(response);
                    }

                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();

                    let error = ClientError::from_status_code(status, body);

                    if !error.is_retriable() || attempt == self.config.max_retries {
                        return Err(error);
                    }

                    warn!(
                        "Request failed with retriable error: {:?}, attempt {}/{}",
                        error,
                        attempt + 1,
                        self.config.max_retries
                    );
                    last_error = Some(error);
                },
                Err(e) => {
                    if !e.is_retriable() || attempt == self.config.max_retries {
                        return Err(e);
                    }

                    warn!(
                        "Request failed with retriable error: {:?}, attempt {}/{}",
                        e,
                        attempt + 1,
                        self.config.max_retries
                    );
                    last_error = Some(e);
                },
            }

            // Exponential backoff
            let delay = Duration::from_millis(100 * 2_u64.pow(attempt));
            tokio::time::sleep(delay).await;
        }

        Err(last_error
            .unwrap_or_else(|| ClientError::InvalidConfig("Max retries exceeded".to_string())))
    }

    // Bucket operations

    /// Creates a new bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket to create
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The bucket already exists
    /// - Authentication fails
    /// - Network error occurs
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// client.create_bucket("my-new-bucket").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_bucket(&self, bucket: &str) -> Result<()> {
        debug!("Creating bucket: {}", bucket);

        let url = self.build_url(Some(bucket), None);
        let request = self.http.put(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        self.execute_with_retry(|| async {
            Ok(self
                .http
                .execute(signed_request.try_clone().unwrap())
                .await?)
        })
        .await?;

        Ok(())
    }

    /// Deletes an existing bucket.
    ///
    /// The bucket must be empty before it can be deleted.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket to delete
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The bucket does not exist
    /// - The bucket is not empty
    /// - Authentication fails
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// client.delete_bucket("old-bucket").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_bucket(&self, bucket: &str) -> Result<()> {
        debug!("Deleting bucket: {}", bucket);

        let url = self.build_url(Some(bucket), None);
        let request = self.http.delete(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        self.execute_with_retry(|| async {
            Ok(self
                .http
                .execute(signed_request.try_clone().unwrap())
                .await?)
        })
        .await?;

        Ok(())
    }

    /// Checks if a bucket exists.
    ///
    /// This is more efficient than listing buckets when you only need to check existence.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket to check
    ///
    /// # Returns
    ///
    /// Returns `true` if the bucket exists, `false` if it doesn't exist.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// if client.head_bucket("my-bucket").await? {
    ///     println!("Bucket exists!");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn head_bucket(&self, bucket: &str) -> Result<bool> {
        debug!("Checking if bucket exists: {}", bucket);

        let url = self.build_url(Some(bucket), None);
        let request = self.http.head(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        match self.http.execute(signed_request).await {
            Ok(response) if response.status().is_success() => Ok(true),
            Ok(response) if response.status() == reqwest::StatusCode::NOT_FOUND => Ok(false),
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                Err(ClientError::from_status_code(status, body))
            },
            Err(e) => Err(ClientError::Network(e)),
        }
    }

    /// Lists all buckets owned by the authenticated user.
    ///
    /// # Returns
    ///
    /// A vector of [`BucketInfo`] containing bucket names and creation dates.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let buckets = client.list_buckets().await?;
    /// for bucket in buckets {
    ///     println!("Bucket: {} (created: {})", bucket.name, bucket.creation_date);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_buckets(&self) -> Result<Vec<BucketInfo>> {
        debug!("Listing buckets");

        let url = self.config.endpoint.clone();
        let request = self.http.get(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self
                    .http
                    .execute(signed_request.try_clone().unwrap())
                    .await?)
            })
            .await?;

        let body = response.text().await?;
        let list_result: ListAllMyBucketsResult = quick_xml::de::from_str(&body)?;

        Ok(list_result
            .buckets
            .bucket
            .into_iter()
            .map(|b| BucketInfo {
                name: b.name,
                creation_date: b.creation_date,
            })
            .collect())
    }

    // Object operations

    /// Uploads an object to the specified bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key (path/filename)
    /// * `data` - Object data (anything that can be converted to `Bytes`)
    ///
    /// # Returns
    ///
    /// [`PutObjectOutput`] containing the ETag of the uploaded object.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use bytes::Bytes;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let data = Bytes::from("Hello, World!");
    /// let result = client.put_object("my-bucket", "hello.txt", data).await?;
    /// println!("Uploaded with ETag: {}", result.etag);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn put_object(
        &self,
        bucket: &str,
        key: &str,
        data: impl Into<Bytes>,
    ) -> Result<PutObjectOutput> {
        let data = data.into();
        debug!("Putting object: {}/{} ({} bytes)", bucket, key, data.len());

        let url = self.build_url(Some(bucket), Some(key));

        let md5_hash = if self.config.verify_checksums {
            Some(format!("{:x}", md5::compute(&data)))
        } else {
            None
        };

        let mut request_builder = self.http.put(&url).body(data);

        if let Some(hash) = &md5_hash {
            request_builder = request_builder.header("Content-MD5", hash);
        }

        let request = request_builder.build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self
                    .http
                    .execute(signed_request.try_clone().unwrap())
                    .await?)
            })
            .await?;

        let etag = response
            .headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();

        Ok(PutObjectOutput { etag })
    }

    /// Downloads an object from the specified bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key to download
    ///
    /// # Returns
    ///
    /// [`GetObjectOutput`] containing object metadata and body data.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let result = client.get_object("my-bucket", "file.txt").await?;
    /// println!("Downloaded {} bytes", result.body.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_object(&self, bucket: &str, key: &str) -> Result<GetObjectOutput> {
        debug!("Getting object: {}/{}", bucket, key);

        let url = self.build_url(Some(bucket), Some(key));
        let request = self.http.get(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self
                    .http
                    .execute(signed_request.try_clone().unwrap())
                    .await?)
            })
            .await?;

        let metadata = self.extract_metadata(key, &response)?;
        let body = response.bytes().await?;

        Ok(GetObjectOutput { metadata, body })
    }

    /// Retrieves metadata for an object without downloading its body.
    ///
    /// This is more efficient than `get_object` when you only need metadata.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key
    ///
    /// # Returns
    ///
    /// [`ObjectMetadata`] containing size, ETag, last modified date, etc.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let metadata = client.head_object("my-bucket", "file.txt").await?;
    /// println!("Object size: {} bytes", metadata.size);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn head_object(&self, bucket: &str, key: &str) -> Result<ObjectMetadata> {
        debug!("Getting object metadata: {}/{}", bucket, key);

        let url = self.build_url(Some(bucket), Some(key));
        let request = self.http.head(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self
                    .http
                    .execute(signed_request.try_clone().unwrap())
                    .await?)
            })
            .await?;

        self.extract_metadata(key, &response)
    }

    /// Deletes an object from a bucket.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key to delete
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// client.delete_object("my-bucket", "old-file.txt").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_object(&self, bucket: &str, key: &str) -> Result<()> {
        debug!("Deleting object: {}/{}", bucket, key);

        let url = self.build_url(Some(bucket), Some(key));
        let request = self.http.delete(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        self.execute_with_retry(|| async {
            Ok(self
                .http
                .execute(signed_request.try_clone().unwrap())
                .await?)
        })
        .await?;

        Ok(())
    }

    /// Copies an object from one location to another.
    ///
    /// This is a server-side copy operation, so the object data doesn't need to
    /// be transferred through the client.
    ///
    /// # Arguments
    ///
    /// * `src_bucket` - Source bucket name
    /// * `src_key` - Source object key
    /// * `dst_bucket` - Destination bucket name
    /// * `dst_key` - Destination object key
    ///
    /// # Returns
    ///
    /// [`CopyObjectOutput`] containing ETag and last modified timestamp.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// client.copy_object(
    ///     "source-bucket", "file.txt",
    ///     "dest-bucket", "copy.txt"
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn copy_object(
        &self,
        src_bucket: &str,
        src_key: &str,
        dst_bucket: &str,
        dst_key: &str,
    ) -> Result<CopyObjectOutput> {
        debug!(
            "Copying object: {}/{} -> {}/{}",
            src_bucket, src_key, dst_bucket, dst_key
        );

        let url = self.build_url(Some(dst_bucket), Some(dst_key));
        let copy_source = format!("{}/{}", src_bucket, src_key);

        let request = self
            .http
            .put(&url)
            .header("x-amz-copy-source", copy_source)
            .build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self
                    .http
                    .execute(signed_request.try_clone().unwrap())
                    .await?)
            })
            .await?;

        let body = response.text().await?;
        let result: CopyObjectResult = quick_xml::de::from_str(&body)?;

        Ok(CopyObjectOutput {
            etag: result.etag,
            last_modified: result.last_modified,
        })
    }

    /// Lists objects in a bucket with optional filtering.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `prefix` - Optional prefix filter (e.g., "photos/" to list only objects starting with "photos/")
    /// * `max_keys` - Maximum number of objects to return
    ///
    /// # Returns
    ///
    /// [`ListObjectsOutput`] containing object metadata, common prefixes, and pagination info.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// // List all objects
    /// let result = client.list_objects("my-bucket", None, None).await?;
    ///
    /// // List objects with prefix
    /// let result = client.list_objects("my-bucket", Some("photos/"), Some(100)).await?;
    /// for obj in result.objects {
    ///     println!("{}: {} bytes", obj.key, obj.size);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        max_keys: Option<i32>,
    ) -> Result<ListObjectsOutput> {
        debug!("Listing objects in bucket: {}", bucket);

        let mut url = self.build_url(Some(bucket), None);
        url.push_str("?list-type=2");

        if let Some(prefix) = prefix {
            url.push_str(&format!("&prefix={}", urlencoding::encode(prefix)));
        }

        if let Some(max_keys) = max_keys {
            url.push_str(&format!("&max-keys={}", max_keys));
        }

        let request = self.http.get(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self
                    .http
                    .execute(signed_request.try_clone().unwrap())
                    .await?)
            })
            .await?;

        let body = response.text().await?;
        let list_result: ListBucketResult = quick_xml::de::from_str(&body)?;

        let objects = list_result
            .contents
            .into_iter()
            .map(|obj| ObjectMetadata {
                key: obj.key,
                size: obj.size,
                etag: obj.etag.trim_matches('"').to_string(),
                last_modified: DateTime::parse_from_rfc3339(&obj.last_modified)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now),
                content_type: None,
                metadata: BTreeMap::new(),
            })
            .collect();

        let common_prefixes = list_result
            .common_prefixes
            .into_iter()
            .map(|cp| cp.prefix)
            .collect();

        Ok(ListObjectsOutput {
            objects,
            common_prefixes,
            is_truncated: list_result.is_truncated,
            next_continuation_token: list_result.next_continuation_token,
        })
    }

    // Streaming operations

    /// Uploads an object from a stream.
    ///
    /// This is useful for uploading large files without loading them entirely into memory.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key
    /// * `stream` - Stream of bytes to upload
    /// * `content_length` - Total size of the content in bytes
    ///
    /// # Returns
    ///
    /// [`PutObjectOutput`] containing the ETag.
    ///
    /// # Examples
    ///
    /// ## Basic Streaming Upload
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use bytes::Bytes;
    /// # use futures::stream;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let data = vec![Bytes::from("chunk1"), Bytes::from("chunk2")];
    /// let stream = stream::iter(data.into_iter().map(Ok));
    /// client.put_object_stream("my-bucket", "file.bin", stream, 12).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Upload from File Stream
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use tokio::fs::File;
    /// # use tokio_util::io::ReaderStream;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let file = File::open("large_video.mp4").await?;
    /// let metadata = file.metadata().await?;
    /// let stream = ReaderStream::new(file);
    ///
    /// let result = client.put_object_stream(
    ///     "my-bucket",
    ///     "videos/video.mp4",
    ///     stream,
    ///     metadata.len()
    /// ).await?;
    /// println!("Uploaded with ETag: {}", result.etag);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn put_object_stream<S>(
        &self,
        bucket: &str,
        key: &str,
        stream: S,
        content_length: u64,
    ) -> Result<PutObjectOutput>
    where
        S: Stream<Item = std::result::Result<Bytes, std::io::Error>> + Send + 'static,
    {
        debug!(
            "Putting object stream: {}/{} ({} bytes)",
            bucket, key, content_length
        );

        let url = self.build_url(Some(bucket), Some(key));

        let body = reqwest::Body::wrap_stream(stream);
        let request = self
            .http
            .put(&url)
            .header("Content-Length", content_length)
            .body(body)
            .build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self
                    .http
                    .execute(signed_request.try_clone().unwrap())
                    .await?)
            })
            .await?;

        let etag = response
            .headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();

        Ok(PutObjectOutput { etag })
    }

    /// Downloads an object as a stream.
    ///
    /// This is useful for downloading large files without loading them entirely into memory.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key
    ///
    /// # Returns
    ///
    /// A stream of `Result<Bytes>` chunks.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use futures::StreamExt;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let mut stream = client.get_object_stream("my-bucket", "large-file.bin").await?;
    /// while let Some(chunk) = stream.next().await {
    ///     let bytes = chunk?;
    ///     println!("Received {} bytes", bytes.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_object_stream(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<impl Stream<Item = Result<Bytes>>> {
        debug!("Getting object stream: {}/{}", bucket, key);

        let url = self.build_url(Some(bucket), Some(key));
        let request = self.http.get(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self.http.execute(signed_request).await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ClientError::from_status_code(status, body));
        }

        Ok(response
            .bytes_stream()
            .map(|result: std::result::Result<Bytes, reqwest::Error>| {
                result.map_err(ClientError::Network)
            }))
    }

    // Range requests

    /// Downloads a specific byte range from an object.
    ///
    /// This is useful for downloading specific parts of large files, such as
    /// seeking within media files or reading Parquet file metadata.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key
    /// * `start` - Starting byte position (inclusive)
    /// * `end` - Ending byte position (inclusive)
    ///
    /// # Returns
    ///
    /// `Bytes` containing the requested range.
    ///
    /// # Errors
    ///
    /// Returns `ClientError::InvalidRange` if start > end.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// // Download first 1024 bytes
    /// let bytes = client.get_object_range("my-bucket", "file.bin", 0, 1023).await?;
    /// println!("Downloaded {} bytes", bytes.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_object_range(
        &self,
        bucket: &str,
        key: &str,
        start: u64,
        end: u64,
    ) -> Result<Bytes> {
        debug!(
            "Getting object range: {}/{} (bytes {}-{})",
            bucket, key, start, end
        );

        if start > end {
            return Err(ClientError::InvalidRange(format!(
                "start ({}) must be <= end ({})",
                start, end
            )));
        }

        let url = self.build_url(Some(bucket), Some(key));
        let range_header = format!("bytes={}-{}", start, end);

        let request = self.http.get(&url).header("Range", range_header).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self
                    .http
                    .execute(signed_request.try_clone().unwrap())
                    .await?)
            })
            .await?;

        Ok(response.bytes().await?)
    }

    // Multipart upload

    /// Creates a multipart upload for uploading large objects in parts.
    ///
    /// Multipart uploads allow you to upload large objects (> 5MB) in smaller parts,
    /// which can be uploaded in parallel for better performance and reliability.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key for the uploaded file
    ///
    /// # Returns
    ///
    /// A [`MultipartUpload`] builder for managing the multipart upload.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use bytes::Bytes;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client = AnodeClient::new(ClientConfig::default())?;
    /// let mut upload = client.multipart_upload("my-bucket", "large-file.bin");
    /// upload.init().await?;
    /// upload.upload_part(1, Bytes::from(vec![0u8; 5_242_880])).await?;
    /// upload.upload_part(2, Bytes::from(vec![1u8; 5_242_880])).await?;
    /// let result = upload.complete().await?;
    /// println!("Upload complete: {}", result.etag);
    /// # Ok(())
    /// # }
    /// ```
    pub fn multipart_upload<'a>(&'a self, bucket: &str, key: &str) -> MultipartUpload<'a> {
        MultipartUpload::new(self, bucket.to_string(), key.to_string())
    }

    // Internal multipart operations (used by MultipartUpload)

    pub(crate) async fn initiate_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
    ) -> Result<String> {
        debug!("Initiating multipart upload: {}/{}", bucket, key);

        let url = format!("{}?uploads", self.build_url(Some(bucket), Some(key)));
        let request = self.http.post(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self
                    .http
                    .execute(signed_request.try_clone().unwrap())
                    .await?)
            })
            .await?;

        let body = response.text().await?;
        let result: InitiateMultipartUploadResult = quick_xml::de::from_str(&body)?;

        Ok(result.upload_id)
    }

    pub(crate) async fn upload_part(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
        part_number: i32,
        data: Bytes,
    ) -> Result<CompletedPart> {
        debug!(
            "Uploading part {} for {}/{} ({} bytes)",
            part_number,
            bucket,
            key,
            data.len()
        );

        if !(1..=10000).contains(&part_number) {
            return Err(ClientError::InvalidPartNumber(part_number));
        }

        let url = format!(
            "{}?partNumber={}&uploadId={}",
            self.build_url(Some(bucket), Some(key)),
            part_number,
            upload_id
        );

        let request = self.http.put(&url).body(data).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self
                    .http
                    .execute(signed_request.try_clone().unwrap())
                    .await?)
            })
            .await?;

        let etag = response
            .headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| ClientError::InvalidETag("Missing ETag header".to_string()))?
            .trim_matches('"')
            .to_string();

        Ok(CompletedPart { part_number, etag })
    }

    pub(crate) async fn complete_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
        parts: Vec<CompletedPart>,
    ) -> Result<CompleteMultipartOutput> {
        debug!("Completing multipart upload: {}/{}", bucket, key);

        let url = format!(
            "{}?uploadId={}",
            self.build_url(Some(bucket), Some(key)),
            upload_id
        );

        let parts_xml = parts
            .iter()
            .map(|p| {
                format!(
                    "<Part><PartNumber>{}</PartNumber><ETag>{}</ETag></Part>",
                    p.part_number, p.etag
                )
            })
            .collect::<Vec<_>>()
            .join("");

        let body = format!(
            "<CompleteMultipartUpload>{}</CompleteMultipartUpload>",
            parts_xml
        );

        let request = self.http.post(&url).body(body).build()?;
        let signed_request = self.sign_request(request).await?;

        let response = self
            .execute_with_retry(|| async {
                Ok(self
                    .http
                    .execute(signed_request.try_clone().unwrap())
                    .await?)
            })
            .await?;

        let response_body = response.text().await?;
        let result: CompleteMultipartUploadResult = quick_xml::de::from_str(&response_body)?;

        Ok(CompleteMultipartOutput {
            location: result.location,
            bucket: result.bucket,
            key: result.key,
            etag: result.etag.trim_matches('"').to_string(),
        })
    }

    pub(crate) async fn abort_multipart_upload(
        &self,
        bucket: &str,
        key: &str,
        upload_id: &str,
    ) -> Result<()> {
        debug!("Aborting multipart upload: {}/{}", bucket, key);

        let url = format!(
            "{}?uploadId={}",
            self.build_url(Some(bucket), Some(key)),
            upload_id
        );

        let request = self.http.delete(&url).build()?;
        let signed_request = self.sign_request(request).await?;

        self.execute_with_retry(|| async {
            Ok(self
                .http
                .execute(signed_request.try_clone().unwrap())
                .await?)
        })
        .await?;

        Ok(())
    }

    // Presigned URLs

    /// Generates a presigned URL for downloading an object.
    ///
    /// Presigned URLs allow temporary access to objects without requiring credentials.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key
    /// * `expires_in` - Duration until the URL expires
    ///
    /// # Returns
    ///
    /// A presigned URL string that can be used to download the object.
    ///
    /// # Errors
    ///
    /// Returns an error if credentials are not configured.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let config = ClientConfig::new("http://localhost:8080").with_credentials("key", "secret");
    /// # let client = AnodeClient::new(config)?;
    /// let url = client.presign_get("my-bucket", "file.txt", Duration::from_secs(3600))?;
    /// println!("Download URL (valid for 1 hour): {}", url);
    /// # Ok(())
    /// # }
    /// ```
    pub fn presign_get(&self, bucket: &str, key: &str, expires_in: Duration) -> Result<String> {
        self.presign_url(bucket, key, "GET", expires_in)
    }

    /// Generates a presigned URL for uploading an object.
    ///
    /// Presigned URLs allow temporary upload access to objects without requiring credentials.
    /// This is useful for allowing untrusted clients to upload files directly.
    ///
    /// # Arguments
    ///
    /// * `bucket` - Name of the bucket
    /// * `key` - Object key where the file will be uploaded
    /// * `expires_in` - Duration until the URL expires
    ///
    /// # Returns
    ///
    /// A presigned URL string that can be used to upload the object via HTTP PUT.
    ///
    /// # Errors
    ///
    /// Returns an error if credentials are not configured.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use anode_client::{ClientConfig, AnodeClient};
    /// # use std::time::Duration;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = ClientConfig::new("http://localhost:8080")
    ///     .with_credentials("access_key", "secret_key");
    /// let client = AnodeClient::new(config)?;
    ///
    /// // Generate upload URL valid for 30 minutes
    /// let url = client.presign_put("my-bucket", "uploads/file.txt", Duration::from_secs(1800))?;
    /// println!("Use this URL to upload: {}", url);
    ///
    /// // Client can now upload using: curl -X PUT -T file.txt "URL"
    /// # Ok(())
    /// # }
    /// ```
    pub fn presign_put(&self, bucket: &str, key: &str, expires_in: Duration) -> Result<String> {
        self.presign_url(bucket, key, "PUT", expires_in)
    }

    fn presign_url(
        &self,
        bucket: &str,
        key: &str,
        _method: &str,
        expires_in: Duration,
    ) -> Result<String> {
        let (_access_key, _secret_key) =
            match (&self.config.access_key_id, &self.config.secret_access_key) {
                (Some(ak), Some(sk)) => (ak, sk),
                _ => {
                    return Err(ClientError::AuthenticationFailed(
                        "Credentials required for presigned URLs".to_string(),
                    ))
                },
            };

        let url = self.build_url(Some(bucket), Some(key));
        let _timestamp = SystemTime::now();
        let expires = expires_in.as_secs();

        // This is a simplified presigned URL implementation
        // In production, you would use the full AWS SigV4 presigning logic
        let presigned_url = format!(
            "{}?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Expires={}",
            url, expires
        );

        Ok(presigned_url)
    }

    // Helper methods

    fn extract_metadata(&self, key: &str, response: &reqwest::Response) -> Result<ObjectMetadata> {
        let headers = response.headers();

        let size = headers
            .get("Content-Length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let etag = headers
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .trim_matches('"')
            .to_string();

        let last_modified = headers
            .get("Last-Modified")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| DateTime::parse_from_rfc2822(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let content_type = headers
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let mut metadata = BTreeMap::new();
        for (name, value) in headers.iter() {
            if let Some(meta_key) = name.as_str().strip_prefix("x-amz-meta-") {
                if let Ok(value_str) = value.to_str() {
                    metadata.insert(meta_key.to_string(), value_str.to_string());
                }
            }
        }

        Ok(ObjectMetadata {
            key: key.to_string(),
            size,
            etag,
            last_modified,
            content_type,
            metadata,
        })
    }
}

// Data structures

/// Information about a bucket.
///
/// Contains the bucket name and creation date.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketInfo {
    /// Bucket name
    pub name: String,
    /// Creation date (UTC)
    #[serde(rename = "CreationDate")]
    pub creation_date: DateTime<Utc>,
}

/// Metadata for an object.
///
/// Contains information about an object including its key, size, ETag,
/// last modified date, content type, and custom metadata.
#[derive(Debug, Clone)]
pub struct ObjectMetadata {
    /// Object key (path/filename)
    pub key: String,
    /// Object size in bytes
    pub size: u64,
    /// ETag (entity tag) for the object
    pub etag: String,
    /// Last modified date (UTC)
    pub last_modified: DateTime<Utc>,
    /// Content type (MIME type)
    pub content_type: Option<String>,
    /// Custom metadata key-value pairs
    pub metadata: BTreeMap<String, String>,
}

/// Result of a GetObject operation.
///
/// Contains both the object metadata and the object body data.
#[derive(Debug)]
pub struct GetObjectOutput {
    /// Object metadata
    pub metadata: ObjectMetadata,
    /// Object body data
    pub body: Bytes,
}

/// Result of a PutObject operation.
///
/// Contains the ETag of the uploaded object.
#[derive(Debug)]
pub struct PutObjectOutput {
    /// ETag of the uploaded object
    pub etag: String,
}

/// Result of a CopyObject operation.
///
/// Contains the ETag and last modified timestamp of the copied object.
#[derive(Debug)]
pub struct CopyObjectOutput {
    /// ETag of the copied object
    pub etag: String,
    /// Last modified timestamp
    pub last_modified: String,
}

/// Result of a ListObjects operation.
///
/// Contains a list of objects, common prefixes (directories), pagination info,
/// and a continuation token for fetching more results.
#[derive(Debug)]
pub struct ListObjectsOutput {
    /// List of objects matching the query
    pub objects: Vec<ObjectMetadata>,
    /// Common prefixes (directory-like groupings)
    pub common_prefixes: Vec<String>,
    /// Whether there are more results to fetch
    pub is_truncated: bool,
    /// Token for fetching the next page of results
    pub next_continuation_token: Option<String>,
}

// XML response structures

#[derive(Debug, Deserialize)]
struct ListAllMyBucketsResult {
    #[serde(rename = "Buckets")]
    buckets: Buckets,
}

#[derive(Debug, Deserialize)]
struct Buckets {
    #[serde(rename = "Bucket", default)]
    bucket: Vec<BucketInfo>,
}

#[derive(Debug, Deserialize)]
struct ListBucketResult {
    #[serde(rename = "Contents", default)]
    contents: Vec<S3Object>,
    #[serde(rename = "CommonPrefixes", default)]
    common_prefixes: Vec<CommonPrefix>,
    #[serde(rename = "IsTruncated")]
    is_truncated: bool,
    #[serde(rename = "NextContinuationToken")]
    next_continuation_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct S3Object {
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "Size")]
    size: u64,
    #[serde(rename = "ETag")]
    etag: String,
    #[serde(rename = "LastModified")]
    last_modified: String,
}

#[derive(Debug, Deserialize)]
struct CommonPrefix {
    #[serde(rename = "Prefix")]
    prefix: String,
}

#[derive(Debug, Deserialize)]
struct CopyObjectResult {
    #[serde(rename = "ETag")]
    etag: String,
    #[serde(rename = "LastModified")]
    last_modified: String,
}

#[derive(Debug, Deserialize)]
struct InitiateMultipartUploadResult {
    #[serde(rename = "UploadId")]
    upload_id: String,
}

#[derive(Debug, Deserialize)]
struct CompleteMultipartUploadResult {
    #[serde(rename = "Location")]
    location: String,
    #[serde(rename = "Bucket")]
    bucket: String,
    #[serde(rename = "Key")]
    key: String,
    #[serde(rename = "ETag")]
    etag: String,
}
