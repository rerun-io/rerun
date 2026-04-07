//! Core parquet-to-chunk loading logic for Rerun.
//!
//! Reads any `.parquet` file, introspects its Arrow schema, and maps columns
//! to Rerun components. Row groups are streamed as individual chunks via a
//! pull-based iterator to reduce peak memory usage.

mod config;
mod grouping;
mod streaming;
mod timeline;

pub use config::{
    ColumnGrouping, ComponentRule, IndexColumn, IndexType, MappedComponent, ParquetConfig,
    ScalarSuffixGroup, TimeUnit,
};
pub use streaming::ParquetError;

use re_chunk::{Chunk, EntityPath};

/// Load a parquet file and return an iterator of chunks.
///
/// The first chunk (if any) contains file-level metadata at `EntityPath::properties()`.
/// Subsequent chunks contain data grouped according to the config.
/// The caller is responsible for forwarding them to a recording, channel, etc.
///
/// The iterator may yield `Err` for individual record batch failures.
/// Callers who want to continue despite errors should skip `Err` items.
pub fn load_parquet(
    path: &std::path::Path,
    config: &ParquetConfig,
    entity_path_prefix: &EntityPath,
) -> Result<impl Iterator<Item = Result<Chunk, ParquetError>>, ParquetError> {
    streaming::load_from_path(path, config, entity_path_prefix)
}

/// Load parquet from in-memory bytes and return an iterator of chunks.
///
/// See [`load_parquet`] for details on the returned iterator.
pub fn load_parquet_from_bytes(
    bytes: &[u8],
    config: &ParquetConfig,
    entity_path_prefix: &EntityPath,
) -> Result<impl Iterator<Item = Result<Chunk, ParquetError>>, ParquetError> {
    streaming::load_from_bytes(bytes, config, entity_path_prefix)
}
