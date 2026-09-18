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
mod dataframe_to_chunks;
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

pub use self::chunk_batch::{ChunkBatch, MismatchedChunkSchemaError};
pub use self::chunk_columns::ChunkColumnDescriptors;
pub use self::chunk_schema::ChunkSchema;
pub use self::column_descriptor::{ColumnDescriptor, ColumnError};
pub use self::column_descriptor_ref::ColumnDescriptorRef;
pub use self::column_kind::{ColumnKind, UnknownColumnKind};
pub use self::component_column_descriptor::ComponentColumnDescriptor;
pub use self::dataframe_to_chunks::{
    DataframeIndex, DataframeToChunksError, chunk_batches_from_dataframe_record_batch,
};
pub use self::error::SorbetError;
pub use self::index_column_descriptor::{IndexColumnDescriptor, IndexColumnError};
pub use self::ipc::{ipc_from_schema, migrated_schema_from_ipc, raw_schema_from_ipc};
pub use self::metadata::{
    ArrowBatchMetadata, ArrowFieldMetadata, MetadataExt, MissingFieldMetadata, MissingMetadataKey,
    RERUN_KIND, SORBET_INDEX_NAME, SORBET_IS_STATIC,
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
