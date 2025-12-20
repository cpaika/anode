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

pub type Result<T> = std::result::Result<T, ClientError>;

impl ClientError {
    pub fn is_not_found(&self) -> bool {
        matches!(
            self,
            ClientError::BucketNotFound(_) | ClientError::ObjectNotFound { .. }
        )
    }

    pub fn is_retriable(&self) -> bool {
        match self {
            ClientError::Network(_) | ClientError::Timeout(_) => true,
            ClientError::InvalidStatus(status) if status.is_server_error() => true,
            _ => false,
        }
    }

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
            }
            http::StatusCode::CONFLICT => {
                if body.contains("BucketAlreadyExists") {
                    ClientError::BucketAlreadyExists(extract_bucket_name(&body))
                } else {
                    ClientError::S3Error {
                        code: "Conflict".to_string(),
                        message: body,
                    }
                }
            }
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
