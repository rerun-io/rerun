//! The error type for parquet loading.

/// Errors that can occur during parquet loading.
///
/// No variant carries the file path: callers own the path and append it at their
/// boundary, such as the Python bindings.
#[derive(Debug, thiserror::Error)]
pub enum ParquetError {
    #[error("Failed to open the file: {source}")]
    Open {
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to open the parquet reader: {source}")]
    OpenReader {
        #[source]
        source: parquet::errors::ParquetError,
    },

    #[error("Failed to read a record batch: {source}")]
    ReadBatch {
        #[source]
        source: arrow::error::ArrowError,
    },

    #[error("Failed to build a chunk from a record batch: {source}")]
    BuildChunk {
        #[source]
        source: re_chunk::ChunkError,
    },

    #[error("Index column '{column}' not found in parquet schema")]
    IndexColumnNotFound { column: String },

    #[error("Invalid timeline name for index column '{column}': {source}")]
    InvalidTimelineName {
        column: String,
        #[source]
        source: re_log_types::InvalidTimelineNameError,
    },

    #[error("Static column '{column}' not found in parquet schema")]
    StaticColumnNotFound { column: String },

    #[error("Static column '{column}' contains non-uniform values")]
    StaticColumnNotUniform { column: String },

    #[error("Static column '{column}' changed between batches: '{before}' → '{after}'")]
    StaticColumnChanged {
        column: String,
        before: String,
        after: String,
    },

    #[error("Row window {start}..{end} is empty or outside the file's {num_rows} rows")]
    RowWindowOutOfBounds { start: u64, end: u64, num_rows: u64 },

    #[error("Parquet metadata reports a negative row count ({count})")]
    NegativeRowCount { count: i64 },

    #[error("Row count {count} does not fit this platform's pointer size")]
    RowCountOverflow { count: u64 },
}

impl ParquetError {
    pub fn open(source: std::io::Error) -> Self {
        Self::Open { source }
    }

    pub fn open_reader(source: parquet::errors::ParquetError) -> Self {
        Self::OpenReader { source }
    }

    pub fn read_batch(source: arrow::error::ArrowError) -> Self {
        Self::ReadBatch { source }
    }

    pub fn build_chunk(source: re_chunk::ChunkError) -> Self {
        Self::BuildChunk { source }
    }

    pub fn index_column_not_found(column: &str) -> Self {
        Self::IndexColumnNotFound {
            column: column.to_owned(),
        }
    }

    pub fn invalid_timeline_name(
        column: &str,
        source: re_log_types::InvalidTimelineNameError,
    ) -> Self {
        Self::InvalidTimelineName {
            column: column.to_owned(),
            source,
        }
    }

    pub fn static_column_not_found(column: &str) -> Self {
        Self::StaticColumnNotFound {
            column: column.to_owned(),
        }
    }

    pub fn static_column_not_uniform(column: &str) -> Self {
        Self::StaticColumnNotUniform {
            column: column.to_owned(),
        }
    }

    pub fn static_column_changed(column: &str, before: String, after: String) -> Self {
        Self::StaticColumnChanged {
            column: column.to_owned(),
            before,
            after,
        }
    }

    pub fn row_window_out_of_bounds(window: re_span::Span<u64>, num_rows: u64) -> Self {
        Self::RowWindowOutOfBounds {
            start: window.start,
            end: window.end(),
            num_rows,
        }
    }

    pub fn negative_row_count(count: i64) -> Self {
        Self::NegativeRowCount { count }
    }

    pub fn row_count_overflow(count: u64) -> Self {
        Self::RowCountOverflow { count }
    }
}
