//! Error types for Parquet operations.

use thiserror::Error;

/// Errors that can occur during Parquet file operations.
#[derive(Debug, Error)]
pub enum ParquetError {
    /// The Parquet file format is invalid or corrupted.
    ///
    /// This includes missing magic bytes, invalid footer, or other format violations.
    #[error("Invalid parquet file: {0}")]
    InvalidFormat(String),

    /// Failed to parse Parquet metadata.
    ///
    /// This occurs when the metadata cannot be deserialized or is malformed.
    #[error("Failed to parse metadata: {0}")]
    ParseError(String),

    /// An I/O error occurred while reading or writing.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// An Arrow library error occurred.
    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    /// A Parquet library error occurred.
    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    /// A storage layer error occurred.
    #[error("Storage error: {0}")]
    Storage(#[from] anode_storage::StorageError),

    /// A cache operation error occurred.
    ///
    /// This includes failures during cache insertion, eviction, or retrieval.
    #[error("Cache error: {0}")]
    Cache(String),
}

/// A specialized Result type for Parquet operations.
///
/// This is a convenience type alias that uses [`ParquetError`] as the error type.
pub type Result<T> = std::result::Result<T, ParquetError>;
