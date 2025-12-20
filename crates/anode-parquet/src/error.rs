use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParquetError {
    #[error("Invalid parquet file: {0}")]
    InvalidFormat(String),

    #[error("Failed to parse metadata: {0}")]
    ParseError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    #[error("Storage error: {0}")]
    Storage(#[from] anode_storage::StorageError),

    #[error("Cache error: {0}")]
    Cache(String),
}

pub type Result<T> = std::result::Result<T, ParquetError>;
