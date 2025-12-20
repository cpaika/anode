pub mod auth;
pub mod batch;
pub mod copy;
pub mod error;
pub mod handlers;
pub mod multipart;
pub mod router;
pub mod xml;

pub use auth::{AuthConfig, AwsSignatureV4};
pub use error::{S3Error, S3Result};
pub use router::S3Router;

// Re-export commonly used types
pub use handlers::{AppState, BucketMetadata};
pub use copy::{CopyHandler, CopyOptions, CopyResult, MetadataDirective};
pub use batch::{
    BatchHandler, DeleteError, DeleteObjectsResult, DeletedObject, ObjectIdentifier,
};
pub use multipart::{
    CompletePart, ListPartsResult, ListUploadsResult, MultipartConfig, MultipartManager,
    MultipartUpload, MultipartUploadInfo, UploadedPart,
};
