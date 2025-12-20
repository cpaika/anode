// S3 error types and responses
// This module defines error types for S3 operations and converts them to S3-compatible XML responses

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

pub type S3Result<T> = Result<T, S3Error>;

#[derive(Error, Debug)]
pub enum S3Error {
    #[error("Access Denied")]
    AccessDenied,

    #[error("Bucket already exists: {0}")]
    BucketAlreadyExists(String),

    #[error("Bucket not empty: {0}")]
    BucketNotEmpty(String),

    #[error("No such bucket: {0}")]
    NoSuchBucket(String),

    #[error("No such key: {0}")]
    NoSuchKey(String),

    #[error("Invalid bucket name: {0}")]
    InvalidBucketName(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Method not allowed")]
    MethodNotAllowed,

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Missing content length")]
    MissingContentLength,

    #[error("Invalid range: {0}")]
    InvalidRange(String),

    #[error("Precondition failed")]
    PreconditionFailed,

    #[error("Invalid part")]
    InvalidPart,

    #[error("No such upload: {0}")]
    NoSuchUpload(String),

    #[error("Invalid part order")]
    InvalidPartOrder,

    #[error("Entity too small")]
    EntityTooSmall,

    #[error("Entity too large")]
    EntityTooLarge,

    #[error("Storage error: {0}")]
    StorageError(#[from] anode_storage::StorageError),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Signature does not match")]
    SignatureDoesNotMatch,
}

impl S3Error {
    /// Returns the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            S3Error::AccessDenied => StatusCode::FORBIDDEN,
            S3Error::BucketAlreadyExists(_) => StatusCode::CONFLICT,
            S3Error::BucketNotEmpty(_) => StatusCode::CONFLICT,
            S3Error::NoSuchBucket(_) => StatusCode::NOT_FOUND,
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
            S3Error::StorageError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            S3Error::AuthenticationFailed(_) => StatusCode::FORBIDDEN,
            S3Error::SignatureDoesNotMatch => StatusCode::FORBIDDEN,
        }
    }

    /// Returns the S3 error code string
    pub fn error_code(&self) -> &'static str {
        match self {
            S3Error::AccessDenied => "AccessDenied",
            S3Error::BucketAlreadyExists(_) => "BucketAlreadyExists",
            S3Error::BucketNotEmpty(_) => "BucketNotEmpty",
            S3Error::NoSuchBucket(_) => "NoSuchBucket",
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
            S3Error::StorageError(_) => "InternalError",
            S3Error::AuthenticationFailed(_) => "InvalidAccessKeyId",
            S3Error::SignatureDoesNotMatch => "SignatureDoesNotMatch",
        }
    }

    /// Converts the error to an S3-compatible XML response
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

impl IntoResponse for S3Error {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = self.to_xml_response();

        (
            status,
            [("Content-Type", "application/xml")],
            body,
        )
            .into_response()
    }
}

/// Escapes special XML characters
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
}
