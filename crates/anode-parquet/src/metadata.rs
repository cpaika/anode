//! Parquet file metadata extraction and serialization.
//!
//! This module provides types and functions for parsing Parquet file metadata
//! from raw bytes and converting it to/from various formats.

use crate::error::{ParquetError, Result};
use bytes::Bytes;
use parquet::file::metadata::{FileMetaData, ParquetMetaData};
use parquet::file::reader::{FileReader, SerializedFileReader};
use serde::{Deserialize, Serialize};

/// The magic bytes that identify a Parquet file ("PAR1").
const PARQUET_MAGIC: &[u8] = b"PAR1";
/// Length of the footer size field in bytes.
const FOOTER_SIZE_LENGTH: usize = 4;
/// Length of the magic bytes.
const MAGIC_LENGTH: usize = 4;

/// Complete metadata for a Parquet file.
///
/// This structure contains all the metadata needed to efficiently query
/// a Parquet file, including schema, row group information, and statistics.
///
/// # Example
///
/// ```no_run
/// use anode_parquet::ParquetMetadata;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let data = std::fs::read("example.parquet")?;
/// let metadata = ParquetMetadata::from_bytes(&data)?;
/// println!("Rows: {}, Row groups: {}", metadata.num_rows, metadata.row_groups.len());
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParquetMetadata {
    /// Total size of the Parquet file in bytes.
    pub file_size: u64,
    /// Size of the footer in bytes.
    pub footer_size: u64,
    /// Schema information for all columns.
    pub schema: SerializableSchema,
    /// Metadata for each row group.
    pub row_groups: Vec<RowGroupMetadata>,
    /// Total number of rows across all row groups.
    pub num_rows: i64,
    /// Optional creator/writer information.
    pub created_by: Option<String>,
    /// User-defined key-value metadata.
    pub key_value_metadata: Vec<(String, String)>,
}

/// Metadata for a single row group in a Parquet file.
///
/// Row groups are the horizontal partitioning unit in Parquet files.
/// Each row group contains a subset of rows and includes separate
/// chunks for each column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowGroupMetadata {
    /// Number of rows in this row group.
    pub num_rows: i64,
    /// Total byte size of all column chunks in this row group.
    pub total_byte_size: i64,
    /// Metadata for each column chunk in this row group.
    pub columns: Vec<ColumnChunkMetadata>,
    /// File offset where this row group starts.
    pub file_offset: i64,
}

/// Metadata for a column chunk within a row group.
///
/// Contains information about storage location, compression, and statistics
/// for a single column's data within a row group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnChunkMetadata {
    /// Column name/path in the schema.
    pub column_path: String,
    /// Optional external file path (usually None for embedded data).
    pub file_path: Option<String>,
    /// Byte offset of this column chunk in the file.
    pub file_offset: i64,
    /// Number of values in this column chunk.
    pub num_values: i64,
    /// Compression codec used (e.g., "SNAPPY", "GZIP").
    pub compression: String,
    /// Compressed size in bytes.
    pub compressed_size: i64,
    /// Uncompressed size in bytes.
    pub uncompressed_size: i64,
    /// Offset to the first data page.
    pub data_page_offset: i64,
    /// Optional offset to the dictionary page.
    pub dictionary_page_offset: Option<i64>,
    /// Optional column statistics for predicate pushdown.
    pub statistics: Option<ColumnStatistics>,
}

/// Statistics for a column chunk.
///
/// These statistics enable predicate pushdown optimization by allowing
/// row groups to be skipped based on min/max values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStatistics {
    /// Number of null values.
    pub null_count: Option<i64>,
    /// Number of distinct values (if available).
    pub distinct_count: Option<i64>,
    /// Minimum value as raw bytes.
    pub min_value: Option<Vec<u8>>,
    /// Maximum value as raw bytes.
    pub max_value: Option<Vec<u8>>,
}

/// A serializable representation of a Parquet schema.
///
/// This is a simplified schema representation that can be easily
/// serialized and cached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableSchema {
    /// List of field/column definitions.
    pub fields: Vec<FieldInfo>,
}

/// Information about a single field/column in the schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    /// Column name.
    pub name: String,
    /// Physical data type (e.g., "INT32", "BYTE_ARRAY").
    pub data_type: String,
    /// Whether the column allows null values.
    pub nullable: bool,
}

impl ParquetMetadata {
    /// Parses Parquet metadata from a complete file's raw bytes.
    ///
    /// This method validates the file format (magic bytes), reads the footer,
    /// and extracts all metadata including schema, row groups, and statistics.
    ///
    /// # Arguments
    ///
    /// * `data` - Complete Parquet file content as a byte slice
    ///
    /// # Returns
    ///
    /// * `Ok(ParquetMetadata)` if parsing succeeds
    /// * `Err(ParquetError)` if the file is invalid or parsing fails
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The file is missing PAR1 magic bytes at start or end
    /// - The file is too small to be valid
    /// - The footer cannot be parsed
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anode_parquet::ParquetMetadata;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let data = std::fs::read("example.parquet")?;
    /// let metadata = ParquetMetadata::from_bytes(&data)?;
    /// println!("Parquet file has {} rows", metadata.num_rows);
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        let file_size = data.len() as u64;

        // Verify magic number at start
        if data.len() < MAGIC_LENGTH || &data[0..MAGIC_LENGTH] != PARQUET_MAGIC {
            return Err(ParquetError::InvalidFormat(
                "Missing PAR1 magic at start".to_string(),
            ));
        }

        // Verify magic number at end
        if data.len() < MAGIC_LENGTH || &data[data.len() - MAGIC_LENGTH..] != PARQUET_MAGIC {
            return Err(ParquetError::InvalidFormat(
                "Missing PAR1 magic at end".to_string(),
            ));
        }

        // Read footer size from last 8 bytes (4 bytes footer size + 4 bytes magic)
        if data.len() < MAGIC_LENGTH + FOOTER_SIZE_LENGTH {
            return Err(ParquetError::InvalidFormat("File too small".to_string()));
        }

        let footer_size_offset = data.len() - MAGIC_LENGTH - FOOTER_SIZE_LENGTH;
        let footer_size_bytes = &data[footer_size_offset..footer_size_offset + FOOTER_SIZE_LENGTH];
        let footer_size = u32::from_le_bytes([
            footer_size_bytes[0],
            footer_size_bytes[1],
            footer_size_bytes[2],
            footer_size_bytes[3],
        ]) as u64;

        // Parse using SerializedFileReader
        // Convert data to Bytes since ChunkReader requires ownership
        let bytes = Bytes::copy_from_slice(data);
        let reader = SerializedFileReader::new(bytes)
            .map_err(|e| ParquetError::ParseError(format!("Failed to create reader: {}", e)))?;

        let metadata = reader.metadata().clone();
        Self::from_parquet_metadata(&metadata, file_size, footer_size)
    }

    /// Parses metadata from footer bytes only (optimization for large files).
    ///
    /// **Note**: This method is not yet implemented and will return an error.
    /// Use [`from_bytes`](Self::from_bytes) with the complete file instead.
    ///
    /// # Arguments
    ///
    /// * `_footer` - Footer bytes (currently unused)
    /// * `_file_size` - Total file size (currently unused)
    ///
    /// # Returns
    ///
    /// Currently always returns an error indicating the feature is not implemented.
    pub fn from_footer(_footer: &[u8], _file_size: u64) -> Result<Self> {
        // For footer-only parsing, we need to reconstruct a minimal valid parquet file
        // This is an optimization that would require custom deserialization
        // For now, we'll return an error and recommend using from_bytes with full file
        Err(ParquetError::ParseError(
            "Footer-only parsing not yet implemented, use from_bytes".to_string(),
        ))
    }

    /// Calculates the byte range needed to read a Parquet file's footer.
    ///
    /// For optimization, this returns the last 64KB of the file (or the entire file
    /// if smaller), which should contain the footer for most Parquet files.
    ///
    /// # Arguments
    ///
    /// * `file_size` - Total size of the Parquet file in bytes
    ///
    /// # Returns
    ///
    /// A tuple `(start, end)` representing the byte range to read
    ///
    /// # Example
    ///
    /// ```
    /// use anode_parquet::ParquetMetadata;
    ///
    /// let (start, end) = ParquetMetadata::footer_byte_range(1_000_000);
    /// // For a 1MB file, reads the last 64KB
    /// assert_eq!(end, 1_000_000);
    /// assert_eq!(start, 1_000_000 - 64 * 1024);
    /// ```
    pub fn footer_byte_range(file_size: u64) -> (u64, u64) {
        // Footer size is typically small (few KB), but can vary
        // We read last 64KB to be safe, or entire file if smaller
        let footer_read_size = std::cmp::min(file_size, 64 * 1024);
        let start = file_size.saturating_sub(footer_read_size);
        (start, file_size)
    }

    /// Estimates the memory size of this metadata structure in bytes.
    ///
    /// This is used by the cache to track memory usage and enforce size limits.
    /// The estimate includes the base structure size plus dynamically sized fields
    /// like schema, row groups, and key-value metadata.
    ///
    /// # Returns
    ///
    /// Estimated size in bytes
    pub fn estimated_size(&self) -> usize {
        let base_size = std::mem::size_of::<Self>();
        let schema_size = self.schema.fields.len() * std::mem::size_of::<FieldInfo>();
        let row_groups_size: usize = self
            .row_groups
            .iter()
            .map(|rg| {
                std::mem::size_of::<RowGroupMetadata>()
                    + rg.columns.len() * std::mem::size_of::<ColumnChunkMetadata>()
            })
            .sum();
        let kv_size: usize = self
            .key_value_metadata
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum();

        base_size + schema_size + row_groups_size + kv_size
    }

    /// Converts this metadata to protocol buffer format for RPC.
    ///
    /// This is used for transmitting metadata between nodes in the cluster.
    /// The schema is serialized as JSON.
    ///
    /// # Returns
    ///
    /// A protocol buffer representation of this metadata
    pub fn to_proto(&self) -> anode_protocol::common::ParquetMeta {
        use anode_protocol::common::{ColumnChunkInfo, ParquetMeta as ProtoMeta, RowGroupInfo};

        let row_groups = self
            .row_groups
            .iter()
            .map(|rg| RowGroupInfo {
                num_rows: rg.num_rows as u64,
                total_byte_size: rg.total_byte_size as u64,
                columns: rg
                    .columns
                    .iter()
                    .map(|col| ColumnChunkInfo {
                        column_path: col.column_path.clone(),
                        offset: col.file_offset as u64,
                        compressed_size: col.compressed_size as u64,
                        uncompressed_size: col.uncompressed_size as u64,
                    })
                    .collect(),
            })
            .collect();

        // Serialize schema as JSON for now (could use Arrow IPC format)
        let schema_bytes = serde_json::to_vec(&self.schema).unwrap_or_default();

        ProtoMeta {
            schema: schema_bytes,
            row_groups,
        }
    }

    /// Converts from protocol buffer format to ParquetMetadata.
    ///
    /// This is the inverse of [`to_proto`](Self::to_proto), used for receiving
    /// metadata from other nodes in the cluster.
    ///
    /// # Arguments
    ///
    /// * `proto` - Protocol buffer representation
    ///
    /// # Returns
    ///
    /// * `Ok(ParquetMetadata)` if conversion succeeds
    /// * `Err(ParquetError)` if the schema cannot be deserialized
    pub fn from_proto(proto: &anode_protocol::common::ParquetMeta) -> Result<Self> {
        let schema: SerializableSchema = serde_json::from_slice(&proto.schema)
            .map_err(|e| ParquetError::ParseError(format!("Failed to parse schema: {}", e)))?;

        let row_groups: Vec<RowGroupMetadata> = proto
            .row_groups
            .iter()
            .map(|rg| RowGroupMetadata {
                num_rows: rg.num_rows as i64,
                total_byte_size: rg.total_byte_size as i64,
                file_offset: 0, // Not stored in proto
                columns: rg
                    .columns
                    .iter()
                    .map(|col| ColumnChunkMetadata {
                        column_path: col.column_path.clone(),
                        file_path: None,
                        file_offset: col.offset as i64,
                        num_values: 0,
                        compression: "UNKNOWN".to_string(),
                        compressed_size: col.compressed_size as i64,
                        uncompressed_size: col.uncompressed_size as i64,
                        data_page_offset: 0,
                        dictionary_page_offset: None,
                        statistics: None,
                    })
                    .collect(),
            })
            .collect();

        let num_rows: i64 = row_groups.iter().map(|rg| rg.num_rows).sum();

        Ok(ParquetMetadata {
            file_size: 0,
            footer_size: 0,
            schema,
            row_groups,
            num_rows,
            created_by: None,
            key_value_metadata: Vec::new(),
        })
    }

    fn from_parquet_metadata(
        metadata: &ParquetMetaData,
        file_size: u64,
        footer_size: u64,
    ) -> Result<Self> {
        let file_metadata = metadata.file_metadata();

        // Extract schema
        let schema = Self::extract_schema(file_metadata)?;

        // Extract row groups
        let row_groups = metadata
            .row_groups()
            .iter()
            .map(|rg| {
                let columns = rg
                    .columns()
                    .iter()
                    .map(|col| {
                        let stats = col.statistics().map(|s| ColumnStatistics {
                            null_count: s.null_count_opt().map(|n| n as i64),
                            distinct_count: s.distinct_count_opt().map(|n| n as i64),
                            min_value: s.min_bytes_opt().map(|b| b.to_vec()),
                            max_value: s.max_bytes_opt().map(|b| b.to_vec()),
                        });

                        ColumnChunkMetadata {
                            column_path: col.column_path().string(),
                            file_path: col.file_path().map(|s| s.to_string()),
                            file_offset: col.file_offset(),
                            num_values: col.num_values(),
                            compression: format!("{:?}", col.compression()),
                            compressed_size: col.compressed_size(),
                            uncompressed_size: col.uncompressed_size(),
                            data_page_offset: col.data_page_offset(),
                            dictionary_page_offset: col.dictionary_page_offset(),
                            statistics: stats,
                        }
                    })
                    .collect();

                RowGroupMetadata {
                    num_rows: rg.num_rows(),
                    total_byte_size: rg.total_byte_size(),
                    file_offset: rg.file_offset().unwrap_or(0),
                    columns,
                }
            })
            .collect();

        // Extract key-value metadata
        let key_value_metadata = file_metadata
            .key_value_metadata()
            .map(|kv_vec| {
                kv_vec
                    .iter()
                    .map(|kv| (kv.key.clone(), kv.value.clone().unwrap_or_default()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(ParquetMetadata {
            file_size,
            footer_size,
            schema,
            row_groups,
            num_rows: file_metadata.num_rows(),
            created_by: file_metadata.created_by().map(|s| s.to_string()),
            key_value_metadata,
        })
    }

    fn extract_schema(file_metadata: &FileMetaData) -> Result<SerializableSchema> {
        let parquet_schema = file_metadata.schema_descr();
        let fields = parquet_schema
            .columns()
            .iter()
            .map(|col| {
                let name = col.name().to_string();
                let data_type = format!("{:?}", col.physical_type());
                // A field is nullable if max_def_level > 0
                // max_rep_level indicates repeated fields (arrays)
                let nullable = col.max_def_level() > 0;

                FieldInfo {
                    name,
                    data_type,
                    nullable,
                }
            })
            .collect();

        Ok(SerializableSchema { fields })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use parquet::file::properties::WriterProperties;
    use parquet::file::writer::SerializedFileWriter;
    use std::sync::Arc;

    fn create_test_parquet_data() -> Vec<u8> {
        use arrow::array::{ArrayRef, Int32Array, StringArray};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;

        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("name", DataType::Utf8, true),
        ]));

        let id_array = Int32Array::from(vec![1, 2, 3, 4, 5]);
        let name_array = StringArray::from(vec![
            Some("Alice"),
            Some("Bob"),
            None,
            Some("David"),
            Some("Eve"),
        ]);

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(id_array) as ArrayRef, Arc::new(name_array)],
        )
        .unwrap();

        let mut buffer = Vec::new();
        let mut writer = ArrowWriter::try_new(&mut buffer, schema, None).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();

        buffer
    }

    #[test]
    fn test_from_bytes_valid() {
        let data = create_test_parquet_data();
        let metadata = ParquetMetadata::from_bytes(&data).unwrap();

        assert_eq!(metadata.num_rows, 5);
        assert_eq!(metadata.row_groups.len(), 1);
        assert_eq!(metadata.schema.fields.len(), 2);
        assert!(metadata.file_size > 0);
    }

    #[test]
    fn test_from_bytes_invalid_magic() {
        let data = vec![0, 1, 2, 3];
        let result = ParquetMetadata::from_bytes(&data);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ParquetError::InvalidFormat(_)
        ));
    }

    #[test]
    fn test_from_bytes_too_small() {
        let data = vec![0, 1];
        let result = ParquetMetadata::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_footer_byte_range() {
        let (start, end) = ParquetMetadata::footer_byte_range(100);
        assert_eq!(start, 0);
        assert_eq!(end, 100);

        let (start, end) = ParquetMetadata::footer_byte_range(100_000);
        assert_eq!(start, 100_000 - 64 * 1024);
        assert_eq!(end, 100_000);
    }

    #[test]
    fn test_estimated_size() {
        let data = create_test_parquet_data();
        let metadata = ParquetMetadata::from_bytes(&data).unwrap();
        let size = metadata.estimated_size();
        assert!(size > 0);
        assert!(size < 10_000); // Should be reasonable size
    }

    #[test]
    fn test_to_proto_roundtrip() {
        let data = create_test_parquet_data();
        let metadata = ParquetMetadata::from_bytes(&data).unwrap();

        let proto = metadata.to_proto();
        assert_eq!(proto.row_groups.len(), 1);

        let restored = ParquetMetadata::from_proto(&proto).unwrap();
        assert_eq!(restored.num_rows, metadata.num_rows);
        assert_eq!(restored.row_groups.len(), metadata.row_groups.len());
    }

    #[test]
    fn test_schema_extraction() {
        let data = create_test_parquet_data();
        let metadata = ParquetMetadata::from_bytes(&data).unwrap();

        assert_eq!(metadata.schema.fields.len(), 2);
        assert_eq!(metadata.schema.fields[0].name, "id");
        assert!(!metadata.schema.fields[0].nullable);
        assert_eq!(metadata.schema.fields[1].name, "name");
        assert!(metadata.schema.fields[1].nullable);
    }

    #[test]
    fn test_row_group_metadata() {
        let data = create_test_parquet_data();
        let metadata = ParquetMetadata::from_bytes(&data).unwrap();

        let rg = &metadata.row_groups[0];
        assert_eq!(rg.num_rows, 5);
        assert!(rg.total_byte_size > 0);
        assert_eq!(rg.columns.len(), 2);
    }

    #[test]
    fn test_column_statistics() {
        let data = create_test_parquet_data();
        let metadata = ParquetMetadata::from_bytes(&data).unwrap();

        let col = &metadata.row_groups[0].columns[0];
        assert_eq!(col.column_path, "id");
        assert!(col.compressed_size > 0);
        assert!(col.uncompressed_size > 0);

        // Statistics may or may not be present depending on writer settings
        if let Some(stats) = &col.statistics {
            assert!(stats.null_count.is_some());
        }
    }

    #[test]
    fn test_multiple_row_groups() {
        use arrow::array::{ArrayRef, Int32Array};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;
        use parquet::file::properties::WriterProperties;

        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]));

        let mut buffer = Vec::new();
        let props = WriterProperties::builder()
            .set_max_row_group_size(2)
            .build();

        let mut writer = ArrowWriter::try_new(&mut buffer, schema.clone(), Some(props)).unwrap();

        for i in 0..3 {
            let id_array = Int32Array::from(vec![i * 2, i * 2 + 1]);
            let batch =
                RecordBatch::try_new(schema.clone(), vec![Arc::new(id_array) as ArrayRef]).unwrap();
            writer.write(&batch).unwrap();
        }

        writer.close().unwrap();

        let metadata = ParquetMetadata::from_bytes(&buffer).unwrap();
        assert_eq!(metadata.num_rows, 6);
        assert_eq!(metadata.row_groups.len(), 3);
        assert_eq!(metadata.row_groups[0].num_rows, 2);
    }

    #[test]
    fn test_footer_parsing() {
        // Create a sample parquet file
        let data = create_test_parquet_data();

        // Verify the file has correct magic numbers
        assert_eq!(&data[0..4], PARQUET_MAGIC, "Start magic should be PAR1");
        assert_eq!(
            &data[data.len() - 4..],
            PARQUET_MAGIC,
            "End magic should be PAR1"
        );

        // Extract footer size from the last 8 bytes (4 bytes footer size + 4 bytes magic)
        let footer_size_offset = data.len() - MAGIC_LENGTH - FOOTER_SIZE_LENGTH;
        let footer_size_bytes = &data[footer_size_offset..footer_size_offset + FOOTER_SIZE_LENGTH];
        let footer_size = u32::from_le_bytes([
            footer_size_bytes[0],
            footer_size_bytes[1],
            footer_size_bytes[2],
            footer_size_bytes[3],
        ]);

        // Footer size should be reasonable (not zero, not larger than file)
        assert!(footer_size > 0, "Footer size should be greater than 0");
        assert!(
            (footer_size as usize) < data.len(),
            "Footer size should be less than file size"
        );

        // Parse metadata and verify footer_size is captured correctly
        let metadata = ParquetMetadata::from_bytes(&data).unwrap();
        assert_eq!(
            metadata.footer_size, footer_size as u64,
            "Parsed footer size should match extracted footer size"
        );
        assert_eq!(
            metadata.file_size,
            data.len() as u64,
            "File size should match data length"
        );

        // Verify that metadata was successfully extracted from footer
        assert_eq!(metadata.num_rows, 5, "Should have 5 rows");
        assert_eq!(metadata.row_groups.len(), 1, "Should have 1 row group");
        assert_eq!(metadata.schema.fields.len(), 2, "Should have 2 fields");
    }

    #[test]
    fn test_footer_parsing_invalid_end_magic() {
        let mut data = create_test_parquet_data();

        // Corrupt the end magic
        let len = data.len();
        data[len - 1] = b'X';
        data[len - 2] = b'X';

        let result = ParquetMetadata::from_bytes(&data);
        assert!(result.is_err(), "Should fail with corrupted end magic");
        assert!(
            matches!(result.unwrap_err(), ParquetError::InvalidFormat(_)),
            "Should return InvalidFormat error"
        );
    }

    #[test]
    fn test_footer_parsing_invalid_start_magic() {
        let mut data = create_test_parquet_data();

        // Corrupt the start magic
        data[0] = b'X';
        data[1] = b'X';

        let result = ParquetMetadata::from_bytes(&data);
        assert!(result.is_err(), "Should fail with corrupted start magic");
        assert!(
            matches!(result.unwrap_err(), ParquetError::InvalidFormat(_)),
            "Should return InvalidFormat error"
        );
    }

    #[test]
    fn test_row_group_extraction() {
        use arrow::array::{ArrayRef, Int32Array, StringArray};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;
        use parquet::file::properties::WriterProperties;

        // Create a schema with multiple columns
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("name", DataType::Utf8, true),
            Field::new("age", DataType::Int32, true),
        ]));

        // Create a parquet file with 3 row groups (2 rows each)
        let mut buffer = Vec::new();
        let props = WriterProperties::builder()
            .set_max_row_group_size(2)
            .set_statistics_enabled(parquet::file::properties::EnabledStatistics::Page)
            .build();

        let mut writer = ArrowWriter::try_new(&mut buffer, schema.clone(), Some(props)).unwrap();

        // Write 3 batches (each will become a row group)
        for batch_idx in 0..3 {
            let id_start = batch_idx * 2;
            let id_array = Int32Array::from(vec![id_start, id_start + 1]);
            let name_array = StringArray::from(vec![
                Some(format!("Name{}", id_start)),
                Some(format!("Name{}", id_start + 1)),
            ]);
            let age_array = Int32Array::from(vec![Some(20 + id_start), Some(21 + id_start)]);

            let batch = RecordBatch::try_new(
                schema.clone(),
                vec![
                    Arc::new(id_array) as ArrayRef,
                    Arc::new(name_array),
                    Arc::new(age_array),
                ],
            )
            .unwrap();
            writer.write(&batch).unwrap();
        }

        writer.close().unwrap();

        // Parse metadata and extract row groups
        let metadata = ParquetMetadata::from_bytes(&buffer).unwrap();

        // Verify we have 3 row groups
        assert_eq!(metadata.row_groups.len(), 3, "Should have 3 row groups");

        // Verify total row count
        assert_eq!(metadata.num_rows, 6, "Should have 6 total rows");

        // Verify each row group
        for (idx, rg) in metadata.row_groups.iter().enumerate() {
            // Each row group should have 2 rows
            assert_eq!(rg.num_rows, 2, "Row group {} should have 2 rows", idx);

            // Each row group should have 3 columns
            assert_eq!(
                rg.columns.len(),
                3,
                "Row group {} should have 3 columns",
                idx
            );

            // Verify total byte size is positive
            assert!(
                rg.total_byte_size > 0,
                "Row group {} should have positive byte size",
                idx
            );

            // Verify file offset is set
            assert!(
                rg.file_offset >= 0,
                "Row group {} should have valid file offset",
                idx
            );

            // Verify column chunk metadata
            for (col_idx, col) in rg.columns.iter().enumerate() {
                // Verify column paths
                let expected_col_name = match col_idx {
                    0 => "id",
                    1 => "name",
                    2 => "age",
                    _ => panic!("Unexpected column index"),
                };
                assert_eq!(
                    col.column_path, expected_col_name,
                    "Column {} in row group {} should have correct path",
                    col_idx, idx
                );

                // Verify num_values
                assert_eq!(
                    col.num_values, 2,
                    "Column {} in row group {} should have 2 values",
                    col_idx, idx
                );

                // Verify sizes
                assert!(
                    col.compressed_size > 0,
                    "Column {} in row group {} should have positive compressed size",
                    col_idx,
                    idx
                );
                assert!(
                    col.uncompressed_size > 0,
                    "Column {} in row group {} should have positive uncompressed size",
                    col_idx,
                    idx
                );

                // Verify file offset is valid
                assert!(
                    col.file_offset >= 0,
                    "Column {} in row group {} should have valid file offset",
                    col_idx,
                    idx
                );

                // Verify data page offset
                assert!(
                    col.data_page_offset > 0,
                    "Column {} in row group {} should have positive data page offset",
                    col_idx,
                    idx
                );

                // Verify compression is set
                assert!(
                    !col.compression.is_empty(),
                    "Column {} in row group {} should have compression set",
                    col_idx,
                    idx
                );
            }
        }

        // Verify row groups are ordered by offset
        for i in 1..metadata.row_groups.len() {
            assert!(
                metadata.row_groups[i].file_offset >= metadata.row_groups[i - 1].file_offset,
                "Row groups should be ordered by file offset"
            );
        }

        // Verify we can extract specific row group information
        let first_rg = &metadata.row_groups[0];
        let last_rg = &metadata.row_groups[2];

        // First row group should come before last row group
        assert!(
            first_rg.file_offset < last_rg.file_offset,
            "First row group should have lower file offset than last"
        );

        // Calculate total size of all row groups
        let total_rg_size: i64 = metadata
            .row_groups
            .iter()
            .map(|rg| rg.total_byte_size)
            .sum();
        assert!(
            total_rg_size > 0,
            "Total size of all row groups should be positive"
        );

        // Verify sum of row counts equals total
        let sum_rows: i64 = metadata.row_groups.iter().map(|rg| rg.num_rows).sum();
        assert_eq!(
            sum_rows, metadata.num_rows,
            "Sum of row group row counts should equal total"
        );
    }

    #[test]
    fn test_row_group_extraction_single_large_group() {
        use arrow::array::{ArrayRef, Int32Array};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;
        use parquet::file::properties::WriterProperties;

        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int32,
            false,
        )]));

        let mut buffer = Vec::new();
        // Set a large row group size so everything goes into one group
        let props = WriterProperties::builder()
            .set_max_row_group_size(10000)
            .build();

        let mut writer = ArrowWriter::try_new(&mut buffer, schema.clone(), Some(props)).unwrap();

        // Write 100 rows in a single batch
        let values: Vec<i32> = (0..100).collect();
        let array = Int32Array::from(values);
        let batch =
            RecordBatch::try_new(schema.clone(), vec![Arc::new(array) as ArrayRef]).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();

        let metadata = ParquetMetadata::from_bytes(&buffer).unwrap();

        // Should have exactly 1 row group
        assert_eq!(metadata.row_groups.len(), 1);
        assert_eq!(metadata.row_groups[0].num_rows, 100);
        assert_eq!(metadata.row_groups[0].columns.len(), 1);

        let col = &metadata.row_groups[0].columns[0];
        assert_eq!(col.column_path, "value");
        assert_eq!(col.num_values, 100);
    }

    #[test]
    fn test_row_group_extraction_column_statistics() {
        use arrow::array::{ArrayRef, Int32Array};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;
        use parquet::file::properties::WriterProperties;

        let schema = Arc::new(Schema::new(vec![Field::new(
            "value",
            DataType::Int32,
            false,
        )]));

        let mut buffer = Vec::new();
        let props = WriterProperties::builder()
            .set_max_row_group_size(3)
            .set_statistics_enabled(parquet::file::properties::EnabledStatistics::Page)
            .build();

        let mut writer = ArrowWriter::try_new(&mut buffer, schema.clone(), Some(props)).unwrap();

        // Write two batches with different ranges
        let batch1 = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(Int32Array::from(vec![1, 2, 3])) as ArrayRef],
        )
        .unwrap();
        writer.write(&batch1).unwrap();

        let batch2 = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(Int32Array::from(vec![100, 200, 300])) as ArrayRef],
        )
        .unwrap();
        writer.write(&batch2).unwrap();

        writer.close().unwrap();

        let metadata = ParquetMetadata::from_bytes(&buffer).unwrap();

        // Should have 2 row groups
        assert_eq!(metadata.row_groups.len(), 2);

        // Check that statistics are present
        for (idx, rg) in metadata.row_groups.iter().enumerate() {
            let col = &rg.columns[0];
            if let Some(stats) = &col.statistics {
                assert!(
                    stats.null_count.is_some(),
                    "Row group {} should have null count statistics",
                    idx
                );
            }
            // Note: Statistics might not always be present depending on Parquet writer settings
        }
    }

    #[test]
    fn test_column_statistics_extraction() {
        use arrow::array::{ArrayRef, Int32Array, StringArray};
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;
        use parquet::file::properties::WriterProperties;

        // Create schema with int and string columns
        let schema = Arc::new(Schema::new(vec![
            Field::new("int_col", DataType::Int32, true),
            Field::new("string_col", DataType::Utf8, true),
        ]));

        let mut buffer = Vec::new();

        // Enable statistics at column level
        let props = WriterProperties::builder()
            .set_statistics_enabled(parquet::file::properties::EnabledStatistics::Chunk)
            .build();

        let mut writer = ArrowWriter::try_new(&mut buffer, schema.clone(), Some(props)).unwrap();

        // Create data with known min/max values and nulls
        // int_col: min=10, max=100, nulls=2
        let int_array = Int32Array::from(vec![
            Some(50),
            Some(10), // min
            None,
            Some(100), // max
            None,
            Some(75),
        ]);

        // string_col: min="apple", max="zebra", nulls=1
        let string_array = StringArray::from(vec![
            Some("banana"),
            Some("apple"), // min (lexicographically)
            Some("zebra"), // max (lexicographically)
            None,
            Some("cherry"),
            Some("mango"),
        ]);

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(int_array) as ArrayRef, Arc::new(string_array)],
        )
        .unwrap();

        writer.write(&batch).unwrap();
        writer.close().unwrap();

        // Parse metadata and extract statistics
        let metadata = ParquetMetadata::from_bytes(&buffer).unwrap();

        assert_eq!(metadata.row_groups.len(), 1);
        assert_eq!(metadata.num_rows, 6);

        let rg = &metadata.row_groups[0];
        assert_eq!(rg.columns.len(), 2);

        // Verify int_col statistics
        let int_col = &rg.columns[0];
        assert_eq!(int_col.column_path, "int_col");
        assert_eq!(int_col.num_values, 6);

        let int_stats = int_col
            .statistics
            .as_ref()
            .expect("int_col should have statistics");

        // Verify null count
        assert_eq!(int_stats.null_count, Some(2), "int_col should have 2 nulls");

        // Verify min/max values for int column
        // Int32 is stored in little-endian bytes
        if let Some(min_bytes) = &int_stats.min_value {
            let min_val =
                i32::from_le_bytes([min_bytes[0], min_bytes[1], min_bytes[2], min_bytes[3]]);
            assert_eq!(min_val, 10, "int_col min should be 10");
        } else {
            panic!("int_col should have min_value");
        }

        if let Some(max_bytes) = &int_stats.max_value {
            let max_val =
                i32::from_le_bytes([max_bytes[0], max_bytes[1], max_bytes[2], max_bytes[3]]);
            assert_eq!(max_val, 100, "int_col max should be 100");
        } else {
            panic!("int_col should have max_value");
        }

        // Verify string_col statistics
        let string_col = &rg.columns[1];
        assert_eq!(string_col.column_path, "string_col");

        let string_stats = string_col
            .statistics
            .as_ref()
            .expect("string_col should have statistics");

        // Verify null count
        assert_eq!(
            string_stats.null_count,
            Some(1),
            "string_col should have 1 null"
        );

        // Verify min/max values for string column
        if let Some(min_bytes) = &string_stats.min_value {
            let min_str = std::str::from_utf8(min_bytes).expect("min should be valid UTF-8");
            assert_eq!(min_str, "apple", "string_col min should be 'apple'");
        } else {
            panic!("string_col should have min_value");
        }

        if let Some(max_bytes) = &string_stats.max_value {
            let max_str = std::str::from_utf8(max_bytes).expect("max should be valid UTF-8");
            assert_eq!(max_str, "zebra", "string_col max should be 'zebra'");
        } else {
            panic!("string_col should have max_value");
        }

        // Verify distinct count if present (optional in parquet)
        // Some writers may not set this, so we just check if it exists
        if let Some(distinct) = int_stats.distinct_count {
            assert!(distinct > 0, "distinct count should be positive if present");
        }
    }
}
