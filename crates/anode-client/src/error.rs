/// Error types for the Anode client.
///
/// All operations in the client can fail with a [`ClientError`]. This enum covers
/// network errors, S3-specific errors, authentication failures, and validation errors.
///
/// # Example
///
/// ```no_run
/// use anode_client::{ClientConfig, AnodeClient, ClientError};
///
/// # #[tokio::main]
/// # async fn main() {
/// let config = ClientConfig::new("http://localhost:8080")
///     .with_credentials("key", "secret");
/// let client = AnodeClient::new(config).unwrap();
///
/// match client.get_object("my-bucket", "missing.txt").await {
///     Ok(result) => println!("Got object: {} bytes", result.body.len()),
///     Err(ClientError::ObjectNotFound { bucket, key }) => {
///         println!("Object {}/{} not found", bucket, key);
///     }
///     Err(e) => println!("Other error: {}", e),
/// }
/// # }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("S3 error: {code} - {message}")]
    S3Error { code: String, message: String },

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Bucket not found: {0}")]
    BucketNotFound(String),

    #[error("Object not found: {bucket}/{key}")]
    ObjectNotFound { bucket: String, key: String },

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Timeout: operation exceeded {0:?}")]
    Timeout(std::time::Duration),

    #[error("XML parsing error: {0}")]
    XmlError(#[from] quick_xml::DeError),

    #[error("URL parsing error: {0}")]
    UrlError(#[from] url::ParseError),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid HTTP header: {0}")]
    InvalidHeader(#[from] http::header::InvalidHeaderValue),

    #[error("Invalid HTTP status: {0}")]
    InvalidStatus(http::StatusCode),

    #[error("Bucket already exists: {0}")]
    BucketAlreadyExists(String),

    #[error("Bucket not empty: {0}")]
    BucketNotEmpty(String),

    #[error("Invalid range: {0}")]
    InvalidRange(String),

    #[error("Part number must be between 1 and 10000, got {0}")]
    InvalidPartNumber(i32),

    #[error("Multipart upload not initialized")]
    MultipartNotInitialized,

    #[error("Multipart upload already completed")]
    MultipartAlreadyCompleted,

    #[error("Invalid ETag format: {0}")]
    InvalidETag(String),

    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    #[error("Invalid parameter value: {parameter} = {value}")]
    InvalidParameter { parameter: String, value: String },
}

/// A type alias for `Result<T, ClientError>`.
///
/// This is used throughout the client API for convenience.
pub type Result<T> = std::result::Result<T, ClientError>;

impl ClientError {
    /// Returns `true` if this error represents a "not found" condition.
    ///
    /// This includes both bucket not found and object not found errors.
    ///
    /// # Example
    ///
    /// ```
    /// use anode_client::ClientError;
    ///
    /// let err = ClientError::BucketNotFound("my-bucket".to_string());
    /// assert!(err.is_not_found());
    ///
    /// let err = ClientError::ObjectNotFound {
    ///     bucket: "my-bucket".to_string(),
    ///     key: "my-key".to_string(),
    /// };
    /// assert!(err.is_not_found());
    /// ```
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            ClientError::BucketNotFound(_) | ClientError::ObjectNotFound { .. }
        )
    }

    /// Returns `true` if this error is retriable.
    ///
    /// Retriable errors include network errors, timeouts, and server errors (5xx).
    /// The client automatically retries these errors up to `max_retries` times.
    ///
    /// # Example
    ///
    /// ```
    /// use anode_client::ClientError;
    ///
    /// let err = ClientError::Timeout(std::time::Duration::from_secs(30));
    /// assert!(err.is_retriable());
    ///
    /// let err = ClientError::BucketNotFound("my-bucket".to_string());
    /// assert!(!err.is_retriable());
    /// ```
    pub fn is_retriable(&self) -> bool {
        match self {
            ClientError::Network(_) | ClientError::Timeout(_) => true,
            ClientError::InvalidStatus(status) if status.is_server_error() => true,
            _ => false,
        }
    }

    /// Creates a `ClientError` from an HTTP status code and response body.
    ///
    /// This is used internally to convert HTTP errors into typed client errors.
    ///
    /// # Arguments
    ///
    /// * `status` - HTTP status code
    /// * `body` - Response body (may contain XML error details)
    pub fn from_status_code(status: http::StatusCode, body: String) -> Self {
        match status {
            http::StatusCode::NOT_FOUND => {
                if body.contains("NoSuchBucket") {
                    ClientError::BucketNotFound(extract_bucket_name(&body))
                } else {
                    ClientError::S3Error {
                        code: "NoSuchKey".to_string(),
                        message: body,
                    }
                }
            },
            http::StatusCode::CONFLICT => {
                if body.contains("BucketAlreadyExists") {
                    ClientError::BucketAlreadyExists(extract_bucket_name(&body))
                } else {
                    ClientError::S3Error {
                        code: "Conflict".to_string(),
                        message: body,
                    }
                }
            },
            http::StatusCode::FORBIDDEN => ClientError::AuthenticationFailed(body),
            _ => ClientError::InvalidStatus(status),
        }
    }
}

fn extract_bucket_name(body: &str) -> String {
    // Simple extraction from XML error response
    body.split("<BucketName>")
        .nth(1)
        .and_then(|s| s.split("</BucketName>").next())
        .unwrap_or("unknown")
        .to_string()
}
