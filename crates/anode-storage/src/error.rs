use thiserror::Error;

pub type Result<T> = std::result::Result<T, StorageError>;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Corrupted data: {0}")]
    Corrupted(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid chunk ID: {0}")]
    InvalidChunkId(String),

    #[error("Invalid hex string: {0}")]
    InvalidHex(String),

    #[error("Chunk too large: {size} bytes (max: {max})")]
    ChunkTooLarge { size: usize, max: usize },

    #[error("Bucket not found: {0}")]
    BucketNotFound(String),

    #[error("Bucket already exists: {0}")]
    BucketAlreadyExists(String),

    #[error("Bucket not empty: {0}")]
    BucketNotEmpty(String),

    #[error("Object not found: {bucket}/{key}")]
    ObjectNotFound { bucket: String, key: String },

    #[error("Invalid object key: {0}")]
    InvalidObjectKey(String),

    #[error("Reference count error: {0}")]
    RefCountError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<redb::Error> for StorageError {
    fn from(err: redb::Error) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}

impl From<redb::DatabaseError> for StorageError {
    fn from(err: redb::DatabaseError) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}

impl From<redb::TransactionError> for StorageError {
    fn from(err: redb::TransactionError) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}

impl From<redb::TableError> for StorageError {
    fn from(err: redb::TableError) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}

impl From<redb::CommitError> for StorageError {
    fn from(err: redb::CommitError) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}

impl From<redb::StorageError> for StorageError {
    fn from(err: redb::StorageError) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(err: serde_json::Error) -> Self {
        StorageError::SerializationError(err.to_string())
    }
}

impl From<hex::FromHexError> for StorageError {
    fn from(err: hex::FromHexError) -> Self {
        StorageError::InvalidHex(err.to_string())
    }
}
