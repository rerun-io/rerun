//! Rerun arrow metadata and record batch definitions.
//!
//! Handles the structure of arrow record batches and their meta data for different use cases for Rerun.
//!
//! An arrow record batch that follows a specific schema is called a [`SorbetBatch`].
//!
//! There is also [`ChunkBatch`], which is a has even more constrained requirements.
//! Every [`ChunkBatch`] is a [`SorbetBatch`], but the opposite does not hold.
//!
//! Each batch type has a matching schema type:
//! * [`SorbetBatch`] has a [`SorbetSchema`] with [`SorbetColumnDescriptors`]
//! * [`ChunkBatch`] has a [`ChunkSchema`] with [`ChunkColumnDescriptors`]

mod chunk_batch;
mod chunk_columns;
mod chunk_schema;
mod column_descriptor;
mod column_descriptor_ref;
mod column_kind;
mod component_column_descriptor;
mod error;
mod index_column_descriptor;
mod ipc;
pub mod metadata;
mod migrations;
mod row_id_column_descriptor;
mod schema_builder;
mod selectors;
mod sorbet_batch;
mod sorbet_columns;
mod sorbet_schema;
pub mod timestamp_metadata;

use arrow::array::RecordBatch;

pub use self::chunk_batch::{ChunkBatch, MismatchedChunkSchemaError};
pub use self::chunk_columns::ChunkColumnDescriptors;
pub use self::chunk_schema::ChunkSchema;
pub use self::column_descriptor::{ColumnDescriptor, ColumnError};
pub use self::column_descriptor_ref::ColumnDescriptorRef;
pub use self::column_kind::{ColumnKind, UnknownColumnKind};
pub use self::component_column_descriptor::ComponentColumnDescriptor;
pub use self::error::SorbetError;
pub use self::index_column_descriptor::{IndexColumnDescriptor, UnsupportedTimeType};
pub use self::ipc::{ipc_from_schema, migrated_schema_from_ipc, raw_schema_from_ipc};
pub use self::metadata::{
    ArrowBatchMetadata, ArrowFieldMetadata, MetadataExt, MissingFieldMetadata, MissingMetadataKey,
};
pub use self::migrations::{migrate_record_batch, migrate_schema_ref};
pub use self::row_id_column_descriptor::RowIdColumnDescriptor;
pub use self::schema_builder::SchemaBuilder;
pub use self::selectors::{
    ColumnSelector, ColumnSelectorParseError, ComponentColumnSelector, TimeColumnSelector,
};
pub use self::sorbet_batch::SorbetBatch;
pub use self::sorbet_columns::{ColumnSelectorResolveError, SorbetColumnDescriptors};
pub use self::sorbet_schema::SorbetSchema;
pub use self::timestamp_metadata::{TimestampLocation, TimestampMetadata};

/// The type of [`SorbetBatch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchType {
    /// Data for one entity
    Chunk,

    /// Potentially multiple entities
    Dataframe,
}

/// Get the chunk ID from the metadata of the Arrow schema
/// of a record batch containing a sorbet chunk.
///
/// Returns one of:
/// * `Ok`
/// * [`SorbetError::MissingChunkId`]
/// * [`SorbetError::ChunkIdDeserializationError`]
// TODO(#10343): remove this
pub fn chunk_id_of_schema(
    schema: &arrow::datatypes::Schema,
) -> Result<re_types_core::ChunkId, SorbetError> {
    let metadata = schema.metadata();
    if let Some(chunk_id_str) = metadata
        .get("rerun:id")
        .or_else(|| metadata.get("rerun.id"))
    {
        chunk_id_str.parse().map_err(|err| {
            SorbetError::ChunkIdDeserializationError(format!(
                "Failed to deserialize chunk id {chunk_id_str:?}: {err}"
            ))
        })
    } else {
        Err(SorbetError::MissingChunkId)
    }
}

/// If this is a [`ChunkBatch`]: does it contain static data?
// TODO(#10343): remove this
pub fn is_static_chunk(batch: &RecordBatch) -> Option<bool> {
    re_tracing::profile_function!();
    ChunkBatch::try_from(batch)
        .ok()
        .map(|chunk| chunk.is_static())
}
