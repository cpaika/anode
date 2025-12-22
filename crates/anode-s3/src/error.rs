//! S3 error types and responses.
//!
//! This module defines error types for S3 operations and converts them to S3-compatible XML responses.
//! Each error type maps to a specific HTTP status code and S3 error code.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

/// Result type for S3 operations.
pub type S3Result<T> = Result<T, S3Error>;

/// S3-compatible error types.
///
/// These errors map to standard S3 error codes and HTTP status codes.
#[derive(Error, Debug)]
pub enum S3Error {
    /// Access to the resource is denied.

    #[error("Access Denied")]
    AccessDenied,

    /// Bucket already exists with the given name.
    #[error("Bucket already exists: {0}")]
    BucketAlreadyExists(String),

    /// Bucket is not empty and cannot be deleted.
    #[error("Bucket not empty: {0}")]
    BucketNotEmpty(String),

    /// The specified bucket does not exist.
    #[error("No such bucket: {0}")]
    NoSuchBucket(String),

    /// The specified bucket does not have a bucket policy.
    #[error("No such bucket policy: {0}")]
    NoSuchBucketPolicy(String),

    /// The specified bucket does not have a lifecycle configuration.
    #[error("No such lifecycle configuration: {0}")]
    NoSuchLifecycleConfiguration(String),

    /// The specified object key does not exist.
    #[error("No such key: {0}")]
    NoSuchKey(String),

    /// Invalid bucket name according to S3 naming rules.
    #[error("Invalid bucket name: {0}")]
    InvalidBucketName(String),

    /// Invalid argument provided in the request.
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Internal server error.
    #[error("Internal error: {0}")]
    InternalError(String),

    /// HTTP method is not allowed for this resource.
    #[error("Method not allowed")]
    MethodNotAllowed,

    /// The request is malformed or invalid.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Content-Length header is missing.
    #[error("Missing content length")]
    MissingContentLength,

    /// Invalid byte range specified.
    #[error("Invalid range: {0}")]
    InvalidRange(String),

    /// Precondition evaluation failed (e.g., If-Match, If-None-Match).
    #[error("Precondition failed")]
    PreconditionFailed,

    /// Invalid part specified in multipart upload.
    #[error("Invalid part")]
    InvalidPart,

    /// The specified multipart upload does not exist.
    #[error("No such upload: {0}")]
    NoSuchUpload(String),

    /// Parts must be uploaded in ascending order.
    #[error("Invalid part order")]
    InvalidPartOrder,

    /// Part size is too small (minimum 5MB except for last part).
    #[error("Entity too small")]
    EntityTooSmall,

    /// Part size exceeds the maximum allowed size (5GB).
    #[error("Entity too large")]
    EntityTooLarge,

    /// Quota exceeded for the bucket.
    #[error("Quota exceeded")]
    QuotaExceeded,

    /// Error from the underlying storage engine.
    #[error("Storage error: {0}")]
    StorageError(anode_storage::StorageError),

    /// Authentication failed due to invalid credentials.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Request signature does not match the calculated signature.
    #[error("Signature does not match")]
    SignatureDoesNotMatch,
}

impl S3Error {
    /// Returns the HTTP status code for this error.
    ///
    /// Maps each error variant to its corresponding HTTP status code.
    pub fn status_code(&self) -> StatusCode {
        match self {
            S3Error::AccessDenied => StatusCode::FORBIDDEN,
            S3Error::BucketAlreadyExists(_) => StatusCode::CONFLICT,
            S3Error::BucketNotEmpty(_) => StatusCode::CONFLICT,
            S3Error::NoSuchBucket(_) => StatusCode::NOT_FOUND,
            S3Error::NoSuchBucketPolicy(_) => StatusCode::NOT_FOUND,
            S3Error::NoSuchLifecycleConfiguration(_) => StatusCode::NOT_FOUND,
            S3Error::NoSuchKey(_) => StatusCode::NOT_FOUND,
            S3Error::InvalidBucketName(_) => StatusCode::BAD_REQUEST,
            S3Error::InvalidArgument(_) => StatusCode::BAD_REQUEST,
            S3Error::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            S3Error::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            S3Error::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            S3Error::MissingContentLength => StatusCode::LENGTH_REQUIRED,
            S3Error::InvalidRange(_) => StatusCode::RANGE_NOT_SATISFIABLE,
            S3Error::PreconditionFailed => StatusCode::PRECONDITION_FAILED,
            S3Error::InvalidPart => StatusCode::BAD_REQUEST,
            S3Error::NoSuchUpload(_) => StatusCode::NOT_FOUND,
            S3Error::InvalidPartOrder => StatusCode::BAD_REQUEST,
            S3Error::EntityTooSmall => StatusCode::BAD_REQUEST,
            S3Error::EntityTooLarge => StatusCode::BAD_REQUEST,
            S3Error::QuotaExceeded => StatusCode::FORBIDDEN,
            S3Error::StorageError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            S3Error::AuthenticationFailed(_) => StatusCode::FORBIDDEN,
            S3Error::SignatureDoesNotMatch => StatusCode::FORBIDDEN,
        }
    }

    /// Returns the S3 error code string.
    ///
    /// Returns the standard S3 error code that corresponds to this error.
    pub fn error_code(&self) -> &'static str {
        match self {
            S3Error::AccessDenied => "AccessDenied",
            S3Error::BucketAlreadyExists(_) => "BucketAlreadyExists",
            S3Error::BucketNotEmpty(_) => "BucketNotEmpty",
            S3Error::NoSuchBucket(_) => "NoSuchBucket",
            S3Error::NoSuchBucketPolicy(_) => "NoSuchBucketPolicy",
            S3Error::NoSuchLifecycleConfiguration(_) => "NoSuchLifecycleConfiguration",
            S3Error::NoSuchKey(_) => "NoSuchKey",
            S3Error::InvalidBucketName(_) => "InvalidBucketName",
            S3Error::InvalidArgument(_) => "InvalidArgument",
            S3Error::InternalError(_) => "InternalError",
            S3Error::MethodNotAllowed => "MethodNotAllowed",
            S3Error::InvalidRequest(_) => "InvalidRequest",
            S3Error::MissingContentLength => "MissingContentLength",
            S3Error::InvalidRange(_) => "InvalidRange",
            S3Error::PreconditionFailed => "PreconditionFailed",
            S3Error::InvalidPart => "InvalidPart",
            S3Error::NoSuchUpload(_) => "NoSuchUpload",
            S3Error::InvalidPartOrder => "InvalidPartOrder",
            S3Error::EntityTooSmall => "EntityTooSmall",
            S3Error::EntityTooLarge => "EntityTooLarge",
            S3Error::QuotaExceeded => "QuotaExceeded",
            S3Error::StorageError(_) => "InternalError",
            S3Error::AuthenticationFailed(_) => "InvalidAccessKeyId",
            S3Error::SignatureDoesNotMatch => "SignatureDoesNotMatch",
        }
    }

    /// Converts the error to an S3-compatible XML response.
    ///
    /// Generates an XML error response in the format expected by S3 clients.
    pub fn to_xml_response(&self) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Error>
    <Code>{}</Code>
    <Message>{}</Message>
</Error>"#,
            self.error_code(),
            xml_escape(&self.to_string())
        )
    }
}

impl From<anode_storage::StorageError> for S3Error {
    fn from(err: anode_storage::StorageError) -> Self {
        match err {
            anode_storage::StorageError::QuotaExceeded { .. } => S3Error::QuotaExceeded,
            _ => S3Error::StorageError(err),
        }
    }
}

impl IntoResponse for S3Error {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = self.to_xml_response();

        (status, [("Content-Type", "application/xml")], body).into_response()
    }
}

/// Escapes special XML characters.
///
/// Replaces XML special characters (&, <, >, ", ') with their entity equivalents.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(S3Error::AccessDenied.error_code(), "AccessDenied");
        assert_eq!(
            S3Error::NoSuchBucket("test".to_string()).error_code(),
            "NoSuchBucket"
        );
        assert_eq!(
            S3Error::NoSuchKey("test.txt".to_string()).error_code(),
            "NoSuchKey"
        );
        assert_eq!(S3Error::QuotaExceeded.error_code(), "QuotaExceeded");
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(S3Error::AccessDenied.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(
            S3Error::NoSuchBucket("test".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            S3Error::InvalidBucketName("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(S3Error::QuotaExceeded.status_code(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_xml_escape() {
        let result = xml_escape("test & <tag> \"quotes\" 'single'");
        assert_eq!(
            result,
            "test &amp; &lt;tag&gt; &quot;quotes&quot; &apos;single&apos;"
        );
    }

    #[test]
    fn test_to_xml_response() {
        let error = S3Error::NoSuchBucket("my-bucket".to_string());
        let xml = error.to_xml_response();
        assert!(xml.contains("<Code>NoSuchBucket</Code>"));
        assert!(xml.contains("<Message>No such bucket: my-bucket</Message>"));
    }

    #[test]
    fn test_quota_exceeded_xml_response() {
        let error = S3Error::QuotaExceeded;
        let xml = error.to_xml_response();
        assert!(xml.contains("<Code>QuotaExceeded</Code>"));
        assert!(xml.contains("<Message>Quota exceeded</Message>"));
    }

    #[test]
    fn test_storage_quota_exceeded_conversion() {
        let storage_error = anode_storage::StorageError::QuotaExceeded {
            bucket: "test-bucket".to_string(),
            quota: 1000,
            current_usage: 900,
            requested: 200,
        };
        let s3_error: S3Error = storage_error.into();
        assert!(matches!(s3_error, S3Error::QuotaExceeded));
        assert_eq!(s3_error.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(s3_error.error_code(), "QuotaExceeded");
    }
}
