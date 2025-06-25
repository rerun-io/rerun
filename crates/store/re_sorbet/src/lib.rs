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
mod metadata;
mod migrations;
mod row_id_column_descriptor;
mod schema_builder;
mod selectors;
mod sorbet_batch;
mod sorbet_columns;
mod sorbet_schema;
pub mod timestamp_metadata;

pub use self::{
    chunk_batch::{ChunkBatch, MismatchedChunkSchemaError},
    chunk_columns::ChunkColumnDescriptors,
    chunk_schema::ChunkSchema,
    column_descriptor::{ColumnDescriptor, ColumnError},
    column_descriptor_ref::ColumnDescriptorRef,
    column_kind::{ColumnKind, UnknownColumnKind},
    component_column_descriptor::ComponentColumnDescriptor,
    error::SorbetError,
    index_column_descriptor::{IndexColumnDescriptor, UnsupportedTimeType},
    ipc::{ipc_from_schema, schema_from_ipc},
    metadata::{
        ArrowBatchMetadata, ArrowFieldMetadata, MetadataExt, MissingFieldMetadata,
        MissingMetadataKey,
    },
    row_id_column_descriptor::{RowIdColumnDescriptor, WrongDatatypeError},
    schema_builder::SchemaBuilder,
    selectors::{
        ColumnSelector, ColumnSelectorParseError, ComponentColumnSelector, TimeColumnSelector,
    },
    sorbet_batch::SorbetBatch,
    sorbet_columns::{ColumnSelectorResolveError, SorbetColumnDescriptors},
    sorbet_schema::SorbetSchema,
    timestamp_metadata::TimestampMetadata,
};

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
