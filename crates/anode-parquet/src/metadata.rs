use crate::error::{ParquetError, Result};
use arrow::datatypes::{DataType, Field, Schema};
use parquet::file::metadata::{FileMetaData, ParquetMetaData};
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::schema::types::Type;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::Arc;

const PARQUET_MAGIC: &[u8] = b"PAR1";
const FOOTER_SIZE_LENGTH: usize = 4;
const MAGIC_LENGTH: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParquetMetadata {
    pub file_size: u64,
    pub footer_size: u64,
    pub schema: SerializableSchema,
    pub row_groups: Vec<RowGroupMetadata>,
    pub num_rows: i64,
    pub created_by: Option<String>,
    pub key_value_metadata: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowGroupMetadata {
    pub num_rows: i64,
    pub total_byte_size: i64,
    pub columns: Vec<ColumnChunkMetadata>,
    pub file_offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnChunkMetadata {
    pub column_path: String,
    pub file_path: Option<String>,
    pub file_offset: i64,
    pub num_values: i64,
    pub compression: String,
    pub compressed_size: i64,
    pub uncompressed_size: i64,
    pub data_page_offset: i64,
    pub dictionary_page_offset: Option<i64>,
    pub statistics: Option<ColumnStatistics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnStatistics {
    pub null_count: Option<i64>,
    pub distinct_count: Option<i64>,
    pub min_value: Option<Vec<u8>>,
    pub max_value: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableSchema {
    pub fields: Vec<FieldInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

impl ParquetMetadata {
    /// Parse metadata from raw parquet bytes (reads footer only)
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
        let cursor = Cursor::new(data);
        let reader = SerializedFileReader::new(cursor)
            .map_err(|e| ParquetError::ParseError(format!("Failed to create reader: {}", e)))?;

        Self::from_parquet_metadata(reader.metadata(), file_size, footer_size)
    }

    /// Parse metadata from file footer bytes only (optimization for large files)
    pub fn from_footer(footer: &[u8], file_size: u64) -> Result<Self> {
        // For footer-only parsing, we need to reconstruct a minimal valid parquet file
        // This is an optimization that would require custom deserialization
        // For now, we'll return an error and recommend using from_bytes with full file
        Err(ParquetError::ParseError(
            "Footer-only parsing not yet implemented, use from_bytes".to_string(),
        ))
    }

    /// Get byte range for parquet footer
    pub fn footer_byte_range(file_size: u64) -> (u64, u64) {
        // Footer size is typically small (few KB), but can vary
        // We read last 64KB to be safe, or entire file if smaller
        let footer_read_size = std::cmp::min(file_size, 64 * 1024);
        let start = file_size.saturating_sub(footer_read_size);
        (start, file_size)
    }

    /// Estimate memory size of cached metadata
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

    /// Convert to protocol buffer format
    pub fn to_proto(&self) -> anode_protocol::common::ParquetMetadata {
        use anode_protocol::common::{ColumnChunkInfo, ParquetMetadata as ProtoMeta, RowGroupInfo};

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

    /// Convert from protocol buffer format
    pub fn from_proto(proto: &anode_protocol::common::ParquetMetadata) -> Result<Self> {
        let schema: SerializableSchema = serde_json::from_slice(&proto.schema)
            .map_err(|e| ParquetError::ParseError(format!("Failed to parse schema: {}", e)))?;

        let row_groups = proto
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
                            null_count: s.null_count_opt(),
                            distinct_count: s.distinct_count_opt(),
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
                let nullable = col.max_rep_level() > 0;

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
        assert!(matches!(result.unwrap_err(), ParquetError::InvalidFormat(_)));
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
        use arrow::record_batch::RecordBatch;
        use parquet::arrow::ArrowWriter;
        use parquet::file::properties::WriterProperties;

        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]));

        let mut buffer = Vec::new();
        let props = WriterProperties::builder()
            .set_max_row_group_size(2)
            .build();

        let mut writer =
            ArrowWriter::try_new(&mut buffer, schema.clone(), Some(props)).unwrap();

        for i in 0..3 {
            let id_array = Int32Array::from(vec![i * 2, i * 2 + 1]);
            let batch =
                RecordBatch::try_new(schema.clone(), vec![Arc::new(id_array) as ArrayRef])
                    .unwrap();
            writer.write(&batch).unwrap();
        }

        writer.close().unwrap();

        let metadata = ParquetMetadata::from_bytes(&buffer).unwrap();
        assert_eq!(metadata.num_rows, 6);
        assert_eq!(metadata.row_groups.len(), 3);
        assert_eq!(metadata.row_groups[0].num_rows, 2);
    }
}
