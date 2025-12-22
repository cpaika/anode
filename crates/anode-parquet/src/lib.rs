//! Parquet file handling and optimization for anode.
//!
//! This crate provides utilities for working with Apache Parquet files in the anode
//! distributed object storage system. It includes:
//!
//! - **Metadata extraction**: Parse and cache Parquet file metadata
//! - **Query optimization**: Use Parquet statistics to optimize reads
//! - **Caching**: LRU cache for Parquet metadata to reduce I/O
//!
//! # Example
//!
//! ```no_run
//! use anode_parquet::{ParquetMetadata, ParquetCache};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Parse metadata from a Parquet file
//! let data = std::fs::read("example.parquet")?;
//! let metadata = ParquetMetadata::from_bytes(&data)?;
//! println!("Rows: {}, Row groups: {}", metadata.num_rows, metadata.row_groups.len());
//!
//! // Use a cache to avoid re-parsing
//! let cache = ParquetCache::new(10_000_000); // 10MB cache
//! cache.insert("example.parquet".to_string(), metadata);
//! # Ok(())
//! # }
//! ```

pub mod cache;
pub mod error;
pub mod metadata;
pub mod optimizer;

pub use cache::ParquetCache;
pub use error::{ParquetError, Result};
pub use metadata::ParquetMetadata;
