//! Query optimization for Parquet file reads.
//!
//! This module provides utilities for optimizing Parquet file access by:
//! - Using column statistics for predicate pushdown
//! - Calculating optimal byte ranges for column projection
//! - Filtering row groups based on predicates
//! - Merging byte ranges to minimize I/O operations

use crate::metadata::{ColumnStatistics, ParquetMetadata, RowGroupMetadata};

/// Optimization plan for reading a Parquet file.
///
/// This structure describes how to efficiently read a Parquet file based on
/// query requirements (column projection, predicates, etc.).
#[derive(Debug, Clone)]
pub struct ReadOptimization {
    /// Indices of row groups to read (0-based).
    pub row_groups: Vec<usize>,
    /// List of column names to project.
    pub columns: Vec<String>,
    /// Byte ranges to fetch from the file as (start, end) pairs.
    pub byte_ranges: Vec<(u64, u64)>,
    /// Estimated total bytes to read.
    pub estimated_bytes: u64,
}

/// Optimizer for Parquet file access patterns.
///
/// Provides static methods for analyzing Parquet metadata and generating
/// optimized read plans.
pub struct ParquetOptimizer;

impl ParquetOptimizer {
    /// Optimizes column projection by calculating byte ranges to read.
    ///
    /// This method analyzes the Parquet metadata to determine exactly which
    /// byte ranges need to be read from the file to retrieve the requested
    /// columns across all row groups. It also merges adjacent ranges to
    /// minimize the number of I/O operations.
    ///
    /// # Arguments
    ///
    /// * `metadata` - The Parquet file metadata
    /// * `columns` - List of column names to read
    ///
    /// # Returns
    ///
    /// A [`ReadOptimization`] plan with byte ranges and estimated size
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anode_parquet::{ParquetMetadata, optimizer::ParquetOptimizer};
    ///
    /// # fn example(metadata: ParquetMetadata) {
    /// let columns = vec!["id".to_string(), "name".to_string()];
    /// let optimization = ParquetOptimizer::optimize_column_projection(&metadata, &columns);
    /// println!("Need to read {} bytes in {} ranges",
    ///     optimization.estimated_bytes,
    ///     optimization.byte_ranges.len());
    /// # }
    /// ```
    pub fn optimize_column_projection(
        metadata: &ParquetMetadata,
        columns: &[String],
    ) -> ReadOptimization {
        let mut byte_ranges = Vec::new();
        let mut estimated_bytes = 0u64;

        // Collect byte ranges for requested columns across all row groups
        for (_rg_idx, rg) in metadata.row_groups.iter().enumerate() {
            for col in &rg.columns {
                if columns.contains(&col.column_path) {
                    let start = col.file_offset as u64;
                    let end = start + col.compressed_size as u64;
                    byte_ranges.push((start, end));
                    estimated_bytes += col.compressed_size as u64;
                }
            }
        }

        // Sort and merge adjacent ranges for efficient I/O
        byte_ranges.sort_by_key(|r| r.0);
        let merged_ranges = Self::merge_byte_ranges(byte_ranges, 4096); // 4KB gap threshold

        ReadOptimization {
            row_groups: (0..metadata.row_groups.len()).collect(),
            columns: columns.to_vec(),
            byte_ranges: merged_ranges,
            estimated_bytes,
        }
    }

    /// Filters row groups based on a predicate using column statistics.
    ///
    /// This implements predicate pushdown optimization by using min/max statistics
    /// to determine which row groups can be skipped. For example, if a predicate is
    /// `WHERE age > 50` and a row group's max age is 30, that entire row group can
    /// be skipped.
    ///
    /// # Arguments
    ///
    /// * `metadata` - The Parquet file metadata
    /// * `predicate` - The predicate to evaluate
    ///
    /// # Returns
    ///
    /// A vector of row group indices that might contain matching rows
    ///
    /// # Example
    ///
    /// ```no_run
    /// use anode_parquet::{ParquetMetadata, optimizer::{ParquetOptimizer, Predicate}};
    ///
    /// # fn example(metadata: ParquetMetadata) {
    /// let predicate = Predicate::Gt {
    ///     column: "age".to_string(),
    ///     value: vec![50, 0, 0, 0], // 50 in little-endian
    /// };
    /// let row_groups = ParquetOptimizer::filter_row_groups(&metadata, &predicate);
    /// println!("Need to scan {} out of {} row groups",
    ///     row_groups.len(),
    ///     metadata.row_groups.len());
    /// # }
    /// ```
    pub fn filter_row_groups(metadata: &ParquetMetadata, predicate: &Predicate) -> Vec<usize> {
        metadata
            .row_groups
            .iter()
            .enumerate()
            .filter(|(_, rg)| Self::row_group_matches(rg, predicate))
            .map(|(idx, _)| idx)
            .collect()
    }

    /// Merges adjacent or nearby byte ranges to reduce I/O operations.
    ///
    /// This function takes a list of byte ranges and merges those that are
    /// adjacent or within a small gap of each other. This reduces the number
    /// of separate read operations needed.
    ///
    /// # Arguments
    ///
    /// * `ranges` - List of byte ranges as (start, end) pairs
    /// * `gap_threshold` - Maximum gap size to merge across (in bytes)
    ///
    /// # Returns
    ///
    /// A sorted, merged list of byte ranges
    ///
    /// # Example
    ///
    /// ```
    /// use anode_parquet::optimizer::ParquetOptimizer;
    ///
    /// let ranges = vec![(0, 100), (105, 200), (500, 600)];
    /// let merged = ParquetOptimizer::merge_byte_ranges(ranges, 10);
    /// // First two ranges merge (gap of 5 < threshold of 10)
    /// assert_eq!(merged, vec![(0, 200), (500, 600)]);
    /// ```
    pub fn merge_byte_ranges(mut ranges: Vec<(u64, u64)>, gap_threshold: u64) -> Vec<(u64, u64)> {
        if ranges.is_empty() {
            return Vec::new();
        }

        ranges.sort_by_key(|r| r.0);

        let mut merged = Vec::new();
        let mut current = ranges[0];

        for range in ranges.into_iter().skip(1) {
            // If ranges overlap or are within gap threshold, merge them
            if range.0 <= current.1 + gap_threshold {
                current.1 = current.1.max(range.1);
            } else {
                merged.push(current);
                current = range;
            }
        }

        merged.push(current);
        merged
    }

    fn row_group_matches(rg: &RowGroupMetadata, predicate: &Predicate) -> bool {
        match predicate {
            Predicate::Eq { column, value } => {
                Self::check_column_stats(rg, column, |stats| {
                    if let (Some(min), Some(max)) = (&stats.min_value, &stats.max_value) {
                        // If value is outside [min, max], we can skip this row group
                        value >= min && value <= max
                    } else {
                        // No statistics, can't filter out
                        true
                    }
                })
            },
            Predicate::Lt { column, value } => {
                Self::check_column_stats(rg, column, |stats| {
                    if let Some(min) = &stats.min_value {
                        // If min >= value, all rows are >= value, can skip
                        min < value
                    } else {
                        true
                    }
                })
            },
            Predicate::Gt { column, value } => {
                Self::check_column_stats(rg, column, |stats| {
                    if let Some(max) = &stats.max_value {
                        // If max <= value, all rows are <= value, can skip
                        max > value
                    } else {
                        true
                    }
                })
            },
            Predicate::Between { column, min, max } => {
                Self::check_column_stats(rg, column, |stats| {
                    if let (Some(stats_min), Some(stats_max)) = (&stats.min_value, &stats.max_value)
                    {
                        // Check if ranges overlap
                        !(stats_max < min || stats_min > max)
                    } else {
                        true
                    }
                })
            },
            Predicate::And(left, right) => {
                Self::row_group_matches(rg, left) && Self::row_group_matches(rg, right)
            },
            Predicate::Or(left, right) => {
                Self::row_group_matches(rg, left) || Self::row_group_matches(rg, right)
            },
        }
    }

    fn check_column_stats<F>(rg: &RowGroupMetadata, column: &str, check: F) -> bool
    where
        F: Fn(&ColumnStatistics) -> bool,
    {
        rg.columns
            .iter()
            .find(|col| col.column_path == column)
            .and_then(|col| col.statistics.as_ref())
            .map(check)
            .unwrap_or(true) // If no stats, can't filter out
    }

    /// Estimates the total bytes to read given an optimization plan.
    ///
    /// This sums up the sizes of all byte ranges in the optimization plan.
    ///
    /// # Arguments
    ///
    /// * `optimization` - The read optimization plan
    ///
    /// # Returns
    ///
    /// Total bytes to read
    pub fn estimate_read_size(optimization: &ReadOptimization) -> u64 {
        optimization
            .byte_ranges
            .iter()
            .map(|(start, end)| end - start)
            .sum()
    }

    /// Calculates the selectivity ratio for a predicate.
    ///
    /// Selectivity is the fraction of rows that might match the predicate,
    /// estimated based on row group filtering. A value of 0.0 means no rows
    /// match, 1.0 means all rows match.
    ///
    /// # Arguments
    ///
    /// * `metadata` - The Parquet file metadata
    /// * `predicate` - The predicate to evaluate
    ///
    /// # Returns
    ///
    /// A ratio between 0.0 and 1.0 representing estimated selectivity
    pub fn calculate_selectivity(metadata: &ParquetMetadata, predicate: &Predicate) -> f64 {
        let total_rows = metadata.num_rows as f64;
        if total_rows == 0.0 {
            return 0.0;
        }

        let selected_rgs = Self::filter_row_groups(metadata, predicate);
        let selected_rows: i64 = selected_rgs
            .iter()
            .filter_map(|&idx| metadata.row_groups.get(idx))
            .map(|rg| rg.num_rows)
            .sum();

        selected_rows as f64 / total_rows
    }
}

/// A predicate for filtering Parquet row groups.
///
/// Predicates are used with column statistics to determine which row groups
/// can be skipped during query execution. Values are represented as raw bytes
/// to support all Parquet data types.
#[derive(Debug, Clone)]
pub enum Predicate {
    /// Equality predicate: `column = value`
    Eq {
        /// Column name to filter on
        column: String,
        /// Value to compare against (raw bytes)
        value: Vec<u8>,
    },
    /// Less than predicate: `column < value`
    Lt {
        /// Column name to filter on
        column: String,
        /// Value to compare against (raw bytes)
        value: Vec<u8>,
    },
    /// Greater than predicate: `column > value`
    Gt {
        /// Column name to filter on
        column: String,
        /// Value to compare against (raw bytes)
        value: Vec<u8>,
    },
    /// Range predicate: `column BETWEEN min AND max`
    Between {
        /// Column name to filter on
        column: String,
        /// Minimum value (inclusive, raw bytes)
        min: Vec<u8>,
        /// Maximum value (inclusive, raw bytes)
        max: Vec<u8>,
    },
    /// Logical AND of two predicates
    And(Box<Predicate>, Box<Predicate>),
    /// Logical OR of two predicates
    Or(Box<Predicate>, Box<Predicate>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{ColumnChunkMetadata, FieldInfo, SerializableSchema};

    fn create_test_metadata() -> ParquetMetadata {
        let schema = SerializableSchema {
            fields: vec![
                FieldInfo {
                    name: "id".to_string(),
                    data_type: "INT32".to_string(),
                    nullable: false,
                },
                FieldInfo {
                    name: "value".to_string(),
                    data_type: "INT32".to_string(),
                    nullable: true,
                },
            ],
        };

        let row_groups = vec![
            RowGroupMetadata {
                num_rows: 100,
                total_byte_size: 1000,
                file_offset: 0,
                columns: vec![
                    ColumnChunkMetadata {
                        column_path: "id".to_string(),
                        file_path: None,
                        file_offset: 100,
                        num_values: 100,
                        compression: "SNAPPY".to_string(),
                        compressed_size: 400,
                        uncompressed_size: 500,
                        data_page_offset: 100,
                        dictionary_page_offset: None,
                        statistics: Some(ColumnStatistics {
                            null_count: Some(0),
                            distinct_count: Some(100),
                            min_value: Some(vec![0, 0, 0, 1]), // 1 in little-endian
                            max_value: Some(vec![0, 0, 0, 100]), // 100 in little-endian
                        }),
                    },
                    ColumnChunkMetadata {
                        column_path: "value".to_string(),
                        file_path: None,
                        file_offset: 500,
                        num_values: 100,
                        compression: "SNAPPY".to_string(),
                        compressed_size: 400,
                        uncompressed_size: 500,
                        data_page_offset: 500,
                        dictionary_page_offset: None,
                        statistics: Some(ColumnStatistics {
                            null_count: Some(5),
                            distinct_count: Some(95),
                            min_value: Some(vec![0, 0, 0, 10]),
                            max_value: Some(vec![0, 0, 0, 200]),
                        }),
                    },
                ],
            },
            RowGroupMetadata {
                num_rows: 100,
                total_byte_size: 1000,
                file_offset: 1000,
                columns: vec![
                    ColumnChunkMetadata {
                        column_path: "id".to_string(),
                        file_path: None,
                        file_offset: 1100,
                        num_values: 100,
                        compression: "SNAPPY".to_string(),
                        compressed_size: 400,
                        uncompressed_size: 500,
                        data_page_offset: 1100,
                        dictionary_page_offset: None,
                        statistics: Some(ColumnStatistics {
                            null_count: Some(0),
                            distinct_count: Some(100),
                            min_value: Some(vec![0, 0, 0, 101]),
                            max_value: Some(vec![0, 0, 0, 200]),
                        }),
                    },
                    ColumnChunkMetadata {
                        column_path: "value".to_string(),
                        file_path: None,
                        file_offset: 1500,
                        num_values: 100,
                        compression: "SNAPPY".to_string(),
                        compressed_size: 400,
                        uncompressed_size: 500,
                        data_page_offset: 1500,
                        dictionary_page_offset: None,
                        statistics: Some(ColumnStatistics {
                            null_count: Some(3),
                            distinct_count: Some(97),
                            min_value: Some(vec![0, 0, 0, 150]),
                            max_value: Some(vec![0, 0, 1, 44]), // 300 in big-endian-ish format (256 + 44)
                        }),
                    },
                ],
            },
        ];

        ParquetMetadata {
            file_size: 2000,
            footer_size: 200,
            schema,
            row_groups,
            num_rows: 200,
            created_by: Some("test".to_string()),
            key_value_metadata: vec![],
        }
    }

    #[test]
    fn test_merge_byte_ranges_no_overlap() {
        let ranges = vec![(0, 100), (200, 300), (400, 500)];
        let merged = ParquetOptimizer::merge_byte_ranges(ranges, 10);

        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0], (0, 100));
        assert_eq!(merged[1], (200, 300));
        assert_eq!(merged[2], (400, 500));
    }

    #[test]
    fn test_merge_byte_ranges_with_overlap() {
        let ranges = vec![(0, 100), (50, 150), (140, 200)];
        let merged = ParquetOptimizer::merge_byte_ranges(ranges, 10);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0], (0, 200));
    }

    #[test]
    fn test_merge_byte_ranges_with_gap_threshold() {
        let ranges = vec![(0, 100), (105, 200), (300, 400)];
        let merged = ParquetOptimizer::merge_byte_ranges(ranges, 10);

        // First two should merge (gap of 5 < threshold 10)
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], (0, 200));
        assert_eq!(merged[1], (300, 400));
    }

    #[test]
    fn test_merge_byte_ranges_unsorted() {
        let ranges = vec![(400, 500), (0, 100), (200, 300)];
        let merged = ParquetOptimizer::merge_byte_ranges(ranges, 10);

        // Should sort first then merge
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0], (0, 100));
    }

    #[test]
    fn test_optimize_column_projection() {
        let metadata = create_test_metadata();
        let columns = vec!["id".to_string()];

        let optimization = ParquetOptimizer::optimize_column_projection(&metadata, &columns);

        assert_eq!(optimization.columns, columns);
        assert_eq!(optimization.row_groups.len(), 2);
        assert!(optimization.estimated_bytes > 0);
        assert!(!optimization.byte_ranges.is_empty());
    }

    #[test]
    fn test_optimize_multiple_columns() {
        let metadata = create_test_metadata();
        let columns = vec!["id".to_string(), "value".to_string()];

        let optimization = ParquetOptimizer::optimize_column_projection(&metadata, &columns);

        assert_eq!(optimization.columns.len(), 2);
        // Should estimate all columns across all row groups
        assert_eq!(optimization.estimated_bytes, 1600); // 4 columns * 400 bytes each
    }

    #[test]
    fn test_filter_row_groups_eq() {
        let metadata = create_test_metadata();
        let predicate = Predicate::Eq {
            column: "id".to_string(),
            value: vec![0, 0, 0, 50], // Value 50
        };

        let filtered = ParquetOptimizer::filter_row_groups(&metadata, &predicate);

        // Should only match first row group (min=1, max=100)
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], 0);
    }

    #[test]
    fn test_filter_row_groups_lt() {
        let metadata = create_test_metadata();
        let predicate = Predicate::Lt {
            column: "id".to_string(),
            value: vec![0, 0, 0, 150],
        };

        let filtered = ParquetOptimizer::filter_row_groups(&metadata, &predicate);

        // Both row groups have min < 150
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_row_groups_gt() {
        let metadata = create_test_metadata();
        let predicate = Predicate::Gt {
            column: "id".to_string(),
            value: vec![0, 0, 0, 150],
        };

        let filtered = ParquetOptimizer::filter_row_groups(&metadata, &predicate);

        // Only second row group has max > 150
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], 1);
    }

    #[test]
    fn test_filter_row_groups_between() {
        let metadata = create_test_metadata();
        let predicate = Predicate::Between {
            column: "value".to_string(),
            min: vec![0, 0, 0, 100],
            max: vec![0, 0, 0, 180],
        };

        let filtered = ParquetOptimizer::filter_row_groups(&metadata, &predicate);

        // Both row groups should match:
        // RG0: [10, 200] overlaps with [100, 180]
        // RG1: [150, 300] overlaps with [100, 180]
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_row_groups_and() {
        let metadata = create_test_metadata();
        let predicate = Predicate::And(
            Box::new(Predicate::Gt {
                column: "id".to_string(),
                value: vec![0, 0, 0, 90],
            }),
            Box::new(Predicate::Lt {
                column: "id".to_string(),
                value: vec![0, 0, 0, 150],
            }),
        );

        let filtered = ParquetOptimizer::filter_row_groups(&metadata, &predicate);

        // Both row groups match: RG0 [1,100] and RG1 [101,200]
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_row_groups_or() {
        let metadata = create_test_metadata();
        let predicate = Predicate::Or(
            Box::new(Predicate::Lt {
                column: "id".to_string(),
                value: vec![0, 0, 0, 10],
            }),
            Box::new(Predicate::Gt {
                column: "id".to_string(),
                value: vec![0, 0, 0, 190],
            }),
        );

        let filtered = ParquetOptimizer::filter_row_groups(&metadata, &predicate);

        // First RG matches first condition (min=1 < 10)
        // Second RG matches second condition (max=200 > 190)
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_estimate_read_size() {
        let optimization = ReadOptimization {
            row_groups: vec![0, 1],
            columns: vec!["id".to_string()],
            byte_ranges: vec![(0, 100), (200, 400)],
            estimated_bytes: 1000,
        };

        let size = ParquetOptimizer::estimate_read_size(&optimization);
        assert_eq!(size, 300); // (100-0) + (400-200)
    }

    #[test]
    fn test_calculate_selectivity_all_rows() {
        let metadata = create_test_metadata();
        let predicate = Predicate::Gt {
            column: "id".to_string(),
            value: vec![0, 0, 0, 0],
        };

        let selectivity = ParquetOptimizer::calculate_selectivity(&metadata, &predicate);
        assert!((selectivity - 1.0).abs() < 0.01); // All rows selected
    }

    #[test]
    fn test_calculate_selectivity_half_rows() {
        let metadata = create_test_metadata();
        let predicate = Predicate::Gt {
            column: "id".to_string(),
            value: vec![0, 0, 0, 150],
        };

        let selectivity = ParquetOptimizer::calculate_selectivity(&metadata, &predicate);
        assert!((selectivity - 0.5).abs() < 0.01); // Half the rows (1 out of 2 row groups)
    }

    #[test]
    fn test_calculate_selectivity_no_rows() {
        let metadata = create_test_metadata();
        let predicate = Predicate::Gt {
            column: "id".to_string(),
            value: vec![0, 0, 0, 250],
        };

        let selectivity = ParquetOptimizer::calculate_selectivity(&metadata, &predicate);
        assert!((selectivity - 0.0).abs() < 0.01); // No rows selected
    }

    #[test]
    fn test_no_statistics_fallback() {
        let mut metadata = create_test_metadata();
        // Remove statistics from first row group
        metadata.row_groups[0].columns[0].statistics = None;

        let predicate = Predicate::Eq {
            column: "id".to_string(),
            value: vec![0, 0, 0, 50],
        };

        let filtered = ParquetOptimizer::filter_row_groups(&metadata, &predicate);

        // Without statistics on RG0, we can't filter it out (included)
        // RG1 has statistics [101, 200] which excludes 50 (filtered out)
        // Result: only RG0 is included
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0], 0);
    }

    #[test]
    fn test_byte_range_merging_multiple_columns() {
        let ranges = vec![
            (100, 200), // Column 1, RG 1
            (200, 300), // Column 2, RG 1 (adjacent)
            (500, 600), // Column 1, RG 2
            (610, 710), // Column 2, RG 2 (small gap)
        ];

        let merged = ParquetOptimizer::merge_byte_ranges(ranges, 20);

        // First two should merge (adjacent)
        // Last two should merge (gap of 10 < threshold 20)
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], (100, 300));
        assert_eq!(merged[1], (500, 710));
    }
}
